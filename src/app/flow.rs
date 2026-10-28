use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use gtk::{gdk, gio, glib};

use super::countdown;
use super::deliver::{Filing, deliver, finish_delivery, finish_edit};
use super::notice::notify;
use super::{Request, Session};
use crate::backends;
use crate::backends::windows::{self, WindowError};
use crate::capture::mapping::{self, SelectionPlan};
use crate::capture::monitors::{gdk_monitors, monitor_info};
use crate::capture::{Capture, CaptureError, CaptureMode, Frame, LogicalRect, MonitorInfo};
use crate::config::{self, AfterSelection};
use crate::editor::Finish;
use crate::notify::Notice;
use crate::output::{self};
use crate::overlay::{self, Outcome, Selection};
use crate::pin;
use crate::sound;
use crate::storage::last_region;

pub(super) fn start_capture(app: &gtk::Application, request: Request, session: &Rc<Session>) {
    if session.busy.replace(true) {
        warn!("a capture is already running, ignoring this trigger");
        return;
    }
    if request.delay.is_zero() {
        grab(app, request, session);
        return;
    }
    let capture_app = app.clone();
    let capture_session = Rc::downgrade(session);
    let canceled_session = Rc::downgrade(session);
    let countdown = countdown::Countdown::start(
        app,
        request.delay,
        move || {
            if let Some(session) = capture_session.upgrade() {
                session.countdown.take();
                let request = Request {
                    trigger_us: glib::monotonic_time(),
                    ..request
                };
                grab(&capture_app, request, &session);
            }
        },
        move || {
            if let Some(session) = canceled_session.upgrade() {
                session.countdown.take();
                session.busy.set(false);
            }
        },
    );
    session.countdown.replace(Some(countdown));
}

fn grab(app: &gtk::Application, request: Request, session: &Rc<Session>) {
    let received_us = glib::monotonic_time();
    let monitors = gdk_monitors();
    let infos: Vec<MonitorInfo> = monitors.iter().map(monitor_info).collect();
    let known_region = session.last_region.get();

    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let repeat =
            (request.mode == CaptureMode::Repeat).then(|| known_region.or_else(last_region::load));
        let capturing = match repeat {
            Some(None) => None,
            _ => {
                let targets = repeat
                    .flatten()
                    .map(|region| repeat_targets(region, &infos))
                    .unwrap_or_else(|| infos.clone());
                Some(thread::spawn(move || {
                    backends::capture_some(request.backend, &infos, &targets, false)
                }))
            }
        };
        let window_list =
            (request.mode == CaptureMode::Window).then(|| thread::spawn(list_windows));
        let config = config::load();
        let windows = match window_list {
            Some(handle) => Some(handle),
            None if request.mode == CaptureMode::Region && config.overlay.window_snap => {
                Some(thread::spawn(|| Ok(snapshot_windows())))
            }
            None => None,
        };
        let notifications = config.notifications;
        if notifications.sound {
            sound::check();
        }
        let inline = config.capture.after_selection == AfterSelection::Edit;
        let previous_region = (request.mode == CaptureMode::Region)
            .then(|| known_region.or_else(last_region::load))
            .flatten();
        let result = match capturing {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| {
                    Err(CaptureError::Unavailable("capture thread panicked".into()))
                })
                .map(|capture| (capture, repeat.flatten())),
            None => Err(CaptureError::NoLastRegion),
        };
        let windows = match windows.map(|handle| handle.join()) {
            Some(Ok(listed)) => listed,
            Some(Err(_)) => Err(CaptureError::WindowList(
                "the window list worker stopped".into(),
            )),
            None => Ok(Vec::new()),
        };
        let result = result.and_then(|(capture, region)| {
            windows.map(|windows| (Capture { windows, ..capture }, region))
        });
        let overlay = config.overlay;
        let drawing = config.drawing;
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking((
            result,
            notifications,
            inline,
            overlay,
            drawing,
            previous_region,
            glib::monotonic_time(),
            config,
        ));
    });

    let app = app.clone();
    let session = session.clone();
    glib::spawn_future_local(async move {
        let Ok((
            result,
            notifications,
            inline,
            overlay,
            drawing,
            previous_region,
            captured_us,
            config,
        )) = receiver.recv().await
        else {
            session.busy.set(false);
            return;
        };
        session.notifications.set(notifications);
        session.trigger_config.replace(Some(config));
        if notifications.sound {
            sound::prepare();
        }
        log_timing("trigger -> instance", request.trigger_us, received_us);
        log_timing("instance -> frames", received_us, captured_us);
        match result {
            Ok((capture, region)) => {
                let options = overlay::Options {
                    mode: request.mode,
                    edit: request.edit || inline,
                    overlay,
                    drawing,
                    last_region: previous_region,
                };
                handle_capture(&app, request, capture, region, &monitors, options, session)
            }
            Err(CaptureError::Canceled) => {
                info!("capture canceled");
                session.busy.set(false);
            }
            Err(error) => {
                error!("capture failed: {error}");
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
    info!("backend {}", capture.backend);
    if let Some(reason) = &capture.fallback_reason {
        info!("skipped earlier backends: {reason}");
    }
    for frame in &capture.frames {
        info!(
            "frame {} {}x{} for logical {}x{}{:+}{:+} ({:?})",
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
            deliver(
                Box::new(move || output::stitch(&frames)),
                request,
                Filing::default(),
                session,
            );
        }
        (CaptureMode::Repeat, Some(region)) => {
            repeat_region(request, capture.frames, region, session)
        }
        (CaptureMode::Repeat, None) => {
            error!("capture failed: {}", CaptureError::NoLastRegion);
            notify(
                &session,
                Notice::CaptureFailed {
                    reason: CaptureError::NoLastRegion.to_string(),
                },
            );
            session.busy.set(false);
        }
        (CaptureMode::Region | CaptureMode::Window, _) => {
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
                            Filing::default(),
                            session,
                        )
                    }
                    Outcome::Edited {
                        selection: Selection { rect, plan },
                        image,
                        finish,
                        annotated,
                    } => {
                        remember_region(&session, rect);
                        log_selection(&selection_frames, plan);
                        if finish == Finish::Pin {
                            pin::open_image(&pin_app, image, Some(rect));
                            session.busy.set(false);
                            return;
                        }
                        finish_edit(image, finish, annotated, request, session);
                    }
                    Outcome::Saved(notice) => finish_delivery(&session, notice),
                    Outcome::Canceled => {
                        info!("selection canceled, nothing saved");
                        session.busy.set(false);
                    }
                    Outcome::MonitorLost(connector) => {
                        warn!(
                            "monitor {connector} was disconnected during the selection, nothing saved"
                        );
                        notify(&session, Notice::MonitorLost);
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

fn repeat_targets(region: LogicalRect, infos: &[MonitorInfo]) -> Vec<MonitorInfo> {
    let logical: Vec<LogicalRect> = infos.iter().map(|info| info.logical).collect();
    let touched = mapping::touched_monitors(region, &logical);
    if touched.is_empty() {
        return infos.to_vec();
    }
    touched
        .into_iter()
        .map(|index| infos[index].clone())
        .collect()
}

fn list_windows() -> Result<Vec<LogicalRect>, CaptureError> {
    match backends::windows::snapshot() {
        Ok((source, windows)) => {
            debug!("{} windows from {source}", windows.len());
            Ok(windows)
        }
        Err(WindowError::Unavailable) => Err(CaptureError::NoWindowSource(windows::UNAVAILABLE)),
        Err(error) => Err(CaptureError::WindowList(error.to_string())),
    }
}

fn snapshot_windows() -> Vec<LogicalRect> {
    match backends::windows::snapshot() {
        Ok((source, windows)) => {
            debug!("{} windows from {source}", windows.len());
            windows
        }
        Err(WindowError::Unavailable) => Vec::new(),
        Err(error) => {
            warn!("window snapping is off for this capture: {error}");
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
        error!("capture failed: {reason}");
        notify(&session, Notice::CaptureFailed { reason });
        session.busy.set(false);
        return;
    };
    log_selection(&frames, plan);
    deliver(
        Box::new(move || Some(output::render_selection(&frames, plan))),
        request,
        Filing::default(),
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
            Ok(Err(error)) => warn!("{error}"),
            Err(_) => error!("the last region worker stopped"),
        }
    });
}

fn log_selection(frames: &[Frame], plan: SelectionPlan) {
    match plan {
        SelectionPlan::Crop { frame_index, rect } => info!(
            "selected {}x{}+{}+{} on {}",
            rect.width, rect.height, rect.x, rect.y, frames[frame_index].monitor.connector,
        ),
        SelectionPlan::Compose {
            bounds,
            width,
            height,
            ..
        } => info!(
            "selected {width}x{height} across monitors from logical {}x{}{:+}{:+}",
            bounds.width, bounds.height, bounds.x, bounds.y,
        ),
    }
}

pub(super) fn log_timing(label: &str, from_us: i64, to_us: i64) {
    info!(
        "timing {label}: {:.1} ms",
        (to_us - from_us) as f64 / 1000.0
    );
}
