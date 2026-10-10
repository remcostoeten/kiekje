mod sprite;

use sprite::SPRITE;

use crate::style::{Rgb, Style, mix};

const SCALE: usize = 2;
const BLOB_WIDTH: usize = sprite::WIDTH / SCALE;
const BLOB_HEIGHT: usize = sprite::HEIGHT / SCALE;
const PAD_X: usize = 3;
const PAD_Y: usize = 2;
const CANVAS_HEIGHT: usize = (BLOB_HEIGHT + 2 * PAD_Y).next_multiple_of(2);

pub const WIDTH: usize = BLOB_WIDTH + 2 * PAD_X;
pub const ROWS: usize = CANVAS_HEIGHT / 2;

const BODY: u32 = 1;
const EYE: u32 = 2;
const ARM: usize = 5;
const THICKNESS: usize = 2;
const SPREAD: usize = 3;
const BRACKET: Rgb = (250, 238, 233);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub glow: f32,
    pub lid: f32,
    pub viewfinder: f32,
    pub unlit: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pixel {
    Empty,
    Body(Rgb),
    Eye(Rgb),
}

fn source(x: usize, y: usize) -> Pixel {
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

fn average(colors: &[Rgb]) -> Rgb {
    let count = colors.len().max(1) as u32;
    let sum = colors.iter().fold((0u32, 0u32, 0u32), |sum, color| {
        (
            sum.0 + u32::from(color.0),
            sum.1 + u32::from(color.1),
            sum.2 + u32::from(color.2),
        )
    });
    (
        (sum.0 / count) as u8,
        (sum.1 / count) as u8,
        (sum.2 / count) as u8,
    )
}

fn decode(x: usize, y: usize) -> Pixel {
    if x >= BLOB_WIDTH || y >= BLOB_HEIGHT {
        return Pixel::Empty;
    }
    let mut body = Vec::new();
    let mut eye = Vec::new();
    for dy in 0..SCALE {
        for dx in 0..SCALE {
            match source(x * SCALE + dx, y * SCALE + dy) {
                Pixel::Empty => {}
                Pixel::Body(color) => body.push(color),
                Pixel::Eye(color) => eye.push(color),
            }
        }
    }
    let block = SCALE * SCALE;
    if eye.len() * 2 >= block {
        Pixel::Eye(average(&eye))
    } else if (body.len() + eye.len()) * 2 >= block {
        Pixel::Body(average(&body))
    } else {
        Pixel::Empty
    }
}

fn eye_span(x: usize, y: usize) -> (usize, usize) {
    let is_eye = |row: usize| matches!(decode(x, row), Pixel::Eye(_));
    let top = (0..=y)
        .rev()
        .take_while(|&row| is_eye(row))
        .last()
        .unwrap_or(y);
    let bottom = (y..BLOB_HEIGHT)
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
    let dx = x as f32 / (BLOB_WIDTH - 1) as f32 - 0.5;
    let dy = y as f32 / (BLOB_HEIGHT - 1) as f32 - 0.5;
    let distance = (dx * dx + dy * dy).sqrt() * 2.0;
    let local = (glow * 1.6 - distance * 0.6).clamp(0.0, 1.0);
    1.0 - (1.0 - local).powi(3)
}

fn blob(pose: Pose, x: usize, row: usize) -> Option<Rgb> {
    let x = x.checked_sub(PAD_X)?;
    let y = row.checked_sub(PAD_Y)?;
    let color = match decode(x, y) {
        Pixel::Empty => return None,
        Pixel::Body(color) => color,
        Pixel::Eye(color) => lidded(x, y, pose.lid).unwrap_or(color),
    };
    Some(mix(pose.unlit, color, reveal(pose.glow, x, y)))
}

fn mirrored(position: usize, extent: usize) -> usize {
    if position * 2 < extent {
        position
    } else {
        extent - 1 - position
    }
}

fn in_bracket(pose: Pose, x: usize, row: usize) -> bool {
    let inset = ((1.0 - pose.viewfinder.clamp(0.0, 1.0)) * SPREAD as f32).round() as usize;
    let Some((x, y)) = mirrored(x, WIDTH)
        .checked_sub(inset)
        .zip(mirrored(row, CANVAS_HEIGHT).checked_sub(inset))
    else {
        return false;
    };
    let horizontal = y < THICKNESS && x < ARM;
    let vertical = x < THICKNESS && y < ARM;
    let rounded = (x == 0 || x == ARM - 1) && (y == 0 || y == ARM - 1);
    (horizontal || vertical) && !rounded
}

fn viewfinder(pose: Pose, x: usize, row: usize) -> Option<Rgb> {
    in_bracket(pose, x, row).then(|| mix(pose.unlit, BRACKET, pose.viewfinder))
}

fn pixel(pose: Pose, x: usize, row: usize) -> Option<Rgb> {
    blob(pose, x, row).or_else(|| viewfinder(pose, x, row))
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

/// Renders the blob mascot inside four corner brackets, as [`ROWS`] lines of half-block
/// characters that are each [`WIDTH`] cells wide.
///
/// The sprite is box-filtered down by `SCALE`. `viewfinder` moves the brackets out from
/// tight around the blob, `1.0` being settled; `lid` shuts the eyes, `1.0` being closed.
///
/// # Examples
///
/// ```ignore
/// let pose = Pose { glow: 1.0, lid: 0.0, viewfinder: 1.0, unlit: (0, 0, 0) };
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

#[cfg(test)]
mod tests {
    use super::*;

    const REST: Pose = Pose {
        glow: 1.0,
        lid: 0.0,
        viewfinder: 1.0,
        unlit: (0, 0, 0),
    };

    fn eyes() -> Vec<(usize, usize)> {
        (0..BLOB_HEIGHT)
            .flat_map(|y| (0..BLOB_WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| matches!(decode(x, y), Pixel::Eye(_)))
            .collect()
    }

    fn bracket_cells(pose: Pose) -> Vec<(usize, usize)> {
        (0..CANVAS_HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| in_bracket(pose, x, y))
            .collect()
    }

    #[test]
    fn keeps_two_eyes_after_scaling_down() {
        let eyes = eyes();
        assert!(eyes.len() >= 4);
        let columns: std::collections::BTreeSet<usize> = eyes.iter().map(|&(x, _)| x).collect();
        assert!(columns.len() >= 2);
        assert!(
            eyes.iter()
                .all(|&(x, y)| matches!(decode(x, y), Pixel::Eye(color)
                    if blob(REST, x + PAD_X, y + PAD_Y) == Some(color)))
        );
    }

    #[test]
    fn closes_every_eye_pixel_when_the_lid_is_down() {
        let shut = Pose { lid: 1.0, ..REST };
        assert!(eyes().iter().all(|&(x, y)| {
            matches!(
                (decode(x, y), blob(shut, x + PAD_X, y + PAD_Y)),
                (Pixel::Eye(eye), Some(lid)) if eye != lid
            )
        }));
    }

    #[test]
    fn starts_dark_and_lights_up_fully() {
        let dark = Pose { glow: 0.0, ..REST };
        let (x, y) = (WIDTH / 2, CANVAS_HEIGHT / 2);
        assert_eq!(pixel(dark, x, y), Some((0, 0, 0)));
        assert_ne!(pixel(REST, x, y), Some((0, 0, 0)));
    }

    #[test]
    fn draws_four_rounded_brackets_at_rest() {
        let per_bracket = 2 * ARM * THICKNESS - THICKNESS * THICKNESS - 3;
        assert_eq!(bracket_cells(REST).len(), 4 * per_bracket);
        assert!(!in_bracket(REST, 0, 0));
        assert!(in_bracket(REST, 1, 0));
        assert!(in_bracket(REST, WIDTH - 2, CANVAS_HEIGHT - 1));
        assert_eq!(pixel(REST, 1, 0), Some(BRACKET));
    }

    #[test]
    fn keeps_the_brackets_clear_of_the_blob() {
        assert!(
            bracket_cells(REST)
                .iter()
                .all(|&(x, y)| blob(REST, x, y).is_none())
        );
    }

    #[test]
    fn opens_the_brackets_out_from_around_the_blob() {
        let tight = Pose {
            viewfinder: 0.0,
            ..REST
        };
        assert_eq!(bracket_cells(tight).len(), bracket_cells(REST).len());
        assert!(!in_bracket(tight, 1, 0));
        assert!(in_bracket(tight, SPREAD + 1, SPREAD));
        assert!(in_bracket(
            tight,
            WIDTH - 2 - SPREAD,
            CANVAS_HEIGHT - 1 - SPREAD
        ));
        assert_eq!(pixel(tight, SPREAD + 1, SPREAD), Some((0, 0, 0)));
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
