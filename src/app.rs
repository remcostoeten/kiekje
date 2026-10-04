use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::backends;
use crate::capture::mapping::{self, SelectionPlan};
use crate::capture::{
    BackendChoice, Capture, CaptureError, CaptureMode, Frame, LogicalRect, MonitorInfo,
};
use crate::clipboard::Clipboard;
use crate::config;
use crate::output::{self, Image};
use crate::overlay::{self, Selection};
use crate::shortcuts::{
    PortalGuard, PortalShortcuts, ShortcutAction, ShortcutRegistrar, hyprland_bind,
};
use crate::storage::{self, Naming, last_region};
use crate::tray::{SniTray, TrayCommand, TrayGuard, TrayHost, TrayOrigin};

pub const APP_ID: &str = "dev.remcostoeten.KiekjeNext";
const TRAY_MENU_CLOSE: Duration = Duration::from_millis(150);

struct Session {
    busy: Cell<bool>,
    last_region: Cell<Option<LogicalRect>>,
    clipboard: Option<Clipboard>,
    shortcuts: RefCell<Option<PortalGuard>>,
    tray: RefCell<Option<Box<dyn TrayGuard>>>,
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
    pub delay: Duration,
    pub trigger_us: i64,
}

type RequestVariant = (String, String, bool, u32, i64);

impl Request {
    fn saved(mode: CaptureMode) -> Self {
        Self {
            mode,
            backend: BackendChoice::Auto,
            save: true,
            delay: Duration::ZERO,
            trigger_us: glib::monotonic_time(),
        }
    }

    fn to_variant(self) -> glib::Variant {
        (
            self.mode.as_str().to_owned(),
            self.backend.as_str().to_owned(),
            self.save,
            u32::try_from(self.delay.as_millis()).unwrap_or(u32::MAX),
            self.trigger_us,
        )
            .to_variant()
    }

    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        let (mode, backend, save, delay_ms, trigger_us) = variant.get::<RequestVariant>()?;
        Some(Self {
            mode: CaptureMode::parse(&mode)?,
            backend: BackendChoice::parse(&backend)?,
            save,
            delay: Duration::from_millis(u64::from(delay_ms)),
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
        last_region: Cell::new(None),
        clipboard,
        shortcuts: RefCell::new(None),
        tray: RefCell::new(None),
    });
    install_actions(&app, &session);
    register_shortcuts(&app, &session);
    start_tray(&app, &session);
    write_default_config();
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
        start_capture(&app, Request::saved(mode), &session);
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

fn start_tray(app: &gtk::Application, session: &Rc<Session>) {
    let host = SniTray;
    let name = host.name();
    let (commands, received) = async_channel::unbounded();
    let (started_sender, started) = async_channel::bounded(1);
    thread::spawn(move || {
        // The receiver only disappears when the app is quitting.
        let _ = started_sender.send_blocking(host.start(commands));
    });

    let weak_app = app.downgrade();
    let weak_session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        match started.recv().await {
            Ok(Ok(guard)) => {
                println!("kiekje: tray shown through {name}");
                match weak_session.upgrade() {
                    Some(session) => {
                        session.tray.replace(Some(Box::new(guard)));
                    }
                    None => guard.stop(),
                }
            }
            Ok(Err(error)) => {
                eprintln!("kiekje: tray is off: {error}");
                return;
            }
            Err(_) => return,
        }
        while let Ok(command) = received.recv().await {
            let (Some(app), Some(session)) = (weak_app.upgrade(), weak_session.upgrade()) else {
                return;
            };
            handle_tray_command(&app, &session, command);
        }
    });
}

fn handle_tray_command(app: &gtk::Application, session: &Rc<Session>, command: TrayCommand) {
    match command {
        TrayCommand::Capture { mode, origin } => {
            println!("kiekje: tray capture-{}", mode.as_str());
            let request = Request::saved(mode);
            match origin {
                TrayOrigin::Icon => start_capture(app, request, session),
                TrayOrigin::Menu => {
                    let app = app.clone();
                    let session = session.clone();
                    glib::timeout_add_local_once(TRAY_MENU_CLOSE, move || {
                        start_capture(&app, request, &session)
                    });
                }
            }
        }
        TrayCommand::OpenCaptures => open_captures(),
        TrayCommand::Quit => app.activate_action("quit", None),
    }
}

fn write_default_config() {
    glib::spawn_future_local(async {
        let path = config::path();
        let target = path.clone();
        match gio::spawn_blocking(move || config::ensure_file(&target)).await {
            Ok(Ok(true)) => println!("kiekje: wrote the default config to {}", path.display()),
            Ok(Ok(false)) => {}
            Ok(Err(error)) => eprintln!("kiekje: {error}"),
            Err(_) => eprintln!("kiekje: the config worker stopped"),
        }
    });
}

fn open_captures() {
    glib::spawn_future_local(async move {
        let created = gio::spawn_blocking(|| {
            let directory = config::load().storage.captures_dir();
            std::fs::create_dir_all(&directory)
                .map(|()| directory.clone())
                .map_err(|error| (directory, error))
        })
        .await;
        let directory = match created {
            Ok(Ok(directory)) => directory,
            Ok(Err((directory, error))) => {
                eprintln!("kiekje: could not create {}: {error}", directory.display());
                return;
            }
            Err(_) => {
                eprintln!("kiekje: the folder worker stopped");
                return;
            }
        };
        let uri = gio::File::for_path(&directory).uri();
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
        {
            eprintln!("kiekje: could not open {uri}: {error}");
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
    let quit_session = Rc::downgrade(session);
    quit.connect_activate(move |_, _| {
        if let Some(guard) = quit_session
            .upgrade()
            .and_then(|session| session.tray.take())
        {
            guard.stop();
        }
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
    if request.delay.is_zero() {
        grab(app, request, session);
        return;
    }
    println!("kiekje: capturing in {:.1} s", request.delay.as_secs_f64());
    let app = app.clone();
    let session = session.clone();
    glib::timeout_add_local_once(request.delay, move || {
        let request = Request {
            trigger_us: glib::monotonic_time(),
            ..request
        };
        grab(&app, request, &session);
    });
}

fn grab(app: &gtk::Application, request: Request, session: &Rc<Session>) {
    let received_us = glib::monotonic_time();
    let monitors = gdk_monitors();
    let infos: Vec<MonitorInfo> = monitors.iter().map(monitor_info).collect();
    let known_region = session.last_region.get();

    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let result = match request.mode {
            CaptureMode::Repeat => match known_region.or_else(last_region::load) {
                Some(region) => backends::capture(request.backend, &infos, false)
                    .map(|capture| (capture, Some(region))),
                None => Err(CaptureError::NoLastRegion),
            },
            CaptureMode::Region | CaptureMode::Fullscreen => {
                backends::capture(request.backend, &infos, false).map(|capture| (capture, None))
            }
        };
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
            Ok((capture, region)) => {
                handle_capture(&app, request, capture, region, &monitors, session)
            }
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
    region: Option<LogicalRect>,
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

    match (request.mode, region) {
        (CaptureMode::Fullscreen, _) => {
            let frames = capture.frames;
            deliver(Box::new(move || output::stitch(&frames)), request, session);
        }
        (CaptureMode::Repeat, Some(region)) => {
            repeat_region(request, capture.frames, region, session)
        }
        (CaptureMode::Repeat, None) => {
            eprintln!("kiekje: capture failed: {}", CaptureError::NoLastRegion);
            session.busy.set(false);
        }
        (CaptureMode::Region, _) => {
            let frames = Arc::new(capture.frames);
            let selection_frames = frames.clone();
            let trigger_us = request.trigger_us;
            overlay::show(
                app,
                frames,
                monitors,
                move |selection| match selection {
                    Some(Selection { rect, plan }) => {
                        remember_region(&session, rect);
                        log_selection(&selection_frames, plan);
                        deliver(
                            Box::new(move || {
                                Some(output::render_selection(&selection_frames, plan))
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

fn repeat_region(request: Request, frames: Vec<Frame>, region: LogicalRect, session: Rc<Session>) {
    let geometry: Vec<(LogicalRect, (u32, u32))> = frames
        .iter()
        .map(|frame| (frame.monitor.logical, (frame.width, frame.height)))
        .collect();
    let Some(plan) = mapping::plan_selection(region, &geometry) else {
        eprintln!(
            "kiekje: capture failed: the last region {}x{}{:+}{:+} is not on any monitor now",
            region.width, region.height, region.x, region.y,
        );
        session.busy.set(false);
        return;
    };
    log_selection(&frames, plan);
    deliver(
        Box::new(move || Some(output::render_selection(&frames, plan))),
        request,
        session,
    );
}

fn remember_region(session: &Session, region: LogicalRect) {
    if session.last_region.replace(Some(region)) == Some(region) {
        return;
    }
    glib::spawn_future_local(async move {
        match gio::spawn_blocking(move || last_region::save_to(&last_region::path(), region)).await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => eprintln!("kiekje: {error}"),
            Err(_) => eprintln!("kiekje: the last region worker stopped"),
        }
    });
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
        let storage = config::load().storage;
        let base = storage.filename.render(&Naming {
            mode: request.mode,
            stamp: storage::now(),
            width,
            height,
        });
        match storage::save_png(&png, &storage.captures_dir(), &base) {
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

fn log_selection(frames: &[Frame], plan: SelectionPlan) {
    match plan {
        SelectionPlan::Crop { frame_index, rect } => println!(
            "kiekje: selected {}x{}+{}+{} on {}",
            rect.width, rect.height, rect.x, rect.y, frames[frame_index].monitor.connector,
        ),
        SelectionPlan::Compose {
            bounds,
            width,
            height,
            ..
        } => println!(
            "kiekje: selected {width}x{height} across monitors from logical {}x{}{:+}{:+}",
            bounds.width, bounds.height, bounds.x, bounds.y,
        ),
    }
}

fn log_timing(label: &str, from_us: i64, to_us: i64) {
    println!(
        "kiekje: timing {label}: {:.1} ms",
        (to_us - from_us) as f64 / 1000.0
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_a_request_through_its_variant() {
        let request = Request {
            mode: CaptureMode::Repeat,
            backend: BackendChoice::Wlroots,
            save: false,
            delay: Duration::from_secs(3),
            trigger_us: 42,
        };
        let decoded = Request::from_variant(&request.to_variant()).unwrap();
        assert_eq!(decoded.mode, CaptureMode::Repeat);
        assert_eq!(decoded.backend, BackendChoice::Wlroots);
        assert!(!decoded.save);
        assert_eq!(decoded.delay, Duration::from_secs(3));
        assert_eq!(decoded.trigger_us, 42);
    }

    #[test]
    fn rejects_a_variant_from_an_older_build() {
        let old = ("region".to_owned(), "auto".to_owned(), true, 7_i64).to_variant();
        assert!(Request::from_variant(&old).is_none());
    }
}
