use super::LogicalRect;

/// Clips each window to the monitors it overlaps and drops windows that are on none of them.
///
/// `windows` holds window frames in desktop-wide logical coordinates, and both it and the result
/// are ordered topmost first. A window that spans monitors keeps the
/// bounding box of its visible parts, so a snapped selection never reaches outside the frames.
///
/// # Examples
///
/// ```ignore
/// let monitors = [LogicalRect { x: 0, y: 0, width: 1920, height: 1080 }];
/// let rects = clip_to_monitors(&windows, &monitors);
/// ```
pub fn clip_to_monitors(windows: &[LogicalRect], monitors: &[LogicalRect]) -> Vec<LogicalRect> {
    windows
        .iter()
        .filter_map(|window| {
            let parts: Vec<_> = monitors
                .iter()
                .filter_map(|monitor| monitor.intersection(*window))
                .collect();
            let left = parts.iter().map(|part| part.x).min()?;
            let top = parts.iter().map(|part| part.y).min()?;
            let right = parts.iter().map(|part| part.right()).max()?;
            let bottom = parts.iter().map(|part| part.bottom()).max()?;
            Some(LogicalRect {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            })
        })
        .collect()
}

/// Returns the topmost window rectangle under a desktop-wide logical point.
///
/// `windows` is ordered topmost first, so the first rectangle that contains the point wins.
///
/// # Examples
///
/// ```ignore
/// let hovered = window_at(&rects, (640.5, 320.0));
/// ```
pub fn window_at(windows: &[LogicalRect], point: (f64, f64)) -> Option<LogicalRect> {
    let (x, y) = (point.0.floor() as i32, point.1.floor() as i32);
    windows
        .iter()
        .copied()
        .find(|rect| rect.contains_point(x, y))
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
    fn picks_the_topmost_window_under_the_cursor() {
        let windows = [rect(100, 100, 200, 200), rect(0, 0, 1000, 1000)];
        assert_eq!(
            window_at(&windows, (150.0, 150.0)),
            Some(rect(100, 100, 200, 200))
        );
        assert_eq!(
            window_at(&windows, (50.0, 50.0)),
            Some(rect(0, 0, 1000, 1000))
        );
    }

    #[test]
    fn misses_when_no_window_is_under_the_cursor() {
        let windows = [rect(100, 100, 200, 200)];
        assert_eq!(window_at(&windows, (300.0, 150.0)), None);
        assert_eq!(window_at(&windows, (99.9, 150.0)), None);
        assert_eq!(window_at(&[], (0.0, 0.0)), None);
    }

    #[test]
    fn hits_windows_at_negative_desktop_coordinates() {
        let windows = [rect(-1920, 0, 800, 600)];
        assert_eq!(
            window_at(&windows, (-0.5 - 1200.0, 10.0)),
            Some(rect(-1920, 0, 800, 600))
        );
        assert_eq!(window_at(&windows, (-0.5, 10.0)), None);
    }

    #[test]
    fn clips_a_window_hanging_off_the_desktop_edge() {
        let monitors = [rect(0, 0, 1920, 1080)];
        assert_eq!(
            clip_to_monitors(&[rect(-50, 1000, 400, 300)], &monitors),
            [rect(0, 1000, 350, 80)]
        );
    }

    #[test]
    fn keeps_the_visible_bounds_of_a_window_across_monitors() {
        let monitors = [rect(0, 0, 1920, 1080), rect(1920, 0, 1280, 720)];
        assert_eq!(
            clip_to_monitors(&[rect(1800, 600, 300, 300)], &monitors),
            [rect(1800, 600, 300, 300)]
        );
        assert_eq!(
            clip_to_monitors(&[rect(1800, 900, 300, 300)], &monitors),
            [rect(1800, 900, 120, 180)]
        );
    }

    #[test]
    fn drops_windows_outside_every_monitor_and_keeps_stacking_order() {
        let monitors = [rect(0, 0, 1000, 1000)];
        let windows = [
            rect(10, 10, 50, 50),
            rect(2000, 2000, 50, 50),
            rect(0, 0, 500, 500),
        ];
        assert_eq!(
            clip_to_monitors(&windows, &monitors),
            [rect(10, 10, 50, 50), rect(0, 0, 500, 500)]
        );
    }
}
