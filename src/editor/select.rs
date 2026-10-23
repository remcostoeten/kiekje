use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::TURN_STEP_DEGREES;
use super::canvas::View;
use super::document::{Color, Edit, Point, Shape, Tool};
use super::shapes::Handle;

const REACH: f64 = 8.0;
const HINT_OFFSET: f64 = 14.0;
const HINT_RADIUS: f64 = 7.0;
const HINT_SWEEP: f64 = 2.2;
const HINT_ARROW: f64 = 4.0;
const HANDLE: f64 = 8.0;
const OUTSET: f64 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(super) enum Grab {
    Body,
    Handle(Handle),
}

pub(super) struct Moving {
    pub(super) index: usize,
    pub(super) preview: Shape,
    original: Shape,
    grab: Grab,
    start: Point,
    others: Vec<(usize, Shape)>,
    delta: (f64, f64),
}

impl Moving {
    pub(super) fn preview_of(&self, index: usize) -> Option<Shape> {
        if index == self.index {
            return Some(self.preview.clone());
        }
        self.others
            .iter()
            .find(|(other, _)| *other == index)
            .map(|(_, shape)| shape.moved(self.delta.0, self.delta.1))
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
                "Drag to choose the region. Drag inside it to move it, a handle to resize it.",
                false,
            ),
            Tool::Crop => self.say(
                "Drag to keep part of the image. Click once to clear the crop.",
                false,
            ),
            Tool::Text => self.say("Click where the text goes, type, then press Enter.", false),
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
                 arrows nudge. Ctrl+click adds to the selection, Ctrl+A selects all.",
                false,
            ),
            _ => {}
        }
    }

    fn slack(&self) -> f64 {
        REACH / self.view.get().zoom()
    }

    fn scale(&self) -> f64 {
        1.0 / self.view.get().zoom()
    }

    pub(super) fn hover_turn_zone(&self, point: Point) {
        let hint = if self.tool.get() == Tool::Select
            && self.moving.borrow().is_none()
            && self.also_selected.borrow().is_empty()
        {
            self.selected.get().and_then(|index| {
                self.document
                    .borrow()
                    .shapes()
                    .get(index)?
                    .turn_zone(point, self.scale())
            })
        } else {
            None
        };
        if self.turn_hint.replace(hint) != hint {
            self.canvas.queue_draw();
        }
    }

    fn paint_turn_hint(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let (Some(corner), Some(index)) = (self.turn_hint.get(), self.selected.get()) else {
            return Ok(());
        };
        let Some(pivot) = self.document.borrow().shapes().get(index).map(Shape::pivot) else {
            return Ok(());
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
            let on_handle = self.selected.get().filter(|_| single).and_then(|index| {
                let shape = document.shapes().get(index)?;
                let handle = shape
                    .handles()
                    .into_iter()
                    .find(|(_, at)| at.distance(point) <= slack)
                    .map(|(handle, _)| handle)
                    .or_else(|| shape.turn_zone(point, self.scale()).map(|_| Handle::Rotate))?;
                Some((index, Grab::Handle(handle)))
            });
            on_handle.or_else(|| document.hit(point, slack).map(|index| (index, Grab::Body)))
        };
        let previous = self.selected.get();
        match hit {
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
                self.also_selected.borrow_mut().clear();
                self.selected.set(None);
            }
        }
        let target = hit.and_then(|(index, grab)| {
            let document = self.document.borrow();
            let original = document.shapes().get(index)?.clone();
            let others = match grab {
                Grab::Body => self
                    .also_selected
                    .borrow()
                    .iter()
                    .filter_map(|other| Some((*other, document.shapes().get(*other)?.clone())))
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

    /// Every selected shape index, lowest first.
    fn selection(&self) -> Vec<usize> {
        let mut all: Vec<usize> = self.also_selected.borrow().clone();
        all.extend(self.selected.get());
        all.sort_unstable();
        all.dedup();
        all
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
        if let Some(moving) = self.moving.borrow_mut().as_mut() {
            moving.delta = (point.x - moving.start.x, point.y - moving.start.y);
            moving.preview = match moving.grab {
                Grab::Body => moving.original.moved(moving.delta.0, moving.delta.1),
                Grab::Handle(Handle::Rotate) if stepped => {
                    let free = moving.original.resized(Handle::Rotate, point);
                    free.turned(stepped_angle(free.angle()))
                }
                Grab::Handle(handle) => moving.original.resized(handle, point),
            };
        }
        self.canvas.queue_draw();
    }

    pub(super) fn release(&self) {
        let Some(moving) = self.moving.take() else {
            return;
        };
        if moving.preview != moving.original {
            let mut edits = vec![Edit::Replace {
                index: moving.index,
                before: moving.original,
                after: moving.preview,
            }];
            for (index, before) in moving.others {
                let after = before.moved(moving.delta.0, moving.delta.1);
                edits.push(Edit::Replace {
                    index,
                    before,
                    after,
                });
            }
            self.document.borrow_mut().apply(batch(edits));
            self.edited();
        }
        self.canvas.queue_draw();
    }

    pub(super) fn cancel_grab(&self) -> bool {
        let had = self.moving.take().is_some();
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
        self.paint_turn_hint(cr, view)
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
