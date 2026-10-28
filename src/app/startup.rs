use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::{gio, glib};

use super::Session;
use super::shortcuts::register_shortcuts;
use super::tray::start_tray;
use crate::backends::windows::{self};
use crate::capture::Availability;
use crate::config::{self};
use crate::library::{self, Library, thumbs};
use crate::notify::Notice;
use crate::update;

const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(30);
const UPDATE_CHECK_TICK: Duration = Duration::from_secs(60 * 60);

pub(super) fn start_session_services(app: &gtk::Application, session: &Rc<Session>) {
    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let window = match gio::spawn_blocking(|| windows::availability(windows::detect())).await {
            Ok(window) => window,
            Err(_) => {
                error!("the window detection worker stopped");
                Availability::Unavailable(windows::UNAVAILABLE)
            }
        };
        if let Availability::Unavailable(reason) = window {
            info!("window capture is off: {reason}");
        }
        session.window_capture.set(Some(window));
        register_shortcuts(&app, &session, window);
        start_tray(&app, &session, window);
    });
}

pub(super) fn schedule_update_checks(session: &Rc<Session>) {
    let session = Rc::downgrade(session);
    glib::timeout_add_local_once(FIRST_UPDATE_CHECK, move || {
        check_for_updates(session.clone());
        glib::timeout_add_local(UPDATE_CHECK_TICK, move || {
            check_for_updates(session.clone());
            glib::ControlFlow::Continue
        });
    });
}

fn check_for_updates(session: Weak<Session>) {
    glib::spawn_future_local(async move {
        let checked =
            gio::spawn_blocking(|| update::automatic(&config::path(), &update::state::path()))
                .await;
        match checked {
            Ok(update::Automatic::Newer {
                available,
                notify: true,
            }) => {
                info!("version {} is available", available.version);
                let Some(session) = session.upgrade() else {
                    return;
                };
                let notice = Notice::UpdateAvailable {
                    version: available.version,
                    url: available.url,
                };
                match &session.notifier {
                    Some(notifier) => notifier.send(&notice),
                    None => info!("no session bus, update notification skipped"),
                }
            }
            Ok(_) => {}
            Err(_) => error!("the update check worker stopped"),
        }
    });
}

pub(super) fn write_default_config() {
    glib::spawn_future_local(async {
        let path = config::path();
        let target = path.clone();
        match gio::spawn_blocking(move || config::ensure_file(&target)).await {
            Ok(Ok(true)) => info!("wrote the default config to {}", path.display()),
            Ok(Ok(false)) => {}
            Ok(Err(error)) => warn!("{error}"),
            Err(_) => error!("the config worker stopped"),
        }
    });
}

pub(super) fn check_library() {
    glib::spawn_future_local(async {
        let checked = gio::spawn_blocking(|| {
            let mut library = Library::open(&library::path())?;
            let keep_days = config::load().saving.keep_unsaved_days;
            let pruned = library.prune_unsaved(glib::real_time(), keep_days)?;
            if pruned > 0 {
                info!("deleted {pruned} unsaved captures older than {keep_days} days");
            }
            let changed = library.check_missing()?;
            Ok::<_, library::LibraryError>((changed, library.present()?))
        })
        .await;
        let present = match checked {
            Ok(Ok((changed, present))) => {
                if changed > 0 {
                    info!("{changed} library entries changed missing state");
                }
                present
            }
            Ok(Err(error)) => {
                error!("library check failed: {error}");
                return;
            }
            Err(_) => {
                error!("the library worker stopped");
                return;
            }
        };
        let pruned = gio::spawn_blocking(move || thumbs::prune(&thumbs::dir(), &present)).await;
        match pruned {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => error!("thumbnail cache cleanup failed: {error}"),
            Err(_) => error!("the thumbnail worker stopped"),
        }
    });
}
