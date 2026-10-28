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

    pub fn has_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    pub fn hovered(&self) -> Option<LogicalRect> {
        self.hovered.get()
    }

    pub(super) fn cycle(&self, backwards: bool) -> Option<LogicalRect> {
        let current = self
            .windows
            .iter()
            .position(|rect| Some(*rect) == self.hovered.get());
        let next = cycle_index(current, self.windows.len(), backwards)?;
        let rect = self.windows[next];
        self.hovered.set(Some(rect));
        Some(rect)
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

pub(super) fn cycle_index(current: Option<usize>, count: usize, backwards: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current.filter(|index| *index < count) {
        None if backwards => count - 1,
        None => 0,
        Some(0) if backwards => count - 1,
        Some(index) if backwards => index - 1,
        Some(index) => (index + 1) % count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_windows_in_both_directions_and_handles_an_empty_desktop() {
        assert_eq!(cycle_index(None, 0, false), None);
        assert_eq!(cycle_index(None, 3, false), Some(0));
        assert_eq!(cycle_index(None, 3, true), Some(2));
        assert_eq!(cycle_index(Some(2), 3, false), Some(0));
        assert_eq!(cycle_index(Some(0), 3, true), Some(2));
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
