use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use super::Session;
use super::notice::notify;
use crate::capture::Frame;
use crate::capture::mapping::SelectionPlan;
use crate::config;
use crate::notify::Notice;
use crate::ocr::{self, OcrError};
use crate::output;

pub(super) fn copy_text(session: &Rc<Session>, frames: Arc<Vec<Frame>>, plan: SelectionPlan) {
    let (sender, receiver) = async_channel::bounded(1);
    thread::spawn(move || {
        let started = Instant::now();
        let image = output::render_selection(&frames, plan);
        let languages = config::load().text.languages;
        let read = ocr::recognize(&image, &languages);
        info!(
            "read text from {}x{} in {:.0} ms",
            image.width,
            image.height,
            started.elapsed().as_secs_f64() * 1000.0
        );
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking(read);
    });
    let session = session.clone();
    glib::spawn_future_local(async move {
        let read = receiver.recv().await.unwrap_or(Err(OcrError::Worker));
        let notice = match read {
            Ok(text) if text.trim().is_empty() => {
                info!("no text in the region, clipboard unchanged");
                Notice::NoText
            }
            Ok(text) => match put_on_clipboard(&session, &text) {
                Ok(()) => {
                    if let Some(tray) = session.tray.borrow().as_ref() {
                        tray.flash();
                    }
                    Notice::TextCopied { text }
                }
                Err(reason) => {
                    error!("could not copy the text: {reason}");
                    Notice::ClipboardFailed { reason }
                }
            },
            Err(error) => {
                error!("text recognition failed: {error}");
                Notice::TextFailed {
                    reason: error.to_string(),
                }
            }
        };
        notify(&session, notice);
        session.busy.set(false);
    });
}

fn put_on_clipboard(session: &Session, text: &str) -> Result<(), String> {
    if let Some(clipboard) = &session.clipboard {
        return clipboard
            .set_text(text.to_owned())
            .map_err(|error| error.to_string());
    }
    let display = gdk::Display::default().ok_or("no display to put the text on the clipboard")?;
    display.clipboard().set_text(text);
    Ok(())
}
