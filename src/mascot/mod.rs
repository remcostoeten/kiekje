mod sprite;

use sprite::{HEIGHT, SPRITE};

use crate::style::{Rgb, Style, mix};

pub use sprite::WIDTH;

pub const ROWS: usize = HEIGHT / 2;

const BODY: u32 = 1;
const EYE: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub lift: usize,
    pub glow: f32,
    pub lid: f32,
    pub unlit: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pixel {
    Empty,
    Body(Rgb),
    Eye(Rgb),
}

fn decode(x: usize, y: usize) -> Pixel {
    let Some(&value) = SPRITE.get(y).and_then(|row| row.get(x)) else {
        return Pixel::Empty;
    };
    let color = ((value >> 16) as u8, (value >> 8) as u8, value as u8);
    match value >> 24 {
        BODY => Pixel::Body(color),
        EYE => Pixel::Eye(color),
        _ => Pixel::Empty,
    }
}

fn eye_span(x: usize, y: usize) -> (usize, usize) {
    let is_eye = |row: usize| matches!(decode(x, row), Pixel::Eye(_));
    let top = (0..=y)
        .rev()
        .take_while(|&row| is_eye(row))
        .last()
        .unwrap_or(y);
    let bottom = (y..HEIGHT)
        .take_while(|&row| is_eye(row))
        .last()
        .unwrap_or(y);
    (top, bottom)
}

fn lidded(x: usize, y: usize, lid: f32) -> Option<Rgb> {
    let (top, bottom) = eye_span(x, y);
    let covered = (lid.clamp(0.0, 1.0) * (bottom - top + 1) as f32).round() as usize;
    if y >= top + covered {
        return None;
    }
    match decode(x, top.checked_sub(1)?) {
        Pixel::Body(color) => Some(color),
        _ => None,
    }
}

fn reveal(glow: f32, x: usize, y: usize) -> f32 {
    let dx = x as f32 / (WIDTH - 1) as f32 - 0.5;
    let dy = y as f32 / (HEIGHT - 1) as f32 - 0.5;
    let distance = (dx * dx + dy * dy).sqrt() * 2.0;
    let local = (glow * 1.6 - distance * 0.6).clamp(0.0, 1.0);
    1.0 - (1.0 - local).powi(3)
}

fn pixel(pose: Pose, x: usize, row: usize) -> Option<Rgb> {
    let y = row + pose.lift;
    let color = match decode(x, y) {
        Pixel::Empty => return None,
        Pixel::Body(color) => color,
        Pixel::Eye(color) => lidded(x, y, pose.lid).unwrap_or(color),
    };
    Some(mix(pose.unlit, color, reveal(pose.glow, x, y)))
}

fn cell(style: &Style, top: Option<Rgb>, bottom: Option<Rgb>) -> String {
    let code = match (top, bottom) {
        (Some(top), Some(bottom)) => format!(
            "38;2;{};{};{};48;2;{};{};{}",
            top.0, top.1, top.2, bottom.0, bottom.1, bottom.2
        ),
        (Some(color), None) | (None, Some(color)) => {
            format!("38;2;{};{};{}", color.0, color.1, color.2)
        }
        (None, None) => return String::from(" "),
    };
    let glyph = if top.is_some() { "▀" } else { "▄" };
    style.paint(&code, glyph)
}

/// Renders the blob mascot as [`ROWS`] lines of half-block characters, each [`WIDTH`] cells wide.
///
/// `lift` raises the sprite into its empty top rows, so a hop never clips it.
///
/// # Examples
///
/// ```ignore
/// let pose = Pose { lift: 0, glow: 1.0, lid: 0.0, unlit: (0, 0, 0) };
/// for line in mascot::rows(&Style::stdout(), pose) {
///     println!("{line}");
/// }
/// ```
pub fn rows(style: &Style, pose: Pose) -> Vec<String> {
    (0..ROWS)
        .map(|cell_row| {
            (0..WIDTH)
                .map(|x| {
                    cell(
                        style,
                        pixel(pose, x, cell_row * 2),
                        pixel(pose, x, cell_row * 2 + 1),
                    )
                })
                .collect()
        })
        .collect()
}

/// Number of empty sprite rows above the mascot, the most it can hop without clipping.
///
/// # Examples
///
/// ```ignore
/// let height = mascot::headroom().min(2);
/// ```
pub fn headroom() -> usize {
    (0..HEIGHT)
        .take_while(|&y| (0..WIDTH).all(|x| decode(x, y) == Pixel::Empty))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REST: Pose = Pose {
        lift: 0,
        glow: 1.0,
        lid: 0.0,
        unlit: (0, 0, 0),
    };

    fn eyes() -> Vec<(usize, usize)> {
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| matches!(decode(x, y), Pixel::Eye(_)))
            .collect()
    }

    #[test]
    fn keeps_room_above_the_mascot_to_hop() {
        assert!(headroom() >= 2);
        assert_eq!(HEIGHT % 2, 0);
    }

    #[test]
    fn has_two_open_eyes_at_rest() {
        let eyes = eyes();
        assert!(eyes.len() >= 8);
        assert!(
            eyes.iter()
                .all(|&(x, y)| matches!(decode(x, y), Pixel::Eye(color)
                    if pixel(REST, x, y) == Some(color)))
        );
    }

    #[test]
    fn closes_every_eye_pixel_when_the_lid_is_down() {
        let shut = Pose { lid: 1.0, ..REST };
        assert!(eyes().iter().all(|&(x, y)| {
            matches!((decode(x, y), pixel(shut, x, y)), (Pixel::Eye(eye), Some(lid)) if eye != lid)
        }));
    }

    #[test]
    fn starts_dark_and_lights_up_fully() {
        let dark = Pose { glow: 0.0, ..REST };
        let (x, y) = (WIDTH / 2, HEIGHT / 2);
        assert_eq!(pixel(dark, x, y), Some((0, 0, 0)));
        assert_ne!(pixel(REST, x, y), Some((0, 0, 0)));
    }

    #[test]
    fn renders_one_cell_per_column() {
        let style = Style { color: true };
        for line in rows(&style, REST) {
            let visible = line
                .split('\x1b')
                .map(|part| part.split_once('m').map_or(part, |(_, text)| text))
                .collect::<String>();
            assert_eq!(visible.chars().count(), WIDTH);
        }
    }
}
