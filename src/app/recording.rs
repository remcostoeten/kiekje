use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::Session;
use super::notice::notify;
use super::panels::with_gallery;
use super::shortcuts::set_stop_key;
use crate::capture::mapping::SelectionPlan;
use crate::capture::{CaptureMode, Frame};
use crate::library::{self, Library, NewCapture, thumbs};
use crate::notify::Notice;
use crate::overlay;
use crate::record::{self};
use crate::shortcuts::ShortcutHost;
use crate::{config, sound};

const RECORDING_COUNTDOWN_SECONDS: u32 = 3;
const UNMAP_WAIT: Duration = Duration::from_millis(150);
const RECORDING_QUIT_POLL: Duration = Duration::from_millis(100);
const RECORDING_QUIT_WAIT: Duration = Duration::from_secs(12);

pub(super) fn quit_after_recording(app: gtk::Application, session: Weak<Session>) {
    info!("saving the recording before quitting");
    let started = std::time::Instant::now();
    glib::timeout_add_local(RECORDING_QUIT_POLL, move || {
        let saving = session
            .upgrade()
            .is_some_and(|session| session.recorder.active.borrow().is_some());
        if saving && started.elapsed() < RECORDING_QUIT_WAIT {
            return glib::ControlFlow::Continue;
        }
        app.quit();
        glib::ControlFlow::Break
    });
}

pub(super) fn toggle_recording(session: &Rc<Session>) {
    if let Some(recording) = session.recorder.active.borrow().as_ref() {
        info!("stopping the recording");
        recording.stop();
        return;
    }
    begin_recording(session, None, false);
}

pub(super) fn record_region(
    session: &Rc<Session>,
    frames: &[Frame],
    plan: SelectionPlan,
    gif: bool,
) {
    let refused = |reason: &str| {
        warn!("not recording the region: {reason}");
        notify(
            session,
            Notice::CaptureFailed {
                reason: reason.to_owned(),
            },
        );
    };
    if session.recorder.active.borrow().is_some() {
        refused("a recording is already running, stop it first");
        return;
    }
    let SelectionPlan::Crop { frame_index, rect } = plan else {
        refused("a region recording needs a region on one screen");
        return;
    };
    let frame = &frames[frame_index];
    let area = record::Area {
        left: rect.x,
        top: rect.y,
        width: rect.width,
        height: rect.height,
        frame: (frame.width, frame.height),
        origin: (frame.monitor.logical.x, frame.monitor.logical.y),
    };
    let marker =
        overlay::recording::marker_for(&area, &frame.monitor.connector, frame.monitor.logical);
    begin_recording(session, Some(area), gif);
    session.recorder.marker.replace(Some(marker));
}

fn begin_recording(session: &Rc<Session>, area: Option<record::Area>, gif: bool) {
    info!("starting a recording");
    session.recorder.gif_after.set(gif);
    let (events, received) = async_channel::unbounded();
    session
        .recorder
        .active
        .replace(Some(record::start(events, area)));
    let weak = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        while let Ok(event) = received.recv().await {
            let Some(session) = weak.upgrade() else {
                return;
            };
            recording_event(&session, event);
        }
    });
}

fn recording_event(session: &Rc<Session>, event: record::Event) {
    let notice = match event {
        record::Event::Countdown { beeps, proceed } => {
            start_countdown(session, beeps, proceed);
            return;
        }
        record::Event::Started => {
            session.recorder.live.set(true);
            set_tray_recording(session, true);
            grab_stop_key(session);
            if let Some(marker) = session.recorder.marker.take() {
                session
                    .recorder
                    .frame
                    .replace(overlay::recording::show(&marker));
            }
            Notice::RecordingStarted
        }
        record::Event::Saved(path) => {
            info!("recording saved to {}", path.display());
            end_recording(session);
            add_recording(session, path.clone());
            if session.recorder.gif_after.replace(false) {
                start_gif_export(session, path.clone());
            }
            Notice::RecordingSaved { path }
        }
        record::Event::Partial { path, error } => {
            warn!("recording ended early: {error}; kept {}", path.display());
            end_recording(session);
            add_recording(session, path.clone());
            Notice::RecordingPartial {
                path,
                reason: error.to_string(),
            }
        }
        record::Event::Failed(record::RecordError::Canceled) => {
            info!("recording cancelled");
            end_recording(session);
            return;
        }
        record::Event::Failed(error) => {
            error!("recording failed: {error}");
            end_recording(session);
            Notice::RecordingFailed {
                reason: error.to_string(),
            }
        }
    };
    notify(session, notice);
}

pub(super) struct Ticker {
    source: Option<glib::SourceId>,
    proceed: mpsc::Sender<bool>,
}

impl Drop for Ticker {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
    }
}

fn start_countdown(session: &Rc<Session>, beeps: bool, proceed: mpsc::Sender<bool>) {
    if beeps {
        sound::prepare();
        sound::play_beep();
    }
    grab_stop_key(session);
    let marker = session.recorder.marker.take();
    let badge = overlay::recording::show_countdown(marker.as_ref());
    if let Some(badge) = &badge {
        badge.set(RECORDING_COUNTDOWN_SECONDS);
    }
    session.recorder.badge.replace(badge);
    if let Some(marker) = &marker {
        session
            .recorder
            .frame
            .replace(overlay::recording::show(marker));
    }
    let elapsed = Cell::new(0u32);
    let weak = Rc::downgrade(session);
    let source = glib::timeout_add_seconds_local(1, move || {
        elapsed.set(elapsed.get() + 1);
        let Some(session) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if elapsed.get() < RECORDING_COUNTDOWN_SECONDS {
            if beeps {
                sound::play_beep();
            }
            if let Some(badge) = session.recorder.badge.borrow().as_ref() {
                badge.set(RECORDING_COUNTDOWN_SECONDS - elapsed.get());
            }
            return glib::ControlFlow::Continue;
        }
        session.recorder.badge.take();
        if let Some(mut ticker) = session.recorder.countdown.take() {
            ticker.source.take();
            let proceed = ticker.proceed.clone();
            glib::timeout_add_local_once(UNMAP_WAIT, move || answer(&proceed, true));
        }
        glib::ControlFlow::Break
    });
    session.recorder.countdown.replace(Some(Ticker {
        source: Some(source),
        proceed,
    }));
}

pub(super) fn cancel_countdown(session: &Session) -> bool {
    let Some(mut ticker) = session.recorder.countdown.take() else {
        return false;
    };
    if let Some(source) = ticker.source.take() {
        source.remove();
    }
    session.recorder.badge.take();
    info!("cancelling the recording before it started");
    answer(&ticker.proceed, false);
    true
}

fn grab_stop_key(session: &Session) {
    if matches!(*session.shortcuts.host.borrow(), ShortcutHost::Kde) {
        session.recorder.escape_grabbed.set(true);
        set_stop_key(true);
    }
}

fn answer(proceed: &mpsc::Sender<bool>, go: bool) {
    if proceed.send(go).is_err() {
        debug!("the recording stopped before the countdown ended");
    }
}

fn list_media(
    path: std::path::PathBuf,
    width: u32,
    height: u32,
) -> Result<i64, library::LibraryError> {
    let bytes = std::fs::metadata(&path).map_or(0, |metadata| metadata.len());
    Library::open(&library::path()).and_then(|library| {
        library.insert(&NewCapture {
            path,
            mode: CaptureMode::Fullscreen,
            width,
            height,
            bytes,
            created_us: glib::real_time(),
            unsaved: false,
            annotated: false,
        })
    })
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn add_recording(session: &Rc<Session>, path: std::path::PathBuf) {
    let session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        let added = gio::spawn_blocking(move || {
            let (width, height) = thumbs::video_size(&path).unwrap_or_else(|error| {
                warn!("recording size unknown: {error}");
                (0, 0)
            });
            list_media(path, width, height)
        })
        .await;
        match added {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                warn!("the recording is saved but not in the library: {error}");
                return;
            }
            Err(_) => {
                error!("the library worker stopped");
                return;
            }
        }
        if let Some(session) = session.upgrade() {
            with_gallery(&session, |gallery| gallery.reload());
        }
    });
}

fn export_and_list(video: &std::path::Path) -> Result<record::Exported, record::RecordError> {
    let gif = record::export_gif(video)?;
    if let Err(error) = list_media(gif.path.clone(), gif.width, gif.height) {
        warn!("the GIF is saved but not in the library: {error}");
    }
    Ok(gif)
}

fn announce_gif_saved(session: &Rc<Session>, source_name: &str, gif: record::Exported) {
    info!("GIF saved to {}", gif.path.display());
    let gif_name = file_name(&gif.path);
    with_gallery(session, |gallery| {
        gallery.say(&format!("Saved {gif_name} next to {source_name}"), false);
        gallery.reveal(&gif.path);
    });
    if let Some(tray) = session.tray.borrow().as_ref() {
        tray.flash();
    }
    notify(
        session,
        Notice::GifSaved {
            path: gif.path,
            width: gif.width,
            height: gif.height,
        },
    );
}

fn announce_gif_failed(session: &Rc<Session>, source_name: &str, reason: String) {
    error!("GIF export failed: {reason}");
    with_gallery(session, |gallery| {
        gallery.say(&format!("No GIF from {source_name}: {reason}"), true);
    });
    notify(session, Notice::GifFailed { reason });
}

pub(super) fn start_gif_export(session: &Rc<Session>, video: std::path::PathBuf) {
    let name = file_name(&video);
    if session.recorder.exporting_gif.replace(true) {
        info!("a GIF export is already running, ignoring {name}");
        with_gallery(session, |gallery| {
            gallery.say("Wait for the running GIF export to finish", true);
        });
        return;
    }
    info!("exporting {name} as a GIF");
    with_gallery(session, |gallery| {
        gallery.working(&format!(
            "Exporting {name} as a GIF, up to 30 seconds at 10 fps"
        ));
    });
    notify(
        session,
        Notice::GifExporting {
            path: video.clone(),
        },
    );
    let weak = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        let made = gio::spawn_blocking(move || export_and_list(&video)).await;
        let Some(session) = weak.upgrade() else {
            return;
        };
        session.recorder.exporting_gif.set(false);
        match made {
            Ok(Ok(gif)) => announce_gif_saved(&session, &name, gif),
            Ok(Err(error)) => announce_gif_failed(&session, &name, error.to_string()),
            Err(_) => announce_gif_failed(&session, &name, "the GIF worker stopped".to_owned()),
        }
    });
}

fn end_recording(session: &Session) {
    session.recorder.active.take();
    session.recorder.countdown.take();
    session.recorder.badge.take();
    session.recorder.escape.reset();
    if session.recorder.escape_grabbed.replace(false) {
        set_stop_key(false);
    }
    session.recorder.marker.take();
    session.recorder.frame.take();
    session.recorder.gif_after.set(false);
    if session.recorder.live.replace(false) {
        set_tray_recording(session, false);
    }
}

fn set_tray_recording(session: &Session, recording: bool) {
    if let Some(tray) = session.tray.borrow().as_ref() {
        tray.set_recording(recording);
    }
}

pub(super) fn toggle_microphone(session: &Rc<Session>) {
    let session = Rc::downgrade(session);
    glib::spawn_future_local(async move {
        let saved = gio::spawn_blocking(|| {
            config::update(&config::path(), |config| {
                config.recording.microphone = !config.recording.microphone;
            })
        })
        .await;
        let on = match saved {
            Ok(Ok(config)) => config.recording.microphone,
            Ok(Err(error)) => {
                warn!("the microphone setting was not saved: {error}");
                return;
            }
            Err(_) => {
                error!("the config worker stopped");
                return;
            }
        };
        info!("microphone recording {}", if on { "on" } else { "off" });
        if let Some(session) = session.upgrade()
            && let Some(tray) = session.tray.borrow().as_ref()
        {
            tray.set_microphone(on);
        }
    });
}
