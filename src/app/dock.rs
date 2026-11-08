use std::rc::{Rc, Weak};

use gtk::prelude::*;

use super::recording::toggle_recording;
use super::tray::capture_after_menu;
use super::{Request, Session};
use crate::capture::{Availability, CaptureMode};
use crate::config;
use crate::dock::{self, Action, Content};
use crate::ocr;
use crate::shortcuts::ShortcutAction;

pub(super) fn start_dock(app: &gtk::Application, session: &Rc<Session>, settings: config::Dock) {
    let content_session = Rc::downgrade(session);
    let pick_app = app.downgrade();
    let pick_session = Rc::downgrade(session);
    dock::install(
        app,
        move || content(&content_session),
        move |action| {
            let (Some(app), Some(session)) = (pick_app.upgrade(), pick_session.upgrade()) else {
                return;
            };
            info!("dock {action:?}");
            let request = match action {
                Action::Screen => Request::saved(CaptureMode::Fullscreen),
                Action::Region => Request::saved(CaptureMode::Region),
                Action::Text => Request::saved(CaptureMode::Text),
                Action::Gif => Request::gif(),
                Action::Video => {
                    toggle_recording(&session);
                    return;
                }
            };
            capture_after_menu(&app, &session, request);
        },
    );
    dock::configure(settings);
}

fn content(session: &Weak<Session>) -> Content {
    let Some(session) = session.upgrade() else {
        return Content {
            shortcuts: Vec::new(),
            text: Availability::Unavailable(ocr::NOT_INSTALLED),
            recording: false,
        };
    };
    let shortcuts = session
        .shortcuts
        .labels
        .borrow()
        .iter()
        .filter_map(|(action, label)| Some((dock_action(*action)?, label.clone())))
        .collect();
    let recording = session.recorder.active.borrow().is_some();
    Content {
        shortcuts,
        text: ocr::availability(),
        recording,
    }
}

fn dock_action(action: ShortcutAction) -> Option<Action> {
    match action {
        ShortcutAction::Region => Some(Action::Region),
        ShortcutAction::Fullscreen => Some(Action::Screen),
        ShortcutAction::Text => Some(Action::Text),
        ShortcutAction::Record => Some(Action::Video),
        ShortcutAction::Window => None,
    }
}
