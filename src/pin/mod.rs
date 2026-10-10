mod geometry;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::app::{gdk_monitors, monitor_info};
use crate::capture::{CaptureMode, LogicalRect};
use crate::config;
use crate::library::{self, Library, NewCapture};
use crate::output::Image;
use crate::overlay;
use crate::storage::{self, Naming};
use crate::ui;

const ACTIONS: &str = "pin";
const DRAG_THRESHOLD: f64 = 4.0;
const NUDGE: f64 = 10.0;
const TOAST: Duration = Duration::from_millis(1400);

thread_local! {
    static PINS: RefCell<Vec<Rc<Pin>>> = const { RefCell::new(Vec::new()) };
}

/// How a pin is kept above other windows in this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacking {
    Layer,
    Toplevel,
}

impl Stacking {
    /// Whether the compositor keeps pins above ordinary windows.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if !pin::stacking().stays_on_top() { println!("pins may fall behind"); }
    /// ```
    pub fn stays_on_top(self) -> bool {
        self == Self::Layer
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: glib::Error },
    #[error("the loading worker stopped")]
    Worker,
}

#[derive(Clone, Copy)]
enum Action {
    Copy,
    Save,
    ActualSize,
    Close,
}

impl Action {
    const ALL: [Self; 4] = [Self::Copy, Self::Save, Self::ActualSize, Self::Close];

    fn name(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Save => "save",
            Self::ActualSize => "actual-size",
            Self::Close => "close",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy image",
            Self::Save => "Save as",
            Self::ActualSize => "Actual size",
            Self::Close => "Close pin",
        }
    }
}

struct Pin {
    window: gtk::Window,
    frame: gtk::Overlay,
    picture: gtk::Picture,
    toast: gtk::Label,
    menu: gtk::PopoverMenu,
    texture: gdk::Texture,
    base: (f64, f64),
    zoom: Cell<f64>,
    stacking: Stacking,
    room: (i32, i32),
    margins: Cell<(f64, f64)>,
    toast_timer: Rc<RefCell<Option<glib::SourceId>>>,
}

/// How pins are kept on top here: a layer-shell surface where the compositor offers one.
///
/// GNOME and X11 get an undecorated window instead, which the window manager may stack
/// below others.
///
/// # Examples
///
/// ```ignore
/// println!("{:?}", pin::stacking());
/// ```
pub fn stacking() -> Stacking {
    if overlay::uses_layer_shell() {
        Stacking::Layer
    } else {
        Stacking::Toplevel
    }
}

/// Pins a freshly captured image where its selection was on the desktop.
///
/// `selection` is in desktop logical coordinates and sets the pin's size at zoom 1. Without
/// it the pin opens centered on the first monitor.
///
/// # Examples
///
/// ```ignore
/// pin::open_image(&app, image, Some(selection.rect));
/// ```
pub fn open_image(app: &gtk::Application, image: Image, selection: Option<LogicalRect>) {
    let texture = image.into_texture().upcast::<gdk::Texture>();
    let monitors = gdk_monitors();
    let rects: Vec<LogicalRect> = monitors
        .iter()
        .map(|monitor| monitor_info(monitor).logical)
        .collect();
    let placed = selection.and_then(|rect| geometry::place(rect, &rects));
    let (monitor, origin) = match placed {
        Some((index, origin)) => (monitors.get(index).cloned(), Some(origin)),
        None => (monitors.first().cloned(), None),
    };
    let base = match selection {
        Some(rect) => (f64::from(rect.width), f64::from(rect.height)),
        None => natural_size(&texture, monitor.as_ref()),
    };
    show(app, texture, base, monitor, origin);
}

/// Loads an image file off the main thread and pins it centered on the monitor of `near`.
///
/// # Examples
///
/// ```ignore
/// pin::open_file(&app, path, Some(&gallery_window)).await?;
/// ```
pub async fn open_file(
    app: &gtk::Application,
    path: PathBuf,
    near: Option<&gtk::Window>,
) -> Result<(), PinError> {
    let target = path.clone();
    let texture = gio::spawn_blocking(move || gdk::Texture::from_filename(&target))
        .await
        .map_err(|_| PinError::Worker)?
        .map_err(|source| PinError::Read { path, source })?;
    let monitor = near
        .and_then(|window| window.surface())
        .and_then(|surface| surface.display().monitor_at_surface(&surface))
        .or_else(|| gdk_monitors().into_iter().next());
    let base = natural_size(&texture, monitor.as_ref());
    show(app, texture, base, monitor, None);
    Ok(())
}

/// Closes every open pin.
///
/// # Examples
///
/// ```ignore
/// pin::close_all();
/// ```
pub fn close_all() {
    let pins = PINS.with(|pins| std::mem::take(&mut *pins.borrow_mut()));
    for pin in pins {
        pin.window.close();
    }
}

fn natural_size(texture: &gdk::Texture, monitor: Option<&gdk::Monitor>) -> (f64, f64) {
    let scale = monitor
        .map(|monitor| monitor.scale())
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    (
        f64::from(texture.width()) / scale,
        f64::from(texture.height()) / scale,
    )
}

fn show(
    app: &gtk::Application,
    texture: gdk::Texture,
    base: (f64, f64),
    monitor: Option<gdk::Monitor>,
    origin: Option<(i32, i32)>,
) {
    ui::load_stylesheet();
    let room = monitor
        .as_ref()
        .map(|monitor| {
            let logical = monitor_info(monitor).logical;
            (logical.width, logical.height)
        })
        .unwrap_or((1920, 1080));
    let zoom = match origin {
        Some(_) => 1.0,
        None => geometry::fit_zoom(base, room),
    };
    let size = geometry::scaled(base, zoom);

    let window = gtk::Window::builder()
        .application(app)
        .decorated(false)
        .resizable(false)
        .title("Kiekje pin")
        .css_classes(["kiekje-pin"])
        .build();
    let stacking = stacking();
    let margins = origin.unwrap_or_else(|| geometry::centered(size, room));
    if stacking == Stacking::Layer {
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace(Some("kiekje-pin"));
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        window.set_exclusive_zone(-1);
        if let Some(monitor) = &monitor {
            window.set_monitor(Some(monitor));
        }
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Top, true);
    }

    let picture = gtk::Picture::for_paintable(&texture);
    picture.set_content_fit(gtk::ContentFit::Fill);
    picture.set_can_shrink(true);
    picture.set_size_request(size.0, size.1);
    let toast = gtk::Label::builder()
        .css_classes(["pin-toast"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::End)
        .visible(false)
        .can_target(false)
        .build();
    let frame = gtk::Overlay::builder()
        .css_classes(["pin"])
        .focusable(true)
        .build();
    frame.set_cursor_from_name(Some("grab"));
    frame.set_child(Some(&picture));
    frame.add_overlay(&toast);
    frame.update_property(&[gtk::accessible::Property::Label(
        "Pinned capture. Drag to move, scroll or plus and minus to zoom, Escape to close.",
    )]);
    window.set_child(Some(&frame));
    let menu = menu(&frame);

    let pin = Rc::new(Pin {
        window: window.clone(),
        frame: frame.clone(),
        picture,
        toast,
        menu,
        texture,
        base,
        zoom: Cell::new(zoom),
        stacking,
        room,
        margins: Cell::new((f64::from(margins.0), f64::from(margins.1))),
        toast_timer: Rc::new(RefCell::new(None)),
    });
    pin.apply_margins();
    install_actions(&pin);
    connect_pointer(&pin);
    connect_keys(&pin);

    window.connect_close_request(|closed| {
        PINS.with(|pins| pins.borrow_mut().retain(|pin| pin.window != *closed));
        glib::Propagation::Proceed
    });
    if stacking == Stacking::Toplevel {
        pin.say("Not kept on top on this desktop");
    }
    PINS.with(|pins| pins.borrow_mut().push(pin));
    window.present();
    frame.grab_focus();
}

fn menu(frame: &gtk::Overlay) -> gtk::PopoverMenu {
    let model = gio::Menu::new();
    let keep = gio::Menu::new();
    let view = gio::Menu::new();
    let end = gio::Menu::new();
    for action in Action::ALL {
        let section = match action {
            Action::Copy | Action::Save => &keep,
            Action::ActualSize => &view,
            Action::Close => &end,
        };
        section.append(
            Some(action.label()),
            Some(&format!("{ACTIONS}.{}", action.name())),
        );
    }
    model.append_section(None, &keep);
    model.append_section(None, &view);
    model.append_section(None, &end);
    let popover = gtk::PopoverMenu::from_model(Some(&model));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(frame);
    let owned = popover.downgrade();
    frame.connect_destroy(move |_| {
        if let Some(popover) = owned.upgrade() {
            popover.unparent();
        }
    });
    popover
}

fn install_actions(pin: &Rc<Pin>) {
    let group = gio::SimpleActionGroup::new();
    for action in Action::ALL {
        let simple = gio::SimpleAction::new(action.name(), None);
        let target = Rc::downgrade(pin);
        simple.connect_activate(move |_, _| {
            if let Some(pin) = target.upgrade() {
                pin.run(action);
            }
        });
        group.add_action(&simple);
    }
    pin.window.insert_action_group(ACTIONS, Some(&group));
}

fn connect_pointer(pin: &Rc<Pin>) {
    let drag = gtk::GestureDrag::new();
    drag.set_button(gdk::BUTTON_PRIMARY);
    let moving = Rc::downgrade(pin);
    let handed_off = Rc::new(Cell::new(false));
    let begin_handed_off = handed_off.clone();
    drag.connect_drag_begin(move |_, _, _| begin_handed_off.set(false));
    drag.connect_drag_update(move |gesture, dx, dy| {
        let Some(pin) = moving.upgrade() else {
            return;
        };
        match pin.stacking {
            Stacking::Layer => {
                let size = pin.size();
                pin.margins.set(geometry::dragged(
                    pin.margins.get(),
                    (dx, dy),
                    size,
                    pin.room,
                ));
                pin.apply_margins();
            }
            Stacking::Toplevel => {
                if handed_off.get() || dx.hypot(dy) < DRAG_THRESHOLD {
                    return;
                }
                handed_off.set(true);
                pin.begin_move(gesture);
            }
        }
    });
    pin.frame.add_controller(drag);

    let primary = gtk::GestureClick::new();
    primary.set_button(gdk::BUTTON_PRIMARY);
    let clicked = Rc::downgrade(pin);
    primary.connect_pressed(move |_, presses, _, _| {
        if let Some(pin) = clicked.upgrade() {
            pin.frame.grab_focus();
            if presses == 2 {
                pin.window.close();
            }
        }
    });
    pin.frame.add_controller(primary);

    let secondary = gtk::GestureClick::new();
    secondary.set_button(gdk::BUTTON_SECONDARY);
    let menu = pin.menu.downgrade();
    secondary.connect_pressed(move |gesture, _, x, y| {
        if let Some(menu) = menu.upgrade() {
            show_menu(&menu, x, y);
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    pin.frame.add_controller(secondary);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let scrolled = Rc::downgrade(pin);
    scroll.connect_scroll(move |_, _, dy| {
        if let Some(pin) = scrolled.upgrade() {
            pin.zoom_to(geometry::zoom_by(pin.zoom.get(), -dy));
        }
        glib::Propagation::Stop
    });
    pin.frame.add_controller(scroll);
}

fn connect_keys(pin: &Rc<Pin>) {
    let keys = gtk::EventControllerKey::new();
    let pressed = Rc::downgrade(pin);
    keys.connect_key_pressed(move |_, key, _, state| {
        let Some(pin) = pressed.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let control = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let name = key.to_lower().name().unwrap_or_default();
        match (control, name.as_str()) {
            (false, "Escape") => pin.window.close(),
            (true, "c") => pin.run(Action::Copy),
            (true, "s") => pin.run(Action::Save),
            (_, "plus" | "equal" | "KP_Add") => {
                pin.zoom_to(geometry::zoom_by(pin.zoom.get(), 1.0));
            }
            (_, "minus" | "KP_Subtract") => {
                pin.zoom_to(geometry::zoom_by(pin.zoom.get(), -1.0));
            }
            (_, "0" | "KP_0") => pin.run(Action::ActualSize),
            (false, "Menu") => pin.popup_centered(),
            (false, "F10") if shift => pin.popup_centered(),
            (false, "Left") => pin.nudge(-NUDGE, 0.0),
            (false, "Right") => pin.nudge(NUDGE, 0.0),
            (false, "Up") => pin.nudge(0.0, -NUDGE),
            (false, "Down") => pin.nudge(0.0, NUDGE),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    pin.window.add_controller(keys);
}

fn show_menu(menu: &gtk::PopoverMenu, x: f64, y: f64) {
    menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    menu.popup();
}

impl Pin {
    fn size(&self) -> (i32, i32) {
        geometry::scaled(self.base, self.zoom.get())
    }

    fn popup_centered(&self) {
        let (width, height) = self.size();
        show_menu(&self.menu, f64::from(width) / 2.0, f64::from(height) / 2.0);
    }

    fn apply_margins(&self) {
        if self.stacking != Stacking::Layer {
            return;
        }
        let (left, top) = self.margins.get();
        self.window.set_margin(Edge::Left, left.round() as i32);
        self.window.set_margin(Edge::Top, top.round() as i32);
    }

    fn nudge(&self, dx: f64, dy: f64) {
        if self.stacking != Stacking::Layer {
            return;
        }
        self.margins.set(geometry::dragged(
            self.margins.get(),
            (dx, dy),
            self.size(),
            self.room,
        ));
        self.apply_margins();
    }

    fn zoom_to(&self, zoom: f64) {
        let before = self.size();
        self.zoom.set(geometry::clamp_zoom(zoom));
        let after = self.size();
        if before == after {
            return;
        }
        self.picture.set_size_request(after.0, after.1);
        self.window.set_default_size(after.0, after.1);
        let moved = geometry::recentered(self.margins.get(), before, after);
        self.margins.set(geometry::contain(moved, after, self.room));
        self.apply_margins();
        self.say(&format!("{:.0}%", self.zoom.get() * 100.0));
    }

    fn begin_move(&self, gesture: &gtk::GestureDrag) {
        let Some(toplevel) = self.window.surface().and_downcast::<gdk::Toplevel>() else {
            return;
        };
        let (Some(device), Some((x, y))) = (gesture.current_event_device(), gesture.start_point())
        else {
            return;
        };
        let point = self
            .frame
            .compute_point(&self.window, &gtk::graphene::Point::new(x as f32, y as f32))
            .map_or((x, y), |point| (f64::from(point.x()), f64::from(point.y())));
        toplevel.begin_move(
            &device,
            gesture.current_button() as i32,
            point.0,
            point.1,
            gesture.current_event_time(),
        );
        gesture.reset();
    }

    fn run(self: &Rc<Self>, action: Action) {
        match action {
            Action::Copy => {
                self.window.clipboard().set_texture(&self.texture);
                self.say("Copied");
            }
            Action::Save => self.save_as(),
            Action::ActualSize => self.zoom_to(1.0),
            Action::Close => self.window.close(),
        }
    }

    fn save_as(self: &Rc<Self>) {
        let pin = self.clone();
        let (width, height) = (
            u32::try_from(self.texture.width()).unwrap_or(0),
            u32::try_from(self.texture.height()).unwrap_or(0),
        );
        glib::spawn_future_local(async move {
            let suggested = gio::spawn_blocking(move || {
                let storage = config::load().storage;
                let base = storage.filename.render(&Naming {
                    mode: CaptureMode::Region,
                    stamp: storage::now(),
                    width,
                    height,
                });
                (storage.captures_dir(), base)
            })
            .await;
            let Ok((folder, base)) = suggested else {
                pin.say("Not saved: the settings worker stopped");
                return;
            };
            let dialog = gtk::FileDialog::builder()
                .title("Save the pinned capture")
                .modal(true)
                .initial_name(format!("{base}.png"))
                .initial_folder(&gio::File::for_path(&folder))
                .build();
            let parent = (pin.stacking == Stacking::Toplevel).then(|| pin.window.clone());
            let path = match dialog.save_future(parent.as_ref()).await {
                Ok(file) => file.path(),
                Err(error) if error.matches(gtk::DialogError::Dismissed) => return,
                Err(error) => {
                    eprintln!("kiekje: the save dialog failed: {error}");
                    pin.say("Not saved");
                    return;
                }
            };
            let Some(path) = path else {
                pin.say("Not saved: pick a local folder");
                return;
            };
            let texture = pin.texture.clone();
            let written = gio::spawn_blocking(move || write(&texture, &path)).await;
            match written {
                Ok(Ok(path)) => {
                    println!("kiekje: saved the pin to {}", path.display());
                    pin.say("Saved");
                }
                Ok(Err(error)) => {
                    eprintln!("kiekje: the pin was not saved: {error}");
                    pin.say("Not saved");
                }
                Err(_) => pin.say("Not saved: the save worker stopped"),
            }
        });
    }

    fn say(&self, message: &str) {
        self.toast.set_label(message);
        self.toast.set_visible(true);
        if let Some(timer) = self.toast_timer.take() {
            timer.remove();
        }
        let toast = self.toast.downgrade();
        let slot = Rc::downgrade(&self.toast_timer);
        let timer = glib::timeout_add_local_once(TOAST, move || {
            if let Some(slot) = slot.upgrade() {
                slot.take();
            }
            if let Some(toast) = toast.upgrade() {
                toast.set_visible(false);
            }
        });
        self.toast_timer.replace(Some(timer));
    }
}

fn write(texture: &gdk::Texture, path: &Path) -> std::io::Result<PathBuf> {
    let png = texture.save_to_png_bytes();
    let saved = storage::save_png_as(&png, path)?;
    let capture = NewCapture {
        path: saved.clone(),
        mode: CaptureMode::Region,
        width: u32::try_from(texture.width()).unwrap_or(0),
        height: u32::try_from(texture.height()).unwrap_or(0),
        bytes: u64::try_from(png.len()).unwrap_or(u64::MAX),
        created_us: glib::real_time(),
    };
    if let Err(error) = Library::open(&library::path()).and_then(|library| library.insert(&capture))
    {
        eprintln!("kiekje: the pin is saved but not in the library: {error}");
    }
    Ok(saved)
}
