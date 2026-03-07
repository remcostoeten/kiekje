use anyhow::{anyhow, Context, Result};
use gdk_pixbuf::PixbufLoader;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionChoice {
    Region(SelectionRect),
    Fullscreen,
}

#[derive(Debug, Clone, Copy, Default)]
struct DragState {
    start: Option<(f64, f64)>,
    current: Option<(f64, f64)>,
}

pub fn choose_region_or_fullscreen(png_data: &[u8]) -> Result<SelectionChoice> {
    let loader = PixbufLoader::new();
    loader
        .write(png_data)
        .context("failed to load screenshot preview into GTK")?;
    loader
        .close()
        .context("failed to finalize screenshot preview for GTK")?;
    let pixbuf = loader
        .pixbuf()
        .ok_or_else(|| anyhow!("failed to decode screenshot preview"))?;

    let image_width = pixbuf.width() as u32;
    let image_height = pixbuf.height() as u32;
    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
    let result = Rc::new(RefCell::new(None::<SelectionChoice>));

    let app = adw::Application::builder()
        .application_id("com.screeny.capture.region-selector")
        .build();

    app.connect_activate({
        let result = Rc::clone(&result);
        move |app| {
            let overlay = gtk::Overlay::new();

            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_can_shrink(true);
            picture.set_keep_aspect_ratio(false);
            overlay.set_child(Some(&picture));

            let drag_state = Rc::new(RefCell::new(DragState::default()));

            let instructions = gtk::Box::new(gtk::Orientation::Vertical, 8);
            instructions.set_margin_top(20);
            instructions.set_margin_start(20);
            instructions.set_halign(gtk::Align::Start);
            instructions.set_valign(gtk::Align::Start);
            instructions.add_css_class("boxed-list");

            let title = gtk::Label::new(Some("Drag to capture an area"));
            title.add_css_class("title-4");
            title.set_xalign(0.0);

            let body = gtk::Label::new(Some(
                "Release to capture the selection. Press Fullscreen to skip selection, or Esc to cancel.",
            ));
            body.set_xalign(0.0);
            body.set_wrap(true);

            instructions.append(&title);
            instructions.append(&body);
            overlay.add_overlay(&instructions);

            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            actions.set_margin_top(20);
            actions.set_margin_end(20);
            actions.set_halign(gtk::Align::End);
            actions.set_valign(gtk::Align::Start);

            let fullscreen_btn = gtk::Button::with_label("Fullscreen");
            let cancel_btn = gtk::Button::with_label("Cancel");
            actions.append(&fullscreen_btn);
            actions.append(&cancel_btn);
            overlay.add_overlay(&actions);

            let selection = gtk::DrawingArea::new();
            selection.set_hexpand(true);
            selection.set_vexpand(true);
            selection.set_focusable(true);
            overlay.add_overlay(&selection);

            selection.set_draw_func({
                let drag_state = Rc::clone(&drag_state);
                move |_, cr, width, height| {
                    let state = *drag_state.borrow();
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.30);
                    cr.rectangle(0.0, 0.0, width as f64, height as f64);
                    let _ = cr.fill();

                    if let Some((x, y, w, h)) = selection_rect(state) {
                        cr.set_operator(gtk::cairo::Operator::Clear);
                        cr.rectangle(x, y, w, h);
                        let _ = cr.fill();
                        cr.set_operator(gtk::cairo::Operator::Over);

                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
                        cr.set_line_width(2.0);
                        cr.rectangle(x + 1.0, y + 1.0, (w - 2.0).max(0.0), (h - 2.0).max(0.0));
                        let _ = cr.stroke();

                        cr.set_source_rgba(0.30, 0.64, 1.0, 0.20);
                        cr.rectangle(x, y, w, h);
                        let _ = cr.fill();
                    }
                }
            });

            let drag = gtk::GestureDrag::new();
            selection.add_controller(drag.clone());
            drag.connect_drag_begin({
                let drag_state = Rc::clone(&drag_state);
                let selection = selection.clone();
                move |_, start_x, start_y| {
                    *drag_state.borrow_mut() = DragState {
                        start: Some((start_x, start_y)),
                        current: Some((start_x, start_y)),
                    };
                    selection.queue_draw();
                }
            });
            drag.connect_drag_update({
                let drag_state = Rc::clone(&drag_state);
                let selection = selection.clone();
                move |_, offset_x, offset_y| {
                    let mut state = drag_state.borrow_mut();
                    if let Some((start_x, start_y)) = state.start {
                        state.current = Some((start_x + offset_x, start_y + offset_y));
                        selection.queue_draw();
                    }
                }
            });
            drag.connect_drag_end({
                let drag_state = Rc::clone(&drag_state);
                let result = Rc::clone(&result);
                let selection = selection.clone();
                let app = app.clone();
                move |_, offset_x, offset_y| {
                    let mut state = drag_state.borrow_mut();
                    if let Some((start_x, start_y)) = state.start {
                        state.current = Some((start_x + offset_x, start_y + offset_y));
                        if let Some(rect) = map_selection_to_image(
                            *state,
                            selection.allocated_width(),
                            selection.allocated_height(),
                            image_width,
                            image_height,
                        ) {
                            *result.borrow_mut() = Some(SelectionChoice::Region(rect));
                            app.quit();
                            return;
                        }
                    }
                    *state = DragState::default();
                    selection.queue_draw();
                }
            });

            let key = gtk::EventControllerKey::new();
            selection.add_controller(key.clone());
            key.connect_key_pressed({
                let result = Rc::clone(&result);
                let app = app.clone();
                move |_, key, _, _| match key {
                    gtk::gdk::Key::Escape => {
                        app.quit();
                        gtk::glib::Propagation::Stop
                    }
                    gtk::gdk::Key::f | gtk::gdk::Key::F => {
                        *result.borrow_mut() = Some(SelectionChoice::Fullscreen);
                        app.quit();
                        gtk::glib::Propagation::Stop
                    }
                    _ => gtk::glib::Propagation::Proceed,
                }
            });

            fullscreen_btn.connect_clicked({
                let result = Rc::clone(&result);
                let app = app.clone();
                move |_| {
                    *result.borrow_mut() = Some(SelectionChoice::Fullscreen);
                    app.quit();
                }
            });
            cancel_btn.connect_clicked({
                let app = app.clone();
                move |_| app.quit()
            });

            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("Capture Area")
                .content(&overlay)
                .decorated(false)
                .default_width(image_width as i32)
                .default_height(image_height as i32)
                .build();
            window.fullscreen();
            window.present();
            selection.grab_focus();
        }
    });

    app.run();
    let selection = result
        .borrow_mut()
        .take()
        .ok_or_else(|| anyhow!("capture canceled"));
    selection
}

fn selection_rect(state: DragState) -> Option<(f64, f64, f64, f64)> {
    let (start_x, start_y) = state.start?;
    let (current_x, current_y) = state.current?;
    let x = start_x.min(current_x);
    let y = start_y.min(current_y);
    let width = (current_x - start_x).abs();
    let height = (current_y - start_y).abs();
    Some((x, y, width, height))
}

fn map_selection_to_image(
    state: DragState,
    widget_width: i32,
    widget_height: i32,
    image_width: u32,
    image_height: u32,
) -> Option<SelectionRect> {
    let (x, y, width, height) = selection_rect(state)?;
    if width < 4.0 || height < 4.0 || widget_width <= 0 || widget_height <= 0 {
        return None;
    }

    let scale_x = image_width as f64 / widget_width as f64;
    let scale_y = image_height as f64 / widget_height as f64;
    let max_x = image_width as f64;
    let max_y = image_height as f64;

    let left = (x * scale_x).floor().clamp(0.0, max_x - 1.0);
    let top = (y * scale_y).floor().clamp(0.0, max_y - 1.0);
    let right = ((x + width) * scale_x).ceil().clamp(left + 1.0, max_x);
    let bottom = ((y + height) * scale_y).ceil().clamp(top + 1.0, max_y);

    Some(SelectionRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::{map_selection_to_image, DragState, SelectionRect};

    #[test]
    fn maps_drag_to_image_space() {
        let rect = map_selection_to_image(
            DragState {
                start: Some((10.0, 20.0)),
                current: Some((110.0, 120.0)),
            },
            200,
            200,
            400,
            400,
        )
        .unwrap();

        assert_eq!(
            rect,
            SelectionRect {
                x: 20,
                y: 40,
                width: 200,
                height: 200,
            }
        );
    }
}
