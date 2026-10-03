use super::LogicalRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Converts a monitor-local logical selection to source pixels of that monitor's frame.
///
/// The scale is derived from the frame size versus the monitor's logical size, so fractional
/// scaling and `native-resolution` captures map correctly. Edges round outward and the
/// result is clamped to the frame.
///
/// # Examples
///
/// ```ignore
/// let selection = LogicalRect { x: 10, y: 10, width: 100, height: 50 };
/// let pixels = logical_to_pixels(selection, (1920, 1080), (2880, 1620));
/// assert_eq!(pixels, Some(PixelRect { x: 15, y: 15, width: 150, height: 75 }));
/// ```
pub fn logical_to_pixels(
    selection: LogicalRect,
    monitor_size: (i32, i32),
    frame_size: (u32, u32),
) -> Option<PixelRect> {
    if monitor_size.0 <= 0 || monitor_size.1 <= 0 || selection.width <= 0 || selection.height <= 0 {
        return None;
    }
    let scale_x = f64::from(frame_size.0) / f64::from(monitor_size.0);
    let scale_y = f64::from(frame_size.1) / f64::from(monitor_size.1);
    let clamp = |value: f64, max: u32| value.clamp(0.0, f64::from(max)) as u32;

    let left = clamp((f64::from(selection.x) * scale_x).floor(), frame_size.0);
    let top = clamp((f64::from(selection.y) * scale_y).floor(), frame_size.1);
    let right = clamp(
        (f64::from(selection.x + selection.width) * scale_x).ceil(),
        frame_size.0,
    );
    let bottom = clamp(
        (f64::from(selection.y + selection.height) * scale_y).ceil(),
        frame_size.1,
    );

    if right <= left || bottom <= top {
        return None;
    }
    Some(PixelRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// Normalizes a drag from `start` to `end` into a rectangle with positive size.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(rect_from_drag((50.0, 40.0), (10.0, 20.0)).x, 10);
/// ```
pub fn rect_from_drag(start: (f64, f64), end: (f64, f64)) -> LogicalRect {
    let left = start.0.min(end.0).floor();
    let top = start.1.min(end.1).floor();
    let right = start.0.max(end.0).ceil();
    let bottom = start.1.max(end.1).ceil();
    LogicalRect {
        x: left as i32,
        y: top as i32,
        width: (right - left) as i32,
        height: (bottom - top) as i32,
    }
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
    fn maps_identity_at_scale_one() {
        let pixels = logical_to_pixels(rect(10, 20, 30, 40), (1024, 768), (1024, 768));
        assert_eq!(
            pixels,
            Some(PixelRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40
            })
        );
    }

    #[test]
    fn maps_selection_at_integer_scale() {
        let pixels = logical_to_pixels(rect(10, 20, 30, 40), (1920, 1080), (3840, 2160));
        assert_eq!(
            pixels,
            Some(PixelRect {
                x: 20,
                y: 40,
                width: 60,
                height: 80
            })
        );
    }

    #[test]
    fn rounds_outward_at_fractional_scale() {
        let pixels = logical_to_pixels(rect(1, 1, 3, 3), (2560, 1440), (3200, 1800));
        assert_eq!(
            pixels,
            Some(PixelRect {
                x: 1,
                y: 1,
                width: 4,
                height: 4
            })
        );
    }

    #[test]
    fn clamps_selection_past_the_frame_edge() {
        let pixels = logical_to_pixels(rect(1000, 700, 100, 100), (1024, 768), (1024, 768));
        assert_eq!(
            pixels,
            Some(PixelRect {
                x: 1000,
                y: 700,
                width: 24,
                height: 68
            })
        );
    }

    #[test]
    fn rejects_empty_selection() {
        assert_eq!(
            logical_to_pixels(rect(5, 5, 0, 10), (100, 100), (100, 100)),
            None
        );
        assert_eq!(
            logical_to_pixels(rect(200, 200, 10, 10), (100, 100), (100, 100)),
            None
        );
    }

    #[test]
    fn normalizes_reverse_drag() {
        assert_eq!(
            rect_from_drag((50.4, 40.0), (10.6, 20.2)),
            rect(10, 20, 41, 20)
        );
    }
}
