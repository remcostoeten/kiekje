use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::thread;

use gtk::{gdk, glib};

use crate::backends::portal::bounding_box;
use crate::capture::mapping::PixelRect;
use crate::capture::{CaptureMode, Frame, PixelFormat};

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

impl Image {
    pub fn texture(&self) -> gdk::MemoryTexture {
        gdk::MemoryTexture::new(
            self.width as i32,
            self.height as i32,
            self.format.memory_format(),
            &glib::Bytes::from(&self.data),
            self.stride,
        )
    }
}

/// Copies a pixel rectangle out of a frame without converting its pixel format.
///
/// # Examples
///
/// ```ignore
/// let image = crop(&frame, PixelRect { x: 0, y: 0, width: 100, height: 100 });
/// ```
pub fn crop(frame: &Frame, rect: PixelRect) -> Image {
    let stride = rect.width as usize * 4;
    let mut data = Vec::with_capacity(stride * rect.height as usize);
    for row in rect.y..rect.y + rect.height {
        let start = row as usize * frame.stride + rect.x as usize * 4;
        data.extend_from_slice(&frame.data[start..start + stride]);
    }
    Image {
        width: rect.width,
        height: rect.height,
        stride,
        format: frame.format,
        data,
    }
}

struct Placement<'a> {
    frame: &'a Frame,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    columns: Vec<usize>,
}

impl Placement<'_> {
    fn fill_row(&self, y: u32, row: &mut [u8]) {
        if y < self.top || y >= self.top + self.height {
            return;
        }
        let frame = self.frame;
        let source_y =
            (u64::from(y - self.top) * u64::from(frame.height) / u64::from(self.height)) as usize;
        let source_row = &frame.data[source_y * frame.stride..];
        let start = self.left as usize * 4;
        let row_bytes = self.width as usize * 4;
        let target_row = &mut row[start..start + row_bytes];
        if self.width == frame.width {
            convert_row(frame.format, &source_row[..row_bytes], target_row);
            return;
        }
        for (target, &source) in target_row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&self.columns)
        {
            let pixel = [
                source_row[source],
                source_row[source + 1],
                source_row[source + 2],
                source_row[source + 3],
            ];
            *target = frame.format.to_rgba(pixel);
        }
    }
}

/// Composes all monitor frames into one RGBA image laid out like the desktop.
///
/// The output uses the largest pixels-per-logical-pixel ratio among the frames; lower density
/// monitors are upscaled with nearest-neighbor sampling so mixed scaling lines up.
///
/// # Examples
///
/// ```ignore
/// let image = stitch(&frames).unwrap();
/// ```
pub fn stitch(frames: &[Frame]) -> Option<Image> {
    let monitors: Vec<_> = frames.iter().map(|frame| frame.monitor.clone()).collect();
    let bounds = bounding_box(&monitors)?;
    let ratio = frames
        .iter()
        .map(|frame| f64::from(frame.width) / f64::from(frame.monitor.logical.width.max(1)))
        .fold(1.0_f64, f64::max);

    let width = (f64::from(bounds.width) * ratio).round() as u32;
    let height = (f64::from(bounds.height) * ratio).round() as u32;
    let stride = width as usize * 4;
    let placements: Vec<Placement> = frames
        .iter()
        .map(|frame| {
            let logical = frame.monitor.logical;
            let left = (f64::from(logical.x - bounds.x) * ratio).round() as u32;
            let top = (f64::from(logical.y - bounds.y) * ratio).round() as u32;
            let width = ((f64::from(logical.width) * ratio).round() as u32).min(width - left);
            let height = ((f64::from(logical.height) * ratio).round() as u32).min(height - top);
            let columns = (0..u64::from(width))
                .map(|x| (x * u64::from(frame.width) / u64::from(width)) as usize * 4)
                .collect();
            Placement {
                frame,
                left,
                top,
                width,
                height,
                columns,
            }
        })
        .collect();

    let mut data = vec![0u8; stride * height as usize];
    let threads = thread::available_parallelism().map_or(1, NonZero::get);
    let band_rows = (height as usize).div_ceil(threads).max(1);
    thread::scope(|scope| {
        for (band, chunk) in data.chunks_mut(band_rows * stride).enumerate() {
            let placements = &placements;
            scope.spawn(move || {
                for (offset, row) in chunk.chunks_mut(stride).enumerate() {
                    let y = (band * band_rows + offset) as u32;
                    for placement in placements {
                        placement.fill_row(y, row);
                    }
                }
            });
        }
    });

    Some(Image {
        width,
        height,
        stride,
        format: PixelFormat::Rgba,
        data,
    })
}

/// Encodes an image as an 8-bit RGBA PNG in memory, converting its pixels in place first.
///
/// # Examples
///
/// ```ignore
/// let png = encode_png(image)?;
/// ```
pub fn encode_png(image: Image) -> io::Result<Vec<u8>> {
    let image = into_rgba(image);
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_filter(png::Filter::Paeth);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&image.data))
        .map_err(io::Error::other)?;
    Ok(png)
}

/// Writes encoded PNG bytes into a new file in `directory`, never overwriting an existing one.
///
/// # Examples
///
/// ```ignore
/// let path = save_png(&png, &captures_dir(), CaptureMode::Region)?;
/// ```
pub fn save_png(png: &[u8], directory: &Path, mode: CaptureMode) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let stamp = glib::DateTime::now_local()
        .and_then(|now| now.format("%Y%m%d-%H%M%S"))
        .map(|value| value.to_string())
        .unwrap_or_else(|_| "unknown-time".into());
    let base = format!("kiekje-{}-{stamp}", mode.as_str());
    let (path, mut file) = create_unique(directory, &base, "png")?;

    if let Err(error) = file.write_all(png) {
        // Best-effort cleanup of the partial file; the write error is the one to report.
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

fn create_unique(directory: &Path, base: &str, extension: &str) -> io::Result<(PathBuf, File)> {
    for attempt in 1..=1000 {
        let name = if attempt == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{attempt}.{extension}")
        };
        let path = directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "no free file name after 1000 attempts",
    ))
}

fn into_rgba(mut image: Image) -> Image {
    let row_bytes = image.width as usize * 4;
    if image.format != PixelFormat::Rgba {
        for row in image.data.chunks_mut(image.stride) {
            for pixel in row[..row_bytes].as_chunks_mut::<4>().0 {
                *pixel = image.format.to_rgba(*pixel);
            }
        }
    }
    if image.stride != row_bytes {
        let mut packed = Vec::with_capacity(row_bytes * image.height as usize);
        for row in image.data.chunks(image.stride).take(image.height as usize) {
            packed.extend_from_slice(&row[..row_bytes]);
        }
        image.data = packed;
        image.stride = row_bytes;
    }
    image.format = PixelFormat::Rgba;
    image
}

fn convert_row(format: PixelFormat, source: &[u8], target: &mut [u8]) {
    let pixels = source.as_chunks::<4>().0.iter();
    for (target, source) in target.as_chunks_mut::<4>().0.iter_mut().zip(pixels) {
        *target = format.to_rgba(*source);
    }
}

pub fn captures_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| glib::home_dir().join("Pictures"))
        .join("Kiekje")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{LogicalRect, MonitorInfo};

    fn frame(x: i32, width: u32, value: u8) -> Frame {
        Frame {
            monitor: MonitorInfo {
                connector: "TEST".into(),
                logical: LogicalRect {
                    x,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            },
            width,
            height: width,
            stride: width as usize * 4,
            format: PixelFormat::Bgrx,
            data: [value, 0, 0, 0].repeat((width * width) as usize),
        }
    }

    #[test]
    fn creates_a_new_file_when_the_name_exists() {
        let directory = std::env::temp_dir().join(format!("kiekje-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let (first, _) = create_unique(&directory, "same", "png").unwrap();
        let (second, _) = create_unique(&directory, "same", "png").unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("same-2.png"));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn stitches_mixed_scale_frames_at_the_highest_density() {
        let image = stitch(&[frame(0, 2, 10), frame(1, 1, 20)]).unwrap();
        assert_eq!((image.width, image.height), (4, 2));
        assert_eq!(&image.data[0..4], &[0, 0, 10, 255]);
        assert_eq!(&image.data[12..16], &[0, 0, 20, 255]);
        assert_eq!(&image.data[16 + 8..16 + 12], &[0, 0, 20, 255]);
    }

    #[test]
    fn crops_rows_without_converting() {
        let mut source = frame(0, 2, 0);
        source.data = (0..16).collect();
        let image = crop(
            &source,
            PixelRect {
                x: 1,
                y: 1,
                width: 1,
                height: 1,
            },
        );
        assert_eq!(image.data, vec![12, 13, 14, 15]);
    }
}
