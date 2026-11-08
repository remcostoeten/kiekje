use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::monitors::{gdk_monitors, monitor_info};
use crate::capture::{Availability, LogicalRect};
use crate::config::{self, Corner};
use crate::overlay;
use crate::ui;
use crate::ui::widgets::keycaps;

pub const UNAVAILABLE: &str =
    "The edge dock needs a compositor with layer shell, such as KDE Plasma, Hyprland or Sway";
const RECORDING: &str = "A recording is running, stop it first";

const TRIGGER_WIDTH: i32 = 2;
const TRIGGER_HEIGHT: i32 = 320;
const SHADOW_ROOM: i32 = 28;
const TIP_GAP: i32 = 12;
const HIDE_DELAY: Duration = Duration::from_millis(350);
const SLIDE: Duration = Duration::from_millis(260);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Screen,
    Region,
    Video,
    Gif,
    Text,
}

impl Action {
    const ALL: [Self; 5] = [
        Self::Screen,
        Self::Region,
        Self::Video,
        Self::Gif,
        Self::Text,
    ];

    fn title(self, recording: bool) -> &'static str {
        match self {
            Self::Screen => "Capture every screen",
            Self::Region => "Capture region",
            Self::Video if recording => "Stop recording",
            Self::Video => "Record screen",
            Self::Gif => "Record a GIF of a region",
            Self::Text => "Copy text from region",
        }
    }

    fn icon(self, recording: bool) -> &'static str {
        match self {
            Self::Screen => "kiekje-screens-symbolic",
            Self::Region => "kiekje-region-symbolic",
            Self::Video if recording => "kiekje-stop-symbolic",
            Self::Video => "kiekje-record-symbolic",
            Self::Gif => "kiekje-gif-symbolic",
            Self::Text => "kiekje-text-symbolic",
        }
    }

    fn availability(self, content: &Content) -> Availability {
        match self {
            Self::Text => content.text,
            Self::Gif if content.recording => Availability::Unavailable(RECORDING),
            _ => Availability::Available,
        }
    }
}

pub struct Content {
    pub shortcuts: Vec<(Action, String)>,
    pub text: Availability,
    pub recording: bool,
}

#[derive(Clone)]
struct Hooks {
    app: glib::WeakRef<gtk::Application>,
    content: Rc<dyn Fn() -> Content>,
    on_pick: Rc<dyn Fn(Action)>,
}

#[derive(Default)]
struct State {
    hooks: Option<Hooks>,
    applied: Option<config::Dock>,
    surfaces: Vec<Rc<Surface>>,
    watch: Option<(gio::ListModel, glib::SignalHandlerId)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Says whether the edge dock can be shown in this session.
///
/// The dock is a layer-shell surface, so it needs a Wayland compositor that offers the protocol.
///
/// # Examples
///
/// ```ignore
/// if let Availability::Unavailable(reason) = dock::availability() {
///     switch.set_sensitive(false);
/// }
/// ```
pub fn availability() -> Availability {
    if overlay::uses_layer_shell() {
        Availability::Available
    } else {
        Availability::Unavailable(UNAVAILABLE)
    }
}

/// Connects the dock to the running app: where its button states come from and what a click does.
///
/// `content` runs each time the dock slides in, so shortcuts and the recording state stay current.
/// Nothing is shown until `configure` turns the dock on.
///
/// # Examples
///
/// ```ignore
/// dock::install(&app, || content(), |action| run(action));
/// dock::configure(config.dock);
/// ```
pub fn install(
    app: &gtk::Application,
    content: impl Fn() -> Content + 'static,
    on_pick: impl Fn(Action) + 'static,
) {
    STATE.with(|state| {
        state.borrow_mut().hooks = Some(Hooks {
            app: app.downgrade(),
            content: Rc::new(content),
            on_pick: Rc::new(on_pick),
        });
    });
}

/// Shows, moves or removes the dock to match the `[dock]` settings.
///
/// Calling it with unchanged settings does nothing, so it is safe to call after every save.
///
/// # Examples
///
/// ```ignore
/// dock::configure(config::Dock { enabled: true, corner: Corner::BottomLeft });
/// ```
pub fn configure(settings: config::Dock) {
    let unchanged = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let unchanged = state.applied == Some(settings);
        state.applied = Some(settings);
        unchanged
    });
    if !unchanged {
        rebuild();
    }
}

fn rebuild() {
    let (old, watch, hooks, settings) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        (
            std::mem::take(&mut state.surfaces),
            state.watch.take(),
            state.hooks.clone(),
            state.applied,
        )
    });
    for surface in old {
        surface.destroy();
    }
    if let Some((monitors, handler)) = watch {
        monitors.disconnect(handler);
    }
    let (Some(hooks), Some(settings)) = (hooks, settings) else {
        return;
    };
    if !settings.enabled {
        return;
    }
    if let Availability::Unavailable(reason) = availability() {
        info!("the edge dock is off: {reason}");
        return;
    }
    let Some(app) = hooks.app.upgrade() else {
        return;
    };
    ui::load_stylesheet();

    let monitors = gdk_monitors();
    let rects: Vec<LogicalRect> = monitors
        .iter()
        .map(|monitor| monitor_info(monitor).logical)
        .collect();
    let reduced = reduced_motion();
    let surfaces: Vec<Rc<Surface>> = monitors
        .iter()
        .enumerate()
        .filter(|(index, _)| on_outer_edge(&rects, *index, settings.corner))
        .map(|(_, monitor)| Surface::new(&app, monitor, settings.corner, reduced, hooks.clone()))
        .collect();

    info!(
        "edge dock in the {} corner of {} of {} screens",
        corner_class(settings.corner),
        surfaces.len(),
        monitors.len()
    );
    let watch = gdk::Display::default().map(|display| {
        let monitors = display.monitors();
        let handler = monitors.connect_items_changed(|_, _, _, _| {
            glib::idle_add_local_once(rebuild);
        });
        (monitors, handler)
    });
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.surfaces = surfaces;
        state.watch = watch;
    });
}

fn on_outer_edge(monitors: &[LogicalRect], index: usize, corner: Corner) -> bool {
    let Some(rect) = monitors.get(index) else {
        return false;
    };
    let (top, bottom) = if corner.is_top() {
        (rect.y, rect.y + TRIGGER_HEIGHT.min(rect.height))
    } else {
        (
            rect.bottom() - TRIGGER_HEIGHT.min(rect.height),
            rect.bottom(),
        )
    };
    monitors
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index)
        .all(|(_, other)| {
            let beside = if corner.is_left() {
                other.right() == rect.x
            } else {
                other.x == rect.right()
            };
            let alongside = beside && other.y < bottom && other.bottom() > top;
            let stacked = if corner.is_top() {
                other.bottom() == rect.y
            } else {
                other.y == rect.bottom()
            };
            let edge = if corner.is_left() {
                rect.x
            } else {
                rect.right() - 1
            };
            let continues = stacked && other.x <= edge && other.right() > edge;
            !(alongside || continues)
        })
}

fn reduced_motion() -> bool {
    let Some(settings) = gtk::Settings::default() else {
        return false;
    };
    if !settings.is_gtk_enable_animations() {
        return true;
    }
    const REDUCED_MOTION: &str = "gtk-interface-reduced-motion";
    if settings.find_property(REDUCED_MOTION).is_none() {
        return false;
    }
    let value = settings.property_value(REDUCED_MOTION);
    glib::EnumValue::from_value(&value).is_some_and(|(_, value)| value.nick() == "reduce")
}

struct Surface {
    window: gtk::Window,
    strip: gtk::Box,
    panel: gtk::Box,
    corner: Corner,
    hooks: Hooks,
    hide: RefCell<Option<glib::SourceId>>,
    collapse: RefCell<Option<glib::SourceId>>,
    tips: RefCell<Vec<gtk::Popover>>,
}

impl Surface {
    fn new(
        app: &gtk::Application,
        monitor: &gdk::Monitor,
        corner: Corner,
        reduced: bool,
        hooks: Hooks,
    ) -> Rc<Self> {
        let window = gtk::Window::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .title("Kiekje dock")
            .css_classes(["kiekje-dock"])
            .build();
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace(Some("kiekje-dock"));
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_monitor(Some(monitor));
        window.set_anchor(
            if corner.is_left() {
                Edge::Left
            } else {
                Edge::Right
            },
            true,
        );
        window.set_anchor(
            if corner.is_top() {
                Edge::Top
            } else {
                Edge::Bottom
            },
            true,
        );

        let horizontal = if corner.is_left() {
            gtk::Align::Start
        } else {
            gtk::Align::End
        };
        let vertical = if corner.is_top() {
            gtk::Align::Start
        } else {
            gtk::Align::End
        };
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(horizontal)
            .valign(vertical)
            .build();
        let strip = gtk::Box::builder()
            .width_request(TRIGGER_WIDTH)
            .height_request(TRIGGER_HEIGHT)
            .halign(horizontal)
            .css_classes(["dock-trigger"])
            .build();
        let panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .halign(horizontal)
            .valign(vertical)
            .visible(false)
            .css_classes(["dock", corner_class(corner), "out"])
            .build();
        if corner.is_left() {
            panel.add_css_class("left");
            panel.set_margin_end(SHADOW_ROOM);
        } else {
            panel.add_css_class("right");
            panel.set_margin_start(SHADOW_ROOM);
        }
        if corner.is_top() {
            panel.set_margin_bottom(SHADOW_ROOM);
        } else {
            panel.set_margin_top(SHADOW_ROOM);
        }
        if reduced {
            panel.add_css_class("reduced");
        }
        panel.update_property(&[gtk::accessible::Property::Label("Kiekje capture dock")]);
        root.append(&strip);
        root.append(&panel);
        window.set_child(Some(&root));

        let surface = Rc::new(Self {
            window,
            strip,
            panel,
            corner,
            hooks,
            hide: RefCell::new(None),
            collapse: RefCell::new(None),
            tips: RefCell::new(Vec::new()),
        });
        let motion = gtk::EventControllerMotion::new();
        let entered = Rc::downgrade(&surface);
        motion.connect_enter(move |_, x, y| {
            debug!("pointer entered the dock at {x:.0},{y:.0}");
            if let Some(surface) = entered.upgrade() {
                surface.reveal();
            }
        });
        let left = Rc::downgrade(&surface);
        motion.connect_leave(move |_| {
            debug!("pointer left the dock");
            if let Some(surface) = left.upgrade() {
                surface.schedule_hide();
            }
        });
        surface.window.add_controller(motion);
        surface.window.present();
        surface
    }

    fn reveal(self: &Rc<Self>) {
        cancel(&self.hide);
        cancel(&self.collapse);
        if self.panel.is_visible() {
            self.panel.remove_css_class("out");
            return;
        }
        self.fill();
        self.strip.set_visible(false);
        self.panel.add_css_class("out");
        self.panel.set_visible(true);
        after_first_frame(&self.panel, |panel| panel.remove_css_class("out"));
    }

    fn schedule_hide(self: &Rc<Self>) {
        cancel(&self.hide);
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(HIDE_DELAY, move || {
            if let Some(surface) = weak.upgrade() {
                surface.hide.take();
                surface.slide_out();
            }
        });
        self.hide.replace(Some(id));
    }

    fn slide_out(self: &Rc<Self>) {
        if !self.panel.is_visible() {
            return;
        }
        self.close_tips();
        self.panel.add_css_class("out");
        cancel(&self.collapse);
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(SLIDE, move || {
            if let Some(surface) = weak.upgrade() {
                surface.collapse.take();
                surface.collapse_now();
            }
        });
        self.collapse.replace(Some(id));
    }

    fn collapse_now(&self) {
        cancel(&self.hide);
        cancel(&self.collapse);
        self.clear();
        self.panel.add_css_class("out");
        self.panel.set_visible(false);
        self.strip.set_visible(true);
    }

    fn destroy(&self) {
        self.collapse_now();
        self.window.destroy();
    }

    fn close_tips(&self) {
        for tip in self.tips.borrow().iter() {
            tip.popdown();
        }
    }

    fn clear(&self) {
        for tip in self.tips.take() {
            tip.popdown();
            tip.unparent();
        }
        while let Some(child) = self.panel.first_child() {
            self.panel.remove(&child);
        }
    }

    fn fill(self: &Rc<Self>) {
        self.clear();
        let content = (self.hooks.content)();
        for action in Action::ALL {
            let button = self.button(action, &content);
            self.panel.append(&button);
        }
    }

    fn button(self: &Rc<Self>, action: Action, content: &Content) -> gtk::Button {
        let title = action.title(content.recording);
        let support = action.availability(content);
        let button = gtk::Button::builder()
            .icon_name(action.icon(content.recording))
            .focus_on_click(false)
            .css_classes(["dock-button"])
            .build();
        if action == Action::Video && content.recording {
            button.add_css_class("recording");
        }
        let detail = match support {
            Availability::Available => content
                .shortcuts
                .iter()
                .find(|(bound, _)| *bound == action)
                .map(|(_, trigger)| Detail::Shortcut(trigger.clone())),
            Availability::Unavailable(reason) => {
                button.add_css_class("unavailable");
                Some(Detail::Reason(reason))
            }
        };
        let label = match &detail {
            Some(Detail::Reason(reason)) => format!("{title}, {reason}"),
            _ => title.to_owned(),
        };
        button.update_property(&[gtk::accessible::Property::Label(&label)]);

        let tip = self.tip(title, detail.as_ref());
        tip.set_parent(&button);
        let motion = gtk::EventControllerMotion::new();
        let shown = tip.downgrade();
        motion.connect_enter(move |_, _, _| {
            if let Some(tip) = shown.upgrade() {
                tip.add_css_class("out");
                tip.popup();
                after_first_frame(&tip, |tip| tip.remove_css_class("out"));
            }
        });
        let hidden = tip.downgrade();
        motion.connect_leave(move |_| {
            if let Some(tip) = hidden.upgrade() {
                tip.popdown();
            }
        });
        button.add_controller(motion);
        self.tips.borrow_mut().push(tip);

        let weak: Weak<Self> = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            let Some(surface) = weak.upgrade() else {
                return;
            };
            if !support.is_available() {
                return;
            }
            surface.collapse_now();
            (surface.hooks.on_pick)(action);
        });
        button
    }

    fn tip(&self, title: &str, detail: Option<&Detail>) -> gtk::Popover {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();
        row.append(
            &gtk::Label::builder()
                .label(title)
                .css_classes(["dock-tip-title"])
                .build(),
        );
        match detail {
            Some(Detail::Shortcut(trigger)) => row.append(&keycaps(trigger)),
            Some(Detail::Reason(reason)) => row.append(
                &gtk::Label::builder()
                    .label(*reason)
                    .css_classes(["dock-tip-reason"])
                    .build(),
            ),
            None => {}
        }
        let left = self.corner.is_left();
        let tip = gtk::Popover::builder()
            .autohide(false)
            .has_arrow(false)
            .can_focus(false)
            .position(if left {
                gtk::PositionType::Right
            } else {
                gtk::PositionType::Left
            })
            .css_classes(["dock-tip"])
            .child(&row)
            .build();
        tip.set_offset(if left { TIP_GAP } else { -TIP_GAP }, 0);
        tip
    }
}

enum Detail {
    Shortcut(String),
    Reason(&'static str),
}

fn corner_class(corner: Corner) -> &'static str {
    match corner {
        Corner::TopLeft => "top-left",
        Corner::TopRight => "top-right",
        Corner::BottomLeft => "bottom-left",
        Corner::BottomRight => "bottom-right",
    }
}

fn cancel(slot: &RefCell<Option<glib::SourceId>>) {
    if let Some(id) = slot.take() {
        id.remove();
    }
}

fn after_first_frame<W: IsA<gtk::Widget>>(widget: &W, then: impl Fn(&W) + 'static) {
    let painted = Cell::new(false);
    widget.add_tick_callback(move |widget, _| {
        if !painted.replace(true) {
            return glib::ControlFlow::Continue;
        }
        then(widget);
        glib::ControlFlow::Break
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(x: i32, y: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width: 1920,
            height: 1080,
        }
    }

    #[test]
    fn skips_a_corner_on_an_edge_shared_with_another_monitor() {
        let monitors = [screen(0, 0), screen(1920, 0)];
        assert!(!on_outer_edge(&monitors, 0, Corner::TopRight));
        assert!(on_outer_edge(&monitors, 0, Corner::TopLeft));
        assert!(!on_outer_edge(&monitors, 1, Corner::BottomLeft));
        assert!(on_outer_edge(&monitors, 1, Corner::BottomRight));
    }

    #[test]
    fn keeps_a_corner_when_the_neighbour_does_not_reach_the_trigger_zone() {
        let tall = screen(0, 0);
        let lower = LogicalRect {
            x: 1920,
            y: 600,
            width: 1280,
            height: 1024,
        };
        let monitors = [tall, lower];
        assert!(on_outer_edge(&monitors, 0, Corner::TopRight));
        assert!(!on_outer_edge(&monitors, 0, Corner::BottomRight));
    }

    #[test]
    fn skips_a_corner_where_a_stacked_monitor_continues_the_edge() {
        let monitors = [screen(0, 0), screen(0, 1080)];
        assert!(!on_outer_edge(&monitors, 0, Corner::BottomLeft));
        assert!(!on_outer_edge(&monitors, 0, Corner::BottomRight));
        assert!(on_outer_edge(&monitors, 0, Corner::TopLeft));
        assert!(!on_outer_edge(&monitors, 1, Corner::TopRight));
        assert!(on_outer_edge(&monitors, 1, Corner::BottomRight));
        let offset = [screen(0, 0), screen(1920, 1080)];
        assert!(on_outer_edge(&offset, 0, Corner::BottomLeft));
        assert!(on_outer_edge(&offset, 0, Corner::BottomRight));
    }

    #[test]
    fn treats_a_single_monitor_as_all_outer_edges() {
        let monitors = [screen(0, 0)];
        for corner in Corner::ALL {
            assert!(on_outer_edge(&monitors, 0, corner));
        }
        assert!(!on_outer_edge(&monitors, 3, Corner::TopLeft));
    }

    #[test]
    fn swaps_the_record_button_for_stop_while_recording() {
        assert_eq!(Action::Video.title(true), "Stop recording");
        assert_eq!(Action::Video.icon(true), "kiekje-stop-symbolic");
        let content = Content {
            shortcuts: Vec::new(),
            text: Availability::Available,
            recording: true,
        };
        assert_eq!(
            Action::Gif.availability(&content),
            Availability::Unavailable(RECORDING)
        );
        assert_eq!(
            Action::Video.availability(&content),
            Availability::Available
        );
    }
}
