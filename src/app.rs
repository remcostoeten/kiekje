use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::backends;
use crate::backends::windows::WindowError;
use crate::capture::mapping::{self, SelectionPlan};
use crate::capture::{
    BackendChoice, Capture, CaptureError, CaptureMode, Frame, LogicalRect, MonitorInfo,
};
use crate::clipboard::Clipboard;
use crate::config::{self, AfterSelection, Notifications};
use crate::editor::{self, Finish};
use crate::library::{self, Library, NewCapture, thumbs};
use crate::notify::{FdoNotifier, Notice};
use crate::output::{self, Image};
use crate::overlay::{self, Outcome, Selection};
use crate::pin;
use crate::shortcuts::{
    Binding, PortalGuard, PortalShortcuts, ShortcutAction, ShortcutHost, ShortcutRegistrar,
    Shortcuts, hyprland, kde,
};
use crate::storage::{self, Naming, last_region};
use crate::tray::popover::{self, Pick};
use crate::tray::{SniTray, TrayCommand, TrayGuard, TrayHost};
use crate::ui::{fonts, gallery, settings};

pub const APP_ID: &str = "dev.remcostoeten.Kiekje";
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
    settings: RefCell<Option<gtk::Window>>,
    gallery: RefCell<Option<gtk::Window>>,
    gallery_handle: RefCell<Option<gallery::Handle>>,
    settings_handle: RefCell<Option<settings::Handle>>,
    shortcut_host: RefCell<ShortcutHost>,
    shortcut_labels: RefCell<Vec<(CaptureMode, String)>>,
}

pub enum Command {
    Daemon,
    Quit,
    Menu,
    Settings,
    Gallery,
    Capture(Request),
    Pin(std::path::PathBuf),
}

#[derive(Clone, Copy)]
pub struct Request {
    pub mode: CaptureMode,
    pub backend: BackendChoice,
    pub save: bool,
    pub edit: bool,
    pub delay: Duration,
    pub trigger_us: i64,
}

type RequestVariant = (String, String, bool, bool, u32, i64);

impl Request {
    fn saved(mode: CaptureMode) -> Self {
        Self {
            mode,
            backend: BackendChoice::Auto,
            save: true,
            edit: false,
            delay: Duration::ZERO,
            trigger_us: glib::monotonic_time(),
        }
    }

    fn annotated() -> Self {
        Self {
            edit: true,
            ..Self::saved(CaptureMode::Region)
        }
    }

    fn to_variant(self) -> glib::Variant {
        (
            self.mode.as_str().to_owned(),
            self.backend.as_str().to_owned(),
            self.save,
            self.edit,
            u32::try_from(self.delay.as_millis()).unwrap_or(u32::MAX),
            self.trigger_us,
        )
            .to_variant()
    }

    fn from_variant(variant: &glib::Variant) -> Option<Self> {
        let (mode, backend, save, edit, delay_ms, trigger_us) = variant.get::<RequestVariant>()?;
        Some(Self {
            mode: CaptureMode::parse(&mode)?,
            backend: BackendChoice::parse(&backend)?,
            save,
            edit,
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

    for error in fonts::register() {
        eprintln!("kiekje: {error}, using the system font");
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
        settings: RefCell::new(None),
        gallery: RefCell::new(None),
        gallery_handle: RefCell::new(None),
        settings_handle: RefCell::new(None),
        shortcut_host: RefCell::new(ShortcutHost::Starting),
        shortcut_labels: RefCell::new(Vec::new()),
    });
    install_actions(&app, &session);
    register_shortcuts(&app, &session);
    start_tray(&app, &session);
    write_default_config();
    check_library();
    let hold = app.hold();
    let initial = Cell::new(Some(command));
    app.connect_activate(move |app| match initial.take() {
        Some(Command::Capture(request)) => start_capture(app, request, &session),
        Some(Command::Menu) => toggle_menu(app, &session, None),
        Some(Command::Settings) => open_settings(app, &session),
        Some(Command::Gallery) => open_gallery(app, &session),
        Some(Command::Pin(path)) => pin_file(app, path),
        Some(Command::Daemon | Command::Quit) | None => {
            glib::idle_add_local_once(overlay::warm_up);
        }
    });

    let code = app.run_with_args(&[env!("CARGO_PKG_NAME")]);
    drop(hold);
    code
}

fn forward(app: &gtk::Application, command: Command) -> glib::ExitCode {
    match command {
        Command::Daemon => println!("kiekje: already running"),
        Command::Quit => app.activate_action("quit", None),
        Command::Menu => app.activate_action("menu", None),
        Command::Settings => app.activate_action("settings", None),
        Command::Gallery => app.activate_action("gallery", None),
        Command::Capture(request) => app.activate_action("capture", Some(&request.to_variant())),
        Command::Pin(path) => {
            let Some(path) = path.to_str() else {
                eprintln!(
                    "kiekje: {} is not valid UTF-8, so it can't be pinned",
                    path.display()
                );
                return glib::ExitCode::FAILURE;
            };
            app.activate_action("pin", Some(&path.to_variant()));
        }
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
    app.run_with_args(&[env!("CARGO_PKG_NAME")])
}

fn register_shortcuts(app: &gtk::Application, session: &Rc<Session>) {
    let Some(connection) = app.dbus_connection() else {
        eprintln!("kiekje: no session bus, global shortcuts are off");
        session
            .shortcut_host
            .replace(ShortcutHost::Off("there is no session bus".to_owned()));
        return;
    };
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
        let shortcuts = gio::spawn_blocking(|| config::load().shortcuts)
            .await
            .unwrap_or_default();
        let registrar = PortalShortcuts::new(connection.clone(), shortcuts.clone());
        match registrar.bind(on_activated).await {
            Ok((mut guard, bindings)) => {
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
                            if let Some(trigger) = shortcuts.slot(binding.action).trigger() {
                                labels.push((mode, trigger.label()));
                            }
                        }
                        None => println!(
                            "kiekje: shortcut {} registered through {} without a trigger; assign one in the desktop settings",
                            binding.action.id(),
                            registrar.name()
                        ),
                    }
                }
                if unbound {
                    install_hyprland_binds(shortcuts);
                }
                let on_kde = !on_hyprland && kde::holds(&connection, APP_ID).await;
                if let Some(session) = session.upgrade() {
                    if !on_hyprland {
                        let changed_session = Rc::downgrade(&session);
                        guard.watch_changes(move |bindings| {
                            if let Some(session) = changed_session.upgrade() {
                                desktop_changed(&session, bindings);
                            }
                        });
                    }
                    session.shortcut_host.replace(if on_hyprland {
                        ShortcutHost::Hyprland
                    } else if on_kde {
                        ShortcutHost::Kde
                    } else {
                        ShortcutHost::Desktop {
                            name: registrar.name(),
                            configurable: guard.can_configure(),
                        }
                    });
                    session.shortcut_labels.replace(labels);
                    session.shortcuts.replace(Some(guard));
                }
            }
            Err(error) => {
                eprintln!("kiekje: global shortcuts are off: {error}");
                if let Some(session) = session.upgrade() {
                    session
                        .shortcut_host
                        .replace(ShortcutHost::Off(error.to_string()));
                }
            }
        }
    });
}

fn desktop_changed(session: &Session, bindings: Vec<Binding>) {
    let mut labels = Vec::new();
    for binding in bindings {
        println!(
            "kiekje: shortcut {} changed to {}",
            binding.action.id(),
            binding.trigger.as_deref().unwrap_or("nothing")
        );
        if let Some(trigger) = binding.trigger {
            labels.push((binding.action, trigger));
        }
    }
    if let Some(handle) = session.settings_handle.borrow().as_ref() {
        handle.show_bound(&labels);
    }
    session.shortcut_labels.replace(
        labels
            .into_iter()
            .map(|(action, label)| (action_mode(action), label))
            .collect(),
    );
}

fn configure_shortcuts(session: &Session) {
    let Some(configure) = session
        .shortcuts
        .borrow()
        .as_ref()
        .map(PortalGuard::configure)
    else {
        eprintln!("kiekje: no shortcut session to configure");
        return;
    };
    glib::spawn_future_local(async move {
        if let Err(error) = configure.await {
            eprintln!("kiekje: the desktop's shortcut dialog did not open: {error}");
        }
    });
}

fn install_hyprland_binds(shortcuts: Shortcuts) {
    glib::spawn_future_local(async move {
        let include = hyprland::include_path();
        let target = include.clone();
        let written = gio::spawn_blocking(move || {
            let changed = hyprland::write_include(&target, APP_ID, &shortcuts)?;
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
            capture_after_menu(app, session, Request::saved(mode));
        }
        TrayCommand::Annotate => {
            println!("kiekje: tray annotate-region");
            capture_after_menu(app, session, Request::annotated());
        }
        TrayCommand::OpenMenu { x, y } => toggle_menu(app, session, Some((x, y))),
        TrayCommand::OpenCaptures => open_captures(),
        TrayCommand::OpenGallery => open_gallery(app, session),
        TrayCommand::OpenSettings => open_settings(app, session),
        TrayCommand::Quit => app.activate_action("quit", None),
    }
}

fn capture_after_menu(app: &gtk::Application, session: &Rc<Session>, request: Request) {
    let app = app.clone();
    let session = session.clone();
    glib::timeout_add_local_once(TRAY_MENU_CLOSE, move || {
        let request = Request {
            trigger_us: glib::monotonic_time(),
            ..request
        };
        start_capture(&app, request, &session)
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
        let content = popover::Content {
            shortcuts: session.shortcut_labels.borrow().clone(),
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
                    capture_after_menu(&app, &session, Request::saved(mode));
                }
                Pick::Annotate => {
                    println!("kiekje: menu annotate-region");
                    capture_after_menu(&app, &session, Request::annotated());
                }
                Pick::OpenCaptures => open_captures(),
                Pick::Settings => open_settings(&app, &session),
                Pick::Gallery => open_gallery(&app, &session),
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
        pin::close_all();
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

    let settings = gio::SimpleAction::new("settings", None);
    let settings_app = app.downgrade();
    let settings_session = Rc::downgrade(session);
    settings.connect_activate(move |_, _| {
        if let (Some(app), Some(session)) = (settings_app.upgrade(), settings_session.upgrade()) {
            open_settings(&app, &session);
        }
    });
    app.add_action(&settings);

    let gallery = gio::SimpleAction::new("gallery", None);
    let gallery_app = app.downgrade();
    let gallery_session = Rc::downgrade(session);
    gallery.connect_activate(move |_, _| {
        if let (Some(app), Some(session)) = (gallery_app.upgrade(), gallery_session.upgrade()) {
            open_gallery(&app, &session);
        }
    });
    app.add_action(&gallery);

    let pin = gio::SimpleAction::new("pin", Some(glib::VariantTy::STRING));
    let pin_app = app.downgrade();
    pin.connect_activate(move |_, parameter| {
        let (Some(app), Some(path)) = (
            pin_app.upgrade(),
            parameter.and_then(glib::Variant::get::<String>),
        ) else {
            eprintln!("kiekje: ignored a malformed pin request");
            return;
        };
        pin_file(&app, std::path::PathBuf::from(path));
    });
    app.add_action(&pin);
}

fn pin_file(app: &gtk::Application, path: std::path::PathBuf) {
    let app = app.clone();
    glib::spawn_future_local(async move {
        let shown = path.display().to_string();
        match pin::open_file(&app, path, None).await {
            Ok(()) => println!("kiekje: pinned {shown}"),
            Err(error) => eprintln!("kiekje: not pinned: {error}"),
        }
    });
}

fn open_gallery(app: &gtk::Application, session: &Rc<Session>) {
    if let Some(open) = session.gallery.borrow().as_ref() {
        open.present();
        return;
    }
    let (window, handle) = gallery::show(app);
    let closed_session = Rc::downgrade(session);
    window.connect_close_request(move |_| {
        if let Some(session) = closed_session.upgrade() {
            session.gallery.take();
            session.gallery_handle.take();
        }
        glib::Propagation::Proceed
    });
    session.gallery_handle.replace(Some(handle));
    session.gallery.replace(Some(window));
}

fn open_settings(app: &gtk::Application, session: &Rc<Session>) {
    if let Some(open) = session.settings.borrow().as_ref() {
        open.present();
        return;
    }
    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let config = gio::spawn_blocking(config::load).await.unwrap_or_default();
        if session.settings.borrow().is_some() {
            return;
        }
        let bound = ShortcutAction::ALL
            .into_iter()
            .filter_map(|action| {
                let mode = action_mode(action);
                session
                    .shortcut_labels
                    .borrow()
                    .iter()
                    .find(|(bound, _)| *bound == mode)
                    .map(|(_, label)| (action, label.clone()))
            })
            .collect();
        let model = settings::Model {
            shortcuts: config.shortcuts,
            storage: config.storage,
            notifications: config.notifications,
            capture: config.capture,
            overlay: config.overlay,
            host: session.shortcut_host.borrow().clone(),
            bound,
        };
        let saved_session = Rc::downgrade(&session);
        let configure_session = Rc::downgrade(&session);
        let cleared_session = Rc::downgrade(&session);
        let (window, handle) = settings::show(
            &app,
            model,
            move |shortcuts| {
                if let Some(session) = saved_session.upgrade() {
                    session.shortcut_labels.replace(hyprland_labels(shortcuts));
                }
            },
            move || {
                if let Some(session) = configure_session.upgrade() {
                    configure_shortcuts(&session);
                }
            },
            move || {
                if let Some(session) = cleared_session.upgrade()
                    && let Some(gallery) = session.gallery_handle.borrow().as_ref()
                {
                    gallery.reload();
                }
            },
        );
        let closed_session = Rc::downgrade(&session);
        window.connect_close_request(move |_| {
            if let Some(session) = closed_session.upgrade() {
                session.settings.take();
                session.settings_handle.take();
            }
            glib::Propagation::Proceed
        });
        session.settings_handle.replace(Some(handle));
        session.settings.replace(Some(window));
    });
}

fn hyprland_labels(shortcuts: &Shortcuts) -> Vec<(CaptureMode, String)> {
    ShortcutAction::ALL
        .into_iter()
        .filter_map(|action| {
            shortcuts
                .slot(action)
                .trigger()
                .map(|trigger| (action_mode(action), trigger.label()))
        })
        .collect()
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
        let config = config::load();
        let windows = (request.mode == CaptureMode::Region && config.overlay.window_snap)
            .then(|| thread::spawn(snapshot_windows));
        let notifications = config.notifications;
        let inline = config.capture.after_selection == AfterSelection::Edit;
        let magnifier = config.overlay.magnifier;
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
        let windows = windows
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();
        let result = result.map(|(capture, region)| (Capture { windows, ..capture }, region));
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking((
            result,
            notifications,
            inline,
            magnifier,
            glib::monotonic_time(),
        ));
    });

    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let Ok((result, notifications, inline, magnifier, captured_us)) = receiver.recv().await
        else {
            session.busy.set(false);
            return;
        };
        session.notifications.set(notifications);
        log_timing("trigger -> instance", request.trigger_us, received_us);
        log_timing("instance -> frames", received_us, captured_us);
        match result {
            Ok((capture, region)) => {
                let options = overlay::Options {
                    edit: request.edit || inline,
                    magnifier,
                };
                handle_capture(&app, request, capture, region, &monitors, options, session)
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
    options: overlay::Options,
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
            let pin_app = app.clone();
            overlay::show(
                app,
                frames,
                &capture.windows,
                monitors,
                options,
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
                    Outcome::Edited {
                        selection: Selection { rect, plan },
                        image,
                        finish,
                    } => {
                        remember_region(&session, rect);
                        log_selection(&selection_frames, plan);
                        if finish == Finish::Pin {
                            pin::open_image(&pin_app, image, Some(rect));
                            session.busy.set(false);
                            return;
                        }
                        finish_edit(image, finish, request, session);
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

fn snapshot_windows() -> Vec<LogicalRect> {
    match backends::windows::snapshot() {
        Ok((source, windows)) => {
            println!("kiekje: {} windows from {source}", windows.len());
            windows
        }
        Err(WindowError::Unavailable) => Vec::new(),
        Err(error) => {
            eprintln!("kiekje: window snapping is off for this capture: {error}");
            Vec::new()
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
    if request.edit {
        annotate(render, request, session);
        return;
    }
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

fn annotate(render: Render, request: Request, session: Rc<Session>) {
    glib::spawn_future_local(async move {
        let rendered = gio::spawn_blocking(render).await;
        session.busy.set(false);
        let image = match rendered {
            Ok(Some(image)) => image,
            Ok(None) => {
                notify(&session, empty_notice());
                return;
            }
            Err(_) => {
                eprintln!("kiekje: the render worker stopped");
                return;
            }
        };
        let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
            eprintln!("kiekje: no running app to open the editor in");
            return;
        };
        log_timing(
            "trigger -> editor opening",
            request.trigger_us,
            glib::monotonic_time(),
        );
        editor::open(
            &app,
            editor::Source::Capture {
                image,
                mode: request.mode,
            },
        );
    });
}

fn finish_edit(image: Image, finish: Finish, request: Request, session: Rc<Session>) {
    let request = Request {
        edit: false,
        ..request
    };
    match finish {
        Finish::Accept => deliver(Box::new(move || Some(image)), request, session),
        Finish::Copy => deliver(
            Box::new(move || Some(image)),
            Request {
                save: false,
                ..request
            },
            session,
        ),
        Finish::Save => save_only(image, request, session),
        Finish::SaveAs => save_as(image, request, session),
        Finish::Pin => {
            if let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() {
                pin::open_image(&app, image, None);
            }
            session.busy.set(false);
        }
    }
}

fn save_only(image: Image, request: Request, session: Rc<Session>) {
    glib::spawn_future_local(async move {
        match gio::spawn_blocking(move || store(image, request, None, false)).await {
            Ok(notice) => finish_delivery(&session, notice),
            Err(_) => {
                eprintln!("kiekje: the save worker stopped");
                session.busy.set(false);
            }
        }
    });
}

fn save_as(image: Image, request: Request, session: Rc<Session>) {
    let (width, height) = (image.width, image.height);
    glib::spawn_future_local(async move {
        let suggested = gio::spawn_blocking(move || {
            let storage = config::load().storage;
            let base = storage.filename.render(&Naming {
                mode: request.mode,
                stamp: storage::now(),
                width,
                height,
            });
            (storage.captures_dir(), base)
        })
        .await;
        let Ok((folder, base)) = suggested else {
            eprintln!("kiekje: the settings worker stopped, nothing saved");
            session.busy.set(false);
            return;
        };
        let dialog = gtk::FileDialog::builder()
            .title("Save the capture")
            .modal(true)
            .initial_name(format!("{base}.png"))
            .initial_folder(&gio::File::for_path(&folder))
            .build();
        let chosen = dialog.save_future(None::<&gtk::Window>).await;
        let path = match chosen {
            Ok(file) => file.path(),
            Err(error) if error.matches(gtk::DialogError::Dismissed) => {
                println!("kiekje: save as canceled, nothing saved");
                session.busy.set(false);
                return;
            }
            Err(error) => {
                eprintln!("kiekje: the save dialog failed: {error}");
                notify(
                    &session,
                    Notice::SaveFailed {
                        reason: format!("the save dialog failed: {error}"),
                        copied: false,
                    },
                );
                session.busy.set(false);
                return;
            }
        };
        let Some(path) = path else {
            notify(
                &session,
                Notice::SaveFailed {
                    reason: "that location is not on this computer".into(),
                    copied: false,
                },
            );
            session.busy.set(false);
            return;
        };
        let stored = gio::spawn_blocking(move || store_at(image, request.mode, &path)).await;
        match stored {
            Ok(notice) => finish_delivery(&session, notice),
            Err(_) => {
                eprintln!("kiekje: the save worker stopped");
                session.busy.set(false);
            }
        }
    });
}

fn store_at(image: Image, mode: CaptureMode, path: &std::path::Path) -> Notice {
    let (width, height) = (image.width, image.height);
    let png = match output::encode_png(image) {
        Ok(png) => png,
        Err(error) => {
            eprintln!("kiekje: png encode failed: {error}");
            return Notice::SaveFailed {
                reason: format!("could not encode the image: {error}"),
                copied: false,
            };
        }
    };
    let target = Target {
        folder: path
            .parent()
            .map_or_else(storage::default_captures_dir, std::path::Path::to_path_buf),
        base: path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kiekje".to_owned()),
        mode,
        width,
        height,
    };
    write_png(&png, target, false)
}

fn empty_notice() -> Notice {
    eprintln!("kiekje: nothing to deliver");
    Notice::CaptureFailed {
        reason: "the capture had no image to deliver".into(),
    }
}

fn finish_delivery(session: &Session, notice: Notice) {
    if let (Notice::Saved { .. }, Some(gallery)) =
        (&notice, session.gallery_handle.borrow().as_ref())
    {
        gallery.reload();
    }
    if matches!(notice, Notice::Saved { .. } | Notice::Copied { .. })
        && let Some(tray) = session.tray.borrow().as_ref()
    {
        tray.flash();
    }
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

fn record(path: &std::path::Path, mode: CaptureMode, width: u32, height: u32, bytes: usize) {
    let capture = NewCapture {
        path: path.to_path_buf(),
        mode,
        width,
        height,
        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
        created_us: glib::real_time(),
    };
    if let Err(error) = Library::open(&library::path()).and_then(|library| library.insert(&capture))
    {
        eprintln!("kiekje: the capture is saved but not in the library: {error}");
    }
}

fn check_library() {
    glib::spawn_future_local(async {
        let checked = gio::spawn_blocking(|| {
            let mut library = Library::open(&library::path())?;
            let changed = library.check_missing()?;
            Ok::<_, library::LibraryError>((changed, library.present()?))
        })
        .await;
        let present = match checked {
            Ok(Ok((changed, present))) => {
                if changed > 0 {
                    println!("kiekje: {changed} library entries changed missing state");
                }
                present
            }
            Ok(Err(error)) => {
                eprintln!("kiekje: library check failed: {error}");
                return;
            }
            Err(_) => {
                eprintln!("kiekje: the library worker stopped");
                return;
            }
        };
        let pruned = gio::spawn_blocking(move || thumbs::prune(&thumbs::dir(), &present)).await;
        match pruned {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => eprintln!("kiekje: thumbnail cache cleanup failed: {error}"),
            Err(_) => eprintln!("kiekje: the thumbnail worker stopped"),
        }
    });
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
    let base = storage.filename.render(&Naming {
        mode: request.mode,
        stamp: storage::now(),
        width,
        height,
    });
    let target = Target {
        folder: storage.captures_dir(),
        base,
        mode: request.mode,
        width,
        height,
    };
    write_png(&png, target, copied)
}

struct Target {
    folder: std::path::PathBuf,
    base: String,
    mode: CaptureMode,
    width: u32,
    height: u32,
}

fn write_png(png: &[u8], target: Target, copied: bool) -> Notice {
    let Target {
        folder,
        base,
        mode,
        width,
        height,
    } = target;
    match storage::save_png(png, &folder, &base) {
        Ok(path) => {
            println!("kiekje: saved {}", path.display());
            record(&path, mode, width, height, png.len());
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
                reason: format!("could not write to {}: {error}", folder.display()),
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
            edit: true,
            delay: Duration::from_secs(3),
            trigger_us: 42,
        };
        let decoded = Request::from_variant(&request.to_variant()).unwrap();
        assert!(decoded.edit);
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
