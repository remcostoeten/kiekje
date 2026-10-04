use std::path::{Path, PathBuf};

use gtk::gdk_pixbuf::Pixbuf;
use gtk::glib;

use crate::capture::PixelFormat;
use crate::output::Image;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("could not read {path}: {source}")]
    Decode { path: PathBuf, source: glib::Error },
    #[error("could not convert {path}: {reason}")]
    Convert { path: PathBuf, reason: String },
}

/// Pixels in cairo's `ARGB32` layout: premultiplied, one native-endian `u32` per pixel, no padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Picture {
    pub fn stride(&self) -> usize {
        self.width as usize * 4
    }

    /// Converts a captured image of any pixel format into cairo's layout.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let picture = Picture::from_image(&image);
    /// ```
    pub fn from_image(image: &Image) -> Self {
        let row = image.width as usize * 4;
        let mut data = Vec::with_capacity(row * image.height as usize);
        for line in image.data.chunks(image.stride).take(image.height as usize) {
            for pixel in line[..row.min(line.len())].as_chunks::<4>().0 {
                data.extend_from_slice(&pack(image.format.to_rgba(*pixel)));
            }
        }
        Self {
            width: image.width,
            height: image.height,
            data,
        }
    }

    /// Converts the pixels back into a straight RGBA image ready for encoding.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let png = output::encode_png(picture.to_image())?;
    /// ```
    pub fn to_image(&self) -> Image {
        let data = self
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| unpack(*pixel))
            .collect();
        Image {
            width: self.width,
            height: self.height,
            stride: self.stride(),
            format: PixelFormat::Rgba,
            data,
        }
    }

    /// Averages square blocks of `block` pixels, aligned to the top left corner.
    ///
    /// The result is as large as the picture, so any part of it can stand in for the same part of
    /// the original.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let hidden = picture.mosaic(block_size(picture.width, picture.height));
    /// ```
    pub fn mosaic(&self, block: u32) -> Self {
        let block = block.max(1) as usize;
        let (width, height) = (self.width as usize, self.height as usize);
        let stride = self.stride();
        let mut data = vec![0; self.data.len()];
        for top in (0..height).step_by(block) {
            let bottom = (top + block).min(height);
            for left in (0..width).step_by(block) {
                let right = (left + block).min(width);
                let mut sums = [0_u64; 4];
                for row in top..bottom {
                    for column in left..right {
                        let start = row * stride + column * 4;
                        for (sum, byte) in sums.iter_mut().zip(&self.data[start..start + 4]) {
                            *sum += u64::from(*byte);
                        }
                    }
                }
                let count = ((bottom - top) * (right - left)) as u64;
                let average = sums.map(|sum| ((sum + count / 2) / count) as u8);
                for row in top..bottom {
                    for column in left..right {
                        let start = row * stride + column * 4;
                        data[start..start + 4].copy_from_slice(&average);
                    }
                }
            }
        }
        Self {
            width: self.width,
            height: self.height,
            data,
        }
    }
}

/// Picks a pixelation block that hides text on both small and large images.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(block_size(1920, 1080), 18);
/// ```
pub fn block_size(width: u32, height: u32) -> u32 {
    (width.min(height) / 60).clamp(8, 32)
}

/// Decodes an image file into straight RGBA.
///
/// # Examples
///
/// ```ignore
/// let image = pixels::decode(&path)?;
/// ```
pub fn decode(path: &Path) -> Result<Image, LoadError> {
    let convert = |reason: String| LoadError::Convert {
        path: path.to_path_buf(),
        reason,
    };
    let mut pixbuf = Pixbuf::from_file(path).map_err(|source| LoadError::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    if !pixbuf.has_alpha() {
        pixbuf = pixbuf
            .add_alpha(false, 0, 0, 0)
            .map_err(|error| convert(error.to_string()))?;
    }
    let width = u32::try_from(pixbuf.width()).map_err(|error| convert(error.to_string()))?;
    let height = u32::try_from(pixbuf.height()).map_err(|error| convert(error.to_string()))?;
    let stride = usize::try_from(pixbuf.rowstride()).map_err(|error| convert(error.to_string()))?;
    let row = width as usize * 4;
    let bytes = pixbuf.read_pixel_bytes();
    let data: Vec<u8> = bytes
        .chunks(stride.max(row))
        .take(height as usize)
        .flat_map(|line| &line[..row.min(line.len())])
        .copied()
        .collect();
    if data.len() != row * height as usize {
        return Err(convert(
            "the decoded pixels are shorter than the image".into(),
        ));
    }
    Ok(Image {
        width,
        height,
        stride: row,
        format: PixelFormat::Rgba,
        data,
    })
}

fn pack([red, green, blue, alpha]: [u8; 4]) -> [u8; 4] {
    let scale = |channel: u8| ((u16::from(channel) * u16::from(alpha) + 127) / 255) as u32;
    let value = (u32::from(alpha) << 24) | (scale(red) << 16) | (scale(green) << 8) | scale(blue);
    value.to_ne_bytes()
}

fn unpack(pixel: [u8; 4]) -> [u8; 4] {
    let value = u32::from_ne_bytes(pixel);
    let alpha = (value >> 24) as u8;
    let channel = |shift: u32| {
        let premultiplied = ((value >> shift) & 0xff) as u16;
        match alpha {
            0 => 0,
            255 => premultiplied as u8,
            _ => ((premultiplied * 255 + u16::from(alpha) / 2) / u16::from(alpha)).min(255) as u8,
        }
    };
    [channel(16), channel(8), channel(0), alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(format: PixelFormat, pixels: &[[u8; 4]], width: u32) -> Image {
        Image {
            width,
            height: pixels.len() as u32 / width,
            stride: width as usize * 4,
            format,
            data: pixels.concat(),
        }
    }

    #[test]
    fn round_trips_opaque_pixels_through_cairo_layout() {
        let source = image(PixelFormat::Bgrx, &[[1, 2, 3, 0], [200, 100, 50, 9]], 2);
        let back = Picture::from_image(&source).to_image();
        assert_eq!(back.format, PixelFormat::Rgba);
        assert_eq!(back.data, [3, 2, 1, 255, 50, 100, 200, 255]);
    }

    #[test]
    fn premultiplies_translucent_pixels() {
        let source = image(PixelFormat::Rgba, &[[255, 0, 0, 128]], 1);
        let picture = Picture::from_image(&source);
        assert_eq!(
            u32::from_ne_bytes(picture.data[..4].try_into().unwrap()),
            0x8080_0000
        );
        assert_eq!(picture.to_image().data, [255, 0, 0, 128]);
    }

    #[test]
    fn skips_row_padding() {
        let source = Image {
            width: 1,
            height: 2,
            stride: 8,
            format: PixelFormat::Rgba,
            data: vec![1, 2, 3, 255, 9, 9, 9, 9, 4, 5, 6, 255, 9, 9, 9, 9],
        };
        let back = Picture::from_image(&source).to_image();
        assert_eq!(back.data, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn averages_each_block_including_partial_edges() {
        let gray = |value: u8| [value, value, value, 255];
        let source = image(
            PixelFormat::Rgba,
            &[gray(0), gray(100), gray(30), gray(200), gray(100), gray(90)],
            3,
        );
        let hidden = Picture::from_image(&source).mosaic(2).to_image();
        let values: Vec<u8> = hidden.data.chunks(4).map(|pixel| pixel[0]).collect();
        assert_eq!(values, [100, 100, 60, 100, 100, 60]);
    }

    #[test]
    fn sizes_blocks_for_the_image() {
        assert_eq!(block_size(300, 200), 8);
        assert_eq!(block_size(1920, 1080), 18);
        assert_eq!(block_size(7680, 4320), 32);
    }
}
