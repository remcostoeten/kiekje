use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::capture::{Availability, BackendChoice, CaptureMode, LogicalRect};
use crate::clipboard::Clipboard;
use crate::config::{self, Notifications};
use crate::ids::APP_ID;
use crate::notify::FdoNotifier;
use crate::overlay::{self};
use crate::pin;
use crate::record::Recording;
use crate::shortcuts::{PortalGuard, ShortcutHost};
use crate::sound;
use crate::tray::TrayGuard;
use crate::ui::{self, fonts, gallery, settings};

mod countdown;
mod deliver;
mod flow;
mod notice;
mod panels;
mod recording;
mod save;
mod shortcuts;
mod startup;
mod tray;

use flow::start_capture;
use panels::{open_gallery, open_settings, pin_file};
use recording::{quit_after_recording, start_gif_export, toggle_recording};
pub(crate) use save::store_at;
use startup::{
    check_library, schedule_update_checks, start_session_services, write_default_config,
};
use tray::toggle_menu;

struct Recorder {
    active: RefCell<Option<Recording>>,
    live: Cell<bool>,
    exporting_gif: Cell<bool>,
}

struct Panels {
    popover: RefCell<Option<gtk::Window>>,
    settings: RefCell<Option<gtk::Window>>,
    gallery: RefCell<Option<gtk::Window>>,
    gallery_handle: RefCell<Option<gallery::Handle>>,
    settings_handle: RefCell<Option<settings::Handle>>,
}

struct ShortcutState {
    guard: RefCell<Option<PortalGuard>>,
    host: RefCell<ShortcutHost>,
    labels: RefCell<Vec<(CaptureMode, String)>>,
}

struct Session {
    busy: Cell<bool>,
    countdown: RefCell<Option<Rc<countdown::Countdown>>>,
    last_region: Cell<Option<LogicalRect>>,
    clipboard: Option<Clipboard>,
    notifier: Option<FdoNotifier>,
    notifications: Cell<Notifications>,
    tray: RefCell<Option<Box<dyn TrayGuard>>>,
    shortcuts: ShortcutState,
    panels: Panels,
    recorder: Recorder,
    window_capture: Cell<Option<Availability>>,
    trigger_config: RefCell<Option<config::Config>>,
}

pub enum Command {
    Daemon,
    Quit,
    Menu,
    Settings,
    Gallery,
    Capture(Request),
    Pin(std::path::PathBuf),
    Record,
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
        if u64::from(delay_ms) > crate::cli::MAX_DELAY_SECONDS * 1000 {
            warn!("ignored a capture request with a {delay_ms} ms delay");
            return None;
        }
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
        warn!("{error}, using the system font");
    }

    let clipboard = match Clipboard::start() {
        Ok(clipboard) => Some(clipboard),
        Err(error) => {
            info!("using the GTK clipboard: {error}");
            None
        }
    };
    let session = Rc::new(Session {
        busy: Cell::new(false),
        countdown: RefCell::new(None),
        last_region: Cell::new(None),
        clipboard,
        notifier: app.dbus_connection().map(FdoNotifier::start),
        notifications: Cell::new(Notifications::default()),
        tray: RefCell::new(None),
        shortcuts: ShortcutState {
            guard: RefCell::new(None),
            host: RefCell::new(ShortcutHost::Starting),
            labels: RefCell::new(Vec::new()),
        },
        panels: Panels {
            popover: RefCell::new(None),
            settings: RefCell::new(None),
            gallery: RefCell::new(None),
            gallery_handle: RefCell::new(None),
            settings_handle: RefCell::new(None),
        },
        recorder: Recorder {
            active: RefCell::new(None),
            live: Cell::new(false),
            exporting_gif: Cell::new(false),
        },
        window_capture: Cell::new(None),
        trigger_config: RefCell::new(None),
    });
    install_actions(&app, &session);
    start_session_services(&app, &session);
    write_default_config();
    ui::apply_saved_theme();
    check_library();
    schedule_update_checks(&session);
    thread::spawn(|| {
        if config::load().notifications.sound {
            sound::check();
        }
    });
    let hold = app.hold();
    let initial = Cell::new(Some(command));
    app.connect_activate(move |app| match initial.take() {
        Some(Command::Capture(request)) => start_capture(app, request, &session),
        Some(Command::Menu) => toggle_menu(app, &session, None),
        Some(Command::Settings) => open_settings(app, &session),
        Some(Command::Gallery) => open_gallery(app, &session),
        Some(Command::Pin(path)) => pin_file(app, path),
        Some(Command::Record) => toggle_recording(&session),
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
        Command::Record => app.activate_action("record", None),
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

fn install_actions(app: &gtk::Application, session: &Rc<Session>) {
    let capture = gio::SimpleAction::new("capture", Some(&RequestVariant::static_variant_type()));
    let capture_session = session.clone();
    let capture_app = app.downgrade();
    capture.connect_activate(move |_, parameter| {
        let (Some(app), Some(request)) = (
            capture_app.upgrade(),
            parameter.and_then(Request::from_variant),
        ) else {
            warn!("ignored a malformed capture request");
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
        if let Some(session) = quit_session.upgrade() {
            let countdown = session.countdown.take();
            if let Some(countdown) = countdown {
                countdown.cancel();
            }
        }
        if let Some(guard) = quit_session
            .upgrade()
            .and_then(|session| session.tray.take())
        {
            guard.stop();
        }
        let Some(app) = quit_app.upgrade() else {
            return;
        };
        let recording = quit_session.upgrade().filter(|session| {
            session
                .recorder
                .active
                .borrow()
                .as_ref()
                .map(Recording::stop)
                .is_some()
        });
        match recording {
            Some(session) => quit_after_recording(app, Rc::downgrade(&session)),
            None => app.quit(),
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

    let toggle_theme = gio::SimpleAction::new("toggle-theme", None);
    toggle_theme.connect_activate(|_, _| ui::toggle_theme());
    app.add_action(&toggle_theme);

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
            warn!("ignored a malformed pin request");
            return;
        };
        pin_file(&app, std::path::PathBuf::from(path));
    });
    app.add_action(&pin);

    let record = gio::SimpleAction::new("record", None);
    let record_session = Rc::downgrade(session);
    record.connect_activate(move |_, _| {
        if let Some(session) = record_session.upgrade() {
            toggle_recording(&session);
        }
    });
    app.add_action(&record);

    let export_gif = gio::SimpleAction::new("export-gif", Some(glib::VariantTy::STRING));
    let gif_session = Rc::downgrade(session);
    export_gif.connect_activate(move |_, parameter| {
        let (Some(session), Some(path)) = (
            gif_session.upgrade(),
            parameter.and_then(glib::Variant::get::<String>),
        ) else {
            warn!("ignored a malformed GIF export request");
            return;
        };
        start_gif_export(&session, std::path::PathBuf::from(path));
    });
    app.add_action(&export_gif);
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
    fn carries_a_window_capture_through_its_variant() {
        let request = Request::saved(CaptureMode::Window);
        let decoded = Request::from_variant(&request.to_variant()).unwrap();
        assert_eq!(decoded.mode, CaptureMode::Window);
    }

    #[test]
    fn rejects_a_variant_from_an_older_build() {
        let old = ("region".to_owned(), "auto".to_owned(), true, 7_i64).to_variant();
        assert!(Request::from_variant(&old).is_none());
    }
}
