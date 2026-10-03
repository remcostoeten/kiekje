use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::Frame;
use crate::capture::mapping::{PixelRect, logical_to_pixels, rect_from_drag};

const ACCENT: (f64, f64, f64) = (254.0 / 255.0, 81.0 / 255.0, 1.0 / 255.0);
const DIM: f64 = 0.45;
const MIN_SELECTION: i32 = 3;

pub struct Selection {
    pub frame_index: usize,
    pub rect: PixelRect,
}

struct Drag {
    frame_index: usize,
    start: (f64, f64),
    current: (f64, f64),
}

type DoneCallback = Box<dyn FnOnce(Option<Selection>)>;
type PaintCallback = Box<dyn FnOnce()>;

struct Shared {
    frames: Arc<Vec<Frame>>,
    drag: RefCell<Option<Drag>>,
    windows: RefCell<Vec<gtk::Window>>,
    areas: RefCell<Vec<gtk::DrawingArea>>,
    finished: Cell<bool>,
    on_done: RefCell<Option<DoneCallback>>,
    on_first_paint: RefCell<Option<PaintCallback>>,
}

/// Shows each monitor's frozen frame on its own surface and lets the user drag a selection.
///
/// Surfaces are layer-shell overlays when the compositor supports it and fullscreen windows
/// otherwise. Escape cancels. `on_done` runs once with the selection in frame pixels, or
/// `None` when canceled.
///
/// # Examples
///
/// ```ignore
/// overlay::show(&app, frames, monitors, |selection| println!("{}", selection.is_some()), || {});
/// ```
pub fn show(
    app: &gtk::Application,
    frames: Arc<Vec<Frame>>,
    monitors: &[gdk::Monitor],
    on_done: impl FnOnce(Option<Selection>) + 'static,
    on_first_paint: impl FnOnce() + 'static,
) {
    let shared = Rc::new(Shared {
        frames: frames.clone(),
        drag: RefCell::new(None),
        windows: RefCell::new(Vec::new()),
        areas: RefCell::new(Vec::new()),
        finished: Cell::new(false),
        on_done: RefCell::new(Some(Box::new(on_done))),
        on_first_paint: RefCell::new(Some(Box::new(on_first_paint))),
    });

    for (frame_index, frame) in frames.iter().enumerate() {
        let Some(monitor) = monitors.iter().find(|monitor| {
            monitor
                .connector()
                .is_some_and(|name| name == frame.monitor.connector)
        }) else {
            eprintln!("kiekje: no GDK monitor for {}", frame.monitor.connector);
            continue;
        };
        build_surface(app, &shared, frame_index, monitor);
    }

    if shared.windows.borrow().is_empty() {
        finish(&shared, None);
    }
}

/// Whether overlays go on a layer-shell surface instead of a fullscreen window.
///
/// # Examples
///
/// ```ignore
/// println!("{}", overlay::uses_layer_shell());
/// ```
pub fn uses_layer_shell() -> bool {
    gdk::Display::default().is_some_and(|display| display.backend().is_wayland())
        && gtk4_layer_shell::is_supported()
}

fn build_surface(
    app: &gtk::Application,
    shared: &Rc<Shared>,
    frame_index: usize,
    monitor: &gdk::Monitor,
) {
    let frame = &shared.frames[frame_index];
    let window = gtk::Window::builder()
        .application(app)
        .decorated(false)
        .build();
    if uses_layer_shell() {
        place_on_layer(&window, monitor);
    } else {
        window.fullscreen_on_monitor(monitor);
    }

    let picture = gtk::Picture::for_paintable(&frame.texture());
    picture.set_content_fit(gtk::ContentFit::Fill);
    picture.set_can_shrink(true);

    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_cursor_from_name(Some("crosshair"));

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&area);
    window.set_child(Some(&overlay));

    let draw_shared = shared.clone();
    area.set_draw_func(move |_, context, width, height| {
        draw(&draw_shared, frame_index, context, width, height);
    });

    let drag = gtk::GestureDrag::new();
    let begin_shared = shared.clone();
    drag.connect_drag_begin(move |_, x, y| {
        begin_shared.drag.replace(Some(Drag {
            frame_index,
            start: (x, y),
            current: (x, y),
        }));
        redraw(&begin_shared);
    });
    let update_shared = shared.clone();
    drag.connect_drag_update(move |_, dx, dy| {
        if let Some(drag) = update_shared.drag.borrow_mut().as_mut() {
            drag.current = (drag.start.0 + dx, drag.start.1 + dy);
        }
        redraw(&update_shared);
    });
    let end_shared = shared.clone();
    drag.connect_drag_end(move |_, dx, dy| {
        let Some(drag) = end_shared.drag.borrow_mut().take() else {
            return;
        };
        let rect = rect_from_drag(drag.start, (drag.start.0 + dx, drag.start.1 + dy));
        if rect.width < MIN_SELECTION || rect.height < MIN_SELECTION {
            redraw(&end_shared);
            return;
        }
        let frame = &end_shared.frames[drag.frame_index];
        let monitor_size = (frame.monitor.logical.width, frame.monitor.logical.height);
        let selection =
            logical_to_pixels(rect, monitor_size, (frame.width, frame.height)).map(|rect| {
                Selection {
                    frame_index: drag.frame_index,
                    rect,
                }
            });
        finish(&end_shared, selection);
    });
    area.add_controller(drag);

    let keys = gtk::EventControllerKey::new();
    let key_shared = shared.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            finish(&key_shared, None);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);

    let paint_shared = shared.clone();
    window.connect_map(move |window| {
        let Some(clock) = window.frame_clock() else {
            return;
        };
        let callback_shared = paint_shared.clone();
        clock.connect_after_paint(move |_| {
            if let Some(callback) = callback_shared.on_first_paint.borrow_mut().take() {
                callback();
            }
        });
    });

    shared.areas.borrow_mut().push(area);
    shared.windows.borrow_mut().push(window.clone());
    window.present();
}

fn place_on_layer(window: &gtk::Window, monitor: &gdk::Monitor) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace(Some("kiekje-overlay"));
    window.set_monitor(Some(monitor));
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
}

fn draw(shared: &Shared, frame_index: usize, context: &cairo::Context, width: i32, height: i32) {
    context.set_source_rgba(0.0, 0.0, 0.0, DIM);
    context.rectangle(0.0, 0.0, f64::from(width), f64::from(height));

    let drag = shared.drag.borrow();
    let Some(drag) = drag.as_ref().filter(|drag| drag.frame_index == frame_index) else {
        report(context.fill());
        return;
    };
    let rect = rect_from_drag(drag.start, drag.current);
    let (x, y, w, h) = (
        f64::from(rect.x),
        f64::from(rect.y),
        f64::from(rect.width),
        f64::from(rect.height),
    );
    context.rectangle(x, y, w, h);
    context.set_fill_rule(cairo::FillRule::EvenOdd);
    report(context.fill());

    context.set_source_rgb(ACCENT.0, ACCENT.1, ACCENT.2);
    context.set_line_width(1.0);
    context.rectangle(x + 0.5, y + 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));
    report(context.stroke());

    let frame = &shared.frames[frame_index];
    let monitor_size = (frame.monitor.logical.width, frame.monitor.logical.height);
    let Some(pixels) = logical_to_pixels(rect, monitor_size, (frame.width, frame.height)) else {
        return;
    };
    draw_size_label(
        context,
        &format!("{} × {}", pixels.width, pixels.height),
        x,
        y + h,
        width,
        height,
    );
}

fn draw_size_label(
    context: &cairo::Context,
    text: &str,
    x: f64,
    bottom: f64,
    width: i32,
    height: i32,
) {
    context.select_font_face(
        "Geist Mono",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    context.set_font_size(12.0);
    let Ok(extents) = context.text_extents(text) else {
        return;
    };
    let padding = 6.0;
    let label_width = extents.x_advance() + padding * 2.0;
    let label_height = 22.0;
    let left = x.min(f64::from(width) - label_width).max(0.0);
    let top = (bottom + 6.0)
        .min(f64::from(height) - label_height)
        .max(0.0);

    context.set_source_rgba(10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 0.9);
    context.rectangle(left, top, label_width, label_height);
    report(context.fill());
    context.set_source_rgb(237.0 / 255.0, 237.0 / 255.0, 237.0 / 255.0);
    context.move_to(left + padding, top + 15.0);
    report(context.show_text(text));
}

fn redraw(shared: &Shared) {
    for area in shared.areas.borrow().iter() {
        area.queue_draw();
    }
}

fn finish(shared: &Shared, selection: Option<Selection>) {
    if shared.finished.replace(true) {
        return;
    }
    // Wayland only accepts a clipboard change from a focused client, so deliver before closing.
    if let Some(callback) = shared.on_done.borrow_mut().take() {
        callback(selection);
    }
    for window in shared.windows.borrow_mut().drain(..) {
        window.close();
    }
    shared.areas.borrow_mut().clear();
}

fn report(result: Result<(), cairo::Error>) {
    if let Err(error) = result {
        eprintln!("kiekje: overlay drawing failed: {error}");
    }
}
