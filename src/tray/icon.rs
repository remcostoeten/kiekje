use std::sync::OnceLock;

use ksni::Icon;

pub const NAME: &str = "dev.remcostoeten.Kiekje-symbolic";

const PIXMAPS: [&[u8]; 5] = [
    include_bytes!("../../resources/tray/kiekje-16.png"),
    include_bytes!("../../resources/tray/kiekje-22.png"),
    include_bytes!("../../resources/tray/kiekje-24.png"),
    include_bytes!("../../resources/tray/kiekje-32.png"),
    include_bytes!("../../resources/tray/kiekje-48.png"),
];

#[derive(Debug, thiserror::Error)]
enum IconError {
    #[error("the bundled tray icon is not a readable PNG: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("the bundled tray icon is {0:?}, not 8-bit RGBA")]
    Format((png::ColorType, png::BitDepth)),
}

pub fn pixmaps() -> Vec<Icon> {
    static DECODED: OnceLock<Vec<Icon>> = OnceLock::new();
    DECODED
        .get_or_init(|| {
            PIXMAPS
                .iter()
                .filter_map(|bytes| match decode(bytes) {
                    Ok(icon) => Some(icon),
                    Err(error) => {
                        eprintln!("kiekje: {error}");
                        None
                    }
                })
                .collect()
        })
        .clone()
}

fn decode(bytes: &[u8]) -> Result<Icon, IconError> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader.next_frame(&mut rgba)?;
    if (info.color_type, info.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
        return Err(IconError::Format((info.color_type, info.bit_depth)));
    }
    rgba.truncate(info.buffer_size());
    Ok(Icon {
        width: info.width as i32,
        height: info.height as i32,
        data: argb(&rgba),
    })
}

fn argb(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, alpha]| [alpha, red, green, blue])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_every_bundled_size() {
        let sizes: Vec<i32> = pixmaps().iter().map(|icon| icon.width).collect();
        assert_eq!(sizes, [16, 22, 24, 32, 48]);
        for icon in pixmaps() {
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
            assert!(
                icon.data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[0] == 255)
            );
        }
    }

    #[test]
    fn reorders_rgba_into_argb() {
        assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
    }
}
