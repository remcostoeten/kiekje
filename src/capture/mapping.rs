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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectionPlan {
    Crop {
        frame_index: usize,
        rect: PixelRect,
    },
    Compose {
        bounds: LogicalRect,
        ratio: f64,
        width: u32,
        height: u32,
    },
}

impl SelectionPlan {
    pub fn size(self) -> (u32, u32) {
        match self {
            Self::Crop { rect, .. } => (rect.width, rect.height),
            Self::Compose { width, height, .. } => (width, height),
        }
    }
}

/// Decides how a desktop-wide logical selection becomes an image.
///
/// A selection inside one monitor crops that frame at its native resolution. A selection that
/// crosses monitors is composed from every frame it touches at the highest density among them.
/// `frames` holds each frame's logical rectangle and pixel size.
///
/// # Examples
///
/// ```ignore
/// let frames = [(LogicalRect { x: 0, y: 0, width: 100, height: 100 }, (200, 200))];
/// let plan = plan_selection(LogicalRect { x: 10, y: 10, width: 20, height: 20 }, &frames);
/// assert_eq!(plan.map(SelectionPlan::size), Some((40, 40)));
/// ```
pub fn plan_selection(
    selection: LogicalRect,
    frames: &[(LogicalRect, (u32, u32))],
) -> Option<SelectionPlan> {
    let touched: Vec<_> = frames
        .iter()
        .filter_map(|(logical, size)| {
            logical
                .intersection(selection)
                .map(|part| (part, density(*logical, *size)))
        })
        .collect();
    let left = touched.iter().map(|(part, _)| part.x).min()?;
    let top = touched.iter().map(|(part, _)| part.y).min()?;
    let right = touched.iter().map(|(part, _)| part.right()).max()?;
    let bottom = touched.iter().map(|(part, _)| part.bottom()).max()?;
    let bounds = LogicalRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };

    if let Some((frame_index, (logical, size))) = frames
        .iter()
        .enumerate()
        .find(|(_, (logical, _))| logical.contains(bounds))
    {
        let local = LogicalRect {
            x: bounds.x - logical.x,
            y: bounds.y - logical.y,
            ..bounds
        };
        return logical_to_pixels(local, (logical.width, logical.height), *size)
            .map(|rect| SelectionPlan::Crop { frame_index, rect });
    }

    let ratio = touched
        .iter()
        .map(|(_, ratio)| *ratio)
        .fold(f64::MIN_POSITIVE, f64::max);
    Some(SelectionPlan::Compose {
        bounds,
        ratio,
        width: ((f64::from(bounds.width) * ratio).round() as u32).max(1),
        height: ((f64::from(bounds.height) * ratio).round() as u32).max(1),
    })
}

/// Pixels per logical pixel of a frame, measured horizontally.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(density(LogicalRect { x: 0, y: 0, width: 100, height: 50 }, (150, 75)), 1.5);
/// ```
pub fn density(logical: LogicalRect, size: (u32, u32)) -> f64 {
    f64::from(size.0) / f64::from(logical.width.max(1))
}

/// Converts a monitor-local logical point to the frame pixel under it.
///
/// Uses the same frame-to-monitor scale as [`logical_to_pixels`], so fractional scaling maps
/// to the physical pixel the point falls in. Points past an edge clamp to the nearest pixel.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(logical_point_to_pixel((10.5, 3.0), (1920, 1080), (2880, 1620)), Some((15, 4)));
/// ```
pub fn logical_point_to_pixel(
    point: (f64, f64),
    monitor_size: (i32, i32),
    frame_size: (u32, u32),
) -> Option<(u32, u32)> {
    if monitor_size.0 <= 0 || monitor_size.1 <= 0 || frame_size.0 == 0 || frame_size.1 == 0 {
        return None;
    }
    let map = |value: f64, monitor: i32, frame: u32| {
        let pixel = (value * f64::from(frame) / f64::from(monitor)).floor();
        pixel.clamp(0.0, f64::from(frame - 1)) as u32
    };
    Some((
        map(point.0, monitor_size.0, frame_size.0),
        map(point.1, monitor_size.1, frame_size.1),
    ))
}

/// Converts frame pixels back to a monitor-local logical rectangle.
///
/// The inverse of [`logical_to_pixels`]: edges round outward, so the logical rectangle always
/// covers every pixel of `rect`, and the result is clamped to the monitor.
///
/// # Examples
///
/// ```ignore
/// let pixels = PixelRect { x: 15, y: 15, width: 150, height: 75 };
/// let logical = pixels_to_logical(pixels, (1920, 1080), (2880, 1620));
/// assert_eq!(logical, Some(LogicalRect { x: 10, y: 10, width: 100, height: 50 }));
/// ```
pub fn pixels_to_logical(
    rect: PixelRect,
    monitor_size: (i32, i32),
    frame_size: (u32, u32),
) -> Option<LogicalRect> {
    if monitor_size.0 <= 0 || monitor_size.1 <= 0 || frame_size.0 == 0 || frame_size.1 == 0 {
        return None;
    }
    let scale_x = f64::from(monitor_size.0) / f64::from(frame_size.0);
    let scale_y = f64::from(monitor_size.1) / f64::from(frame_size.1);
    let clamp = |value: f64, max: i32| value.clamp(0.0, f64::from(max)) as i32;
    let left = clamp((f64::from(rect.x) * scale_x).floor(), monitor_size.0);
    let top = clamp((f64::from(rect.y) * scale_y).floor(), monitor_size.1);
    let right = clamp(
        (f64::from(rect.x + rect.width) * scale_x).ceil(),
        monitor_size.0,
    );
    let bottom = clamp(
        (f64::from(rect.y + rect.height) * scale_y).ceil(),
        monitor_size.1,
    );
    (right > left && bottom > top).then_some(LogicalRect {
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
    fn maps_pixels_back_to_logical_at_fractional_scale() {
        let pixels = PixelRect {
            x: 15,
            y: 15,
            width: 150,
            height: 75,
        };
        assert_eq!(
            pixels_to_logical(pixels, (1920, 1080), (2880, 1620)),
            Some(rect(10, 10, 100, 50))
        );
    }

    #[test]
    fn rounds_odd_pixels_outward_to_cover_them() {
        let pixels = PixelRect {
            x: 1,
            y: 1,
            width: 1,
            height: 1,
        };
        assert_eq!(
            pixels_to_logical(pixels, (1920, 1080), (2880, 1620)),
            Some(rect(0, 0, 2, 2))
        );
    }

    #[test]
    fn round_trips_a_full_monitor() {
        let full = logical_to_pixels(rect(0, 0, 2560, 1440), (2560, 1440), (3840, 2160));
        assert_eq!(
            full.and_then(|pixels| pixels_to_logical(pixels, (2560, 1440), (3840, 2160))),
            Some(rect(0, 0, 2560, 1440))
        );
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
    fn crops_natively_when_the_selection_fits_one_monitor() {
        let frames = [
            (rect(0, 0, 1920, 1080), (1920, 1080)),
            (rect(1920, 0, 1280, 720), (2560, 1440)),
        ];
        assert_eq!(
            plan_selection(rect(2000, 100, 100, 50), &frames),
            Some(SelectionPlan::Crop {
                frame_index: 1,
                rect: PixelRect {
                    x: 160,
                    y: 200,
                    width: 200,
                    height: 100
                }
            })
        );
    }

    #[test]
    fn composes_a_selection_across_monitors_at_the_highest_density() {
        let frames = [
            (rect(0, 0, 1920, 1080), (1920, 1080)),
            (rect(1920, 0, 1280, 720), (2560, 1440)),
        ];
        assert_eq!(
            plan_selection(rect(1820, 100, 200, 50), &frames),
            Some(SelectionPlan::Compose {
                bounds: rect(1820, 100, 200, 50),
                ratio: 2.0,
                width: 400,
                height: 100
            })
        );
    }

    #[test]
    fn clips_a_selection_to_the_monitors_it_touches() {
        let frames = [
            (rect(-1000, 0, 1000, 800), (1000, 800)),
            (rect(0, 0, 1000, 500), (1000, 500)),
        ];
        assert_eq!(
            plan_selection(rect(-100, 400, 300, 300), &frames).map(SelectionPlan::size),
            Some((300, 300))
        );
        assert_eq!(
            plan_selection(rect(-100, 700, 300, 300), &frames),
            Some(SelectionPlan::Crop {
                frame_index: 0,
                rect: PixelRect {
                    x: 900,
                    y: 700,
                    width: 100,
                    height: 100
                }
            })
        );
    }

    #[test]
    fn rejects_a_selection_outside_every_monitor() {
        let frames = [(rect(0, 0, 100, 100), (100, 100))];
        assert_eq!(plan_selection(rect(200, 200, 10, 10), &frames), None);
    }

    #[test]
    fn maps_a_point_to_its_pixel_at_scale_one() {
        assert_eq!(
            logical_point_to_pixel((10.9, 20.0), (1024, 768), (1024, 768)),
            Some((10, 20))
        );
    }

    #[test]
    fn maps_a_point_to_its_pixel_at_fractional_scale() {
        assert_eq!(
            logical_point_to_pixel((10.5, 3.0), (1920, 1080), (2880, 1620)),
            Some((15, 4))
        );
        assert_eq!(
            logical_point_to_pixel((1.0, 1.0), (2560, 1440), (3200, 1800)),
            Some((1, 1))
        );
        assert_eq!(
            logical_point_to_pixel((2.0, 2.0), (2560, 1440), (3200, 1800)),
            Some((2, 2))
        );
    }

    #[test]
    fn clamps_a_point_past_the_frame_edge_to_the_last_pixel() {
        assert_eq!(
            logical_point_to_pixel((1920.0, -4.0), (1920, 1080), (3840, 2160)),
            Some((3839, 0))
        );
    }

    #[test]
    fn rejects_a_point_on_an_empty_monitor_or_frame() {
        assert_eq!(
            logical_point_to_pixel((1.0, 1.0), (0, 100), (100, 100)),
            None
        );
        assert_eq!(
            logical_point_to_pixel((1.0, 1.0), (100, 100), (0, 100)),
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
