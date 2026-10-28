use std::sync::Arc;
use std::thread;

use gtk::glib;

use super::Request;
use super::deliver::{Filing, Placement};
use super::flow::log_timing;
use crate::capture::CaptureMode;
use crate::clipboard::Clipboard;
use crate::config::{self};
use crate::library::{self, Library, NewCapture};
use crate::notify::Notice;
use crate::output::{self, Image};
use crate::storage::{self, ImageFormat, Naming};

pub(crate) fn store_at(
    image: Image,
    mode: CaptureMode,
    annotated: bool,
    path: &std::path::Path,
) -> Notice {
    let (width, height) = (image.width, image.height);
    let storage = config::load().storage;
    let format = ImageFormat::from_path(path).unwrap_or(storage.format);
    let bytes = match output::encode(image, format, storage.quality) {
        Ok(bytes) => bytes,
        Err(error) => {
            error!("{} encode failed: {error}", format.label());
            return Notice::SaveFailed {
                reason: format!("could not encode the image: {error}"),
                copied: false,
            };
        }
    };
    match storage::save_as(&bytes, path, format) {
        Ok(path) => {
            info!("saved {}", path.display());
            record(&path, mode, (width, height), bytes.len(), false, annotated);
            Notice::Saved {
                path,
                width,
                height,
                copied: false,
            }
        }
        Err(error) => {
            error!("save failed: {error}");
            Notice::SaveFailed {
                reason: format!("could not write {}: {error}", path.display()),
                copied: false,
            }
        }
    }
}

pub(super) fn record(
    path: &std::path::Path,
    mode: CaptureMode,
    (width, height): (u32, u32),
    bytes: usize,
    unsaved: bool,
    annotated: bool,
) -> bool {
    let capture = NewCapture {
        path: path.to_path_buf(),
        mode,
        width,
        height,
        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
        created_us: glib::real_time(),
        unsaved,
        annotated,
    };
    match Library::open(&library::path()).and_then(|library| library.insert(&capture)) {
        Ok(_) => true,
        Err(error) => {
            warn!("the capture is saved but not in the library: {error}");
            false
        }
    }
}

pub(super) fn store(
    image: Image,
    request: Request,
    filing: Filing,
    clipboard: Option<&Clipboard>,
    copied: bool,
    config: Option<config::Config>,
) -> Notice {
    let (width, height) = (image.width, image.height);
    let config = config.unwrap_or_else(config::load);
    let unsaved = filing.placement == Placement::Configured && !config.saving.enabled;
    let storage = config.storage;
    let format = storage.format;
    let rgba = output::into_rgba(image);
    let encoded = |format: ImageFormat| {
        output::encode_rgba(&rgba, format, storage.quality).map_err(|error| {
            error!("{} encode failed: {error}", format.label());
            Notice::CaptureFailed {
                reason: format!("could not encode the image: {error}"),
            }
        })
    };
    let encoded_for_clipboard = || {
        output::encode_clipboard_png(&rgba).map_err(|error| {
            error!("png encode failed: {error}");
            Notice::CaptureFailed {
                reason: format!("could not encode the image: {error}"),
            }
        })
    };
    let offer = |clipboard: &Clipboard, png: &Arc<Vec<u8>>| match clipboard.set_png(png.clone()) {
        Ok(()) => {
            log_timing(
                "trigger -> clipboard offered",
                request.trigger_us,
                glib::monotonic_time(),
            );
            info!("copied {width}x{height} to the clipboard");
            true
        }
        Err(error) => {
            warn!("clipboard not set: {error}");
            false
        }
    };
    let mut copied = copied;
    let file = match clipboard {
        Some(clipboard) if request.save && format == ImageFormat::Png => {
            let png = match encoded(ImageFormat::Png) {
                Ok(bytes) => Arc::new(bytes),
                Err(notice) => return notice,
            };
            copied |= offer(clipboard, &png);
            Some(png)
        }
        Some(clipboard) if request.save => {
            let both = thread::scope(|scope| {
                let file = scope.spawn(|| encoded(format));
                let png = encoded_for_clipboard().map(Arc::new);
                if let Ok(png) = &png {
                    copied |= offer(clipboard, png);
                }
                let file = file.join().unwrap_or_else(|_| {
                    Err(Notice::CaptureFailed {
                        reason: "the encoder stopped".into(),
                    })
                });
                png.and(file)
            });
            match both {
                Ok(bytes) => Some(Arc::new(bytes)),
                Err(notice) => return notice,
            }
        }
        Some(clipboard) => {
            match encoded_for_clipboard() {
                Ok(bytes) => copied |= offer(clipboard, &Arc::new(bytes)),
                Err(notice) => return notice,
            }
            None
        }
        None if request.save => match encoded(format) {
            Ok(bytes) => Some(Arc::new(bytes)),
            Err(notice) => return notice,
        },
        None => None,
    };
    let Some(bytes) = file else {
        return if copied {
            Notice::Copied { width, height }
        } else {
            Notice::ClipboardFailed {
                reason: "could not put the image on the clipboard".into(),
            }
        };
    };
    let base = storage.filename.render(&Naming {
        mode: request.mode,
        stamp: storage::now(),
        width,
        height,
    });
    let target = Target {
        folder: if unsaved {
            storage::unsaved_dir()
        } else {
            storage.captures_dir()
        },
        base,
        format,
        mode: request.mode,
        width,
        height,
        unsaved,
        annotated: filing.annotated,
    };
    write_file(&bytes, target, copied)
}

struct Target {
    folder: std::path::PathBuf,
    base: String,
    format: ImageFormat,
    mode: CaptureMode,
    width: u32,
    height: u32,
    unsaved: bool,
    annotated: bool,
}

fn write_file(bytes: &[u8], target: Target, copied: bool) -> Notice {
    let Target {
        folder,
        base,
        format,
        mode,
        width,
        height,
        unsaved,
        annotated,
    } = target;
    let saved = storage::save(bytes, &folder, &base, format);
    if unsaved {
        match saved {
            Ok(path) => {
                info!("kept {} as unsaved", path.display());
                if !record(&path, mode, (width, height), bytes.len(), true, annotated)
                    && let Err(error) = std::fs::remove_file(&path)
                {
                    error!("could not remove the untracked {}: {error}", path.display());
                }
            }
            Err(error) => warn!("the unsaved capture was not kept: {error}"),
        }
        return if copied {
            Notice::Copied { width, height }
        } else {
            Notice::ClipboardFailed {
                reason: "could not put the image on the clipboard, and saving is off".into(),
            }
        };
    }
    match saved {
        Ok(path) => {
            info!("saved {}", path.display());
            record(&path, mode, (width, height), bytes.len(), false, annotated);
            Notice::Saved {
                path,
                width,
                height,
                copied,
            }
        }
        Err(error) => {
            error!("save failed: {error}");
            Notice::SaveFailed {
                reason: format!("could not write to {}: {error}", folder.display()),
                copied,
            }
        }
    }
}
