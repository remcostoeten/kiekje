use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::Editor;
use super::document::{Edit, Point, Shape};
use super::shapes;

pub(super) struct Typing {
    pub(super) editing: Option<usize>,
    at: Point,
    before: Option<Shape>,
}

pub(super) fn attach(editor: &Rc<Editor>) {
    editor.writer.set_parent(&editor.canvas);
    editor.writer.set_child(Some(&editor.entry));
    editor.writer.set_has_arrow(false);
    editor.writer.set_position(gtk::PositionType::Bottom);
    editor.writer.set_halign(gtk::Align::Start);

    let changed = Rc::downgrade(editor);
    editor.entry.connect_changed(move |_| {
        if let Some(editor) = changed.upgrade() {
            editor.canvas.queue_draw();
        }
    });
    let entered = Rc::downgrade(editor);
    editor.entry.connect_activate(move |_| {
        if let Some(editor) = entered.upgrade() {
            editor.finish_text(true);
        }
    });
    let closed = Rc::downgrade(editor);
    editor.writer.connect_closed(move |_| {
        if let Some(editor) = closed.upgrade() {
            editor.finish_text(true);
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let escaped = Rc::downgrade(editor);
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(editor) = escaped.upgrade() {
            editor.finish_text(false);
        }
        glib::Propagation::Stop
    });
    editor.writer.add_controller(keys);
    let owned = editor.writer.downgrade();
    editor.canvas.connect_destroy(move |_| {
        if let Some(writer) = owned.upgrade() {
            writer.unparent();
        }
    });
}

impl Editor {
    pub(super) fn start_text(&self, click: Point, editing: Option<usize>) {
        self.finish_text(true);
        let (at, text, before) = match editing {
            Some(index) => match self.document.borrow().shapes().get(index) {
                Some(shape @ Shape::Text { at, text, .. }) => {
                    (*at, text.clone(), Some(shape.clone()))
                }
                _ => return,
            },
            None => {
                let size = shapes::font_size(self.stroke().width);
                (
                    Point {
                        x: click.x,
                        y: click.y - size / 2.0,
                    },
                    String::new(),
                    None,
                )
            }
        };
        self.typing.replace(Some(Typing {
            editing,
            at,
            before,
        }));
        self.entry.set_text(&text);
        let (x, y) = self.view.get().to_widget(Point {
            x: at.x,
            y: at.y + shapes::font_size(self.stroke().width),
        });
        self.writer
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32 + 4, 1, 1)));
        self.writer.popup();
        self.entry.grab_focus();
        self.entry.set_position(-1);
        self.canvas.queue_draw();
    }

    pub(super) fn typed_shape(&self) -> Option<Shape> {
        let typing = self.typing.borrow();
        let typing = typing.as_ref()?;
        let text = self.entry.text();
        if text.trim().is_empty() {
            return None;
        }
        let stroke = match &typing.before {
            Some(Shape::Text { stroke, .. }) => *stroke,
            _ => self.stroke(),
        };
        Some(Shape::Text {
            at: typing.at,
            text: text.to_string(),
            stroke,
        })
    }

    pub(super) fn typing_over(&self) -> Option<usize> {
        self.typing
            .borrow()
            .as_ref()
            .and_then(|typing| typing.editing)
    }

    pub(super) fn finish_text(&self, keep: bool) {
        let shape = if keep { self.typed_shape() } else { None };
        let Some(typing) = self.typing.take() else {
            return;
        };
        let edit = match (typing.editing, typing.before, shape) {
            (Some(index), Some(before), Some(after)) if keep && after != before => {
                Some(Edit::Replace {
                    index,
                    before,
                    after,
                })
            }
            (Some(index), Some(shape), None) if keep => Some(Edit::Remove { index, shape }),
            (None, _, Some(shape)) => Some(Edit::Add(shape)),
            _ => None,
        };
        if let Some(edit) = edit {
            if matches!(edit, Edit::Remove { .. }) {
                self.deselect();
            }
            self.document.borrow_mut().apply(edit);
            self.edited();
        }
        self.writer.popdown();
        self.canvas.grab_focus();
        self.canvas.queue_draw();
    }
}
