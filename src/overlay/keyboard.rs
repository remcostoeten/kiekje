use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, graphene};

use super::{
    LogicalRect, MIN_SELECTION, Shared, choose, picks_windows, pointer_frame, rect_from_drag,
};

#[derive(Clone, Copy, Default)]
pub(super) struct State {
    point: Option<(f64, f64)>,
    anchor: Option<(f64, f64)>,
    monitor: Option<usize>,
    pub(super) preview: Option<LogicalRect>,
}

fn moved(point: (f64, f64), delta: (f64, f64), bounds: LogicalRect) -> (f64, f64) {
    (
        (point.0 + delta.0).clamp(f64::from(bounds.x), f64::from(bounds.right())),
        (point.1 + delta.1).clamp(f64::from(bounds.y), f64::from(bounds.bottom())),
    )
}

fn bounds(shared: &Shared) -> LogicalRect {
    let first = shared.geometry[0].0;
    shared.geometry.iter().fold(first, |bounds, (rect, _)| {
        let x = bounds.x.min(rect.x);
        let y = bounds.y.min(rect.y);
        LogicalRect {
            x,
            y,
            width: bounds.right().max(rect.right()) - x,
            height: bounds.bottom().max(rect.bottom()) - y,
        }
    })
}

fn point(shared: &Shared, state: State) -> (f64, f64) {
    state.point.or(shared.pointer.get()).unwrap_or_else(|| {
        let monitor = shared.geometry[pointer_frame(shared)].0;
        (
            f64::from(monitor.x) + f64::from(monitor.width) / 2.0,
            f64::from(monitor.y) + f64::from(monitor.height) / 2.0,
        )
    })
}

fn update(shared: &Shared, state: State, message: &str) {
    shared.keyboard.set(state);
    if let Some(point) = state.point {
        super::point_at(shared, Some(point));
    }
    for area in shared.areas.borrow().iter() {
        area.layer.queue_draw();
    }
    for window in shared.windows.borrow().iter() {
        window.update_property(&[gtk::accessible::Property::Label(message)]);
    }
}

pub(super) fn press(shared: &Rc<Shared>, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    if modifiers.intersects(
        gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK,
    ) {
        return false;
    }
    let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
    let mut state = shared.keyboard.get();
    let key = key.to_lower();
    match key {
        gdk::Key::w | gdk::Key::Tab | gdk::Key::ISO_Left_Tab => {
            state = State {
                preview: shared.snap.cycle(shift || key == gdk::Key::ISO_Left_Tab),
                ..State::default()
            };
            update(
                shared,
                state,
                "Window selected. W or Tab cycles windows, Enter captures, Escape cancels.",
            );
        }
        gdk::Key::f if !picks_windows(shared) => {
            if shared.geometry.len() == 1 {
                return choose(shared, shared.geometry[0].0);
            }
            let index = match state.monitor {
                None => pointer_frame(shared),
                current => {
                    super::snap::cycle_index(current, shared.geometry.len(), shift).unwrap_or(0)
                }
            };
            let monitor = shared.geometry[index].0;
            state = State {
                monitor: Some(index),
                preview: Some(monitor),
                point: Some((
                    f64::from(monitor.x) + f64::from(monitor.width) / 2.0,
                    f64::from(monitor.y) + f64::from(monitor.height) / 2.0,
                )),
                ..State::default()
            };
            update(
                shared,
                state,
                &format!(
                    "Monitor {} selected. F cycles monitors, Enter captures, Escape cancels.",
                    index + 1
                ),
            );
        }
        gdk::Key::Left | gdk::Key::Right | gdk::Key::Up | gdk::Key::Down
            if !picks_windows(shared) =>
        {
            let step = if shift { 10.0 } else { 1.0 };
            let (monitor, pixels) = shared.geometry[pointer_frame(shared)];
            let dx = step * f64::from(monitor.width) / f64::from(pixels.0.max(1));
            let dy = step * f64::from(monitor.height) / f64::from(pixels.1.max(1));
            let delta = match key {
                gdk::Key::Left => (-dx, 0.0),
                gdk::Key::Right => (dx, 0.0),
                gdk::Key::Up => (0.0, -dy),
                _ => (0.0, dy),
            };
            let next = moved(point(shared, state), delta, bounds(shared));
            state.point = Some(next);
            state.preview = state.anchor.map(|start| rect_from_drag(start, next));
            state.monitor = None;
            update(
                shared,
                state,
                &format!(
                    "Selection cursor at {:.0}, {:.0}. Space sets a corner, arrows move, Shift moves faster, Enter finishes.",
                    next.0, next.1
                ),
            );
        }
        gdk::Key::space if !picks_windows(shared) => {
            if state.anchor.is_some() {
                if let Some(rect) = state.preview.filter(valid_region) {
                    return choose(shared, rect);
                }
            } else {
                let at = point(shared, state);
                state.point = Some(at);
                state.anchor = Some(at);
                state.preview = None;
                state.monitor = None;
            }
            update(
                shared,
                state,
                "First corner set. Arrows move the second corner, Space or Enter finishes, Escape cancels.",
            );
        }
        gdk::Key::Return | gdk::Key::KP_Enter => {
            let rect = if state.anchor.is_some() {
                state.preview.filter(valid_region)
            } else {
                state
                    .preview
                    .or(shared.snap.hovered())
                    .or_else(|| shared.snap.cycle(false))
            };
            if let Some(rect) = rect {
                return choose(shared, rect);
            }
            update(
                shared,
                state,
                "Choose a region with Space and arrows, or a window with W. Escape cancels.",
            );
        }
        _ => return false,
    }
    true
}

fn valid_region(rect: &LogicalRect) -> bool {
    rect.width >= MIN_SELECTION && rect.height >= MIN_SELECTION
}

pub(super) fn paint(shared: &Shared, frame_index: usize, snapshot: &gtk::Snapshot) {
    let Some((x, y)) = shared.keyboard.get().point else {
        return;
    };
    let monitor = shared.geometry[frame_index].0;
    let (x, y) = (
        (x - f64::from(monitor.x)) as f32,
        (y - f64::from(monitor.y)) as f32,
    );
    for (colour, thickness) in [(gdk::RGBA::BLACK, 3.0), (gdk::RGBA::WHITE, 1.0)] {
        snapshot.append_color(
            &colour,
            &graphene::Rect::new(x - 9.0, y - thickness / 2.0, 18.0, thickness),
        );
        snapshot.append_color(
            &colour,
            &graphene::Rect::new(x - thickness / 2.0, y - 9.0, thickness, 18.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_fractional_cursor_across_negative_monitor_coordinates() {
        let bounds = LogicalRect {
            x: -100,
            y: -50,
            width: 200,
            height: 100,
        };
        assert_eq!(moved((-0.5, 0.0), (0.5, 0.0), bounds), (0.0, 0.0));
        assert_eq!(
            moved((-99.0, -49.0), (-10.0, -10.0), bounds),
            (-100.0, -50.0)
        );
        assert_eq!(moved((99.0, 49.0), (10.0, 10.0), bounds), (100.0, 50.0));
    }

    #[test]
    fn refuses_empty_keyboard_regions() {
        assert!(!valid_region(&rect_from_drag((10.0, 10.0), (10.0, 30.0))));
        assert!(valid_region(&rect_from_drag((10.0, 10.0), (30.0, 30.0))));
    }
}
