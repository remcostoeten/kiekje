use std::rc::Rc;
use std::thread;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

use super::flow::start_capture;
use super::panels::{open_captures, open_gallery, open_settings, window_capture};
use super::recording::toggle_recording;
use super::{Request, Session};
use crate::capture::Availability;
use crate::tray::popover::{self, Pick};
use crate::tray::{SniTray, TrayCommand, TrayGuard, TrayHost};

const TRAY_MENU_CLOSE: Duration = Duration::from_millis(150);

pub(super) fn start_tray(app: &gtk::Application, session: &Rc<Session>, window: Availability) {
    let host = SniTray { window };
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
                info!("tray shown through {name}");
                match weak_session.upgrade() {
                    Some(session) => {
                        session.tray.replace(Some(Box::new(guard)));
                    }
                    None => guard.stop(),
                }
            }
            Ok(Err(error)) => {
                warn!("tray is off: {error}");
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
        TrayCommand::Capture { mode } => {
            info!("tray capture-{}", mode.as_str());
            capture_after_menu(app, session, Request::saved(mode));
        }
        TrayCommand::Annotate => {
            info!("tray annotate-region");
            capture_after_menu(app, session, Request::annotated());
        }
        TrayCommand::Record => toggle_recording(session),
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

pub(super) fn toggle_menu(
    app: &gtk::Application,
    session: &Rc<Session>,
    click: Option<(i32, i32)>,
) {
    if let Some(open) = session.panels.popover.take() {
        open.close();
        return;
    }
    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let content = popover::Content {
            shortcuts: session.shortcuts.labels.borrow().clone(),
            window: window_capture(&session),
            recording: session.recorder.active.borrow().is_some(),
        };
        let pick_app = app.downgrade();
        let pick_session = Rc::downgrade(&session);
        let window = popover::show(&app, click, content, move |pick| {
            let (Some(app), Some(session)) = (pick_app.upgrade(), pick_session.upgrade()) else {
                return;
            };
            match pick {
                Pick::Capture(mode) => {
                    info!("menu capture-{}", mode.as_str());
                    capture_after_menu(&app, &session, Request::saved(mode));
                }
                Pick::Annotate => {
                    info!("menu annotate-region");
                    capture_after_menu(&app, &session, Request::annotated());
                }
                Pick::Record => toggle_recording(&session),
                Pick::OpenCaptures => open_captures(),
                Pick::Settings => open_settings(&app, &session),
                Pick::Gallery => open_gallery(&app, &session),
                Pick::Quit => app.activate_action("quit", None),
            }
        });
        let closed_session = Rc::downgrade(&session);
        window.connect_close_request(move |closed| {
            if let Some(session) = closed_session.upgrade() {
                let current = session.panels.popover.borrow().clone();
                if current.as_ref() == Some(closed) {
                    session.panels.popover.take();
                }
            }
            glib::Propagation::Proceed
        });
        session.panels.popover.replace(Some(window));
    });
}
