use crate::capture::LogicalRect;

pub const MIN_ZOOM: f64 = 0.1;
pub const MAX_ZOOM: f64 = 8.0;
const ZOOM_STEP: f64 = 1.1;
const FIT_SHARE: f64 = 0.8;
const MIN_SIDE: i32 = 16;
const KEEP_VISIBLE: i32 = 32;

/// Keeps a zoom factor between the smallest and largest a pin allows.
///
/// A factor that is not a finite number falls back to 1.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(clamp_zoom(20.0), MAX_ZOOM);
/// ```
pub fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

/// Zooms in by `steps` notches, or out for negative steps, and clamps the result.
///
/// # Examples
///
/// ```ignore
/// let bigger = zoom_by(1.0, 1.0);
/// assert!(bigger > 1.0);
/// ```
pub fn zoom_by(zoom: f64, steps: f64) -> f64 {
    clamp_zoom(zoom * ZOOM_STEP.powf(steps))
}

/// Picks the starting zoom so a pin fits in most of the monitor, never enlarging it.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(fit_zoom((800.0, 600.0), (1920, 1080)), 1.0);
/// ```
pub fn fit_zoom(base: (f64, f64), room: (i32, i32)) -> f64 {
    if base.0 <= 0.0 || base.1 <= 0.0 || room.0 <= 0 || room.1 <= 0 {
        return 1.0;
    }
    let wide = f64::from(room.0) * FIT_SHARE / base.0;
    let high = f64::from(room.1) * FIT_SHARE / base.1;
    clamp_zoom(wide.min(high).min(1.0))
}

/// The on-screen size of a pin at `zoom`, never smaller than a grabbable minimum.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(scaled((400.0, 300.0), 0.5), (200, 150));
/// ```
pub fn scaled(base: (f64, f64), zoom: f64) -> (i32, i32) {
    let side = |length: f64| ((length * zoom).round() as i32).max(MIN_SIDE);
    (side(base.0), side(base.1))
}

/// Moves a pin's margins by a pointer offset and keeps part of it on the monitor.
///
/// On a layer-shell surface the pointer offset is measured in the surface, which moves with
/// each update, so adding it to the current margins tracks the pointer.
///
/// # Examples
///
/// ```ignore
/// let margins = dragged((100.0, 100.0), (20.0, -10.0), (300, 200), (1920, 1080));
/// assert_eq!(margins, (120.0, 90.0));
/// ```
pub fn dragged(
    margins: (f64, f64),
    offset: (f64, f64),
    size: (i32, i32),
    monitor: (i32, i32),
) -> (f64, f64) {
    contain((margins.0 + offset.0, margins.1 + offset.1), size, monitor)
}

/// Keeps margins so at least a strip of the pin stays on the monitor.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(contain((-500.0, 0.0), (300, 200), (1920, 1080)).0, -268.0);
/// ```
pub fn contain(margins: (f64, f64), size: (i32, i32), monitor: (i32, i32)) -> (f64, f64) {
    let axis = |margin: f64, length: i32, room: i32| {
        let keep = KEEP_VISIBLE.min(length);
        let low = f64::from(keep - length);
        let high = f64::from(room - keep).max(low);
        margin.clamp(low, high)
    };
    (
        axis(margins.0, size.0, monitor.0),
        axis(margins.1, size.1, monitor.1),
    )
}

/// Shifts margins so a pin resized from `before` to `after` keeps its center in place.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(recentered((100.0, 100.0), (200, 200), (100, 100)), (150.0, 150.0));
/// ```
pub fn recentered(margins: (f64, f64), before: (i32, i32), after: (i32, i32)) -> (f64, f64) {
    (
        margins.0 + f64::from(before.0 - after.0) / 2.0,
        margins.1 + f64::from(before.1 - after.1) / 2.0,
    )
}

/// Finds the monitor a desktop rectangle starts on and its offset inside that monitor.
///
/// # Examples
///
/// ```ignore
/// let (index, margins) = place(selection, &monitors).unwrap();
/// ```
pub fn place(rect: LogicalRect, monitors: &[LogicalRect]) -> Option<(usize, (i32, i32))> {
    monitors
        .iter()
        .position(|monitor| monitor.contains_point(rect.x, rect.y))
        .map(|index| {
            let monitor = monitors[index];
            (index, (rect.x - monitor.x, rect.y - monitor.y))
        })
}

/// Margins that center a pin of `size` on a monitor of `room`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(centered((200, 100), (1920, 1080)), (860, 490));
/// ```
pub fn centered(size: (i32, i32), room: (i32, i32)) -> (i32, i32) {
    ((room.0 - size.0) / 2, (room.1 - size.1) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32) = (1920, 1080);

    fn rect(x: i32, y: i32, width: i32, height: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn clamps_zoom_to_its_range() {
        assert_eq!(clamp_zoom(0.01), MIN_ZOOM);
        assert_eq!(clamp_zoom(40.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(2.5), 2.5);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn zooms_in_and_back_out_by_the_same_steps() {
        let bigger = zoom_by(1.0, 3.0);
        assert!(bigger > 1.0);
        assert!((zoom_by(bigger, -3.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn stops_zooming_at_the_limits() {
        assert_eq!(zoom_by(MAX_ZOOM, 5.0), MAX_ZOOM);
        assert_eq!(zoom_by(MIN_ZOOM, -5.0), MIN_ZOOM);
    }

    #[test]
    fn keeps_a_small_pin_at_its_own_size() {
        assert_eq!(fit_zoom((800.0, 600.0), SCREEN), 1.0);
    }

    #[test]
    fn shrinks_a_pin_larger_than_the_monitor() {
        let zoom = fit_zoom((3840.0, 2160.0), SCREEN);
        assert!((zoom - 0.4).abs() < 1e-9);
    }

    #[test]
    fn never_scales_a_pin_below_a_grabbable_size() {
        assert_eq!(scaled((400.0, 300.0), 0.5), (200, 150));
        assert_eq!(scaled((40.0, 10.0), 0.1), (16, 16));
    }

    #[test]
    fn follows_the_pointer_while_dragging() {
        assert_eq!(
            dragged((100.0, 100.0), (20.0, -10.0), (300, 200), SCREEN),
            (120.0, 90.0)
        );
    }

    #[test]
    fn keeps_a_strip_of_the_pin_on_screen() {
        assert_eq!(
            dragged((0.0, 0.0), (-5000.0, -5000.0), (300, 200), SCREEN),
            (-268.0, -168.0)
        );
        assert_eq!(
            dragged((0.0, 0.0), (5000.0, 5000.0), (300, 200), SCREEN),
            (1888.0, 1048.0)
        );
    }

    #[test]
    fn keeps_a_tiny_pin_fully_reachable() {
        assert_eq!(contain((-50.0, 2000.0), (16, 16), SCREEN), (0.0, 1064.0));
    }

    #[test]
    fn zooms_around_the_center() {
        assert_eq!(
            recentered((100.0, 100.0), (200, 200), (100, 100)),
            (150.0, 150.0)
        );
        assert_eq!(
            recentered((100.0, 100.0), (100, 100), (300, 200)),
            (0.0, 50.0)
        );
    }

    #[test]
    fn places_a_selection_on_the_monitor_it_starts_on() {
        let monitors = [rect(0, 0, 1920, 1080), rect(1920, 0, 2560, 1440)];
        assert_eq!(
            place(rect(2000, 300, 400, 200), &monitors),
            Some((1, (80, 300)))
        );
        assert_eq!(
            place(rect(100, 50, 400, 200), &monitors),
            Some((0, (100, 50)))
        );
        assert_eq!(place(rect(-500, 0, 10, 10), &monitors), None);
    }

    #[test]
    fn centers_a_pin_on_its_monitor() {
        assert_eq!(centered((200, 100), SCREEN), (860, 490));
    }
}
