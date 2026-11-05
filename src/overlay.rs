use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

mod keyboard;
mod magnifier;
mod precision;
pub mod recording;

use magnifier::Magnifier;
use precision::Precision;

use crate::capture::mapping::{
    PixelRect, SelectionPlan, logical_to_pixels, pixels_to_logical, plan_selection, rect_from_drag,
};
use crate::capture::{CaptureMode, Frame, LogicalRect};
use crate::config;
use crate::editor::{Ending, Finish, Inline, Starter, Tool};
use crate::output::Image;
use crate::ui;

mod snap;
mod unplug;

use snap::Snap;

const ACCENT: gdk::RGBA = gdk::RGBA::new(123.0 / 255.0, 126.0 / 255.0, 246.0 / 255.0, 1.0);
const DIM: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.45);
const LABEL_BACKGROUND: gdk::RGBA = gdk::RGBA::new(10.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0, 0.9);
const LABEL_TEXT: gdk::RGBA = gdk::RGBA::new(237.0 / 255.0, 237.0 / 255.0, 237.0 / 255.0, 1.0);
const MIN_SELECTION: i32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub mode: CaptureMode,
    pub edit: bool,
    pub overlay: config::Overlay,
    pub drawing: config::Drawing,
    pub last_region: Option<LogicalRect>,
}

#[derive(Clone, Copy)]
pub struct Selection {
    pub rect: LogicalRect,
    pub plan: SelectionPlan,
}

pub enum Outcome {
    Selected(Selection),
    Saved(crate::notify::Notice),
    Edited {
        selection: Selection,
        image: Image,
        finish: Finish,
        annotated: bool,
    },
    Record {
        selection: Selection,
        gif: bool,
    },
    Canceled,
    MonitorLost(String),
}

struct Drag {
    frame_index: usize,
    pointer: (f64, f64),
    start: (f64, f64),
    current: (f64, f64),
}

impl Drag {
    fn moved(&self) -> bool {
        !snap::is_click(self.start, self.current)
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Film {
    Off,
    Video,
    Gif,
}

impl Film {
    const ALL: [Self; 3] = [Self::Off, Self::Video, Self::Gif];

    fn next(self) -> Self {
        match self {
            Self::Off => Self::Video,
            Self::Video => Self::Gif,
            Self::Gif => Self::Off,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Off => "Screenshot",
            Self::Video => "Video",
            Self::Gif => "GIF",
        }
    }

    fn digit(self) -> &'static str {
        match self {
            Self::Off => "1",
            Self::Video => "2",
            Self::Gif => "3",
        }
    }

    fn for_key(key: gdk::Key) -> Option<Self> {
        match key {
            gdk::Key::_1 | gdk::Key::KP_1 => Some(Self::Off),
            gdk::Key::_2 | gdk::Key::KP_2 => Some(Self::Video),
            gdk::Key::_3 | gdk::Key::KP_3 => Some(Self::Gif),
            _ => None,
        }
    }
}

struct ModeBar {
    bar: gtk::Box,
    buttons: Vec<(Film, gtk::Button)>,
}

struct Mounted {
    frame_index: usize,
    layer: gtk::Fixed,
    lens: SelectionLayer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Stage {
    drawing: bool,
    region: bool,
    windows: bool,
    last_region: bool,
    loupe: bool,
    record_key: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hint {
    keys: &'static str,
    label: &'static str,
    press: Option<(gdk::Key, gdk::ModifierType)>,
    on: bool,
}

struct Shared {
    frames: Arc<Vec<Frame>>,
    geometry: Vec<(LogicalRect, (u32, u32))>,
    mode: CaptureMode,
    edit: bool,
    snap: Snap,
    keyboard: Cell<keyboard::State>,
    drag: RefCell<Option<Drag>>,
    chosen: Cell<Option<LogicalRect>>,
    inline: RefCell<Option<Inline>>,
    editing: Cell<Option<usize>>,
    windows: RefCell<Vec<gtk::Window>>,
    areas: RefCell<Vec<Area>>,
    surfaces: RefCell<Vec<(usize, gtk::Overlay)>>,
    finished: Cell<bool>,
    on_done: RefCell<Option<DoneCallback>>,
    watched: RefCell<Vec<(glib::Object, glib::SignalHandlerId)>>,
    on_first_paint: RefCell<Option<PaintCallback>>,
    magnifier: Magnifier,
    precision: Precision,
    last_region: Option<LogicalRect>,
    pointer: Cell<Option<(f64, f64)>>,
    hints: RefCell<Vec<(usize, gtk::Box)>>,
    hints_on: Cell<bool>,
    lens: Cell<bool>,
    mounted: RefCell<Option<Mounted>>,
    film: Cell<Film>,
    modes: RefCell<Vec<ModeBar>>,
    drawing: config::Drawing,
    starter: Starter,
}

/// Shows each monitor's frozen frame on its own surface and lets the user drag a selection.
///
/// Surfaces are layer-shell overlays when the compositor supports it and fullscreen windows
/// otherwise. A drag may cross monitor edges. With `edit`, a selection inside one monitor stays
/// on screen with the annotation tools until the user accepts or cancels. With the magnifier on, a
/// loupe follows the pointer until a selection is made. Escape cancels, and
/// unplugging a monitor that shows a frame ends the selection with `Outcome::MonitorLost`.
/// `on_done` runs once with the outcome. `windows` are the visible window frames at trigger
/// time, topmost first; hovering one highlights it and a click without a drag selects it. Before
/// a selection, F takes the monitor under the pointer, W the window under it, Shift+R the last
/// region, and D, a tool letter or a tool in the top bar starts annotating the frame, after which
/// a region can still be chosen. In `CaptureMode::Window` the overlay only picks windows: a click
/// or Enter takes the hovered one and dragging selects nothing. A hint bar at the bottom lists the
/// keys, each one clickable, and stays while drawing, where G records and Z shows the loupe; ?
/// hides it.
///
/// # Examples
///
/// ```ignore
/// overlay::show(&app, frames, &windows, monitors, Options::default(), |outcome| {}, || {});
/// ```
pub fn show(
    app: &gtk::Application,
    frames: Arc<Vec<Frame>>,
    windows: &[LogicalRect],
    monitors: &[gdk::Monitor],
    options: Options,
    on_done: impl FnOnce(Outcome) + 'static,
    on_first_paint: impl FnOnce() + 'static,
) {
    let geometry: Vec<(LogicalRect, (u32, u32))> = frames
        .iter()
        .map(|frame| (frame.monitor.logical, (frame.width, frame.height)))
        .collect();
    let logical: Vec<LogicalRect> = geometry.iter().map(|(rect, _)| *rect).collect();
    ui::load_stylesheet();
    let shared = Rc::new(Shared {
        frames: frames.clone(),
        geometry,
        mode: options.mode,
        edit: options.edit,
        snap: Snap::new(windows, &logical),
        keyboard: Cell::new(keyboard::State::default()),
        drag: RefCell::new(None),
        chosen: Cell::new(None),
        inline: RefCell::new(None),
        editing: Cell::new(None),
        windows: RefCell::new(Vec::new()),
        areas: RefCell::new(Vec::new()),
        surfaces: RefCell::new(Vec::new()),
        finished: Cell::new(false),
        on_done: RefCell::new(Some(Box::new(on_done))),
        watched: RefCell::new(Vec::new()),
        on_first_paint: RefCell::new(Some(Box::new(on_first_paint))),
        magnifier: Magnifier::new(options.overlay.magnifier, options.overlay.magnifier_pixels),
        precision: Precision::new(options.overlay.slow_pointer.factor()),
        last_region: options.last_region,
        pointer: Cell::new(None),
        hints: RefCell::new(Vec::new()),
        hints_on: Cell::new(options.overlay.hints),
        lens: Cell::new(false),
        mounted: RefCell::new(None),
        film: Cell::new(Film::Off),
        modes: RefCell::new(Vec::new()),
        drawing: options.drawing,
        starter: Starter::new(options.drawing),
    });

    let shown: Vec<String> = frames
        .iter()
        .map(|frame| frame.monitor.connector.clone())
        .collect();
    if let Some(lost) = unplug::lost_monitor(&shown, &connected_monitors()) {
        finish(&shared, Outcome::MonitorLost(lost.to_owned()));
        return;
    }

    for (frame_index, frame) in frames.iter().enumerate() {
        let Some(monitor) = monitors.iter().find(|monitor| {
            monitor.is_valid()
                && monitor
                    .connector()
                    .is_some_and(|name| name == frame.monitor.connector)
        }) else {
            warn!("no GDK monitor for {}", frame.monitor.connector);
            continue;
        };
        build_surface(app, &shared, frame_index, monitor);
        watch_monitor(&shared, monitor, &frame.monitor.connector);
    }
    watch_unplug(&shared, shown);

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
    info!(
        "overlay warmed up in {:.1} ms",
        (glib::monotonic_time() - started) as f64 / 1000.0
    );
}

fn build_surface(
    app: &gtk::Application,
    shared: &Rc<Shared>,
    frame_index: usize,
    monitor: &gdk::Monitor,
) {
    let window = gtk::Window::builder()
        .application(app)
        .decorated(false)
        .build();
    if uses_layer_shell() {
        place_on_layer(&window, monitor);
    } else {
        window.fullscreen_on_monitor(monitor);
    }

    let picture = gtk::Picture::for_paintable(&Frame::shared_texture(&shared.frames, frame_index));
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
    let hints = hint_bar(shared);
    hints.set_visible(shared.hints_on.get());
    overlay.add_overlay(&hints);
    shared.hints.borrow_mut().push((frame_index, hints));
    let modes = mode_bar(shared, frame_index);
    overlay.add_overlay(&modes.bar);
    shared.modes.borrow_mut().push(modes);
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
        let pointer = to_desktop(x, y);
        let start = track(&begin_shared, pointer, gesture.current_event_state());
        begin_shared.drag.replace(Some(Drag {
            frame_index,
            pointer,
            start,
            current: start,
        }));
        point_at(&begin_shared, Some(start));
        show_hints(&begin_shared, false);
        show_modes(&begin_shared, false);
        redraw(&begin_shared);
    });
    let update_shared = shared.clone();
    drag.connect_drag_update(move |gesture, dx, dy| {
        let pointer = update_shared
            .drag
            .borrow()
            .as_ref()
            .map(|drag| (drag.pointer.0 + dx, drag.pointer.1 + dy));
        let current = pointer.map(|pointer| {
            let current = track(&update_shared, pointer, gesture.current_event_state());
            if let Some(drag) = update_shared.drag.borrow_mut().as_mut() {
                drag.current = current;
            }
            current
        });
        point_at(&update_shared, current);
        redraw(&update_shared);
    });
    let end_shared = shared.clone();
    drag.connect_drag_end(move |gesture, dx, dy| {
        let Some(drag) = end_shared.drag.borrow_mut().take() else {
            return;
        };
        let end = track(
            &end_shared,
            (drag.pointer.0 + dx, drag.pointer.1 + dy),
            gesture.current_event_state(),
        );
        let rect = if picks_windows(&end_shared) {
            end_shared.snap.at(end)
        } else if snap::is_click(drag.start, end) {
            end_shared.snap.at(drag.start)
        } else {
            Some(rect_from_drag(drag.start, end))
        };
        let chosen = rect
            .filter(|rect| rect.width >= MIN_SELECTION && rect.height >= MIN_SELECTION)
            .is_some_and(|rect| choose(&end_shared, rect));
        if !chosen {
            show_hints(&end_shared, end_shared.hints_on.get());
            show_modes(&end_shared, true);
            redraw(&end_shared);
        }
    });
    area.add_controller(drag);

    let motion = gtk::EventControllerMotion::new();
    let motion_shared = shared.clone();
    let hover = move |controller: &gtk::EventControllerMotion, x: f64, y: f64| {
        let point = track(
            &motion_shared,
            to_desktop(x, y),
            controller.current_event_state(),
        );
        if motion_shared.chosen.get().is_none() && motion_shared.snap.hover(point) {
            redraw(&motion_shared);
        }
        point_at(&motion_shared, Some(point));
    };
    let enter = hover.clone();
    motion.connect_enter(move |controller, x, y| enter(controller, x, y));
    motion.connect_motion(move |controller, x, y| hover(controller, x, y));
    let leave_shared = shared.clone();
    motion.connect_leave(move |_| {
        if leave_shared.drag.borrow().is_none() {
            point_at(&leave_shared, None);
        }
    });
    area.add_controller(motion);

    let scroll = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
    );
    let scroll_shared = shared.clone();
    scroll.connect_scroll(move |_, _, dy| {
        if scroll_shared.chosen.get().is_some() || !scroll_shared.magnifier.zoom(dy) {
            return glib::Propagation::Proceed;
        }
        for area in scroll_shared.areas.borrow().iter() {
            area.layer.queue_draw();
        }
        glib::Propagation::Stop
    });
    area.add_controller(scroll);

    let keys = gtk::EventControllerKey::new();
    let key_shared = shared.clone();
    keys.connect_key_pressed(move |_, key, _, state| key_pressed(&key_shared, key, state));
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
    if shared.editing.get() == Some(frame_index) {
        return;
    }
    paint_selection(shared, frame_index, widget, snapshot);
    keyboard::paint(shared, frame_index, snapshot);
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
    let moved = drag
        .as_ref()
        .filter(|drag| drag.moved() && !picks_windows(shared));
    let preview = shared.keyboard.get().preview;
    let (rect, dragged_on) = match (
        moved,
        shared.chosen.get().or(preview).or(shared.snap.hovered()),
    ) {
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
    layout.set_font_description(Some(&magnifier::font(12)));
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

fn key_pressed(shared: &Rc<Shared>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
    let inline = shared.inline.borrow().clone();
    if let Some(inline) = inline {
        if drawing_shortcut(shared, &inline, key, state) {
            return glib::Propagation::Stop;
        }
        return inline.key(key, state);
    }
    if key == gdk::Key::Escape {
        finish(shared, Outcome::Canceled);
        return glib::Propagation::Stop;
    }
    if shared.chosen.get().is_some() || shared.drag.borrow().is_some() {
        return glib::Propagation::Proceed;
    }
    if keyboard::press(shared, key, state) {
        return glib::Propagation::Stop;
    }
    if shared.magnifier.key(key, state, &shared.frames) {
        for area in shared.areas.borrow().iter() {
            area.layer.queue_draw();
        }
        refresh_hints(shared);
        return glib::Propagation::Stop;
    }
    if shortcut(shared, key, state) {
        glib::Propagation::Stop
    } else {
        glib::Propagation::Proceed
    }
}

fn drawing_shortcut(
    shared: &Rc<Shared>,
    inline: &Inline,
    key: gdk::Key,
    state: gdk::ModifierType,
) -> bool {
    if inline.typing()
        || state.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        )
    {
        return false;
    }
    match key.to_lower() {
        _ if key == gdk::Key::question => toggle_hints(shared),
        gdk::Key::z => set_lens(shared, !shared.lens.get()),
        gdk::Key::c if shared.lens.get() => {
            if shared.magnifier.copy_colour(&shared.frames) {
                redraw_lens(shared);
            }
        }
        gdk::Key::g if stage(shared).record_key => record_from_drawing(shared, inline),
        _ => return false,
    }
    true
}

fn shortcut(shared: &Rc<Shared>, key: gdk::Key, state: gdk::ModifierType) -> bool {
    if state.intersects(
        gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK,
    ) {
        return false;
    }
    let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
    if let Some(film) = Film::for_key(key).filter(|_| !shift) {
        set_film(shared, film);
        return true;
    }
    match key.to_lower() {
        _ if key == gdk::Key::question => {
            toggle_hints(shared);
            true
        }
        gdk::Key::w | gdk::Key::Return | gdk::Key::KP_Enter if !shift => shared
            .snap
            .hovered()
            .is_some_and(|window| choose(shared, window)),
        gdk::Key::g if !shift => {
            set_film(shared, shared.film.get().next());
            true
        }
        _ if picks_windows(shared) => false,
        gdk::Key::r if shift => shared
            .last_region
            .is_some_and(|region| choose(shared, region)),
        _ if shift => false,
        gdk::Key::f => choose(shared, shared.geometry[pointer_frame(shared)].0),
        gdk::Key::d => {
            begin_edit(shared, pointer_frame(shared), None, Some(Tool::Pen));
            true
        }
        _ => match Inline::tool_for_key(&shared.drawing.keys, key) {
            Some(tool) => {
                begin_edit(shared, pointer_frame(shared), None, Some(tool));
                true
            }
            None => false,
        },
    }
}

fn choose(shared: &Rc<Shared>, rect: LogicalRect) -> bool {
    let Some(plan) = plan_selection(rect, &shared.geometry) else {
        return false;
    };
    let selection = Selection { rect, plan };
    match plan {
        _ if shared.film.get() != Film::Off => finish(
            shared,
            Outcome::Record {
                selection,
                gif: shared.film.get() == Film::Gif,
            },
        ),
        SelectionPlan::Crop { frame_index, .. } if shared.edit => {
            begin_edit(shared, frame_index, Some(rect), None);
        }
        _ => finish(shared, Outcome::Selected(selection)),
    }
    true
}

fn picks_windows(shared: &Shared) -> bool {
    shared.mode == CaptureMode::Window
}

fn pointer_frame(shared: &Shared) -> usize {
    shared
        .pointer
        .get()
        .and_then(|(x, y)| {
            shared
                .geometry
                .iter()
                .position(|(logical, _)| logical.contains_point(x.floor() as i32, y.floor() as i32))
        })
        .unwrap_or(0)
}

fn hint_bar(shared: &Rc<Shared>) -> gtk::Box {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .margin_bottom(32)
        .can_focus(false)
        .css_classes(["panel", "overlay-hints"])
        .build();
    fill_hints(shared, &bar);
    bar
}

fn stage(shared: &Shared) -> Stage {
    let drawing = shared.chosen.get().is_some();
    Stage {
        drawing,
        region: !picks_windows(shared),
        windows: shared.snap.has_windows(),
        last_region: shared.last_region.is_some(),
        loupe: if drawing {
            shared.lens.get()
        } else {
            shared.magnifier.enabled()
        },
        record_key: shared.drawing.keys.action('g').is_none(),
    }
}

fn hints_for(stage: Stage) -> Vec<Hint> {
    let plain = |key| Some((key, gdk::ModifierType::empty()));
    let loupe = stage.loupe;
    let rows = if stage.drawing {
        vec![
            (true, "S", "region", plain(gdk::Key::s), false),
            (true, "Enter", "accept", plain(gdk::Key::Return), false),
            (stage.record_key, "G", "record", plain(gdk::Key::g), false),
            (true, "Z", "loupe", plain(gdk::Key::z), loupe),
            (loupe, "C", "colour", None, false),
            (true, "?", "hide", plain(gdk::Key::question), false),
            (true, "Esc", "cancel", plain(gdk::Key::Escape), false),
        ]
    } else {
        let (region, windows) = (stage.region, stage.windows);
        vec![
            (!region && !windows, "", "no windows to pick", None, false),
            (!region && windows, "Click", "window", None, false),
            (region, "Drag", "select", None, false),
            (region, "Arrows", "move cursor", None, false),
            (region, "Space", "set corner", None, false),
            (region, "F", "screen", plain(gdk::Key::f), false),
            (windows, "W", "next window", plain(gdk::Key::w), false),
            (
                windows || region,
                "Enter",
                "capture",
                plain(gdk::Key::Return),
                false,
            ),
            (
                region && stage.last_region,
                "Shift R",
                "last region",
                Some((gdk::Key::R, gdk::ModifierType::SHIFT_MASK)),
                false,
            ),
            (true, "G", "film mode", plain(gdk::Key::g), false),
            (region, "D", "draw", plain(gdk::Key::d), false),
            (true, "Z", "loupe", plain(gdk::Key::z), loupe),
            (loupe, "C", "colour", None, false),
            (true, "?", "hide", plain(gdk::Key::question), false),
            (true, "Esc", "cancel", plain(gdk::Key::Escape), false),
        ]
    };
    rows.into_iter()
        .filter(|(shown, ..)| *shown)
        .map(|(_, keys, label, press, on)| Hint {
            keys,
            label,
            press,
            on,
        })
        .collect()
}

fn fill_hints(shared: &Rc<Shared>, bar: &gtk::Box) {
    while let Some(child) = bar.first_child() {
        bar.remove(&child);
    }
    for hint in hints_for(stage(shared)) {
        bar.append(&hint_item(shared, hint));
    }
    bar.queue_resize();
}

fn hint_item(shared: &Rc<Shared>, hint: Hint) -> gtk::Widget {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    if !hint.keys.is_empty() {
        content.append(
            &gtk::Label::builder()
                .label(hint.keys)
                .css_classes(["keycap"])
                .build(),
        );
    }
    content.append(
        &gtk::Label::builder()
            .label(hint.label)
            .css_classes(["hint"])
            .build(),
    );
    let Some((key, state)) = hint.press else {
        content.add_css_class("hint-item");
        return content.upcast();
    };
    let button = gtk::Button::builder()
        .child(&content)
        .focus_on_click(false)
        .build();
    button.set_css_classes(&hint_classes(hint.on));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{} ({})",
        hint.label, hint.keys
    ))]);
    let weak = Rc::downgrade(shared);
    button.connect_clicked(move |_| {
        if let Some(shared) = weak.upgrade() {
            key_pressed(&shared, key, state);
        }
    });
    button.upcast()
}

fn hint_classes(on: bool) -> Vec<&'static str> {
    if on {
        vec!["act", "selected"]
    } else {
        vec!["act"]
    }
}

fn refresh_hints(shared: &Rc<Shared>) {
    for (_, bar) in shared.hints.borrow().iter() {
        fill_hints(shared, bar);
    }
}

fn hint_reserve(shared: &Shared) -> i32 {
    if !shared.hints_on.get() {
        return 0;
    }
    shared.hints.borrow().first().map_or(0, |(_, bar)| {
        let (_, natural, _, _) = bar.measure(gtk::Orientation::Vertical, -1);
        natural + bar.margin_bottom()
    })
}

fn toggle_hints(shared: &Shared) {
    let on = !shared.hints_on.get();
    shared.hints_on.set(on);
    show_hints(shared, on);
    if let Some(inline) = shared.inline.borrow().as_ref() {
        inline.reserve(0, hint_reserve(shared));
    }
}

fn mode_bar(shared: &Rc<Shared>, frame_index: usize) -> ModeBar {
    let region = !picks_windows(shared);
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .margin_top(if region { 8 } else { 32 })
        .can_focus(false)
        .css_classes(["editor"])
        .build();
    if region {
        let weak = Rc::downgrade(shared);
        column.append(&shared.starter.bar(move |tool| {
            if let Some(shared) = weak
                .upgrade()
                .filter(|shared| shared.chosen.get().is_none())
            {
                begin_edit(&shared, frame_index, None, Some(tool));
            }
        }));
    }
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .halign(gtk::Align::Center)
        .css_classes(["panel", "overlay-modes"])
        .build();
    let mut buttons = Vec::new();
    for film in Film::ALL {
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();
        content.append(
            &gtk::Label::builder()
                .label(film.digit())
                .css_classes(["keycap"])
                .build(),
        );
        content.append(&gtk::Label::new(Some(film.label())));
        let button = gtk::Button::builder()
            .child(&content)
            .tooltip_text(format!("{} (key {}, G cycles)", film.label(), film.digit()))
            .css_classes(["act"])
            .focus_on_click(false)
            .build();
        button.update_property(&[gtk::accessible::Property::Label(&format!(
            "{} ({})",
            film.label(),
            film.digit()
        ))]);
        button.set_css_classes(&mode_classes(film, shared.film.get()));
        let weak = Rc::downgrade(shared);
        button.connect_clicked(move |_| {
            if let Some(shared) = weak.upgrade() {
                set_film(&shared, film);
            }
        });
        bar.append(&button);
        buttons.push((film, button));
    }
    column.append(&bar);
    ModeBar {
        bar: column,
        buttons,
    }
}

fn mode_classes(button: Film, current: Film) -> Vec<&'static str> {
    if button == current {
        vec!["act", "selected"]
    } else {
        vec!["act"]
    }
}

fn set_film(shared: &Shared, film: Film) {
    shared.film.set(film);
    for modes in shared.modes.borrow().iter() {
        for (own, button) in &modes.buttons {
            button.set_css_classes(&mode_classes(*own, film));
        }
    }
}

fn show_modes(shared: &Shared, shown: bool) {
    for modes in shared.modes.borrow().iter() {
        modes.bar.set_visible(shown);
    }
}

fn show_hints(shared: &Shared, shown: bool) {
    for (_, hints) in shared.hints.borrow().iter() {
        hints.set_visible(shown);
    }
}

fn set_lens(shared: &Rc<Shared>, on: bool) {
    shared.lens.set(on);
    redraw_lens(shared);
    refresh_hints(shared);
}

fn redraw_lens(shared: &Shared) {
    if let Some(mounted) = shared.mounted.borrow().as_ref() {
        mounted.lens.queue_draw();
    }
}

fn lens_layer(shared: &Rc<Shared>, frame_index: usize) -> SelectionLayer {
    let weak = Rc::downgrade(shared);
    let lens = SelectionLayer::new(move |widget, snapshot| {
        if let Some(shared) = weak.upgrade().filter(|shared| shared.lens.get()) {
            shared
                .magnifier
                .draw(&shared.frames[frame_index], widget, snapshot);
        }
    });
    lens.set_can_target(false);
    lens.set_hexpand(true);
    lens.set_vexpand(true);
    lens
}

fn record_from_drawing(shared: &Rc<Shared>, inline: &Inline) {
    if inline.has_region() {
        inline.record(false);
        return;
    }
    stop_drawing(shared);
    set_film(shared, Film::Video);
}

fn stop_drawing(shared: &Rc<Shared>) {
    shared.inline.replace(None);
    if let Some(mounted) = shared.mounted.take()
        && let Some((_, surface)) = shared
            .surfaces
            .borrow()
            .iter()
            .find(|(index, _)| *index == mounted.frame_index)
    {
        surface.remove_overlay(&mounted.layer);
        surface.remove_overlay(&mounted.lens);
    }
    shared.editing.set(None);
    shared.chosen.set(None);
    shared.lens.set(false);
    shared.keyboard.set(keyboard::State::default());
    shared.starter.reset();
    for area in shared.areas.borrow().iter() {
        area.layer.set_cursor_from_name(Some("crosshair"));
        area.layer.queue_draw();
    }
    refresh_hints(shared);
    show_hints(shared, shared.hints_on.get());
    show_modes(shared, true);
}

fn begin_edit(
    shared: &Rc<Shared>,
    frame_index: usize,
    region: Option<LogicalRect>,
    first: Option<Tool>,
) {
    let monitor = shared.frames[frame_index].monitor.logical;
    shared.chosen.set(Some(region.unwrap_or(monitor)));
    set_film(shared, Film::Off);
    show_hints(shared, false);
    show_modes(shared, false);
    for area in shared.areas.borrow().iter() {
        area.layer.set_cursor_from_name(Some("default"));
    }
    redraw(shared);
    let frames = shared.frames.clone();
    let weak = Rc::downgrade(shared);
    glib::spawn_future_local(async move {
        let Some(shared) = weak.upgrade().filter(|shared| !shared.finished.get()) else {
            return;
        };
        let ending = Rc::downgrade(&shared);
        let suspended = Rc::downgrade(&shared);
        let suspend = move |hidden: bool| {
            let Some(shared) = suspended.upgrade().filter(|shared| !shared.finished.get()) else {
                return;
            };
            for window in shared.windows.borrow().iter() {
                window.set_visible(!hidden);
            }
            if !hidden && let Some(window) = shared.windows.borrow().get(frame_index) {
                window.present();
            }
        };
        let opened = Inline::open(
            frames,
            frame_index,
            shared.mode,
            shared.starter.drawing(),
            suspend,
            move |end| {
                let Some(shared) = ending.upgrade() else {
                    return;
                };
                match end {
                    Ending::Canceled => finish(&shared, Outcome::Canceled),
                    Ending::Saved(notice) => finish(&shared, Outcome::Saved(notice)),
                    Ending::Record { region, gif } => {
                        match edited_selection(&shared, frame_index, region) {
                            Some(selection) => finish(&shared, Outcome::Record { selection, gif }),
                            None => {
                                warn!(
                                    "the region to record is not on its monitor, nothing recorded"
                                );
                                finish(&shared, Outcome::Canceled);
                            }
                        }
                    }
                    Ending::Finished {
                        image,
                        finish: how,
                        region,
                        annotated,
                    } => match edited_selection(&shared, frame_index, region) {
                        Some(selection) => finish(
                            &shared,
                            Outcome::Edited {
                                selection,
                                image,
                                finish: how,
                                annotated,
                            },
                        ),
                        None => {
                            warn!("the edited region is not on its monitor, nothing saved");
                            finish(&shared, Outcome::Canceled);
                        }
                    },
                }
            },
        )
        .await;
        if shared.finished.get() {
            return;
        }
        let inline = match opened {
            Ok(inline) => inline,
            Err(error) => {
                warn!("inline editing is unavailable, finishing without it: {error}");
                finish_unedited(&shared, region);
                return;
            }
        };
        let surfaces = shared.surfaces.borrow();
        let Some((_, surface)) = surfaces.iter().find(|(index, _)| *index == frame_index) else {
            drop(surfaces);
            finish_unedited(&shared, region);
            return;
        };
        let frame = &shared.frames[frame_index];
        let pixels = region.and_then(|rect| {
            let local = LogicalRect {
                x: rect.x - monitor.x,
                y: rect.y - monitor.y,
                ..rect
            };
            logical_to_pixels(
                local,
                (monitor.width, monitor.height),
                (frame.width, frame.height),
            )
        });
        let layer = gtk::Fixed::new();
        surface.add_overlay(&layer);
        let lens = lens_layer(&shared, frame_index);
        surface.add_overlay(&lens);
        refresh_hints(&shared);
        show_hints(&shared, shared.hints_on.get());
        for (_, bar) in shared
            .hints
            .borrow()
            .iter()
            .filter(|(index, _)| *index == frame_index)
        {
            surface.remove_overlay(bar);
            surface.add_overlay(bar);
        }
        inline.mount(
            &layer,
            (monitor.width, monitor.height),
            pixels,
            (0, hint_reserve(&shared)),
        );
        drop(surfaces);
        let pointed = Rc::downgrade(&shared);
        inline.on_pointer(move |point| {
            let Some(shared) = pointed.upgrade() else {
                return;
            };
            shared
                .magnifier
                .move_to(point.map(|(x, y)| (f64::from(monitor.x) + x, f64::from(monitor.y) + y)));
            if shared.lens.get() {
                redraw_lens(&shared);
            }
        });
        shared.mounted.replace(Some(Mounted {
            frame_index,
            layer,
            lens,
        }));
        shared.editing.set(Some(frame_index));
        for area in shared.areas.borrow().iter() {
            area.layer.queue_draw();
        }
        if let Some(tool) = first {
            inline.pick(tool);
        }
        shared.inline.replace(Some(inline));
    });
}

fn finish_unedited(shared: &Shared, region: Option<LogicalRect>) {
    let selection = region.and_then(|rect| {
        plan_selection(rect, &shared.geometry).map(|plan| Selection { rect, plan })
    });
    match selection {
        Some(selection) => finish(shared, Outcome::Selected(selection)),
        None => finish(shared, Outcome::Canceled),
    }
}

fn edited_selection(
    shared: &Shared,
    frame_index: usize,
    region: Option<PixelRect>,
) -> Option<Selection> {
    let frame = &shared.frames[frame_index];
    let monitor = frame.monitor.logical;
    let pixels = region.unwrap_or(PixelRect {
        x: 0,
        y: 0,
        width: frame.width,
        height: frame.height,
    });
    let local = pixels_to_logical(
        pixels,
        (monitor.width, monitor.height),
        (frame.width, frame.height),
    )?;
    Some(Selection {
        rect: LogicalRect {
            x: monitor.x + local.x,
            y: monitor.y + local.y,
            ..local
        },
        plan: SelectionPlan::Crop {
            frame_index,
            rect: pixels,
        },
    })
}

fn redraw(shared: &Shared) {
    let selection = shared
        .drag
        .borrow()
        .as_ref()
        .filter(|drag| drag.moved() && !picks_windows(shared))
        .map(|drag| rect_from_drag(drag.start, drag.current))
        .or(shared.chosen.get())
        .or(shared.snap.hovered());
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

fn track(shared: &Shared, pointer: (f64, f64), state: gdk::ModifierType) -> (f64, f64) {
    let slow = state.contains(gdk::ModifierType::SHIFT_MASK)
        && shared.magnifier.enabled()
        && shared.chosen.get().is_none();
    let point = shared.precision.track(pointer, slow);
    let precise = shared.precision.active();
    if !shared.magnifier.set_precise(precise) {
        return point;
    }
    let cursor = if precise { "none" } else { "crosshair" };
    for area in shared.areas.borrow().iter() {
        area.layer.set_cursor_from_name(Some(cursor));
    }
    point
}

fn point_at(shared: &Shared, point: Option<(f64, f64)>) {
    if point.is_some() {
        shared.pointer.set(point);
    }
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
    shared
        .watched
        .borrow_mut()
        .push((monitor.clone().upcast(), handler));
}

fn watch_unplug(shared: &Rc<Shared>, shown: Vec<String>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let model = display.monitors();
    let weak = Rc::downgrade(shared);
    let handler = model.connect_items_changed(move |_, _, removed, _| {
        if removed == 0 {
            return;
        }
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if let Some(lost) = unplug::lost_monitor(&shown, &connected_monitors()) {
            finish(&shared, Outcome::MonitorLost(lost.to_owned()));
        }
    });
    shared.watched.borrow_mut().push((model.upcast(), handler));
}

fn connected_monitors() -> Vec<String> {
    let Some(display) = gdk::Display::default() else {
        return Vec::new();
    };
    let model = display.monitors();
    (0..model.n_items())
        .filter_map(|index| model.item(index).and_downcast::<gdk::Monitor>())
        .filter(|monitor| monitor.is_valid())
        .filter_map(|monitor| monitor.connector().map(|name| name.to_string()))
        .collect()
}

fn finish(shared: &Shared, outcome: Outcome) {
    if shared.finished.replace(true) {
        return;
    }
    shared.magnifier.remember_zoom();
    for (source, handler) in shared.watched.borrow_mut().drain(..) {
        source.disconnect(handler);
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
    shared.mounted.borrow_mut().take();
    shared.inline.borrow_mut().take();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_every_film_mode_back_to_screenshot() {
        assert_eq!(Film::Off.next(), Film::Video);
        assert_eq!(Film::Video.next(), Film::Gif);
        assert_eq!(Film::Gif.next(), Film::Off);
    }

    fn labels(stage: Stage) -> Vec<&'static str> {
        hints_for(stage).iter().map(|hint| hint.label).collect()
    }

    #[test]
    fn keeps_record_and_loupe_in_the_hint_bar_while_drawing() {
        let drawing = Stage {
            drawing: true,
            region: true,
            record_key: true,
            ..Stage::default()
        };
        assert_eq!(
            labels(drawing),
            ["region", "accept", "record", "loupe", "hide", "cancel"]
        );
        let lens = Stage {
            loupe: true,
            ..drawing
        };
        assert!(labels(lens).contains(&"colour"));
        let loupe = hints_for(lens)
            .into_iter()
            .find(|hint| hint.label == "loupe")
            .unwrap();
        assert!(loupe.on);
        assert_eq!(loupe.press, Some((gdk::Key::z, gdk::ModifierType::empty())));
    }

    #[test]
    fn drops_the_record_hint_when_g_draws_instead() {
        let drawing = Stage {
            drawing: true,
            region: true,
            ..Stage::default()
        };
        assert!(!labels(drawing).contains(&"record"));
    }

    #[test]
    fn lists_region_keys_before_a_selection() {
        let selecting = Stage {
            region: true,
            last_region: true,
            ..Stage::default()
        };
        let hints = hints_for(selecting);
        assert!(hints.iter().any(|hint| hint.label == "film mode"));
        assert!(hints.iter().any(|hint| hint.label == "draw"));
        let last = hints
            .iter()
            .find(|hint| hint.label == "last region")
            .unwrap();
        assert_eq!(
            last.press,
            Some((gdk::Key::R, gdk::ModifierType::SHIFT_MASK))
        );
        let drag = hints.iter().find(|hint| hint.label == "select").unwrap();
        assert_eq!(drag.press, None);
    }

    #[test]
    fn maps_digit_keys_to_film_modes() {
        assert_eq!(Film::for_key(gdk::Key::_1), Some(Film::Off));
        assert_eq!(Film::for_key(gdk::Key::KP_2), Some(Film::Video));
        assert_eq!(Film::for_key(gdk::Key::_3), Some(Film::Gif));
        assert_eq!(Film::for_key(gdk::Key::_4), None);
    }

    #[test]
    fn marks_only_the_current_film_mode_as_selected() {
        assert_eq!(mode_classes(Film::Video, Film::Video), ["act", "selected"]);
        assert_eq!(mode_classes(Film::Gif, Film::Video), ["act"]);
    }
}
