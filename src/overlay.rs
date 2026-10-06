use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

mod magnifier;

use magnifier::Magnifier;

use crate::capture::mapping::{SelectionPlan, plan_selection, rect_from_drag};
use crate::capture::{CaptureMode, Frame, LogicalRect};
use crate::editor::{Ending, Finish, Inline};
use crate::output::{self, Image};

const ACCENT: gdk::RGBA = gdk::RGBA::new(254.0 / 255.0, 81.0 / 255.0, 1.0 / 255.0, 1.0);
const DIM: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.45);
const LABEL_BACKGROUND: gdk::RGBA = gdk::RGBA::new(10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 0.9);
const LABEL_TEXT: gdk::RGBA = gdk::RGBA::new(237.0 / 255.0, 237.0 / 255.0, 237.0 / 255.0, 1.0);
const MIN_SELECTION: i32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub edit: bool,
    pub magnifier: bool,
}

#[derive(Clone, Copy)]
pub struct Selection {
    pub rect: LogicalRect,
    pub plan: SelectionPlan,
}

pub enum Outcome {
    Selected(Selection),
    Edited {
        selection: Selection,
        image: Image,
        finish: Finish,
    },
    Canceled,
    MonitorLost(String),
}

struct Drag {
    frame_index: usize,
    start: (f64, f64),
    current: (f64, f64),
}

type DoneCallback = Box<dyn FnOnce(Outcome)>;
type PaintCallback = Box<dyn FnOnce()>;
type Painter = Box<dyn Fn(&gtk::Widget, &gtk::Snapshot)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SelectionLayer {
        pub painter: RefCell<Option<Painter>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SelectionLayer {
        const NAME: &'static str = "KiekjeSelectionLayer";
        type Type = super::SelectionLayer;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for SelectionLayer {}

    impl WidgetImpl for SelectionLayer {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(painter) = self.painter.borrow().as_ref() {
                painter(self.obj().upcast_ref(), snapshot);
            }
        }
    }
}

glib::wrapper! {
    pub struct SelectionLayer(ObjectSubclass<imp::SelectionLayer>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SelectionLayer {
    fn new(painter: impl Fn(&gtk::Widget, &gtk::Snapshot) + 'static) -> Self {
        let layer: Self = glib::Object::new();
        layer.imp().painter.replace(Some(Box::new(painter)));
        layer
    }
}

struct Area {
    frame_index: usize,
    layer: SelectionLayer,
    showing: Cell<bool>,
}

struct Shared {
    frames: Arc<Vec<Frame>>,
    geometry: Vec<(LogicalRect, (u32, u32))>,
    edit: bool,
    drag: RefCell<Option<Drag>>,
    chosen: Cell<Option<LogicalRect>>,
    inline: RefCell<Option<Inline>>,
    windows: RefCell<Vec<gtk::Window>>,
    areas: RefCell<Vec<Area>>,
    surfaces: RefCell<Vec<(usize, gtk::Overlay)>>,
    finished: Cell<bool>,
    on_done: RefCell<Option<DoneCallback>>,
    watched: RefCell<Vec<(gdk::Monitor, glib::SignalHandlerId)>>,
    on_first_paint: RefCell<Option<PaintCallback>>,
    magnifier: Magnifier,
}

/// Shows each monitor's frozen frame on its own surface and lets the user drag a selection.
///
/// Surfaces are layer-shell overlays when the compositor supports it and fullscreen windows
/// otherwise. A drag may cross monitor edges. With `edit`, a selection inside one monitor stays
/// on screen with the annotation tools until the user accepts or cancels. With `magnifier`, a
/// loupe follows the pointer until a selection is made. Escape cancels, and
/// unplugging a monitor that shows a frame ends the selection. `on_done` runs once with the
/// outcome.
///
/// # Examples
///
/// ```ignore
/// overlay::show(&app, frames, monitors, Options::default(), |outcome| {}, || {});
/// ```
pub fn show(
    app: &gtk::Application,
    frames: Arc<Vec<Frame>>,
    monitors: &[gdk::Monitor],
    options: Options,
    on_done: impl FnOnce(Outcome) + 'static,
    on_first_paint: impl FnOnce() + 'static,
) {
    let geometry = frames
        .iter()
        .map(|frame| (frame.monitor.logical, (frame.width, frame.height)))
        .collect();
    let shared = Rc::new(Shared {
        frames: frames.clone(),
        geometry,
        edit: options.edit,
        drag: RefCell::new(None),
        chosen: Cell::new(None),
        inline: RefCell::new(None),
        windows: RefCell::new(Vec::new()),
        areas: RefCell::new(Vec::new()),
        surfaces: RefCell::new(Vec::new()),
        finished: Cell::new(false),
        on_done: RefCell::new(Some(Box::new(on_done))),
        watched: RefCell::new(Vec::new()),
        on_first_paint: RefCell::new(Some(Box::new(on_first_paint))),
        magnifier: Magnifier::new(options.magnifier),
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
        watch_monitor(&shared, monitor, &frame.monitor.connector);
    }

    if shared.windows.borrow().is_empty() {
        finish(&shared, Outcome::Canceled);
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

/// Loads the renderer, GL driver and fonts the overlay needs before the first capture.
///
/// Realizes a window that is never mapped and renders one small overlay-like frame offscreen,
/// so the first overlay after startup opens as fast as later ones.
///
/// # Examples
///
/// ```ignore
/// glib::idle_add_local_once(overlay::warm_up);
/// ```
pub fn warm_up() {
    let started = glib::monotonic_time();
    let window = gtk::Window::new();
    WidgetExt::realize(&window);
    if let Some(renderer) = window.renderer() {
        let bounds = graphene::Rect::new(0.0, 0.0, 64.0, 64.0);
        let texture = gdk::MemoryTexture::new(
            2,
            2,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &glib::Bytes::from_owned(vec![255u8; 16]),
            8,
        );
        let snapshot = gtk::Snapshot::new();
        snapshot.append_texture(&texture, &bounds);
        snapshot.append_color(&DIM, &bounds);
        paint_size_label(
            window.upcast_ref(),
            &snapshot,
            "1 × 1",
            0.0,
            32.0,
            64.0,
            64.0,
        );
        if let Some(node) = snapshot.to_node() {
            renderer.render_texture(&node, Some(&bounds));
        }
    }
    window.destroy();
    println!(
        "kiekje: overlay warmed up in {:.1} ms",
        (glib::monotonic_time() - started) as f64 / 1000.0
    );
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

    let paint_shared = shared.clone();
    let area = SelectionLayer::new(move |widget, snapshot| {
        paint(&paint_shared, frame_index, widget, snapshot);
    });
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_cursor_from_name(Some("crosshair"));

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&area);
    window.set_child(Some(&overlay));

    let drag = gtk::GestureDrag::new();
    let origin = shared.frames[frame_index].monitor.logical;
    let to_desktop = move |x: f64, y: f64| (f64::from(origin.x) + x, f64::from(origin.y) + y);
    let begin_shared = shared.clone();
    drag.connect_drag_begin(move |gesture, x, y| {
        if begin_shared.chosen.get().is_some() {
            gesture.set_state(gtk::EventSequenceState::Denied);
            return;
        }
        let start = to_desktop(x, y);
        begin_shared.drag.replace(Some(Drag {
            frame_index,
            start,
            current: start,
        }));
        point_at(&begin_shared, Some(start));
        redraw(&begin_shared);
    });
    let update_shared = shared.clone();
    drag.connect_drag_update(move |_, dx, dy| {
        let current = update_shared.drag.borrow_mut().as_mut().map(|drag| {
            drag.current = (drag.start.0 + dx, drag.start.1 + dy);
            drag.current
        });
        point_at(&update_shared, current);
        redraw(&update_shared);
    });
    let end_shared = shared.clone();
    drag.connect_drag_end(move |_, dx, dy| {
        let Some(drag) = end_shared.drag.borrow_mut().take() else {
            return;
        };
        let rect = rect_from_drag(drag.start, (drag.start.0 + dx, drag.start.1 + dy));
        let plan = (rect.width >= MIN_SELECTION && rect.height >= MIN_SELECTION)
            .then(|| plan_selection(rect, &end_shared.geometry))
            .flatten();
        match plan {
            Some(plan @ SelectionPlan::Crop { frame_index, .. }) if end_shared.edit => {
                begin_edit(&end_shared, Selection { rect, plan }, frame_index);
            }
            Some(plan) => finish(&end_shared, Outcome::Selected(Selection { rect, plan })),
            None => redraw(&end_shared),
        }
    });
    area.add_controller(drag);

    let motion = gtk::EventControllerMotion::new();
    let motion_shared = shared.clone();
    motion.connect_motion(move |_, x, y| point_at(&motion_shared, Some(to_desktop(x, y))));
    let leave_shared = shared.clone();
    motion.connect_leave(move |_| {
        if leave_shared.drag.borrow().is_none() {
            point_at(&leave_shared, None);
        }
    });
    area.add_controller(motion);

    let keys = gtk::EventControllerKey::new();
    let key_shared = shared.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let inline = key_shared.inline.borrow().clone();
        if let Some(inline) = inline {
            return inline.key(key, state);
        }
        if key_shared.chosen.get().is_none()
            && key_shared.magnifier.key(key, state, &key_shared.frames)
        {
            for area in key_shared.areas.borrow().iter() {
                area.layer.queue_draw();
            }
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Escape {
            finish(&key_shared, Outcome::Canceled);
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

    shared.areas.borrow_mut().push(Area {
        frame_index,
        layer: area,
        showing: Cell::new(false),
    });
    shared.surfaces.borrow_mut().push((frame_index, overlay));
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

fn paint(shared: &Shared, frame_index: usize, widget: &gtk::Widget, snapshot: &gtk::Snapshot) {
    paint_selection(shared, frame_index, widget, snapshot);
    if shared.chosen.get().is_none() {
        shared
            .magnifier
            .paint(&shared.frames[frame_index], widget, snapshot);
    }
}

fn paint_selection(
    shared: &Shared,
    frame_index: usize,
    widget: &gtk::Widget,
    snapshot: &gtk::Snapshot,
) {
    let (width, height) = (widget.width() as f32, widget.height() as f32);
    let drag = shared.drag.borrow();
    let (rect, dragged_on) = match (drag.as_ref(), shared.chosen.get()) {
        (Some(drag), _) => (
            rect_from_drag(drag.start, drag.current),
            Some(drag.frame_index),
        ),
        (None, Some(rect)) => (rect, None),
        (None, None) => {
            snapshot.append_color(&DIM, &graphene::Rect::new(0.0, 0.0, width, height));
            return;
        }
    };
    let origin = shared.frames[frame_index].monitor.logical;
    let (x, y, w, h) = (
        (rect.x - origin.x) as f32,
        (rect.y - origin.y) as f32,
        rect.width as f32,
        rect.height as f32,
    );
    let (left, right) = (x.clamp(0.0, width), (x + w).clamp(0.0, width));
    let (top, bottom) = (y.clamp(0.0, height), (y + h).clamp(0.0, height));
    for (dim_x, dim_y, dim_w, dim_h) in [
        (0.0, 0.0, width, top),
        (0.0, bottom, width, height - bottom),
        (0.0, top, left, bottom - top),
        (right, top, width - right, bottom - top),
    ] {
        if dim_w > 0.0 && dim_h > 0.0 {
            snapshot.append_color(&DIM, &graphene::Rect::new(dim_x, dim_y, dim_w, dim_h));
        }
    }
    let border = match dragged_on {
        Some(_) => graphene::Rect::new(x, y, w, h),
        None => graphene::Rect::new(x - 1.0, y - 1.0, w + 2.0, h + 2.0),
    };
    if w > 0.0 && h > 0.0 {
        snapshot.append_border(
            &gsk::RoundedRect::from_rect(border, 0.0),
            &[1.0; 4],
            &[ACCENT; 4],
        );
    }

    let Some(dragged_on) = dragged_on else {
        return;
    };
    if label_frame(shared, rect, dragged_on) != frame_index {
        return;
    }
    let Some(plan) = plan_selection(rect, &shared.geometry) else {
        return;
    };
    let (pixels_wide, pixels_high) = plan.size();
    paint_size_label(
        widget,
        snapshot,
        &format!("{pixels_wide} × {pixels_high}"),
        x,
        y + h,
        width,
        height,
    );
}

fn label_frame(shared: &Shared, rect: LogicalRect, fallback: usize) -> usize {
    shared
        .geometry
        .iter()
        .position(|(logical, _)| logical.contains_point(rect.x, rect.bottom() - 1))
        .unwrap_or(fallback)
}

fn paint_size_label(
    widget: &gtk::Widget,
    snapshot: &gtk::Snapshot,
    text: &str,
    x: f32,
    bottom: f32,
    width: f32,
    height: f32,
) {
    let layout = widget.create_pango_layout(Some(text));
    layout.set_font_description(Some(&pango::FontDescription::from_string(
        "Geist Mono 12px",
    )));
    let (_, logical) = layout.pixel_extents();
    let padding = 6.0;
    let label_width = logical.width() as f32 + padding * 2.0;
    let label_height = 22.0;
    let left = x.min(width - label_width).max(0.0);
    let top = (bottom + 6.0).min(height - label_height).max(0.0);

    snapshot.append_color(
        &LABEL_BACKGROUND,
        &graphene::Rect::new(left, top, label_width, label_height),
    );
    snapshot.save();
    snapshot.translate(&graphene::Point::new(
        left + padding,
        top + (label_height - logical.height() as f32) / 2.0,
    ));
    snapshot.append_layout(&layout, &LABEL_TEXT);
    snapshot.restore();
}

fn begin_edit(shared: &Rc<Shared>, selection: Selection, frame_index: usize) {
    shared.chosen.set(Some(selection.rect));
    for area in shared.areas.borrow().iter() {
        area.layer.set_cursor_from_name(Some("default"));
    }
    redraw(shared);
    let frames = shared.frames.clone();
    let plan = selection.plan;
    let weak = Rc::downgrade(shared);
    glib::spawn_future_local(async move {
        let rendered = gio::spawn_blocking(move || output::render_selection(&frames, plan)).await;
        let Some(shared) = weak.upgrade().filter(|shared| !shared.finished.get()) else {
            return;
        };
        let Ok(image) = rendered else {
            eprintln!("kiekje: the render worker stopped, finishing without editing");
            finish(&shared, Outcome::Selected(selection));
            return;
        };
        let ending = Rc::downgrade(&shared);
        let opened = Inline::open(image, CaptureMode::Region, move |end| {
            let Some(shared) = ending.upgrade() else {
                return;
            };
            match end {
                Ending::Canceled => finish(&shared, Outcome::Canceled),
                Ending::Finished { image, finish: how } => finish(
                    &shared,
                    Outcome::Edited {
                        selection,
                        image,
                        finish: how,
                    },
                ),
            }
        })
        .await;
        if shared.finished.get() {
            return;
        }
        let inline = match opened {
            Ok(inline) => inline,
            Err(error) => {
                eprintln!("kiekje: inline editing is unavailable, finishing without it: {error}");
                finish(&shared, Outcome::Selected(selection));
                return;
            }
        };
        let surfaces = shared.surfaces.borrow();
        let Some((_, surface)) = surfaces.iter().find(|(index, _)| *index == frame_index) else {
            drop(surfaces);
            finish(&shared, Outcome::Selected(selection));
            return;
        };
        let monitor = shared.frames[frame_index].monitor.logical;
        let layer = gtk::Fixed::new();
        surface.add_overlay(&layer);
        let local = LogicalRect {
            x: selection.rect.x - monitor.x,
            y: selection.rect.y - monitor.y,
            ..selection.rect
        };
        inline.mount(&layer, local, (monitor.width, monitor.height));
        drop(surfaces);
        shared.inline.replace(Some(inline));
    });
}

fn redraw(shared: &Shared) {
    let selection = shared
        .drag
        .borrow()
        .as_ref()
        .map(|drag| rect_from_drag(drag.start, drag.current))
        .or(shared.chosen.get());
    for area in shared.areas.borrow().iter() {
        let monitor = shared.frames[area.frame_index].monitor.logical;
        let showing = selection.is_some_and(|rect| {
            rect.intersection(monitor).is_some() || monitor.contains_point(rect.x, rect.y)
        });
        if showing || area.showing.get() {
            area.layer.queue_draw();
        }
        area.showing.set(showing);
    }
}

fn point_at(shared: &Shared, point: Option<(f64, f64)>) {
    let previous = shared.magnifier.move_to(point);
    if !shared.magnifier.enabled() || shared.chosen.get().is_some() {
        return;
    }
    for area in shared.areas.borrow().iter() {
        let monitor = shared.frames[area.frame_index].monitor.logical;
        if [previous, point]
            .into_iter()
            .flatten()
            .any(|point| magnifier::on_monitor(point, monitor))
        {
            area.layer.queue_draw();
        }
    }
}

fn watch_monitor(shared: &Rc<Shared>, monitor: &gdk::Monitor, connector: &str) {
    let weak = Rc::downgrade(shared);
    let connector = connector.to_owned();
    let handler = monitor.connect_invalidate(move |_| {
        if let Some(shared) = weak.upgrade() {
            finish(&shared, Outcome::MonitorLost(connector.clone()));
        }
    });
    shared.watched.borrow_mut().push((monitor.clone(), handler));
}

fn finish(shared: &Shared, outcome: Outcome) {
    if shared.finished.replace(true) {
        return;
    }
    for (monitor, handler) in shared.watched.borrow_mut().drain(..) {
        monitor.disconnect(handler);
    }
    // Wayland only accepts a clipboard change from a focused client, so deliver before closing.
    if let Some(callback) = shared.on_done.borrow_mut().take() {
        callback(outcome);
    }
    for window in shared.windows.borrow_mut().drain(..) {
        window.close();
    }
    shared.areas.borrow_mut().clear();
    shared.surfaces.borrow_mut().clear();
    shared.inline.borrow_mut().take();
}
