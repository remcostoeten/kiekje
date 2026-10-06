use std::cell::Cell;

use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

use super::{ACCENT, LABEL_BACKGROUND, LABEL_TEXT};
use crate::capture::mapping::logical_point_to_pixel;
use crate::capture::{Frame, LogicalRect};

const SPAN: u32 = 11;
const CELL: f32 = 11.0;
const GAP: f32 = 20.0;
const PADDING: f32 = 6.0;
const ROW: f32 = 18.0;
const SWATCH: f32 = 10.0;
const GRID: gdk::RGBA = gdk::RGBA::new(10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 0.35);
const LINE: gdk::RGBA = gdk::RGBA::new(237.0 / 255.0, 237.0 / 255.0, 237.0 / 255.0, 0.16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    Position,
    Copied,
}

pub struct Magnifier {
    enabled: Cell<bool>,
    pointer: Cell<Option<(f64, f64)>>,
    notice: Cell<Notice>,
}

impl Magnifier {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Cell::new(enabled),
            pointer: Cell::new(None),
            notice: Cell::new(Notice::Position),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Records the pointer in desktop logical coordinates and returns where it was before.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let previous = magnifier.move_to(Some((120.0, 40.5)));
    /// ```
    pub fn move_to(&self, point: Option<(f64, f64)>) -> Option<(f64, f64)> {
        let previous = self.pointer.replace(point);
        if previous != point {
            self.notice.set(Notice::Position);
        }
        previous
    }

    /// Handles the magnifier keys and reports whether the key was used.
    ///
    /// `Z` shows or hides the loupe. `C` copies the hex colour of the pixel under the pointer
    /// to the clipboard while the loupe shows.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if magnifier.key(key, state, &frames) {
    ///     return glib::Propagation::Stop;
    /// }
    /// ```
    pub fn key(&self, key: gdk::Key, state: gdk::ModifierType, frames: &[Frame]) -> bool {
        let modifiers = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;
        if state.intersects(modifiers) {
            return false;
        }
        match key.to_lower() {
            gdk::Key::z => {
                self.enabled.set(!self.enabled.get());
                true
            }
            gdk::Key::c if self.enabled.get() => {
                let Some(colour) = self
                    .pointer
                    .get()
                    .and_then(|point| colour_at(frames, point))
                else {
                    return false;
                };
                let Some(display) = gdk::Display::default() else {
                    eprintln!("kiekje: no display, colour not copied");
                    return true;
                };
                display.clipboard().set_text(&hex(colour));
                self.notice.set(Notice::Copied);
                true
            }
            _ => false,
        }
    }

    /// Draws the loupe on the surface of `frame` when the pointer is over that monitor.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// shared.magnifier.paint(&shared.frames[frame_index], widget, snapshot);
    /// ```
    pub fn paint(&self, frame: &Frame, widget: &gtk::Widget, snapshot: &gtk::Snapshot) {
        let monitor = frame.monitor.logical;
        let Some(point) = self
            .pointer
            .get()
            .filter(|point| on_monitor(*point, monitor))
        else {
            return;
        };
        if !self.enabled.get() {
            return;
        }
        let local = (
            point.0 - f64::from(monitor.x),
            point.1 - f64::from(monitor.y),
        );
        let Some(center) = logical_point_to_pixel(
            local,
            (monitor.width, monitor.height),
            (frame.width, frame.height),
        ) else {
            return;
        };
        let grid = SPAN as f32 * CELL;
        let (left, top) = place(
            (local.0 as f32, local.1 as f32),
            (grid, grid + panel_height()),
            (widget.width() as f32, widget.height() as f32),
        );
        let zoom = graphene::Rect::new(left, top, grid, grid);
        snapshot.append_color(
            &LABEL_BACKGROUND,
            &graphene::Rect::new(left, top, grid, grid + panel_height()),
        );
        let texture = gdk::MemoryTexture::new(
            SPAN as i32,
            SPAN as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(zoom_pixels(frame, center, SPAN)),
            SPAN as usize * 4,
        );
        snapshot.append_scaled_texture(&texture, gsk::ScalingFilter::Nearest, &zoom);
        for line in 1..SPAN {
            let offset = line as f32 * CELL;
            snapshot.append_color(&GRID, &graphene::Rect::new(left + offset, top, 1.0, grid));
            snapshot.append_color(&GRID, &graphene::Rect::new(left, top + offset, grid, 1.0));
        }
        let middle = (SPAN / 2) as f32 * CELL;
        snapshot.append_border(
            &gsk::RoundedRect::from_rect(
                graphene::Rect::new(left + middle, top + middle, CELL + 1.0, CELL + 1.0),
                0.0,
            ),
            &[1.0; 4],
            &[ACCENT; 4],
        );
        snapshot.append_border(
            &gsk::RoundedRect::from_rect(
                graphene::Rect::new(
                    left - 1.0,
                    top - 1.0,
                    grid + 2.0,
                    grid + panel_height() + 2.0,
                ),
                0.0,
            ),
            &[1.0; 4],
            &[LINE; 4],
        );

        let colour = pixel(frame, center.0, center.1).unwrap_or([0, 0, 0, 255]);
        let first = match self.notice.get() {
            Notice::Position => format!("X {} Y {}", center.0, center.1),
            Notice::Copied => "COPIED".to_owned(),
        };
        let panel_top = top + grid + PADDING;
        paint_text(widget, snapshot, &first, left + PADDING, panel_top);
        let second_top = panel_top + ROW;
        snapshot.append_color(
            &rgba(colour),
            &graphene::Rect::new(
                left + PADDING,
                second_top + (ROW - SWATCH) / 2.0,
                SWATCH,
                SWATCH,
            ),
        );
        paint_text(
            widget,
            snapshot,
            &hex(colour),
            left + PADDING + SWATCH + PADDING,
            second_top,
        );
    }
}

/// Whether a desktop logical point lies on `monitor`.
///
/// # Examples
///
/// ```ignore
/// assert!(on_monitor((10.5, 0.0), LogicalRect { x: 0, y: 0, width: 20, height: 20 }));
/// ```
pub fn on_monitor(point: (f64, f64), monitor: LogicalRect) -> bool {
    let (x, y) = point;
    x >= f64::from(monitor.x)
        && x < f64::from(monitor.right())
        && y >= f64::from(monitor.y)
        && y < f64::from(monitor.bottom())
}

fn panel_height() -> f32 {
    PADDING * 2.0 + ROW * 2.0
}

fn paint_text(widget: &gtk::Widget, snapshot: &gtk::Snapshot, text: &str, x: f32, top: f32) {
    let layout = widget.create_pango_layout(Some(text));
    layout.set_font_description(Some(&pango::FontDescription::from_string(
        "Geist Mono 12px",
    )));
    let (_, logical) = layout.pixel_extents();
    snapshot.save();
    snapshot.translate(&graphene::Point::new(
        x,
        top + (ROW - logical.height() as f32) / 2.0,
    ));
    snapshot.append_layout(&layout, &LABEL_TEXT);
    snapshot.restore();
}

fn rgba(colour: [u8; 4]) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(colour[0]) / 255.0,
        f32::from(colour[1]) / 255.0,
        f32::from(colour[2]) / 255.0,
        1.0,
    )
}

fn colour_at(frames: &[Frame], point: (f64, f64)) -> Option<[u8; 4]> {
    let frame = frames
        .iter()
        .find(|frame| on_monitor(point, frame.monitor.logical))?;
    let monitor = frame.monitor.logical;
    let (x, y) = logical_point_to_pixel(
        (
            point.0 - f64::from(monitor.x),
            point.1 - f64::from(monitor.y),
        ),
        (monitor.width, monitor.height),
        (frame.width, frame.height),
    )?;
    pixel(frame, x, y)
}

fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    if x >= frame.width || y >= frame.height {
        return None;
    }
    let offset = y as usize * frame.stride + x as usize * 4;
    let bytes: [u8; 4] = frame.data.get(offset..offset + 4)?.try_into().ok()?;
    Some(frame.format.to_rgba(bytes))
}

fn zoom_pixels(frame: &Frame, center: (u32, u32), span: u32) -> Vec<u8> {
    let reach = i64::from(span / 2);
    let mut pixels = Vec::with_capacity(span as usize * span as usize * 4);
    for row in 0..i64::from(span) {
        for column in 0..i64::from(span) {
            let x = i64::from(center.0) + column - reach;
            let y = i64::from(center.1) + row - reach;
            let sampled = u32::try_from(x)
                .ok()
                .zip(u32::try_from(y).ok())
                .and_then(|(x, y)| pixel(frame, x, y));
            pixels.extend_from_slice(&sampled.unwrap_or([0, 0, 0, 0]));
        }
    }
    pixels
}

fn hex(colour: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", colour[0], colour[1], colour[2])
}

fn place(cursor: (f32, f32), size: (f32, f32), bounds: (f32, f32)) -> (f32, f32) {
    let axis = |cursor: f32, size: f32, bound: f32| {
        let after = cursor + GAP;
        let start = if after + size <= bound {
            after
        } else {
            cursor - GAP - size
        };
        start.min(bound - size).max(0.0).round()
    };
    (
        axis(cursor.0, size.0, bounds.0),
        axis(cursor.1, size.1, bounds.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{MonitorInfo, PixelFormat};

    fn frame(width: u32, height: u32, logical: LogicalRect) -> Frame {
        let stride = width as usize * 4 + 8;
        let mut data = vec![0u8; stride * height as usize];
        for y in 0..height {
            for x in 0..width {
                let offset = y as usize * stride + x as usize * 4;
                data[offset..offset + 4].copy_from_slice(&[x as u8, y as u8, 200, 0]);
            }
        }
        Frame {
            monitor: MonitorInfo {
                connector: "DP-1".into(),
                logical,
            },
            width,
            height,
            stride,
            format: PixelFormat::Bgrx,
            data,
        }
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn samples_a_pixel_as_straight_rgba_through_the_stride() {
        let frame = frame(8, 6, rect(0, 0, 8, 6));
        assert_eq!(pixel(&frame, 3, 5), Some([200, 5, 3, 255]));
        assert_eq!(pixel(&frame, 8, 0), None);
        assert_eq!(pixel(&frame, 0, 6), None);
    }

    #[test]
    fn samples_a_square_around_the_center_pixel() {
        let frame = frame(8, 8, rect(0, 0, 8, 8));
        let pixels = zoom_pixels(&frame, (4, 4), 3);
        assert_eq!(pixels.len(), 3 * 3 * 4);
        assert_eq!(&pixels[0..4], &[200, 3, 3, 255]);
        assert_eq!(&pixels[16..20], &[200, 4, 4, 255]);
        assert_eq!(&pixels[32..36], &[200, 5, 5, 255]);
    }

    #[test]
    fn leaves_cells_past_the_frame_edge_transparent() {
        let frame = frame(4, 4, rect(0, 0, 4, 4));
        let pixels = zoom_pixels(&frame, (0, 3), 3);
        assert_eq!(&pixels[0..4], &[0, 0, 0, 0]);
        assert_eq!(&pixels[4..8], &[200, 2, 0, 255]);
        assert_eq!(&pixels[16..20], &[200, 3, 0, 255]);
        assert_eq!(&pixels[28..32], &[0, 0, 0, 0]);
        assert_eq!(&pixels[32..36], &[0, 0, 0, 0]);
    }

    #[test]
    fn reads_the_colour_under_a_point_at_fractional_scale() {
        let frames = [
            frame(4, 4, rect(0, 0, 4, 4)),
            frame(15, 15, rect(4, 0, 10, 10)),
        ];
        assert_eq!(colour_at(&frames, (2.5, 1.0)), Some([200, 1, 2, 255]));
        assert_eq!(colour_at(&frames, (9.0, 3.4)), Some([200, 5, 7, 255]));
        assert_eq!(colour_at(&frames, (20.0, 3.0)), None);
    }

    #[test]
    fn formats_a_colour_as_uppercase_hex() {
        assert_eq!(hex([254, 81, 1, 255]), "#FE5101");
        assert_eq!(hex([0, 10, 255, 0]), "#000AFF");
    }

    #[test]
    fn places_the_loupe_below_right_of_the_cursor() {
        assert_eq!(
            place((100.0, 100.0), (120.0, 160.0), (1920.0, 1080.0)),
            (120.0, 120.0)
        );
    }

    #[test]
    fn flips_the_loupe_away_from_the_right_and_bottom_edges() {
        assert_eq!(
            place((1900.0, 1070.0), (120.0, 160.0), (1920.0, 1080.0)),
            (1760.0, 890.0)
        );
    }

    #[test]
    fn keeps_the_loupe_on_a_monitor_smaller_than_both_sides() {
        assert_eq!(
            place((100.0, 90.0), (120.0, 160.0), (200.0, 200.0)),
            (0.0, 0.0)
        );
    }

    #[test]
    fn finds_the_monitor_under_a_point() {
        let monitor = rect(-100, 0, 100, 50);
        assert!(on_monitor((-100.0, 0.0), monitor));
        assert!(on_monitor((-0.5, 49.9), monitor));
        assert!(!on_monitor((0.0, 10.0), monitor));
        assert!(!on_monitor((-50.0, 50.0), monitor));
    }
}
