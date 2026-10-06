use std::cell::Cell;

use crate::capture::LogicalRect;
use crate::capture::windows::{clip_to_monitors, window_at};

const CLICK_SLOP: f64 = 3.0;

pub struct Snap {
    windows: Vec<LogicalRect>,
    hovered: Cell<Option<LogicalRect>>,
}

impl Snap {
    pub fn new(windows: &[LogicalRect], monitors: &[LogicalRect]) -> Self {
        Self {
            windows: clip_to_monitors(windows, monitors),
            hovered: Cell::new(None),
        }
    }

    pub fn hovered(&self) -> Option<LogicalRect> {
        self.hovered.get()
    }

    pub fn hover(&self, point: (f64, f64)) -> bool {
        let hovered = window_at(&self.windows, point);
        self.hovered.replace(hovered) != hovered
    }

    pub fn at(&self, point: (f64, f64)) -> Option<LogicalRect> {
        window_at(&self.windows, point)
    }
}

pub fn is_click(start: (f64, f64), end: (f64, f64)) -> bool {
    (end.0 - start.0).abs() < CLICK_SLOP && (end.1 - start.1).abs() < CLICK_SLOP
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn treats_a_press_without_movement_as_a_click() {
        assert!(is_click((10.0, 10.0), (12.0, 8.5)));
        assert!(!is_click((10.0, 10.0), (13.0, 10.0)));
        assert!(!is_click((10.0, 10.0), (10.0, 6.0)));
    }

    #[test]
    fn reports_a_hover_change_only_when_the_window_under_the_cursor_changes() {
        let snap = Snap::new(
            &[rect(100, 100, 200, 200), rect(-50, 0, 1000, 1000)],
            &[rect(0, 0, 1920, 1080)],
        );
        assert!(snap.hover((150.0, 150.0)));
        assert!(!snap.hover((160.0, 150.0)));
        assert_eq!(snap.hovered(), Some(rect(100, 100, 200, 200)));
        assert!(snap.hover((10.0, 10.0)));
        assert_eq!(snap.hovered(), Some(rect(0, 0, 950, 1000)));
        assert!(snap.hover((1500.0, 10.0)));
        assert_eq!(snap.hovered(), None);
    }
}
