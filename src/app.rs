use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::backends;
use crate::capture::{BackendChoice, Capture, CaptureError, CaptureMode, LogicalRect, MonitorInfo};
use crate::clipboard::Clipboard;
use crate::output::{self, Image};
use crate::overlay;
use crate::shortcuts::{
    PortalGuard, PortalShortcuts, ShortcutAction, ShortcutRegistrar, hyprland_bind,
};

pub const APP_ID: &str = "dev.remcostoeten.KiekjeNext";

struct Session {
    busy: Cell<bool>,
    clipboard: Option<Clipboard>,
    shortcuts: RefCell<Option<PortalGuard>>,
}

pub enum Command {
    Daemon,
    Quit,
    Capture(Request),
}

#[derive(Clone, Copy)]
pub struct Request {
    pub mode: CaptureMode,
    pub backend: BackendChoice,
    pub save: bool,
    pub trigger_us: i64,
}

type RequestVariant = (String, String, bool, i64);

impl Request {
    fn to_variant(self) -> glib::Variant {
        (
            self.mode.as_str().to_owned(),
            self.backend.as_str().to_owned(),
            self.save,
            self.trigger_us,
        )
            .to_variant()
    }

    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        let (mode, backend, save, trigger_us) = variant.get::<RequestVariant>()?;
        Some(Self {
            mode: CaptureMode::parse(&mode)?,
            backend: BackendChoice::parse(&backend)?,
            save,
            trigger_us,
        })
    }
}

/// Runs the command against the single running instance, starting one when none exists.
///
/// # Examples
///
/// ```ignore
/// app::run(Command::Daemon);
/// ```
pub fn run(command: Command) -> glib::ExitCode {
    let app = gtk::Application::builder().application_id(APP_ID).build();
    if let Err(error) = app.register(gio::Cancellable::NONE) {
        eprintln!("kiekje: could not register the application: {error}");
        return glib::ExitCode::FAILURE;
    }

    if app.is_remote() {
        let code = forward(&app, command);
        return exit_without_running(&app, code);
    }

    if matches!(command, Command::Quit) {
        println!("kiekje: not running");
        return exit_without_running(&app, glib::ExitCode::SUCCESS);
    }

    let clipboard = match Clipboard::start() {
        Ok(clipboard) => Some(clipboard),
        Err(error) => {
            println!("kiekje: using the GTK clipboard: {error}");
            None
        }
    };
    let session = Rc::new(Session {
        busy: Cell::new(false),
        clipboard,
        shortcuts: RefCell::new(None),
    });
    install_actions(&app, &session);
    register_shortcuts(&app, &session);
    let hold = app.hold();
    let initial = match command {
        Command::Capture(request) => Some(request),
        Command::Daemon | Command::Quit => None,
    };
    let initial = Cell::new(initial);
    app.connect_activate(move |app| match initial.take() {
        Some(request) => start_capture(app, request, &session),
        None => {
            glib::idle_add_local_once(overlay::warm_up);
        }
    });

    let code = app.run_with_args(&["kiekje-next"]);
    drop(hold);
    code
}

fn forward(app: &gtk::Application, command: Command) -> glib::ExitCode {
    match command {
        Command::Daemon => println!("kiekje: already running"),
        Command::Quit => app.activate_action("quit", None),
        Command::Capture(request) => app.activate_action("capture", Some(&request.to_variant())),
    }
    if let Some(connection) = app.dbus_connection()
        && let Err(error) = connection.flush_sync(gio::Cancellable::NONE)
    {
        eprintln!("kiekje: could not reach the running instance: {error}");
        return glib::ExitCode::FAILURE;
    }
    glib::ExitCode::SUCCESS
}

fn exit_without_running(app: &gtk::Application, code: glib::ExitCode) -> glib::ExitCode {
    // Only g_application_run releases the D-Bus registration; dropping a registered app makes GLib warn.
    app.connect_handle_local_options(move |_, _| ControlFlow::Break(code));
    app.run_with_args(&["kiekje-next"])
}

fn register_shortcuts(app: &gtk::Application, session: &Rc<Session>) {
    let Some(connection) = app.dbus_connection() else {
        eprintln!("kiekje: no session bus, global shortcuts are off");
        return;
    };
    let registrar = PortalShortcuts::new(connection);
    let weak_app = app.downgrade();
    let weak_session = Rc::downgrade(session);
    let on_activated = move |action: ShortcutAction| {
        let (Some(app), Some(session)) = (weak_app.upgrade(), weak_session.upgrade()) else {
            return;
        };
        println!("kiekje: shortcut {}", action.id());
        let mode = match action {
            ShortcutAction::CaptureRegion => CaptureMode::Region,
            ShortcutAction::CaptureFullscreen => CaptureMode::Fullscreen,
        };
        let request = Request {
            mode,
            backend: BackendChoice::Auto,
            save: true,
            trigger_us: glib::monotonic_time(),
        };
        start_capture(&app, request, &session);
    };
    let session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        match registrar.bind(on_activated).await {
            Ok((guard, bindings)) => {
                for binding in bindings {
                    match binding.trigger {
                        Some(trigger) => println!(
                            "kiekje: shortcut {} bound to {trigger} through {}",
                            binding.action.id(),
                            registrar.name()
                        ),
                        None if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() => {
                            println!(
                                "kiekje: shortcut {} registered without a key; add to hyprland.conf: {}",
                                binding.action.id(),
                                hyprland_bind(binding.action, APP_ID)
                            )
                        }
                        None => println!(
                            "kiekje: shortcut {} registered through {} without a trigger; assign one in the desktop settings",
                            binding.action.id(),
                            registrar.name()
                        ),
                    }
                }
                if let Some(session) = session.upgrade() {
                    session.shortcuts.replace(Some(guard));
                }
            }
            Err(error) => eprintln!("kiekje: global shortcuts are off: {error}"),
        }
    });
}

fn install_actions(app: &gtk::Application, session: &Rc<Session>) {
    let capture = gio::SimpleAction::new("capture", Some(&RequestVariant::static_variant_type()));
    let capture_session = session.clone();
    let capture_app = app.downgrade();
    capture.connect_activate(move |_, parameter| {
        let (Some(app), Some(request)) = (
            capture_app.upgrade(),
            parameter.and_then(Request::from_variant),
        ) else {
            eprintln!("kiekje: ignored a malformed capture request");
            return;
        };
        start_capture(&app, request, &capture_session);
    });
    app.add_action(&capture);

    let quit = gio::SimpleAction::new("quit", None);
    let quit_app = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = quit_app.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
}

fn start_capture(app: &gtk::Application, request: Request, session: &Rc<Session>) {
    if session.busy.replace(true) {
        eprintln!("kiekje: a capture is already running, ignoring this trigger");
        return;
    }
    let received_us = glib::monotonic_time();
    let monitors = gdk_monitors();
    let infos: Vec<MonitorInfo> = monitors.iter().map(monitor_info).collect();

    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let result = backends::capture(request.backend, &infos, false);
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking((result, glib::monotonic_time()));
    });

    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let Ok((result, captured_us)) = receiver.recv().await else {
            session.busy.set(false);
            return;
        };
        log_timing("trigger -> instance", request.trigger_us, received_us);
        log_timing("instance -> frames", received_us, captured_us);
        match result {
            Ok(capture) => handle_capture(&app, request, capture, &monitors, session),
            Err(CaptureError::Canceled) => {
                println!("kiekje: capture canceled");
                session.busy.set(false);
            }
            Err(error) => {
                eprintln!("kiekje: capture failed: {error}");
                session.busy.set(false);
            }
        }
    });
}

fn handle_capture(
    app: &gtk::Application,
    request: Request,
    capture: Capture,
    monitors: &[gdk::Monitor],
    session: Rc<Session>,
) {
    println!("kiekje: backend {}", capture.backend);
    if let Some(reason) = &capture.fallback_reason {
        println!("kiekje: skipped earlier backends: {reason}");
    }
    for frame in &capture.frames {
        println!(
            "kiekje: frame {} {}x{} for logical {}x{}{:+}{:+} ({:?})",
            frame.monitor.connector,
            frame.width,
            frame.height,
            frame.monitor.logical.width,
            frame.monitor.logical.height,
            frame.monitor.logical.x,
            frame.monitor.logical.y,
            frame.format,
        );
    }

    match request.mode {
        CaptureMode::Fullscreen => {
            let frames = capture.frames;
            deliver(Box::new(move || output::stitch(&frames)), request, session);
        }
        CaptureMode::Region => {
            let frames = Arc::new(capture.frames);
            let selection_frames = frames.clone();
            let trigger_us = request.trigger_us;
            overlay::show(
                app,
                frames,
                monitors,
                move |selection| match selection {
                    Some(selection) => {
                        let rect = selection.rect;
                        println!(
                            "kiekje: selected {}x{}+{}+{} on {}",
                            rect.width,
                            rect.height,
                            rect.x,
                            rect.y,
                            selection_frames[selection.frame_index].monitor.connector,
                        );
                        deliver(
                            Box::new(move || {
                                Some(output::crop(
                                    &selection_frames[selection.frame_index],
                                    selection.rect,
                                ))
                            }),
                            request,
                            session,
                        )
                    }
                    None => {
                        println!("kiekje: selection canceled, nothing saved");
                        session.busy.set(false);
                    }
                },
                move || {
                    log_timing(
                        "trigger -> overlay painted",
                        trigger_us,
                        glib::monotonic_time(),
                    )
                },
            );
        }
    }
}

type Render = Box<dyn FnOnce() -> Option<Image> + Send>;

fn deliver(render: Render, request: Request, session: Rc<Session>) {
    let clipboard = session.clipboard.clone();
    let render: Render = match clipboard {
        Some(_) => render,
        None => {
            let Some(image) = render() else {
                eprintln!("kiekje: nothing to deliver");
                session.busy.set(false);
                return;
            };
            copy_with_gtk(&image);
            if !request.save {
                session.busy.set(false);
                return;
            }
            Box::new(move || Some(image))
        }
    };

    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        match render() {
            Some(image) => store(image, request, clipboard.as_ref()),
            None => eprintln!("kiekje: nothing to deliver"),
        }
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking(());
    });
    glib::spawn_future_local(async move {
        if receiver.recv().await.is_err() {
            eprintln!("kiekje: delivery worker stopped");
        }
        session.busy.set(false);
    });
}

fn store(image: Image, request: Request, clipboard: Option<&Clipboard>) {
    let (width, height) = (image.width, image.height);
    let png = match output::encode_png(image) {
        Ok(png) => Arc::new(png),
        Err(error) => {
            eprintln!("kiekje: png encode failed: {error}");
            return;
        }
    };
    if let Some(clipboard) = clipboard {
        match clipboard.set_png(png.clone()) {
            Ok(()) => {
                log_timing(
                    "trigger -> clipboard offered",
                    request.trigger_us,
                    glib::monotonic_time(),
                );
                println!("kiekje: copied {width}x{height} to the clipboard");
            }
            Err(error) => eprintln!("kiekje: clipboard not set: {error}"),
        }
    }
    if request.save {
        match output::save_png(&png, &output::captures_dir(), request.mode) {
            Ok(path) => println!("kiekje: saved {}", path.display()),
            Err(error) => eprintln!("kiekje: save failed: {error}"),
        }
    }
}

fn copy_with_gtk(image: &Image) {
    match gdk::Display::default() {
        Some(display) => {
            display.clipboard().set_texture(&image.texture());
            println!(
                "kiekje: copied {}x{} to the GTK clipboard",
                image.width, image.height
            );
        }
        None => eprintln!("kiekje: no display, clipboard not set"),
    }
}

pub fn gdk_monitors() -> Vec<gdk::Monitor> {
    let Some(display) = gdk::Display::default() else {
        return Vec::new();
    };
    let model = display.monitors();
    (0..model.n_items())
        .filter_map(|index| model.item(index).and_downcast::<gdk::Monitor>())
        .collect()
}

pub fn monitor_info(monitor: &gdk::Monitor) -> MonitorInfo {
    let geometry = monitor.geometry();
    MonitorInfo {
        connector: monitor
            .connector()
            .map(|name| name.to_string())
            .unwrap_or_default(),
        logical: LogicalRect {
            x: geometry.x(),
            y: geometry.y(),
            width: geometry.width(),
            height: geometry.height(),
        },
    }
}

fn log_timing(label: &str, from_us: i64, to_us: i64) {
    println!(
        "kiekje: timing {label}: {:.1} ms",
        (to_us - from_us) as f64 / 1000.0
    );
}
