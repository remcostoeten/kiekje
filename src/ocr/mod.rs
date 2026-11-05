mod tesseract;

use std::sync::OnceLock;

use fast_image_resize::images::Image as Resized;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capture::{Availability, PixelFormat};
use crate::output::Image;

pub const NOT_INSTALLED: &str = "Tesseract is not installed";
pub const NO_LANGUAGE_DATA: &str = "Tesseract has no English language data";

const UPSCALE: u32 = 2;
const MAX_UPSCALED_PIXELS: u64 = 4_000_000;
const DARK_BELOW: u64 = 128;

static PROBE: OnceLock<Availability> = OnceLock::new();

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OcrError {
    #[error("{NOT_INSTALLED}, install the tesseract package")]
    NotInstalled,
    #[error("Tesseract is missing {0}")]
    MissingSymbol(&'static str),
    #[error("Tesseract did not start")]
    Start,
    #[error("Tesseract has no language data for {0}; install it or pick another language")]
    Language(String),
    #[error("the region is too large to read")]
    TooLarge,
    #[error("Tesseract could not read the region")]
    Recognize,
    #[error("the text recognition worker stopped")]
    Worker,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Language(String);

#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a Tesseract language name such as eng, nld or chi_sim")]
pub struct LanguageError(String);

impl Language {
    pub fn english() -> Self {
        Self("eng".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Language {
    type Error = LanguageError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let valid = (2..=32).contains(&value.len())
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(Self(value))
        } else {
            Err(LanguageError(value))
        }
    }
}

impl From<Language> for String {
    fn from(language: Language) -> Self {
        language.0
    }
}

pub(crate) struct Gray {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Vec<u8>,
}

/// Whether text recognition works here, probed once and then cached.
///
/// Loads libtesseract and starts it with English, so call it off the main thread the first time.
///
/// # Examples
///
/// ```ignore
/// if let Availability::Unavailable(reason) = ocr::availability() {
///     info!("text capture is off: {reason}");
/// }
/// ```
pub fn availability() -> Availability {
    *PROBE.get_or_init(|| match tesseract::Engine::start("eng") {
        Ok(_) => Availability::Available,
        Err(OcrError::Language(_)) => Availability::Unavailable(NO_LANGUAGE_DATA),
        Err(error) => {
            debug!("Tesseract did not load: {error}");
            Availability::Unavailable(NOT_INSTALLED)
        }
    })
}

/// The Tesseract version, or why it could not be loaded.
///
/// # Examples
///
/// ```ignore
/// println!("{}", ocr::version()?);
/// ```
pub fn version() -> Result<String, OcrError> {
    tesseract::version()
}

/// The languages Tesseract has data for, sorted, without the script detection models.
///
/// Starts Tesseract to ask it, so call it off the main thread.
///
/// # Examples
///
/// ```ignore
/// let installed = ocr::installed_languages()?;
/// ```
pub fn installed_languages() -> Result<Vec<Language>, OcrError> {
    let engine = tesseract::Engine::start("eng")?;
    Ok(engine
        .languages()
        .into_iter()
        .filter_map(|name| Language::try_from(name).ok())
        .collect())
}

/// Reads the text in `image` with the given languages, English when the list is empty.
///
/// The image is turned grey, inverted when it is mostly dark and doubled in size when it is small
/// enough, which is what Tesseract reads screen text best from. Blocks for the whole read.
///
/// # Examples
///
/// ```ignore
/// let text = ocr::recognize(&image, &config.text.languages)?;
/// ```
pub fn recognize(image: &Image, languages: &[Language]) -> Result<String, OcrError> {
    let engine = tesseract::Engine::start(&language_spec(languages))?;
    let prepared = prepare(image)?;
    engine.read(&prepared).map(|text| tidy(&text))
}

fn language_spec(languages: &[Language]) -> String {
    if languages.is_empty() {
        return Language::english().0;
    }
    languages
        .iter()
        .map(Language::as_str)
        .collect::<Vec<_>>()
        .join("+")
}

fn prepare(image: &Image) -> Result<Gray, OcrError> {
    let gray = grayscale(image);
    let gray = if is_dark(&gray.data) {
        Gray {
            data: gray.data.iter().map(|value| 255 - value).collect(),
            ..gray
        }
    } else {
        gray
    };
    let factor = upscale_factor(gray.width, gray.height);
    if factor == 1 {
        return Ok(gray);
    }
    let (width, height) = (gray.width, gray.height);
    let source = Resized::from_vec_u8(width, height, gray.data, PixelType::U8)
        .map_err(|_| OcrError::TooLarge)?;
    let mut target = Resized::new(width * factor, height * factor, PixelType::U8);
    Resizer::new()
        .resize(
            &source,
            &mut target,
            &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Mitchell)),
        )
        .map_err(|_| OcrError::TooLarge)?;
    Ok(Gray {
        width: target.width(),
        height: target.height(),
        data: target.into_vec(),
    })
}

fn grayscale(image: &Image) -> Gray {
    let (red, green, blue) = match image.format {
        PixelFormat::Bgra | PixelFormat::BgraPremultiplied | PixelFormat::Bgrx => (2, 1, 0),
        PixelFormat::Rgba | PixelFormat::RgbaPremultiplied | PixelFormat::Rgbx => (0, 1, 2),
    };
    let width = image.width as usize;
    let mut data = Vec::with_capacity(width * image.height as usize);
    for row in image.data.chunks(image.stride).take(image.height as usize) {
        for pixel in row[..width * 4].as_chunks::<4>().0 {
            let luma = 299 * u32::from(pixel[red])
                + 587 * u32::from(pixel[green])
                + 114 * u32::from(pixel[blue]);
            data.push((luma / 1000) as u8);
        }
    }
    Gray {
        width: image.width,
        height: image.height,
        data,
    }
}

fn is_dark(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let total: u64 = data.iter().map(|value| u64::from(*value)).sum();
    total / (data.len() as u64) < DARK_BELOW
}

fn upscale_factor(width: u32, height: u32) -> u32 {
    if u64::from(width) * u64::from(height) <= MAX_UPSCALED_PIXELS {
        UPSCALE
    } else {
        1
    }
}

fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{c}'))
        .collect();
    let mut tidied: Vec<&str> = Vec::with_capacity(lines.len());
    for line in lines {
        let blank = line.is_empty();
        if blank && tidied.last().is_none_or(|last| last.is_empty()) {
            continue;
        }
        tidied.push(line);
    }
    while tidied.last().is_some_and(|line| line.is_empty()) {
        tidied.pop();
    }
    tidied.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(format: PixelFormat, pixels: &[[u8; 4]], width: u32, stride: usize) -> Image {
        let height = pixels.len() as u32 / width;
        let mut data = Vec::new();
        for row in pixels.chunks(width as usize) {
            let mut bytes: Vec<u8> = row.iter().flatten().copied().collect();
            bytes.resize(stride, 0);
            data.extend(bytes);
        }
        Image {
            width,
            height,
            stride,
            format,
            data,
        }
    }

    #[test]
    fn reads_luma_from_both_byte_orders_and_skips_row_padding() {
        let red_first = image(
            PixelFormat::Rgbx,
            &[[255, 0, 0, 255], [0, 0, 255, 255]],
            2,
            12,
        );
        assert_eq!(grayscale(&red_first).data, [76, 29]);
        let blue_first = image(
            PixelFormat::Bgra,
            &[[255, 0, 0, 255], [0, 0, 255, 255]],
            2,
            8,
        );
        assert_eq!(grayscale(&blue_first).data, [29, 76]);
    }

    #[test]
    fn inverts_a_dark_theme_so_text_is_dark_on_light() {
        let dark = image(PixelFormat::Rgbx, &[[10, 10, 10, 255]; 4], 2, 8);
        let prepared = prepare(&dark).unwrap();
        assert!(prepared.data.iter().all(|value| *value > 200));
        let light = image(PixelFormat::Rgbx, &[[240, 240, 240, 255]; 4], 2, 8);
        assert!(
            prepare(&light)
                .unwrap()
                .data
                .iter()
                .all(|value| *value > 200)
        );
    }

    #[test]
    fn doubles_small_regions_and_leaves_large_ones() {
        let small = image(PixelFormat::Rgbx, &[[255, 255, 255, 255]; 6], 3, 12);
        let prepared = prepare(&small).unwrap();
        assert_eq!((prepared.width, prepared.height), (6, 4));
        assert_eq!(prepared.data.len(), 24);
        assert_eq!(upscale_factor(1920, 1080), 2);
        assert_eq!(upscale_factor(3840, 2160), 1);
    }

    #[test]
    fn trims_trailing_space_form_feeds_and_extra_blank_lines() {
        let read = "\nfn main() {  \n\n\n    let a = 1;\n}\n\n\u{c}";
        assert_eq!(tidy(read), "fn main() {\n\n    let a = 1;\n}");
        assert_eq!(tidy(" \n\u{c}"), "");
    }

    #[test]
    fn joins_languages_with_a_plus_and_falls_back_to_english() {
        let dutch = Language::try_from("nld".to_owned()).unwrap();
        assert_eq!(language_spec(&[Language::english(), dutch]), "eng+nld");
        assert_eq!(language_spec(&[]), "eng");
    }

    #[test]
    fn accepts_tesseract_language_names_only() {
        assert!(Language::try_from("chi_sim".to_owned()).is_ok());
        assert!(Language::try_from("e".to_owned()).is_err());
        assert!(Language::try_from("eng+nld".to_owned()).is_err());
        assert!(Language::try_from("../eng".to_owned()).is_err());
    }
}
