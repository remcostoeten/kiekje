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
use crate::config::{self, Notifications};
use crate::notify::{FdoNotifier, Notice};
use crate::output::{self, Image};
use crate::overlay::{self, Outcome, Selection};
use crate::shortcuts::{
    PortalGuard, PortalShortcuts, ShortcutAction, ShortcutRegistrar, display_trigger, hyprland,
};
use crate::storage::{self, Naming, last_region};
use crate::tray::popover::{self, Pick};
use crate::tray::{SniTray, TrayCommand, TrayGuard, TrayHost};

pub const APP_ID: &str = "dev.remcostoeten.KiekjeNext";
const TRAY_MENU_CLOSE: Duration = Duration::from_millis(150);

struct Session {
    busy: Cell<bool>,
    last_region: Cell<Option<LogicalRect>>,
    clipboard: Option<Clipboard>,
    notifier: Option<FdoNotifier>,
    notifications: Cell<Notifications>,
    shortcuts: RefCell<Option<PortalGuard>>,
    tray: RefCell<Option<Box<dyn TrayGuard>>>,
    popover: RefCell<Option<gtk::Window>>,
    shortcut_labels: RefCell<Vec<(CaptureMode, String)>>,
}

pub enum Command {
    Daemon,
    Quit,
    Menu,
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
        notifier: app.dbus_connection().map(FdoNotifier::start),
        notifications: Cell::new(Notifications::default()),
        shortcuts: RefCell::new(None),
        tray: RefCell::new(None),
        popover: RefCell::new(None),
        shortcut_labels: RefCell::new(Vec::new()),
    });
    install_actions(&app, &session);
    register_shortcuts(&app, &session);
    start_tray(&app, &session);
    write_default_config();
    let hold = app.hold();
    let initial = Cell::new(Some(command));
    app.connect_activate(move |app| match initial.take() {
        Some(Command::Capture(request)) => start_capture(app, request, &session),
        Some(Command::Menu) => toggle_menu(app, &session, None),
        Some(Command::Daemon | Command::Quit) | None => {
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
        Command::Menu => app.activate_action("menu", None),
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
        start_capture(&app, Request::saved(action_mode(action)), &session);
    };
    let session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        match registrar.bind(on_activated).await {
            Ok((guard, bindings)) => {
                let on_hyprland = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some();
                let mut unbound = false;
                let mut labels = Vec::new();
                for binding in bindings {
                    let mode = action_mode(binding.action);
                    match binding.trigger {
                        Some(trigger) => {
                            println!(
                                "kiekje: shortcut {} bound to {trigger} through {}",
                                binding.action.id(),
                                registrar.name()
                            );
                            labels.push((mode, trigger));
                        }
                        None if on_hyprland => {
                            unbound = true;
                            labels
                                .push((mode, display_trigger(binding.action.preferred_trigger())));
                        }
                        None => println!(
                            "kiekje: shortcut {} registered through {} without a trigger; assign one in the desktop settings",
                            binding.action.id(),
                            registrar.name()
                        ),
                    }
                }
                if unbound {
                    install_hyprland_binds();
                }
                if let Some(session) = session.upgrade() {
                    session.shortcut_labels.replace(labels);
                    session.shortcuts.replace(Some(guard));
                }
            }
            Err(error) => eprintln!("kiekje: global shortcuts are off: {error}"),
        }
    });
}

fn install_hyprland_binds() {
    glib::spawn_future_local(async {
        let include = hyprland::include_path();
        let target = include.clone();
        let written = gio::spawn_blocking(move || {
            let changed = hyprland::write_include(&target, APP_ID)?;
            let sourced =
                hyprland::is_sourced(&hyprland::main_config_path(), &target, &glib::home_dir());
            Ok::<_, std::io::Error>((changed, sourced))
        })
        .await;
        match written {
            Ok(Ok((changed, sourced))) => {
                if changed {
                    println!("kiekje: wrote Hyprland binds to {}", include.display());
                }
                if !sourced {
                    println!(
                        "kiekje: shortcuts have no keys yet; add `{}` to {}",
                        hyprland::source_line(&include),
                        hyprland::main_config_path().display()
                    );
                }
            }
            Ok(Err(error)) => eprintln!(
                "kiekje: could not write Hyprland binds to {}: {error}",
                include.display()
            ),
            Err(_) => eprintln!("kiekje: the Hyprland bind worker stopped"),
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

fn action_mode(action: ShortcutAction) -> CaptureMode {
    match action {
        ShortcutAction::CaptureRegion => CaptureMode::Region,
        ShortcutAction::CaptureFullscreen => CaptureMode::Fullscreen,
    }
}

fn handle_tray_command(app: &gtk::Application, session: &Rc<Session>, command: TrayCommand) {
    match command {
        TrayCommand::Capture { mode } => {
            println!("kiekje: tray capture-{}", mode.as_str());
            capture_after_menu(app, session, mode);
        }
        TrayCommand::OpenMenu { x, y } => toggle_menu(app, session, Some((x, y))),
        TrayCommand::OpenCaptures => open_captures(),
        TrayCommand::Quit => app.activate_action("quit", None),
    }
}

fn capture_after_menu(app: &gtk::Application, session: &Rc<Session>, mode: CaptureMode) {
    let app = app.clone();
    let session = session.clone();
    glib::timeout_add_local_once(TRAY_MENU_CLOSE, move || {
        start_capture(&app, Request::saved(mode), &session)
    });
}

fn toggle_menu(app: &gtk::Application, session: &Rc<Session>, click: Option<(i32, i32)>) {
    if let Some(open) = session.popover.take() {
        open.close();
        return;
    }
    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let captures_dir = gio::spawn_blocking(|| config::load().storage.captures_dir())
            .await
            .unwrap_or_else(|_| storage::default_captures_dir());
        let content = popover::Content {
            shortcuts: session.shortcut_labels.borrow().clone(),
            captures_dir,
        };
        let pick_app = app.downgrade();
        let pick_session = Rc::downgrade(&session);
        let window = popover::show(&app, click, content, move |pick| {
            let (Some(app), Some(session)) = (pick_app.upgrade(), pick_session.upgrade()) else {
                return;
            };
            match pick {
                Pick::Capture(mode) => {
                    println!("kiekje: menu capture-{}", mode.as_str());
                    capture_after_menu(&app, &session, mode);
                }
                Pick::OpenCaptures => open_captures(),
                Pick::Quit => app.activate_action("quit", None),
            }
        });
        let closed_session = Rc::downgrade(&session);
        window.connect_close_request(move |closed| {
            if let Some(session) = closed_session.upgrade() {
                let current = session.popover.borrow().clone();
                if current.as_ref() == Some(closed) {
                    session.popover.take();
                }
            }
            glib::Propagation::Proceed
        });
        session.popover.replace(Some(window));
    });
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

    let menu = gio::SimpleAction::new("menu", None);
    let menu_app = app.downgrade();
    let menu_session = Rc::downgrade(session);
    menu.connect_activate(move |_, _| {
        if let (Some(app), Some(session)) = (menu_app.upgrade(), menu_session.upgrade()) {
            toggle_menu(&app, &session, None);
        }
    });
    app.add_action(&menu);
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
        let notifications = config::load().notifications;
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
        let _ = sender.send_blocking((result, notifications, glib::monotonic_time()));
    });

    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let Ok((result, notifications, captured_us)) = receiver.recv().await else {
            session.busy.set(false);
            return;
        };
        session.notifications.set(notifications);
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
                notify(
                    &session,
                    Notice::CaptureFailed {
                        reason: error.to_string(),
                    },
                );
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
            notify(
                &session,
                Notice::CaptureFailed {
                    reason: CaptureError::NoLastRegion.to_string(),
                },
            );
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
                    Outcome::Selected(Selection { rect, plan }) => {
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
                    Outcome::Canceled => {
                        println!("kiekje: selection canceled, nothing saved");
                        session.busy.set(false);
                    }
                    Outcome::MonitorLost(connector) => {
                        let reason = format!(
                            "monitor {connector} was disconnected during the selection, nothing was saved"
                        );
                        eprintln!("kiekje: {reason}");
                        notify(&session, Notice::CaptureFailed { reason });
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
        let reason = format!(
            "the last region {}x{}{:+}{:+} is not on any monitor now",
            region.width, region.height, region.x, region.y,
        );
        eprintln!("kiekje: capture failed: {reason}");
        notify(&session, Notice::CaptureFailed { reason });
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
    let mut copied = false;
    let render: Render = match clipboard {
        Some(_) => render,
        None => {
            let Some(image) = render() else {
                finish_delivery(&session, empty_notice());
                return;
            };
            copied = copy_with_gtk(&image);
            if !request.save {
                let notice = if copied {
                    Notice::Copied {
                        width: image.width,
                        height: image.height,
                    }
                } else {
                    Notice::CaptureFailed {
                        reason: "no display to put the image on the clipboard".into(),
                    }
                };
                finish_delivery(&session, notice);
                return;
            }
            Box::new(move || Some(image))
        }
    };

    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let notice = match render() {
            Some(image) => store(image, request, clipboard.as_ref(), copied),
            None => empty_notice(),
        };
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking(notice);
    });
    glib::spawn_future_local(async move {
        match receiver.recv().await {
            Ok(notice) => finish_delivery(&session, notice),
            Err(_) => {
                eprintln!("kiekje: delivery worker stopped");
                session.busy.set(false);
            }
        }
    });
}

fn empty_notice() -> Notice {
    eprintln!("kiekje: nothing to deliver");
    Notice::CaptureFailed {
        reason: "the capture had no image to deliver".into(),
    }
}

fn finish_delivery(session: &Session, notice: Notice) {
    notify(session, notice);
    session.busy.set(false);
}

fn notify(session: &Session, notice: Notice) {
    let settings = session.notifications.get();
    if !settings.enabled {
        return;
    }
    match &session.notifier {
        Some(notifier) => notifier.send(&notice, settings.sound),
        None => println!("kiekje: no session bus, notification skipped"),
    }
}

fn store(image: Image, request: Request, clipboard: Option<&Clipboard>, copied: bool) -> Notice {
    let (width, height) = (image.width, image.height);
    let png = match output::encode_png(image) {
        Ok(png) => Arc::new(png),
        Err(error) => {
            eprintln!("kiekje: png encode failed: {error}");
            return Notice::CaptureFailed {
                reason: format!("could not encode the image: {error}"),
            };
        }
    };
    let mut copied = copied;
    if let Some(clipboard) = clipboard {
        match clipboard.set_png(png.clone()) {
            Ok(()) => {
                log_timing(
                    "trigger -> clipboard offered",
                    request.trigger_us,
                    glib::monotonic_time(),
                );
                println!("kiekje: copied {width}x{height} to the clipboard");
                copied = true;
            }
            Err(error) => eprintln!("kiekje: clipboard not set: {error}"),
        }
    }
    if !request.save {
        return if copied {
            Notice::Copied { width, height }
        } else {
            Notice::CaptureFailed {
                reason: "could not put the image on the clipboard".into(),
            }
        };
    }
    let storage = config::load().storage;
    let directory = storage.captures_dir();
    let base = storage.filename.render(&Naming {
        mode: request.mode,
        stamp: storage::now(),
        width,
        height,
    });
    match storage::save_png(&png, &directory, &base) {
        Ok(path) => {
            println!("kiekje: saved {}", path.display());
            Notice::Saved {
                path,
                width,
                height,
                copied,
            }
        }
        Err(error) => {
            eprintln!("kiekje: save failed: {error}");
            Notice::SaveFailed {
                reason: format!("could not write to {}: {error}", directory.display()),
                copied,
            }
        }
    }
}

fn copy_with_gtk(image: &Image) -> bool {
    match gdk::Display::default() {
        Some(display) => {
            display.clipboard().set_texture(&image.texture());
            println!(
                "kiekje: copied {}x{} to the GTK clipboard",
                image.width, image.height
            );
            true
        }
        None => {
            eprintln!("kiekje: no display, clipboard not set");
            false
        }
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
