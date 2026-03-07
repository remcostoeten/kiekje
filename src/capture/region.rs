use super::CaptureResult;
use crate::app::region_selector::{choose_region_or_fullscreen, SelectionChoice, SelectionRect};
use crate::platform::linux::grim;
use anyhow::{bail, Context, Result};
use image::{imageops, ImageFormat};
use std::io::Cursor;

pub fn capture() -> Result<CaptureResult> {
    let fullscreen_png = grim::capture_fullscreen()
        .context("grim failed to capture fullscreen for area selection")?;

    let png_data = match choose_region_or_fullscreen(&fullscreen_png)? {
        SelectionChoice::Fullscreen => fullscreen_png,
        SelectionChoice::Region(rect) => crop_png(&fullscreen_png, rect)?,
    };

    Ok(CaptureResult { png_data })
}

fn crop_png(png_data: &[u8], rect: SelectionRect) -> Result<Vec<u8>> {
    let image = image::load_from_memory(png_data)
        .context("failed to decode fullscreen capture for area crop")?
        .to_rgba8();
    let (image_width, image_height) = image.dimensions();

    if rect.width == 0 || rect.height == 0 {
        bail!("capture area must be larger than zero");
    }
    if rect.x >= image_width || rect.y >= image_height {
        bail!("capture area was outside the screenshot bounds");
    }

    let width = rect.width.min(image_width - rect.x);
    let height = rect.height.min(image_height - rect.y);
    let cropped = imageops::crop_imm(&image, rect.x, rect.y, width, height).to_image();

    let mut output = Cursor::new(Vec::new());
    cropped
        .write_to(&mut output, ImageFormat::Png)
        .context("failed to encode selected area as PNG")?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::crop_png;
    use crate::app::region_selector::SelectionRect;
    use image::{GenericImageView, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    #[test]
    fn crops_selected_area_from_fullscreen_image() {
        let mut image = RgbaImage::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                image.put_pixel(x, y, Rgba([x as u8, y as u8, 0, 255]));
            }
        }

        let mut encoded = Cursor::new(Vec::new());
        image.write_to(&mut encoded, ImageFormat::Png).unwrap();

        let cropped = crop_png(
            &encoded.into_inner(),
            SelectionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        )
        .unwrap();

        let cropped = image::load_from_memory(&cropped).unwrap();
        assert_eq!(cropped.dimensions(), (2, 2));
        assert_eq!(cropped.get_pixel(0, 0), Rgba([1, 1, 0, 255]));
    }
}
