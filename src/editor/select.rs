use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::canvas::View;
use super::document::{Color, Edit, Point, Shape, Tool};
use super::shapes::Handle;

const REACH: f64 = 8.0;
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
}

pub(super) fn tool_cursor(tool: Tool) -> &'static str {
    match tool {
        Tool::Text => "text",
        Tool::Select => "default",
        _ => "crosshair",
    }
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
            Tool::Select => self.say(
                "Click a shape, then drag to move, drag a handle to resize, Delete removes, \
                 arrows nudge.",
                false,
            ),
            _ => {}
        }
    }

    fn slack(&self) -> f64 {
        REACH / self.view.get().zoom()
    }

    pub(super) fn grab(&self, point: Point) {
        let slack = self.slack();
        let target = {
            let document = self.document.borrow();
            let on_handle = self.selected.get().and_then(|index| {
                let (handle, _) = document
                    .shapes()
                    .get(index)?
                    .handles()
                    .into_iter()
                    .find(|(_, at)| at.distance(point) <= slack)?;
                Some((index, Grab::Handle(handle)))
            });
            on_handle
                .or_else(|| document.hit(point, slack).map(|index| (index, Grab::Body)))
                .and_then(|(index, grab)| {
                    let original = document.shapes().get(index)?.clone();
                    Some(Moving {
                        index,
                        preview: original.clone(),
                        original,
                        grab,
                        start: point,
                    })
                })
        };
        let picked = target.as_ref().map(|moving| moving.index);
        let previous = self.selected.replace(picked);
        self.moving.replace(target);
        if let Some(index) = picked.filter(|index| previous != Some(*index)) {
            self.adopt(index);
        }
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

    pub(super) fn drag_to(&self, point: Point) {
        if let Some(moving) = self.moving.borrow_mut().as_mut() {
            moving.preview = match moving.grab {
                Grab::Body => moving
                    .original
                    .moved(point.x - moving.start.x, point.y - moving.start.y),
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
            self.document.borrow_mut().apply(Edit::Replace {
                index: moving.index,
                before: moving.original,
                after: moving.preview,
            });
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
        let Some(before) = self.document.borrow().shapes().get(index).cloned() else {
            return;
        };
        let after = before.restyled(self.stroke());
        if after != before {
            self.document.borrow_mut().restyle(index, after);
            self.edited();
        }
    }

    pub(super) fn delete_selected(&self) -> bool {
        let Some(index) = self.selected.get() else {
            return false;
        };
        let Some(shape) = self.document.borrow().shapes().get(index).cloned() else {
            return false;
        };
        self.document
            .borrow_mut()
            .apply(Edit::Remove { index, shape });
        self.deselect();
        self.edited();
        true
    }

    fn replace_selected(&self, change: impl FnOnce(&Shape) -> Shape) -> bool {
        let Some(index) = self.selected.get() else {
            return false;
        };
        let Some(before) = self.document.borrow().shapes().get(index).cloned() else {
            return false;
        };
        let after = change(&before);
        if after != before {
            self.document.borrow_mut().apply(Edit::Replace {
                index,
                before,
                after,
            });
            self.edited();
        }
        true
    }

    pub(super) fn paint_selection(
        &self,
        cr: &cairo::Context,
        view: View,
    ) -> Result<(), cairo::Error> {
        let Some(index) = self.selected.get() else {
            return Ok(());
        };
        let moving = self.moving.borrow();
        let document = self.document.borrow();
        let shape = match moving.as_ref() {
            Some(moving) if moving.index == index => &moving.preview,
            _ => match document.shapes().get(index) {
                Some(shape) => shape,
                None => return Ok(()),
            },
        };
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
        cr.rectangle(
            left.floor() - OUTSET + 0.5,
            top.floor() - OUTSET + 0.5,
            (right - left).ceil() + OUTSET * 2.0,
            (bottom - top).ceil() + OUTSET * 2.0,
        );
        cr.stroke()?;
        cr.set_dash(&[], 0.0);
        for (_, at) in shape.handles() {
            let (x, y) = view.to_widget(at);
            cr.rectangle(
                x.round() - HANDLE / 2.0 + 0.5,
                y.round() - HANDLE / 2.0 + 0.5,
                HANDLE - 1.0,
                HANDLE - 1.0,
            );
            cr.set_source_rgb(0.04, 0.04, 0.04);
            cr.fill_preserve()?;
            cr.set_source_rgb(accent_red, accent_green, accent_blue);
            cr.stroke()?;
        }
        Ok(())
    }
}
