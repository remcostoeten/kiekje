use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::TURN_STEP_DEGREES;
use super::canvas::View;
use super::document::{Area, Color, Edit, Point, Shape, Tool};
use super::shapes::Handle;

const REACH: f64 = 8.0;
const HINT_OFFSET: f64 = 14.0;
const HINT_RADIUS: f64 = 7.0;
const HINT_SWEEP: f64 = 2.2;
const HINT_ARROW: f64 = 4.0;
const HANDLE: f64 = 8.0;
const OUTSET: f64 = 4.0;

fn cycled_shape(current: Option<usize>, count: usize, backwards: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current.filter(|index| *index < count) {
        Some(0) if backwards => count - 1,
        Some(index) if backwards => index - 1,
        Some(index) => (index + 1) % count,
        None if backwards => count - 1,
        None => 0,
    })
}

#[cfg(test)]
mod keyboard_tests {
    use super::cycled_shape;

    #[test]
    fn cycles_in_drawing_order_and_wraps_both_ways() {
        assert_eq!(cycled_shape(None, 3, false), Some(0));
        assert_eq!(cycled_shape(Some(0), 3, false), Some(1));
        assert_eq!(cycled_shape(Some(2), 3, false), Some(0));
        assert_eq!(cycled_shape(None, 3, true), Some(2));
        assert_eq!(cycled_shape(Some(0), 3, true), Some(2));
        assert_eq!(cycled_shape(Some(2), 3, true), Some(1));
    }

    #[test]
    fn handles_empty_and_stale_selections() {
        assert_eq!(cycled_shape(Some(0), 0, false), None);
        assert_eq!(cycled_shape(Some(9), 3, false), Some(0));
        assert_eq!(cycled_shape(Some(9), 3, true), Some(2));
        assert_eq!(cycled_shape(Some(0), 1, true), Some(0));
    }
}

fn shape_name(shape: &Shape) -> &'static str {
    match shape {
        Shape::Rectangle { .. } => "Rectangle",
        Shape::Ellipse { .. } => "Ellipse",
        Shape::Line { .. } => "Line",
        Shape::Arrow { .. } => "Arrow",
        Shape::Pen { .. } => "Pen",
        Shape::Highlight { .. } => "Marker",
        Shape::Step { .. } => "Step",
        Shape::Pixelate { .. } => "Pixelate",
        Shape::Text { .. } => "Text",
        Shape::Mark { .. } => "Mark",
        Shape::Emoji { .. } => "Emoji",
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Grab {
    Body,
    Handle(Handle),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Marquee {
    start: Point,
    end: Point,
    extend: bool,
}

impl Marquee {
    fn area(self) -> Area {
        Area::between(self.start, self.end)
    }
}

pub(super) struct Moving {
    pub(super) index: usize,
    pub(super) preview: Shape,
    original: Shape,
    grab: Grab,
    start: Point,
    others: Vec<(usize, Shape, Shape)>,
    delta: (f64, f64),
    group: Option<Point>,
}

impl Moving {
    pub(super) fn preview_of(&self, index: usize) -> Option<Shape> {
        if index == self.index {
            return Some(self.preview.clone());
        }
        self.others
            .iter()
            .find(|(other, _, _)| *other == index)
            .map(|(_, _, preview)| preview.clone())
    }
}

pub(super) fn tool_cursor(tool: Tool) -> &'static str {
    match tool {
        Tool::Text => "text",
        Tool::Select => "default",
        _ => "crosshair",
    }
}

fn batch(mut edits: Vec<Edit>) -> Edit {
    if edits.len() == 1 {
        edits.remove(0)
    } else {
        Edit::Batch(edits)
    }
}

fn group_swing(pivot: Point, start: Point, now: Point, stepped: bool) -> f64 {
    let before = (start.y - pivot.y).atan2(start.x - pivot.x);
    let after = (now.y - pivot.y).atan2(now.x - pivot.x);
    let swing = (after - before).to_degrees();
    if stepped { stepped_angle(swing) } else { swing }
}

fn stepped_angle(degrees: f64) -> f64 {
    (degrees / TURN_STEP_DEGREES).round() * TURN_STEP_DEGREES
}

impl Editor {
    pub(super) fn set_tool(&self, tool: Tool) {
        self.tool.set(tool);
        self.cancel_draft();
        self.finish_text(true);
        self.canvas.set_cursor_from_name(Some(tool_cursor(tool)));
        if tool != Tool::Select {
            self.deselect();
        }
        match tool {
            Tool::Crop if self.is_inline() => self.say(
                "Space starts a region, arrows set its endpoint, Space or Enter finishes. Arrows move an existing region; Ctrl+arrows resize it.",
                false,
            ),
            Tool::Crop => self.say(
                "Space starts a crop, arrows set its endpoint, Space or Enter finishes. Click once to clear the crop.",
                false,
            ),
            Tool::Text => self.say("Arrows position text, Space starts typing, Enter commits it. Clicking also places text.", false),
            Tool::Check | Tool::Cross => self.say(
                "Click to place it. Arrows move the cursor, Space places it, scroll sizes it.",
                false,
            ),
            Tool::Emoji => self.say(
                "Click to place it. Arrows move the cursor, Space places it, scroll sizes it, \
                 right-click or Shift+F10 picks the emoji.",
                false,
            ),
            Tool::Select => self.say(
                "Click a shape, then drag to move, drag a handle to resize, Delete removes, \
                 arrows nudge. Page Up and Page Down select shapes. Drag on empty canvas to select several, Ctrl adds to the selection.",
                false,
            ),
            _ => self.say("Arrows move 1 pixel, Shift moves 10. Space starts a shape, Space or Enter finishes it. Page Up and Page Down select shapes.", false),
        }
    }

    fn slack(&self) -> f64 {
        REACH / self.view.get().zoom()
    }

    fn scale(&self) -> f64 {
        1.0 / self.view.get().zoom()
    }

    pub(super) fn hover_turn_zone(&self, point: Point) {
        let hint = if self.tool.get() != Tool::Select || self.moving.borrow().is_some() {
            None
        } else if let Some(group) = self.group_bounds() {
            group.turn_zone(point, self.scale())
        } else {
            self.selected.get().and_then(|index| {
                self.document
                    .borrow()
                    .shapes()
                    .get(index)?
                    .turn_zone(point, self.scale())
            })
        };
        if self.turn_hint.replace(hint) != hint {
            self.canvas.queue_draw();
        }
    }

    fn paint_turn_hint(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let (Some(corner), Some(index)) = (self.turn_hint.get(), self.selected.get()) else {
            return Ok(());
        };
        let pivot = match self.group_bounds() {
            Some(group) => group.centre(),
            None => match self.document.borrow().shapes().get(index) {
                Some(shape) => shape.pivot(),
                None => return Ok(()),
            },
        };
        let (x, y) = view.to_widget(corner);
        let (px, py) = view.to_widget(pivot);
        let outward = (y - py).atan2(x - px);
        let (cx, cy) = (
            x + outward.cos() * HINT_OFFSET,
            y + outward.sin() * HINT_OFFSET,
        );
        let (from, to) = (outward - HINT_SWEEP / 2.0, outward + HINT_SWEEP / 2.0);
        let (red, green, blue) = Color::Accent.rgb();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        cr.set_line_width(3.0);
        cr.arc(cx, cy, HINT_RADIUS, from, to);
        cr.stroke_preserve()?;
        cr.set_source_rgb(red, green, blue);
        cr.set_line_width(1.5);
        cr.stroke()?;
        for (angle, sign) in [(from, -1.0), (to, 1.0)] {
            let tip = (
                cx + angle.cos() * HINT_RADIUS,
                cy + angle.sin() * HINT_RADIUS,
            );
            let along = angle + sign * std::f64::consts::FRAC_PI_2;
            for spread in [-0.6, 0.6] {
                let direction = along + std::f64::consts::PI + spread;
                cr.move_to(tip.0, tip.1);
                cr.line_to(
                    tip.0 + direction.cos() * HINT_ARROW,
                    tip.1 + direction.sin() * HINT_ARROW,
                );
            }
        }
        cr.set_source_rgb(red, green, blue);
        cr.stroke()
    }

    pub(super) fn grab(&self, point: Point, extend: bool) {
        let slack = self.slack();
        let single = self.also_selected.borrow().is_empty();
        let hit = {
            let document = self.document.borrow();
            let on_handle = self.selected.get().and_then(|index| {
                let turning = if single {
                    let shape = document.shapes().get(index)?;
                    shape
                        .handles()
                        .into_iter()
                        .find(|(_, at)| at.distance(point) <= slack)
                        .map(|(handle, _)| handle)
                        .or_else(|| shape.turn_zone(point, self.scale()).map(|_| Handle::Rotate))
                } else {
                    let group = self.group_bounds()?;
                    (group.turn_handle().distance(point) <= slack
                        || group.turn_zone(point, self.scale()).is_some())
                    .then_some(Handle::Rotate)
                };
                Some((index, Grab::Handle(turning?)))
            });
            on_handle.or_else(|| document.hit(point, slack).map(|index| (index, Grab::Body)))
        };
        let previous = self.selected.get();
        match hit {
            Some((_, Grab::Handle(_))) => {}
            Some((index, Grab::Body)) if extend => {
                if !self.toggle_selected(index) {
                    self.moving.replace(None);
                    self.canvas.queue_draw();
                    return;
                }
            }
            Some((index, Grab::Body)) if self.is_selected(index) => {
                if let Some(primary) = previous.filter(|primary| *primary != index) {
                    let mut also = self.also_selected.borrow_mut();
                    also.retain(|other| *other != index);
                    also.push(primary);
                }
                self.selected.set(Some(index));
            }
            Some((index, _)) => {
                self.also_selected.borrow_mut().clear();
                self.selected.set(Some(index));
            }
            None => {
                if !extend {
                    self.also_selected.borrow_mut().clear();
                    self.selected.set(None);
                }
                self.moving.replace(None);
                self.marquee.set(Some(Marquee {
                    start: point,
                    end: point,
                    extend,
                }));
                self.canvas.queue_draw();
                return;
            }
        }
        let target = hit.and_then(|(index, grab)| {
            let group = self.group_bounds().map(Area::centre);
            let document = self.document.borrow();
            let original = document.shapes().get(index)?.clone();
            let others = match grab {
                Grab::Body | Grab::Handle(Handle::Rotate) => self
                    .also_selected
                    .borrow()
                    .iter()
                    .filter_map(|other| {
                        let shape = document.shapes().get(*other)?.clone();
                        Some((*other, shape.clone(), shape))
                    })
                    .collect(),
                Grab::Handle(_) => Vec::new(),
            };
            Some(Moving {
                index,
                preview: original.clone(),
                original,
                grab,
                start: point,
                others,
                delta: (0.0, 0.0),
                group,
            })
        });
        self.moving.replace(target);
        if let Some(index) = self.selected.get().filter(|index| previous != Some(*index)) {
            self.adopt(index);
        }
        self.canvas.queue_draw();
    }

    fn is_selected(&self, index: usize) -> bool {
        self.selected.get() == Some(index) || self.also_selected.borrow().contains(&index)
    }

    fn toggle_selected(&self, index: usize) -> bool {
        if self.selected.get() == Some(index) {
            let next = self.also_selected.borrow_mut().pop();
            self.selected.set(next);
            return false;
        }
        let mut also = self.also_selected.borrow_mut();
        if let Some(position) = also.iter().position(|other| *other == index) {
            also.remove(position);
            return false;
        }
        if let Some(primary) = self.selected.get() {
            also.push(primary);
        }
        drop(also);
        self.selected.set(Some(index));
        true
    }

    /// The upright area around every selected shape, when more than one is selected.
    fn group_bounds(&self) -> Option<Area> {
        if self.also_selected.borrow().is_empty() {
            return None;
        }
        let document = self.document.borrow();
        self.selection()
            .into_iter()
            .filter_map(|index| document.shapes().get(index).map(Shape::bounds))
            .reduce(Area::union)
    }

    pub(super) fn group_pivot(&self) -> Option<Point> {
        self.group_bounds().map(Area::centre)
    }

    /// Every selected shape index, lowest first.
    fn selection(&self) -> Vec<usize> {
        let mut all: Vec<usize> = self.also_selected.borrow().clone();
        all.extend(self.selected.get());
        all.sort_unstable();
        all.dedup();
        all
    }

    pub(super) fn cycle_shape(&self, backwards: bool) -> bool {
        let count = self.document.borrow().shapes().len();
        let Some(index) = cycled_shape(self.selected.get(), count, backwards) else {
            self.say("There are no annotations to select.", false);
            return true;
        };
        self.pick(Tool::Select);
        self.keyboard_cursor.set(false);
        self.moving.take();
        self.selected.set(Some(index));
        self.also_selected.borrow_mut().clear();
        self.adopt(index);
        let document = self.document.borrow();
        let shape = &document.shapes()[index];
        let bounds = shape.bounds();
        let description = format!(
            "{} {} of {} selected at {:.0}, {:.0}, size {:.0} by {:.0}. Arrows move 1 pixel, Shift moves 10. Delete removes it. Page Up and Page Down select shapes.",
            shape_name(shape),
            index + 1,
            count,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        );
        self.canvas
            .update_property(&[gtk::accessible::Property::Description(&description)]);
        self.say(&description, false);
        self.canvas.queue_draw();
        true
    }

    pub(super) fn select_all(&self) {
        let count = self.document.borrow().shapes().len();
        let Some(last) = count.checked_sub(1) else {
            return;
        };
        self.moving.take();
        self.selected.set(Some(last));
        self.also_selected.replace((0..last).collect());
        self.say(&format!("{count} shapes selected."), false);
        self.canvas.queue_draw();
    }

    fn adopt(&self, index: usize) {
        let Some(shape) = self.document.borrow().shapes().get(index).cloned() else {
            return;
        };
        self.adopting.set(true);
        match (shape.stroke(), &shape) {
            (Some(stroke), _) => {
                let (size, opacity) = stroke.settings((self.picture.width, self.picture.height));
                self.set_color(stroke.color);
                self.size.set_value(f64::from(size));
                self.opacity.set_value(f64::from(opacity));
            }
            (None, Shape::Highlight { color, .. }) => self.set_color(*color),
            (None, _) => {}
        }
        self.adopting.set(false);
    }

    pub(super) fn drag_to(&self, point: Point, stepped: bool) {
        if let Some(marquee) = self.marquee.get() {
            self.marquee.set(Some(Marquee {
                end: point,
                ..marquee
            }));
            self.canvas.queue_draw();
            return;
        }
        if let Some(moving) = self.moving.borrow_mut().as_mut() {
            moving.delta = (point.x - moving.start.x, point.y - moving.start.y);
            match (moving.grab, moving.group) {
                (Grab::Handle(Handle::Rotate), Some(pivot)) => {
                    let swing = group_swing(pivot, moving.start, point, stepped);
                    moving.preview = moving.original.orbited(swing, pivot);
                    for (_, original, preview) in moving.others.iter_mut() {
                        *preview = original.orbited(swing, pivot);
                    }
                }
                (Grab::Body, _) => {
                    moving.preview = moving.original.moved(moving.delta.0, moving.delta.1);
                    for (_, original, preview) in moving.others.iter_mut() {
                        *preview = original.moved(moving.delta.0, moving.delta.1);
                    }
                }
                (Grab::Handle(Handle::Rotate), None) if stepped => {
                    let free = moving.original.resized(Handle::Rotate, point);
                    moving.preview = free.turned(stepped_angle(free.angle()));
                }
                (Grab::Handle(handle), _) => {
                    moving.preview = moving.original.resized(handle, point);
                }
            }
        }
        self.canvas.queue_draw();
    }

    pub(super) fn release(&self) {
        if let Some(marquee) = self.marquee.take() {
            self.select_within(marquee);
            return;
        }
        let Some(moving) = self.moving.take() else {
            return;
        };
        if moving.preview != moving.original {
            let mut edits = vec![Edit::Replace {
                index: moving.index,
                before: moving.original,
                after: moving.preview,
            }];
            for (index, before, after) in moving.others {
                if after != before {
                    edits.push(Edit::Replace {
                        index,
                        before,
                        after,
                    });
                }
            }
            self.document.borrow_mut().apply(batch(edits));
            self.edited();
        }
        self.canvas.queue_draw();
    }

    fn select_within(&self, marquee: Marquee) {
        let area = marquee.area();
        let mut picked: Vec<usize> = if area.width < 2.0 && area.height < 2.0 {
            Vec::new()
        } else {
            self.document
                .borrow()
                .shapes()
                .iter()
                .enumerate()
                .filter(|(_, shape)| area.overlaps(shape.extent()))
                .map(|(index, _)| index)
                .collect()
        };
        if marquee.extend {
            for index in self.selection() {
                if !picked.contains(&index) {
                    picked.push(index);
                }
            }
            picked.sort_unstable();
        }
        let primary = picked.pop();
        self.selected.set(primary);
        self.also_selected.replace(picked);
        if let Some(index) = primary {
            self.adopt(index);
            let count = self.selection().len();
            if count > 1 {
                self.say(&format!("{count} shapes selected."), false);
            }
        }
        self.canvas.queue_draw();
    }

    pub(super) fn cancel_grab(&self) -> bool {
        let had = self.moving.take().is_some() | self.marquee.take().is_some();
        self.canvas.queue_draw();
        had
    }

    pub(super) fn deselect(&self) -> bool {
        self.moving.take();
        self.also_selected.borrow_mut().clear();
        let had = self.selected.take().is_some();
        self.canvas.queue_draw();
        had
    }

    pub(super) fn nudge(&self, dx: f64, dy: f64) -> bool {
        self.replace_selected(|shape| shape.moved(dx, dy))
    }

    pub(super) fn restyle(&self) {
        if self.adopting.get() {
            return;
        }
        let Some(index) = self.selected.get() else {
            return;
        };
        let stroke = self.stroke();
        if !self.also_selected.borrow().is_empty() {
            self.replace_selected(|shape| shape.restyled(stroke));
            return;
        }
        let Some(before) = self.document.borrow().shapes().get(index).cloned() else {
            return;
        };
        let after = before.restyled(stroke);
        if after == before {
            return;
        }
        if let Some(moving) = self
            .moving
            .borrow_mut()
            .as_mut()
            .filter(|moving| moving.index == index)
        {
            moving.original = after.clone();
            moving.preview = moving.preview.restyled(stroke);
        }
        self.document.borrow_mut().restyle(index, after);
        self.edited();
    }

    pub(super) fn delete_selected(&self) -> bool {
        let edits: Vec<Edit> = {
            let document = self.document.borrow();
            self.selection()
                .into_iter()
                .rev()
                .filter_map(|index| {
                    let shape = document.shapes().get(index)?.clone();
                    Some(Edit::Remove { index, shape })
                })
                .collect()
        };
        if edits.is_empty() {
            return false;
        }
        self.document.borrow_mut().apply(batch(edits));
        self.deselect();
        self.edited();
        true
    }

    pub(super) fn replace_selected(&self, change: impl Fn(&Shape) -> Shape) -> bool {
        let selection = self.selection();
        if selection.is_empty() {
            return false;
        }
        let edits: Vec<Edit> = {
            let document = self.document.borrow();
            selection
                .into_iter()
                .filter_map(|index| {
                    let before = document.shapes().get(index)?.clone();
                    let after = change(&before);
                    (after != before).then_some(Edit::Replace {
                        index,
                        before,
                        after,
                    })
                })
                .collect()
        };
        if !edits.is_empty() {
            self.document.borrow_mut().apply(batch(edits));
            self.edited();
        }
        true
    }

    pub(super) fn paint_selection(
        &self,
        cr: &cairo::Context,
        view: View,
    ) -> Result<(), cairo::Error> {
        let single = self.also_selected.borrow().is_empty();
        for index in self.selection() {
            let moving = self.moving.borrow();
            let document = self.document.borrow();
            let shape = match moving.as_ref().and_then(|moving| moving.preview_of(index)) {
                Some(preview) => preview,
                None => match document.shapes().get(index) {
                    Some(shape) => shape.clone(),
                    None => continue,
                },
            };
            self.paint_outline(cr, view, &shape, single)?;
        }
        self.paint_group_frame(cr, view)?;
        self.paint_marquee(cr, view)?;
        self.paint_turn_hint(cr, view)
    }

    fn paint_marquee(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let Some(marquee) = self.marquee.get() else {
            return Ok(());
        };
        let area = marquee.area();
        let (left, top) = view.to_widget(Point {
            x: area.x,
            y: area.y,
        });
        let (right, bottom) = view.to_widget(Point {
            x: area.x + area.width,
            y: area.y + area.height,
        });
        let (red, green, blue) = Color::Accent.rgb();
        cr.rectangle(
            left.round() + 0.5,
            top.round() + 0.5,
            (right - left).round(),
            (bottom - top).round(),
        );
        cr.set_source_rgba(red, green, blue, 0.12);
        cr.fill_preserve()?;
        cr.set_source_rgb(red, green, blue);
        cr.set_line_width(1.0);
        cr.set_dash(&[4.0, 3.0], 0.0);
        cr.stroke()?;
        cr.set_dash(&[], 0.0);
        Ok(())
    }

    fn paint_group_frame(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let Some(group) = self.group_bounds() else {
            return Ok(());
        };
        if self.moving.borrow().is_some() {
            return Ok(());
        }
        let (red, green, blue) = Color::Accent.rgb();
        let outset = OUTSET * 2.0;
        let (left, top) = view.to_widget(Point {
            x: group.x,
            y: group.y,
        });
        let (right, bottom) = view.to_widget(Point {
            x: group.x + group.width,
            y: group.y + group.height,
        });
        cr.set_source_rgba(red, green, blue, 0.8);
        cr.set_line_width(1.0);
        cr.rectangle(
            left.floor() - outset + 0.5,
            top.floor() - outset + 0.5,
            (right - left).ceil() + outset * 2.0,
            (bottom - top).ceil() + outset * 2.0,
        );
        cr.stroke()?;
        let (x, y) = view.to_widget(group.turn_handle());
        cr.move_to((left + right) / 2.0, top.floor() - outset + 0.5);
        cr.line_to(x, y);
        cr.stroke()?;
        cr.arc(
            x.round() + 0.5,
            y.round() + 0.5,
            HANDLE / 2.0,
            0.0,
            std::f64::consts::TAU,
        );
        cr.set_source_rgb(0.04, 0.04, 0.04);
        cr.fill_preserve()?;
        cr.set_source_rgb(red, green, blue);
        cr.stroke()
    }

    fn paint_outline(
        &self,
        cr: &cairo::Context,
        view: View,
        shape: &Shape,
        with_handles: bool,
    ) -> Result<(), cairo::Error> {
        let (accent_red, accent_green, accent_blue) = Color::Accent.rgb();
        let bounds = shape.bounds();
        let (left, top) = view.to_widget(Point {
            x: bounds.x,
            y: bounds.y,
        });
        let (right, bottom) = view.to_widget(Point {
            x: bounds.x + bounds.width,
            y: bounds.y + bounds.height,
        });
        cr.set_source_rgb(accent_red, accent_green, accent_blue);
        cr.set_line_width(1.0);
        cr.set_dash(&[4.0, 3.0], 0.0);
        if shape.angle() == 0.0 {
            cr.rectangle(
                left.floor() - OUTSET + 0.5,
                top.floor() - OUTSET + 0.5,
                (right - left).ceil() + OUTSET * 2.0,
                (bottom - top).ceil() + OUTSET * 2.0,
            );
        } else {
            let outset = OUTSET / view.zoom();
            let pivot = shape.pivot();
            let corners = bounds.grown(outset).corners();
            for (index, corner) in corners.into_iter().enumerate() {
                let (x, y) = view.to_widget(corner.rotated(shape.angle(), pivot));
                if index == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            cr.close_path();
        }
        cr.stroke()?;
        cr.set_dash(&[], 0.0);
        if !with_handles {
            return Ok(());
        }
        for (handle, at) in shape.handles() {
            let (x, y) = view.to_widget(at);
            if handle == Handle::Rotate {
                let top = Point {
                    x: bounds.x + bounds.width / 2.0,
                    y: bounds.y,
                }
                .rotated(shape.angle(), shape.pivot());
                let (top_x, top_y) = view.to_widget(top);
                cr.move_to(top_x, top_y);
                cr.line_to(x, y);
                cr.stroke()?;
                cr.arc(
                    x.round() + 0.5,
                    y.round() + 0.5,
                    HANDLE / 2.0,
                    0.0,
                    std::f64::consts::TAU,
                );
            } else {
                cr.rectangle(
                    x.round() - HANDLE / 2.0 + 0.5,
                    y.round() - HANDLE / 2.0 + 0.5,
                    HANDLE - 1.0,
                    HANDLE - 1.0,
                );
            }
            cr.set_source_rgb(0.04, 0.04, 0.04);
            cr.fill_preserve()?;
            cr.set_source_rgb(accent_red, accent_green, accent_blue);
            cr.stroke()?;
        }
        Ok(())
    }
}
