mod gst;
mod portal;

use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use gtk::glib;
use thiserror::Error;

use crate::storage::{self, Stamp};

const FINISH_WAIT: Duration = Duration::from_secs(10);
const ERROR_POLL: Duration = Duration::from_millis(200);

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("the screen picker was closed")]
    Canceled,
    #[error("{0}")]
    Unavailable(String),
    #[error("the ScreenCast portal failed: {0}")]
    Portal(String),
    #[error("D-Bus: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("the recording pipeline failed: {0}")]
    Pipeline(String),
    #[error("the recording could not be saved: {0}")]
    Save(#[from] std::io::Error),
}

#[derive(Debug)]
pub enum Event {
    Started,
    Saved(PathBuf),
    Failed(RecordError),
}

/// A recording in progress; [`Recording::stop`] finishes the file.
pub struct Recording {
    stop: Sender<()>,
}

impl Recording {
    /// Asks the worker to finish the file; a [`Event::Saved`] or [`Event::Failed`] follows.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// recording.stop();
    /// ```
    pub fn stop(&self) {
        // A gone worker has already sent its last event, so there is nothing left to stop.
        let _ = self.stop.send(());
    }
}

/// Checks whether this system can record: GStreamer with the PipeWire, VP8 and WebM elements.
///
/// The ScreenCast portal is checked when a recording starts. Loads GStreamer, so call it off the
/// GTK main thread.
///
/// # Examples
///
/// ```ignore
/// if let Err(reason) = record::check() { println!("recording is off: {reason}"); }
/// ```
pub fn check() -> Result<(), String> {
    gst::check()
}

/// Returns `$XDG_VIDEOS_DIR/Kiekje`, where recordings are saved.
///
/// # Examples
///
/// ```ignore
/// let folder = record::folder();
/// ```
pub fn folder() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Videos)
        .unwrap_or_else(|| glib::home_dir().join("Videos"))
        .join("Kiekje")
}

/// Starts a recording on a worker thread and reports its progress over `events`.
///
/// The desktop's screen picker opens first. The file is written to the cache and moved into
/// [`folder`] when the recording finishes.
///
/// # Examples
///
/// ```ignore
/// let recording = record::start(sender);
/// ```
pub fn start(events: async_channel::Sender<Event>) -> Recording {
    let (stop, stopped) = mpsc::channel();
    thread::spawn(move || {
        let event = match run(&events, &stopped) {
            Ok(path) => Event::Saved(path),
            Err(error) => Event::Failed(error),
        };
        if events.send_blocking(event).is_err() {
            eprintln!("kiekje: a recording event was dropped, the app is shutting down");
        }
    });
    Recording { stop }
}

fn run(
    events: &async_channel::Sender<Event>,
    stopped: &Receiver<()>,
) -> Result<PathBuf, RecordError> {
    gst::check().map_err(RecordError::Unavailable)?;
    let stream = portal::open()?;
    if stopped.try_recv().is_ok() {
        return Err(RecordError::Canceled);
    }
    let scratch = glib::user_cache_dir().join("kiekje").join("recording");
    fs::create_dir_all(&scratch)?;
    let file = scratch.join(format!("{}.webm", file_stem(&storage::now())));
    let description = pipeline(stream.fd.as_raw_fd(), stream.node, &file)?;
    let pipeline = gst::Pipeline::start(&description)?;
    if events.send_blocking(Event::Started).is_err() {
        return Err(RecordError::Canceled);
    }
    let finished = loop {
        if let Some(reason) = pipeline.error(Duration::ZERO) {
            break Err(RecordError::Pipeline(reason));
        }
        if pipeline.ended() {
            break Ok(());
        }
        match stopped.recv_timeout(ERROR_POLL) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break pipeline.finish(FINISH_WAIT),
            Err(RecvTimeoutError::Timeout) => {}
        }
    };
    drop(pipeline);
    drop(stream);
    if let Err(error) = finished {
        if fs::metadata(&file).map_or(0, |metadata| metadata.len()) == 0 {
            discard(&file);
            return Err(error);
        }
        eprintln!("kiekje: {error}; keeping what was written");
    }
    Ok(storage::move_into(&file, &folder())?)
}

fn discard(file: &Path) {
    if let Err(error) = fs::remove_file(file)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("kiekje: could not remove {}: {error}", file.display());
    }
}

fn file_stem(stamp: &Stamp) -> String {
    format!(
        "kiekje-recording-{:04}-{:02}-{:02}-{:02}-{:02}-{:02}",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    )
}

fn pipeline(fd: i32, node: u32, file: &Path) -> Result<String, RecordError> {
    let location = file
        .to_str()
        .filter(|location| !location.contains(['"', '\\']))
        .ok_or_else(|| {
            RecordError::Unavailable(format!(
                "the cache path {} can't be passed to GStreamer",
                file.display()
            ))
        })?;
    let threads = thread::available_parallelism().map_or(2, |count| count.get().min(8));
    Ok(format!(
        "pipewiresrc fd={fd} path={node} do-timestamp=true \
         ! videoconvert ! queue \
         ! vp8enc deadline=1 cpu-used=8 threads={threads} end-usage=vbr target-bitrate=8000000 \
         ! webmmux ! filesink location=\"{location}\""
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_recording_after_its_start_time() {
        let stamp = Stamp {
            year: 2026,
            month: 10,
            day: 5,
            hour: 9,
            minute: 3,
            second: 7,
        };
        assert_eq!(file_stem(&stamp), "kiekje-recording-2026-10-05-09-03-07");
    }

    #[test]
    fn quotes_the_file_in_the_pipeline() {
        let description = pipeline(7, 42, Path::new("/home/a b/rec.webm")).unwrap();
        assert!(description.starts_with("pipewiresrc fd=7 path=42 "));
        assert!(description.ends_with("filesink location=\"/home/a b/rec.webm\""));
    }

    #[test]
    fn refuses_a_file_gstreamer_would_misread() {
        assert!(matches!(
            pipeline(7, 42, Path::new("/home/a\"b/rec.webm")),
            Err(RecordError::Unavailable(_))
        ));
    }
}
