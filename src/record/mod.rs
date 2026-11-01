mod animation;
mod gst;
mod portal;
mod tokens;

use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

pub use animation::{Exported, export as export_gif};
pub use gst::GstError;

use gtk::glib;
use thiserror::Error;

use crate::storage::{self, Stamp};
use crate::{config, sound};

const FINISH_WAIT: Duration = Duration::from_secs(10);
const POSTER_WAIT: Duration = Duration::from_secs(10);
const ERROR_POLL: Duration = Duration::from_millis(200);
const POSTER_LANES: usize = 2;
const MAX_RATE: u32 = 30;
const MAX_WIDTH: u32 = 2560;

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("the screen picker was closed")]
    Canceled,
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Gst(#[from] GstError),
    #[error("the ScreenCast portal failed: {0}")]
    Portal(String),
    #[error("D-Bus: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("the recording pipeline failed: {0}")]
    Pipeline(String),
    #[error("the GIF could not be made: {0}")]
    Gif(String),
    #[error("the recording could not be saved: {0}")]
    Save(#[from] std::io::Error),
    #[error("the recording ended early; kept {path}: {source}")]
    Partial {
        path: PathBuf,
        source: Box<RecordError>,
    },
}

/// A region of one monitor to record, in the monitor's frame pixels.
///
/// `origin` is the monitor's logical position, used to check that the screen picked in the
/// portal is the one the region was selected on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    pub frame: (u32, u32),
    pub origin: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Edges {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

impl Area {
    fn edges(self) -> Option<Edges> {
        let left = self.left & !1;
        let top = self.top & !1;
        let right_edge = (self.left + self.width).min(self.frame.0);
        let bottom_edge = (self.top + self.height).min(self.frame.1);
        let width = right_edge.checked_sub(left)? & !1;
        let height = bottom_edge.checked_sub(top)? & !1;
        (width >= 2 && height >= 2).then_some(Edges {
            left,
            right: self.frame.0 - left - width,
            top,
            bottom: self.frame.1 - top - height,
        })
    }
}

#[derive(Debug)]
pub enum Event {
    Countdown { beeps: bool, proceed: Sender<bool> },
    Started,
    Saved(PathBuf),
    Partial { path: PathBuf, error: RecordError },
    Failed(RecordError),
}

/// A recording in progress; [`Recording::stop`] finishes the file.
pub struct Recording {
    stop: Sender<Stop>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Save,
    Discard,
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
        let _ = self.stop.send(Stop::Save);
    }

    /// Asks the worker to stop and delete what it recorded; a [`Event::Failed`] with
    /// [`RecordError::Canceled`] follows.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// recording.cancel();
    /// ```
    pub fn cancel(&self) {
        // A gone worker has already sent its last event, so there is nothing left to cancel.
        let _ = self.stop.send(Stop::Discard);
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
pub fn check() -> Result<(), GstError> {
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
/// The desktop's screen picker opens first. With an `area` the picker offers monitors only and the
/// recording is cropped to that region. The file is written to the cache and moved into [`folder`]
/// when the recording finishes.
///
/// # Examples
///
/// ```ignore
/// let recording = record::start(sender, None);
/// ```
pub fn start(events: async_channel::Sender<Event>, area: Option<Area>) -> Recording {
    let (stop, stopped) = mpsc::channel();
    thread::spawn(move || {
        let event = outcome(run(&events, &stopped, area));
        if events.send_blocking(event).is_err() {
            warn!("a recording event was dropped, the app is shutting down");
        }
    });
    Recording { stop }
}

fn outcome(result: Result<PathBuf, RecordError>) -> Event {
    match result {
        Ok(path) => Event::Saved(path),
        Err(RecordError::Partial { path, source }) => Event::Partial {
            path,
            error: *source,
        },
        Err(error) => Event::Failed(error),
    }
}

fn run(
    events: &async_channel::Sender<Event>,
    stopped: &Receiver<Stop>,
    area: Option<Area>,
) -> Result<PathBuf, RecordError> {
    gst::check()?;
    let edges =
        match area {
            Some(area) => Some(area.edges().ok_or_else(|| {
                RecordError::Unavailable("the region is too small to record".into())
            })?),
            None => None,
        };
    let key = area.map_or_else(
        || "any".to_owned(),
        |area| format!("monitor-{}-{}", area.origin.0, area.origin.1),
    );
    let stream = portal::open(area.is_some(), tokens::load(&key).as_deref())?;
    if let (Some(area), Some(position)) = (area, stream.position)
        && position != area.origin
    {
        tokens::forget(&key);
        return Err(RecordError::Unavailable(
            "the screen you picked is not the one the region is on".into(),
        ));
    }
    if let Some(token) = &stream.restore_token {
        tokens::store(&key, token);
    }
    await_countdown(events, stopped)?;
    if stopped.try_recv().is_ok() {
        return Err(RecordError::Canceled);
    }
    let scratch = glib::user_cache_dir().join("kiekje").join("recording");
    fs::create_dir_all(&scratch)?;
    let file = scratch.join(format!("{}.webm", file_stem(&storage::now())));
    let description = pipeline(stream.fd.as_raw_fd(), stream.node, &file, area.zip(edges))?;
    let pipeline = gst::Pipeline::start(&description)?;
    if events.send_blocking(Event::Started).is_err() {
        return Err(RecordError::Canceled);
    }
    let mut stop = Stop::Save;
    let finished = loop {
        match pipeline.poll(Duration::ZERO) {
            Some(gst::Poll::Failed(reason)) => break Err(RecordError::Pipeline(reason)),
            Some(gst::Poll::Ended) => break Ok(()),
            None => {}
        }
        match stopped.recv_timeout(ERROR_POLL) {
            Ok(requested) => {
                stop = requested;
                break pipeline.finish(FINISH_WAIT);
            }
            Err(RecvTimeoutError::Disconnected) => break pipeline.finish(FINISH_WAIT),
            Err(RecvTimeoutError::Timeout) => {}
        }
    };
    drop(pipeline);
    drop(stream);
    if stop == Stop::Discard {
        discard(&file);
        return Err(RecordError::Canceled);
    }
    let finished = match finished {
        Err(error) if fs::metadata(&file).map_or(0, |metadata| metadata.len()) == 0 => {
            discard(&file);
            return Err(error);
        }
        Err(error) => {
            warn!("{error}; keeping what was written");
            Err(error)
        }
        Ok(()) => Ok(()),
    };
    let path = match storage::move_into(&file, &folder()) {
        Ok(path) => path,
        Err(error) if finished.is_err() => {
            error!("could not move the partial recording: {error}");
            file
        }
        Err(error) => return Err(error.into()),
    };
    match finished {
        Ok(()) => Ok(path),
        Err(source) => Err(RecordError::Partial {
            path,
            source: Box::new(source),
        }),
    }
}

fn await_countdown(
    events: &async_channel::Sender<Event>,
    stopped: &Receiver<Stop>,
) -> Result<(), RecordError> {
    let beeps = config::load().notifications.sound;
    if beeps {
        sound::check();
    }
    let (proceed, answer) = mpsc::channel();
    events
        .send_blocking(Event::Countdown { beeps, proceed })
        .map_err(|_| RecordError::Canceled)?;
    loop {
        match answer.recv_timeout(ERROR_POLL) {
            Ok(true) => return Ok(()),
            Ok(false) | Err(RecvTimeoutError::Disconnected) => return Err(RecordError::Canceled),
            Err(RecvTimeoutError::Timeout) => {
                if stopped.try_recv().is_ok() {
                    return Err(RecordError::Canceled);
                }
            }
        }
    }
}

fn discard(file: &Path) {
    if let Err(error) = fs::remove_file(file)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        error!("could not remove {}: {error}", file.display());
    }
}

fn file_stem(stamp: &Stamp) -> String {
    format!(
        "kiekje-recording-{:04}-{:02}-{:02}-{:02}-{:02}-{:02}",
        stamp.year, stamp.month, stamp.day, stamp.hour, stamp.minute, stamp.second
    )
}

fn quoted(file: &Path) -> Result<&str, RecordError> {
    file.to_str()
        .filter(|location| !location.contains(['"', '\\']))
        .ok_or_else(|| {
            RecordError::Unavailable(format!(
                "the path {} can't be passed to GStreamer",
                file.display()
            ))
        })
}

fn crop_stage(region: Option<(Area, Edges)>) -> String {
    region.map_or_else(String::new, |(area, edges)| {
        format!(
            "! videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 \
             ! videocrop left={} right={} top={} bottom={} ",
            area.frame.0, area.frame.1, edges.left, edges.right, edges.top, edges.bottom
        )
    })
}

fn pipeline(
    fd: i32,
    node: u32,
    file: &Path,
    region: Option<(Area, Edges)>,
) -> Result<String, RecordError> {
    let location = quoted(file)?;
    let threads = thread::available_parallelism().map_or(2, |count| count.get().min(8));
    let crop = crop_stage(region);
    Ok(format!(
        "pipewiresrc fd={fd} path={node} do-timestamp=true \
         ! video/x-raw \
         ! videorate drop-only=true max-rate={MAX_RATE} \
         {crop}! videoscale ! video/x-raw,width=[1,{MAX_WIDTH}],pixel-aspect-ratio=1/1 \
         ! videoconvert \
         ! queue leaky=downstream max-size-buffers=4 max-size-time=0 max-size-bytes=0 \
         ! vp8enc deadline=1 cpu-used=8 threads={threads} end-usage=vbr target-bitrate=8000000 \
         ! webmmux ! filesink location=\"{location}\""
    ))
}

fn poster_pipeline(video: &Path, png: &Path, edge: Option<u32>) -> Result<String, RecordError> {
    let (video, png) = (quoted(video)?, quoted(png)?);
    let scale = edge.map_or_else(String::new, |edge| {
        format!("! videoscale ! video/x-raw,width=[1,{edge}],pixel-aspect-ratio=1/1 ")
    });
    Ok(format!(
        "filesrc location=\"{video}\" ! decodebin {scale}! videoconvert \
         ! pngenc snapshot=true compression-level=1 ! filesink location=\"{png}\""
    ))
}

struct Gate {
    open: Mutex<usize>,
    freed: Condvar,
}

struct Permit<'a>(&'a Gate);

impl Gate {
    fn enter(&self) -> Permit<'_> {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        while *open >= POSTER_LANES {
            open = self
                .freed
                .wait(open)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *open += 1;
        Permit(self)
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *self.0.open.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        self.0.freed.notify_one();
    }
}

static POSTERS: Gate = Gate {
    open: Mutex::new(0),
    freed: Condvar::new(),
};

/// Writes the first frame of the recording at `video` to `png`, at full size or fitted to `edge` px wide.
///
/// At most two posters decode at once, the rest wait. Loads GStreamer and decodes, so call it off
/// the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// record::poster(Path::new("/home/me/Videos/Kiekje/clip.webm"), Path::new("/tmp/poster.png"), Some(640))?;
/// ```
pub fn poster(video: &Path, png: &Path, edge: Option<u32>) -> Result<(), RecordError> {
    let description = poster_pipeline(video, png, edge)?;
    let _permit = POSTERS.enter();
    gst::Pipeline::start(&description)?.wait_end(POSTER_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_preserved_partial_recording_to_its_own_event() {
        let path = PathBuf::from("/videos/partial.webm");
        let event = outcome(Err(RecordError::Partial {
            path: path.clone(),
            source: Box::new(RecordError::Pipeline("stopped early".into())),
        }));
        assert!(
            matches!(event, Event::Partial { path: saved, error: RecordError::Pipeline(_) } if saved == path)
        );
    }

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
        let description = pipeline(7, 42, Path::new("/home/a b/rec.webm"), None).unwrap();
        assert!(description.starts_with("pipewiresrc fd=7 path=42 "));
        assert!(description.ends_with("filesink location=\"/home/a b/rec.webm\""));
    }

    #[test]
    fn caps_the_recording_rate_and_width_before_the_encoder() {
        let description = pipeline(7, 42, Path::new("/rec.webm"), None).unwrap();
        let system_memory = description.find("! video/x-raw ").unwrap();
        let rate = description
            .find("videorate drop-only=true max-rate=30")
            .unwrap();
        let scale = description
            .find("videoscale ! video/x-raw,width=[1,2560]")
            .unwrap();
        let queue = description.find("queue leaky=downstream").unwrap();
        let encoder = description.find("vp8enc").unwrap();
        assert!(system_memory < rate && rate < scale && scale < queue && queue < encoder);
    }

    fn area(left: u32, top: u32, width: u32, height: u32) -> Area {
        Area {
            left,
            top,
            width,
            height,
            frame: (3840, 2160),
            origin: (0, 0),
        }
    }

    #[test]
    fn crops_a_region_to_even_edges() {
        let edges = area(101, 51, 400, 301).edges().unwrap();
        assert_eq!(
            edges,
            Edges {
                left: 100,
                right: 3840 - 100 - 400,
                top: 50,
                bottom: 2160 - 50 - 302,
            }
        );
    }

    #[test]
    fn keeps_a_region_inside_its_frame() {
        let edges = area(3800, 2100, 500, 500).edges().unwrap();
        assert_eq!((edges.right, edges.bottom), (0, 0));
        assert_eq!(area(10, 10, 1, 1).edges(), None);
        assert_eq!(area(4000, 0, 100, 100).edges(), None);
    }

    #[test]
    fn crops_after_scaling_to_the_frame_and_before_the_encoder() {
        let area = area(100, 50, 400, 300);
        let description = pipeline(
            7,
            42,
            Path::new("/rec.webm"),
            area.edges().map(|edges| (area, edges)),
        )
        .unwrap();
        let rate = description.find("videorate").unwrap();
        let frame = description
            .find("video/x-raw,width=3840,height=2160")
            .unwrap();
        let crop = description
            .find("videocrop left=100 right=3340 top=50 bottom=1810")
            .unwrap();
        let limit = description.find("width=[1,2560]").unwrap();
        assert!(rate < frame && frame < crop && crop < limit);
    }

    #[test]
    fn scales_a_poster_only_when_asked() {
        let video = Path::new("/v/clip.webm");
        let png = Path::new("/c/poster.png");
        assert!(
            !poster_pipeline(video, png, None)
                .unwrap()
                .contains("videoscale")
        );
        assert!(
            poster_pipeline(video, png, Some(640))
                .unwrap()
                .contains("videoscale ! video/x-raw,width=[1,640],pixel-aspect-ratio=1/1")
        );
    }

    #[test]
    fn lets_only_two_posters_decode_at_once() {
        let first = POSTERS.enter();
        let second = POSTERS.enter();
        let (sender, receiver) = mpsc::channel();
        let waiter = thread::spawn(move || {
            let _third = POSTERS.enter();
            sender.send(()).unwrap();
        });
        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        waiter.join().unwrap();
        drop(second);
    }

    #[test]
    fn decodes_one_frame_for_a_poster() {
        let description =
            poster_pipeline(Path::new("/v/clip.webm"), Path::new("/c/poster.png"), None).unwrap();
        assert!(description.starts_with("filesrc location=\"/v/clip.webm\" ! decodebin"));
        assert!(description.contains("pngenc snapshot=true"));
        assert!(description.ends_with("filesink location=\"/c/poster.png\""));
    }

    #[test]
    fn refuses_a_file_gstreamer_would_misread() {
        assert!(matches!(
            pipeline(7, 42, Path::new("/home/a\"b/rec.webm"), None),
            Err(RecordError::Unavailable(_))
        ));
    }
}
