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

pub const FRAME_COUNT: usize = 12;

const ACCENT: [u8; 3] = [0xfe, 0x51, 0x01];
const SHINE_WIDTH: f32 = 0.35;

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
                        warn!("{error}");
                        None
                    }
                })
                .collect()
        })
        .clone()
}

/// Returns one frame of the accent shine that sweeps across the icon after a capture.
///
/// The band runs diagonally from the top left to the bottom right over `FRAME_COUNT` frames;
/// an index past the end gives the still icon.
///
/// # Examples
///
/// ```ignore
/// let halfway = icon::frame(icon::FRAME_COUNT / 2);
/// ```
pub fn frame(index: usize) -> Vec<Icon> {
    static FRAMES: OnceLock<Vec<Vec<Icon>>> = OnceLock::new();
    FRAMES
        .get_or_init(|| {
            let still = pixmaps();
            (0..FRAME_COUNT)
                .map(|index| still.iter().map(|icon| shine(icon, index)).collect())
                .collect()
        })
        .get(index)
        .cloned()
        .unwrap_or_else(pixmaps)
}

fn shine(icon: &Icon, index: usize) -> Icon {
    let width = icon.width as usize;
    let span = (width + icon.height as usize).saturating_sub(2).max(1) as f32;
    let travel = 1.0 + 2.0 * SHINE_WIDTH;
    let centre = -SHINE_WIDTH + travel * index as f32 / (FRAME_COUNT - 1) as f32;
    let mut data = icon.data.clone();
    for (position, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let diagonal = (position % width + position / width) as f32 / span;
        let strength = (1.0 - (diagonal - centre).abs() / SHINE_WIDTH).clamp(0.0, 1.0);
        for (channel, accent) in pixel[1..].iter_mut().zip(ACCENT) {
            *channel =
                (*channel as f32 + (accent as f32 - *channel as f32) * strength).round() as u8;
        }
    }
    Icon {
        width: icon.width,
        height: icon.height,
        data,
    }
}

/// Returns the icon at every bundled size with its eyes shut.
///
/// The eyes are the transparent pixels the body encloses, found by flooding the transparency
/// in from the border.
///
/// # Examples
///
/// ```ignore
/// handle.update(|item| item.pixmaps = icon::blink());
/// ```
pub fn blink() -> Vec<Icon> {
    static SHUT: OnceLock<Vec<Icon>> = OnceLock::new();
    SHUT.get_or_init(|| pixmaps().iter().map(shut_eyes).collect())
        .clone()
}

/// Returns the icon at every bundled size with an accent dot in the bottom right corner.
///
/// The tray shows it for as long as a recording runs.
///
/// # Examples
///
/// ```ignore
/// handle.update(|item| item.pixmaps = icon::recording());
/// ```
pub fn recording() -> Vec<Icon> {
    static MARKED: OnceLock<Vec<Icon>> = OnceLock::new();
    MARKED
        .get_or_init(|| pixmaps().iter().map(mark).collect())
        .clone()
}

fn mark(icon: &Icon) -> Icon {
    let width = icon.width as usize;
    let size = width as f32;
    let (centre, radius) = (size * 0.72, size * 0.27);
    let mut data = icon.data.clone();
    for (position, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (
            (position % width) as f32 + 0.5,
            (position / width) as f32 + 0.5,
        );
        let distance = ((x - centre).powi(2) + (y - centre).powi(2)).sqrt();
        let cover = (radius + 0.5 - distance).clamp(0.0, 1.0);
        if cover > 0.0 {
            let [alpha, rest @ ..] = pixel;
            *alpha = (*alpha as f32 + (255.0 - *alpha as f32) * cover).round() as u8;
            for (channel, accent) in rest.iter_mut().zip(ACCENT) {
                *channel =
                    (*channel as f32 + (accent as f32 - *channel as f32) * cover).round() as u8;
            }
        }
    }
    Icon {
        width: icon.width,
        height: icon.height,
        data,
    }
}

fn shut_eyes(icon: &Icon) -> Icon {
    let (width, height) = (icon.width as usize, icon.height as usize);
    let clear = |index: usize| icon.data[index * 4] < 128;
    let mut outside = vec![false; width * height];
    let mut pending: Vec<usize> = (0..width * height)
        .filter(|&index| {
            let (x, y) = (index % width, index / width);
            x == 0 || y == 0 || x == width - 1 || y == height - 1
        })
        .filter(|&index| clear(index))
        .collect();
    while let Some(index) = pending.pop() {
        if outside[index] {
            continue;
        }
        outside[index] = true;
        let (x, y) = (index % width, index / width);
        let neighbours = [
            (x > 0).then(|| index - 1),
            (x + 1 < width).then(|| index + 1),
            (y > 0).then(|| index - width),
            (y + 1 < height).then(|| index + width),
        ];
        pending.extend(
            neighbours
                .into_iter()
                .flatten()
                .filter(|&neighbour| clear(neighbour) && !outside[neighbour]),
        );
    }
    let mut data = icon.data.clone();
    for (index, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        if clear(index) && !outside[index] {
            *pixel = [255; 4];
        }
    }
    Icon {
        width: icon.width,
        height: icon.height,
        data,
    }
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
    fn sweeps_the_accent_from_top_left_to_bottom_right() {
        let distance = |index: usize, position: usize| -> u32 {
            let pixel = frame(index)[4].data.as_chunks::<4>().0[position];
            pixel[1..]
                .iter()
                .zip(ACCENT)
                .map(|(&channel, accent)| channel.abs_diff(accent) as u32)
                .sum()
        };
        let (top_left, bottom_right) = (48 * 4 + 4, 48 * 48 - 48 * 4 - 5);
        assert!(distance(2, top_left) < distance(2, bottom_right));
        assert!(distance(FRAME_COUNT - 3, bottom_right) < distance(FRAME_COUNT - 3, top_left));
    }

    #[test]
    fn keeps_the_icon_shape_while_shining() {
        for index in 0..FRAME_COUNT {
            for (still, lit) in pixmaps().iter().zip(frame(index)) {
                let alpha = |icon: &Icon| -> Vec<u8> {
                    icon.data
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|pixel| pixel[0])
                        .collect()
                };
                assert_eq!(alpha(still), alpha(&lit));
            }
        }
    }

    #[test]
    fn shuts_only_the_eyes() {
        for (open, shut) in pixmaps().iter().zip(blink()) {
            let changed: Vec<usize> = open
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .zip(shut.data.as_chunks::<4>().0)
                .enumerate()
                .filter(|(_, (before, after))| before != after)
                .map(|(index, _)| index)
                .collect();
            assert!(!changed.is_empty(), "{} px has no eyes", open.width);
            let width = open.width as usize;
            let rows: Vec<usize> = changed.iter().map(|index| index / width).collect();
            assert!(rows.iter().max().unwrap() - rows.iter().min().unwrap() < width / 3);
        }
    }

    #[test]
    fn marks_only_the_bottom_right_while_recording() {
        for (plain, marked) in pixmaps().iter().zip(recording()) {
            let width = plain.width as usize;
            let changed: Vec<usize> = plain
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .zip(marked.data.as_chunks::<4>().0)
                .enumerate()
                .filter(|(_, (before, after))| before != after)
                .map(|(index, _)| index)
                .collect();
            assert!(!changed.is_empty(), "{} px has no dot", plain.width);
            assert!(
                changed
                    .iter()
                    .all(|index| index % width > width / 3 && index / width > width / 3)
            );
        }
    }

    #[test]
    fn reorders_rgba_into_argb() {
        assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
    }
}
