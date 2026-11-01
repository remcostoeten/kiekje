use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::thread;

use thiserror::Error;

use crate::backends::windows::kwin::{self as kwin_windows, KwinWindow};
use crate::backends::windows::{self, WindowError, WindowSourceKind};
use crate::backends::{self, kwin};
use crate::capture::monitors::{gdk_monitors, monitor_info};
use crate::capture::{BackendKind, CaptureError, MonitorInfo};
use crate::cli::{Headless, HeadlessTarget};
use crate::output::{self, Image};
use crate::storage::Quality;

#[derive(Debug, Error)]
pub enum HeadlessError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error("could not list the windows: {0}")]
    Windows(#[from] WindowError),
    #[error(
        "window ids and window capture without the overlay need KWin, which this session lacks"
    )]
    NeedsKwin,
    #[error("could not open the display to read the monitor layout")]
    NoDisplay,
    #[error("--crop reaches outside the {width}x{height} capture")]
    CropOutside { width: u32, height: u32 },
    #[error("could not encode the capture: {0}")]
    Encode(io::Error),
    #[error("could not write {path}: {source}")]
    Write { path: String, source: io::Error },
}

/// Captures a window or every monitor straight into a new file, without the running instance.
///
/// Nothing shows on screen and nothing reaches the clipboard, notifications, the gallery or
/// the last region. The file is created exclusively, so an existing path is an error.
///
/// # Examples
///
/// ```ignore
/// headless::capture(&request)?;
/// ```
pub fn capture(request: &Headless) -> Result<(), HeadlessError> {
    thread::sleep(request.delay);
    let image = match &request.target {
        HeadlessTarget::Window(id) => window(id, request)?,
        HeadlessTarget::Screen => screen(request)?,
    };
    let image = match request.crop {
        Some(rect) => output::crop_image(&image, rect).ok_or(HeadlessError::CropOutside {
            width: image.width,
            height: image.height,
        })?,
        None => image,
    };
    let bytes =
        output::encode(image, request.format, Quality::default()).map_err(HeadlessError::Encode)?;
    write_new(&request.output, &bytes)
}

/// Lists the visible windows with the ids `capture window --id` takes, topmost first.
///
/// # Examples
///
/// ```ignore
/// for window in headless::windows()? {
///     println!("{} {}", window.id, window.title);
/// }
/// ```
pub fn windows() -> Result<Vec<KwinWindow>, HeadlessError> {
    if windows::detect() != Some(WindowSourceKind::Kwin) {
        return Err(HeadlessError::NeedsKwin);
    }
    Ok(kwin_windows::list()?)
}

fn window(id: &str, request: &Headless) -> Result<Image, HeadlessError> {
    if !request.backend.allows(BackendKind::Kwin)
        || !backends::detect::probe_compositor().has_kwin()
    {
        return Err(HeadlessError::NeedsKwin);
    }
    Ok(kwin::capture_window(id)?)
}

fn screen(request: &Headless) -> Result<Image, HeadlessError> {
    gtk::init().map_err(|_| HeadlessError::NoDisplay)?;
    let monitors: Vec<MonitorInfo> = gdk_monitors().iter().map(monitor_info).collect();
    let capture = backends::capture_some(request.backend, &monitors, &monitors, false)?;
    output::stitch(&capture.frames).ok_or(HeadlessError::NoDisplay)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), HeadlessError> {
    let failed = |source| HeadlessError::Write {
        path: path.display().to_string(),
        source,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(failed)?;
    if let Err(error) = file.write_all(bytes) {
        // Best-effort cleanup of the partial file; the write error is the one to report.
        let _ = fs::remove_file(path);
        return Err(failed(error));
    }
    Ok(())
}
