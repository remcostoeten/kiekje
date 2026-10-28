use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::Session;
use super::shortcuts::{action_mode, configure_shortcuts, hyprland_labels};
use crate::capture::Availability;
use crate::config::{self};
use crate::pin;
use crate::shortcuts::ShortcutAction;
use crate::ui::{gallery, settings};

pub(super) fn window_capture(session: &Session) -> Availability {
    session
        .window_capture
        .get()
        .unwrap_or(Availability::Available)
}

pub(super) fn open_captures() {
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
                error!("could not create {}: {error}", directory.display());
                return;
            }
            Err(_) => {
                error!("the folder worker stopped");
                return;
            }
        };
        let uri = gio::File::for_path(&directory).uri();
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
        {
            error!("could not open {uri}: {error}");
        }
    });
}

pub(super) fn with_gallery(session: &Session, act: impl FnOnce(&gallery::Handle)) {
    if let Some(gallery) = session.panels.gallery_handle.borrow().as_ref() {
        act(gallery);
    }
}

pub(super) fn pin_file(app: &gtk::Application, path: std::path::PathBuf) {
    let app = app.clone();
    glib::spawn_future_local(async move {
        let shown = path.display().to_string();
        match pin::open_file(&app, path, None).await {
            Ok(()) => info!("pinned {shown}"),
            Err(error) => error!("not pinned: {error}"),
        }
    });
}

pub(super) fn open_gallery(app: &gtk::Application, session: &Rc<Session>) {
    if let Some(open) = session.panels.gallery.borrow().as_ref() {
        open.present();
        return;
    }
    let (window, handle) = gallery::show(app);
    let closed_session = Rc::downgrade(session);
    window.connect_close_request(move |_| {
        if let Some(session) = closed_session.upgrade() {
            session.panels.gallery.take();
            session.panels.gallery_handle.take();
        }
        glib::Propagation::Proceed
    });
    session.panels.gallery_handle.replace(Some(handle));
    session.panels.gallery.replace(Some(window));
}

pub(super) fn open_settings(app: &gtk::Application, session: &Rc<Session>) {
    if let Some(open) = session.panels.settings.borrow().as_ref() {
        open.present();
        return;
    }
    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let config = gio::spawn_blocking(config::load).await.unwrap_or_default();
        if session.panels.settings.borrow().is_some() {
            return;
        }
        let bound = ShortcutAction::ALL
            .into_iter()
            .filter_map(|action| {
                let mode = action_mode(action);
                session
                    .shortcuts
                    .labels
                    .borrow()
                    .iter()
                    .find(|(bound, _)| *bound == mode)
                    .map(|(_, label)| (action, label.clone()))
            })
            .collect();
        let model = settings::Model {
            shortcuts: config.shortcuts,
            storage: config.storage,
            saving: config.saving,
            notifications: config.notifications,
            capture: config.capture,
            overlay: config.overlay,
            general: config.general,
            upload: config.upload,
            drawing_keys: config.drawing.keys,
            host: session.shortcuts.host.borrow().clone(),
            bound,
            window_capture: window_capture(&session),
        };
        let saved_session = Rc::downgrade(&session);
        let configure_session = Rc::downgrade(&session);
        let cleared_session = Rc::downgrade(&session);
        let (window, handle) = settings::show(
            &app,
            model,
            move |shortcuts| {
                if let Some(session) = saved_session.upgrade() {
                    session.shortcuts.labels.replace(hyprland_labels(shortcuts));
                }
            },
            move || {
                if let Some(session) = configure_session.upgrade() {
                    configure_shortcuts(&session);
                }
            },
            move || {
                if let Some(session) = cleared_session.upgrade()
                    && let Some(gallery) = session.panels.gallery_handle.borrow().as_ref()
                {
                    gallery.reload();
                }
            },
        );
        let closed_session = Rc::downgrade(&session);
        window.connect_close_request(move |_| {
            if let Some(session) = closed_session.upgrade() {
                session.panels.settings.take();
                session.panels.settings_handle.take();
            }
            glib::Propagation::Proceed
        });
        session.panels.settings_handle.replace(Some(handle));
        session.panels.settings.replace(Some(window));
    });
}
