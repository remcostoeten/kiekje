use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk};

use super::Editor;
use super::document::{Point, Shape, Tool};
use super::render;

const MIN_PEN_STEP: f64 = 1.0;
const TEXT_REACH: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct View {
    zoom: f64,
    left: f64,
    top: f64,
}

impl View {
    pub(super) const IDENTITY: Self = Self {
        zoom: 1.0,
        left: 0.0,
        top: 0.0,
    };

    pub(super) fn fit(area: (f64, f64), image: (f64, f64), max_zoom: f64) -> Self {
        let zoom = (area.0 / image.0).min(area.1 / image.1).min(max_zoom);
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        Self {
            zoom,
            left: ((area.0 - image.0 * zoom) / 2.0).max(0.0),
            top: ((area.1 - image.1 * zoom) / 2.0).max(0.0),
        }
    }

    pub(super) fn zoom(self) -> f64 {
        self.zoom
    }

    pub(super) fn to_widget(self, point: Point) -> (f64, f64) {
        (
            self.left + point.x * self.zoom,
            self.top + point.y * self.zoom,
        )
    }

    fn to_image(self, x: f64, y: f64, image: (f64, f64)) -> Point {
        Point {
            x: ((x - self.left) / self.zoom).clamp(0.0, image.0),
            y: ((y - self.top) / self.zoom).clamp(0.0, image.1),
        }
    }
}

pub(super) fn attach(editor: &Rc<Editor>) {
    let painter = Rc::downgrade(editor);
    editor.canvas.set_draw_func(move |area, cr, width, height| {
        let Some(editor) = painter.upgrade() else {
            return;
        };
        let max_zoom = if editor.is_inline() {
            f64::INFINITY
        } else {
            1.0 / f64::from(area.scale_factor().max(1))
        };
        let view = View::fit(
            (f64::from(width), f64::from(height)),
            editor.size(),
            max_zoom,
        );
        editor.view.set(view);
        if let Err(error) = paint(&editor, cr, view) {
            eprintln!("kiekje: the editor canvas was not drawn: {error}");
        }
    });

    let drag = gtk::GestureDrag::builder()
        .button(gdk::BUTTON_PRIMARY)
        .build();
    let started = Rc::downgrade(editor);
    drag.connect_drag_begin(move |_, x, y| {
        if let Some(editor) = started.upgrade() {
            editor.canvas.grab_focus();
            let point = editor.view.get().to_image(x, y, editor.size());
            if editor.grab_region(point) {
                return;
            }
            match editor.tool.get() {
                Tool::Crop => editor.hide_bars(),
                Tool::Select => {
                    editor.grab(point);
                    return;
                }
                Tool::Text => {
                    editor.start_text(point, None);
                    return;
                }
                _ => {}
            }
            editor.draft.replace(vec![point]);
            editor.canvas.queue_draw();
        }
    });
    let moved = Rc::downgrade(editor);
    drag.connect_drag_update(move |gesture, dx, dy| {
        if let Some(editor) = moved.upgrade() {
            extend(&editor, gesture, dx, dy);
        }
    });
    let ended = Rc::downgrade(editor);
    drag.connect_drag_end(move |gesture, dx, dy| {
        if let Some(editor) = ended.upgrade() {
            extend(&editor, gesture, dx, dy);
            if editor.release_region() {
                return;
            }
            match editor.tool.get() {
                Tool::Select => editor.release(),
                Tool::Text => {}
                Tool::Crop => editor.commit_crop(),
                _ => editor.commit(),
            }
        }
    });
    editor.canvas.add_controller(drag);

    let clicks = gtk::GestureClick::builder()
        .button(gdk::BUTTON_PRIMARY)
        .build();
    let clicked = Rc::downgrade(editor);
    clicks.connect_released(move |_, presses, x, y| {
        let Some(editor) = clicked.upgrade() else {
            return;
        };
        if presses != 2 || !matches!(editor.tool.get(), Tool::Select | Tool::Text) {
            return;
        }
        let view = editor.view.get();
        let point = view.to_image(x, y, editor.size());
        let index = editor
            .document
            .borrow()
            .hit(point, TEXT_REACH / view.zoom());
        let is_text = index.is_some_and(|index| {
            matches!(
                editor.document.borrow().shapes().get(index),
                Some(Shape::Text { .. })
            )
        });
        if is_text {
            editor.start_text(point, index);
        }
    });
    editor.canvas.add_controller(clicks);

    let motion = gtk::EventControllerMotion::new();
    let hovered = Rc::downgrade(editor);
    motion.connect_motion(move |_, x, y| {
        if let Some(editor) = hovered.upgrade() {
            let point = editor.view.get().to_image(x, y, editor.size());
            editor.hover_region(point);
        }
    });
    editor.canvas.add_controller(motion);
    editor.canvas.set_cursor_from_name(Some("crosshair"));
}

fn extend(editor: &Editor, gesture: &gtk::GestureDrag, dx: f64, dy: f64) {
    let Some((x, y)) = gesture.start_point() else {
        return;
    };
    let point = editor.view.get().to_image(x + dx, y + dy, editor.size());
    if editor.reshape_to(point) {
        return;
    }
    match editor.tool.get() {
        Tool::Select => {
            editor.drag_to(point);
            return;
        }
        Tool::Text => return,
        _ => {}
    }
    let mut draft = editor.draft.borrow_mut();
    let Some(&start) = draft.first() else {
        return;
    };
    match editor.tool.get() {
        Tool::Pen => {
            if draft
                .last()
                .is_none_or(|last| last.distance(point) >= MIN_PEN_STEP)
            {
                draft.push(point);
            }
        }
        _ => *draft = vec![start, point],
    }
    drop(draft);
    editor.canvas.queue_draw();
}

fn paint(editor: &Editor, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
    let (width, height) = editor.size();
    cr.save()?;
    cr.translate(view.left, view.top);
    cr.scale(view.zoom, view.zoom);
    cr.rectangle(0.0, 0.0, width, height);
    cr.clip();
    let drawn = {
        let document = editor.document.borrow();
        let moving = editor.moving.borrow();
        let draft = editor.draft_shape();
        let typed = editor.typed_shape();
        let hidden = editor.typing_over();
        let shapes = document
            .shapes()
            .iter()
            .enumerate()
            .filter(|(index, _)| hidden != Some(*index))
            .map(|(index, shape)| match moving.as_ref() {
                Some(moving) if moving.index == index => &moving.preview,
                _ => shape,
            });
        let shapes: Vec<&Shape> = shapes.chain(&draft).chain(&typed).collect();
        let marked = shapes
            .iter()
            .any(|shape| matches!(shape, Shape::Highlight { .. }));
        if editor.is_inline() && !marked {
            render::draw_shapes(cr, &editor.layers, shapes)
        } else {
            render::draw(cr, &editor.layers, shapes)
        }
    };
    cr.restore()?;
    drawn?;
    editor.paint_crop(cr, view)?;
    editor.paint_selection(cr, view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_a_large_image_and_centers_it() {
        let view = View::fit((1000.0, 600.0), (3840.0, 2160.0), 1.0);
        assert!((view.zoom - 1000.0 / 3840.0).abs() < 1e-9);
        assert_eq!(view.left, 0.0);
        assert!((view.top - (600.0 - 2160.0 * view.zoom) / 2.0).abs() < 1e-9);
    }

    #[test]
    fn never_enlarges_past_one_device_pixel() {
        let view = View::fit((1000.0, 1000.0), (100.0, 50.0), 0.5);
        assert_eq!(view.zoom, 0.5);
        assert_eq!((view.left, view.top), (475.0, 487.5));
    }

    #[test]
    fn maps_widget_points_to_clamped_image_pixels() {
        let view = View::fit((200.0, 100.0), (400.0, 100.0), 1.0);
        let image = (400.0, 100.0);
        assert_eq!(
            view.to_image(100.0, 50.0, image),
            Point { x: 200.0, y: 50.0 }
        );
        assert_eq!(
            view.to_image(-5.0, 999.0, image),
            Point { x: 0.0, y: 100.0 }
        );
    }
}
