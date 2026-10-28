use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use super::Session;
use super::panels::open_gallery;
use crate::notify::Notice;
use crate::sound;
use crate::ui::widgets::Failure;

pub(super) fn notify(session: &Rc<Session>, notice: Notice) {
    let settings = session.notifications.get();
    if settings.sound && matches!(notice, Notice::Saved { .. } | Notice::Copied { .. }) {
        sound::play_capture();
    }
    let failure = failure_for(&notice);
    if matches!(notice, Notice::RecordingPartial { .. })
        && let Some(failure) = failure.clone()
    {
        show_failure(session, failure);
    }
    if !settings.enabled {
        if let Some(failure) = failure {
            show_failure(session, failure);
        }
        return;
    }
    match &session.notifier {
        Some(notifier) => {
            let weak = Rc::downgrade(session);
            notifier.send_with_fallback(&notice, move || {
                if let (Some(session), Some(failure)) = (weak.upgrade(), failure) {
                    show_failure(&session, failure);
                }
            });
        }
        None => {
            info!("no session bus, notification skipped");
            if let Some(failure) = failure {
                show_failure(session, failure);
            }
        }
    }
}

fn failure_for(notice: &Notice) -> Option<Failure> {
    match notice {
        Notice::CaptureFailed { reason } => Some(Failure::Capture(reason.clone())),
        Notice::SaveFailed { reason, .. } => Some(Failure::Save(reason.clone())),
        Notice::ClipboardFailed { reason } => Some(Failure::Clipboard(reason.clone())),
        Notice::RecordingFailed { reason } => Some(Failure::Recording(reason.clone())),
        Notice::RecordingPartial { path, reason } => Some(Failure::PartialRecording {
            path: path.clone(),
            reason: reason.clone(),
        }),
        Notice::GifFailed { reason } => Some(Failure::Gif(reason.clone())),
        _ => None,
    }
}

fn show_failure(session: &Rc<Session>, failure: Failure) {
    let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
        warn!("no application window for the failure banner");
        return;
    };
    open_gallery(&app, session);
    if let Some(gallery) = session.panels.gallery_handle.borrow().as_ref() {
        gallery.failure(failure);
    }
}
