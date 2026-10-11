use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gtk::glib;
use gtk::prelude::*;
use thiserror::Error;

use crate::ui::RESOURCE_PREFIX;

const NAMES: [&str; 2] = ["screen-capture", "camera-shutter"];
const FALLBACK_THEME: &str = "freedesktop";
const EXTENSIONS: [&str; 3] = ["oga", "ogg", "wav"];

const GSTREAMER: &CStr = c"libgstreamer-1.0.so.0";
const PLAYBIN: &CStr = c"playbin";

static PLAYABLE: OnceLock<bool> = OnceLock::new();

thread_local! {
    static PLAYER: RefCell<Option<gtk::MediaFile>> = const { RefCell::new(None) };
}

#[derive(Debug, Error)]
enum SoundError {
    #[error("GStreamer is not installed")]
    NoGstreamer,
    #[error("GStreamer is missing {0}")]
    MissingSymbol(&'static str),
    #[error("GStreamer did not start")]
    Init,
    #[error("GStreamer's playbin is missing, install the GStreamer base plugins")]
    NoPlaybin,
}

/// Checks once whether GTK's media backend can play sounds, and logs why not.
///
/// GTK's GStreamer backend aborts the process when playbin is missing, so nothing may
/// open a media file before this returns true. It loads GStreamer, so call it off the
/// main thread.
///
/// # Examples
///
/// ```ignore
/// thread::spawn(sound::check);
/// ```
pub fn check() -> bool {
    *PLAYABLE.get_or_init(|| match probe() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("kiekje: no capture sound: {error}");
            false
        }
    })
}

fn probe() -> Result<(), SoundError> {
    // SAFETY: the symbols are looked up by their documented C names and called with
    // the signatures from gst/gst.h; the library stays loaded for the process lifetime.
    unsafe {
        let library = libc::dlopen(GSTREAMER.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if library.is_null() {
            return Err(SoundError::NoGstreamer);
        }
        let init: unsafe extern "C" fn(
            *mut c_int,
            *mut *mut *mut c_char,
            *mut *mut c_void,
        ) -> c_int = std::mem::transmute(symbol(library, c"gst_init_check")?);
        let find: unsafe extern "C" fn(*const c_char) -> *mut c_void =
            std::mem::transmute(symbol(library, c"gst_element_factory_find")?);
        let unref: unsafe extern "C" fn(*mut c_void) =
            std::mem::transmute(symbol(library, c"gst_object_unref")?);
        if init(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(SoundError::Init);
        }
        let factory = find(PLAYBIN.as_ptr());
        if factory.is_null() {
            return Err(SoundError::NoPlaybin);
        }
        unref(factory);
        Ok(())
    }
}

unsafe fn symbol(library: *mut c_void, name: &'static CStr) -> Result<*mut c_void, SoundError> {
    // SAFETY: `library` is a live handle from dlopen and `name` is nul-terminated.
    let pointer = unsafe { libc::dlsym(library, name.as_ptr()) };
    if pointer.is_null() {
        return Err(SoundError::MissingSymbol(
            name.to_str().unwrap_or("a symbol"),
        ));
    }
    Ok(pointer)
}

/// Plays the capture sound without waiting for it to finish.
///
/// Uses the sound theme's `screen-capture` or `camera-shutter` sound and falls back to the
/// bundled one. Stays silent until [`check`] has found a working media backend.
///
/// # Examples
///
/// ```ignore
/// sound::play_capture();
/// ```
pub fn play_capture() {
    if PLAYABLE.get() != Some(&true) {
        return;
    }
    PLAYER.with(|player| {
        let mut player = player.borrow_mut();
        let media = player.get_or_insert_with(open);
        if media.is_ended() || media.is_playing() {
            media.seek(0);
        }
        media.play();
    });
}

/// Loads the capture sound ahead of time so the first capture plays it without delay.
///
/// # Examples
///
/// ```ignore
/// sound::prepare();
/// ```
pub fn prepare() {
    if PLAYABLE.get() != Some(&true) {
        return;
    }
    PLAYER.with(|player| {
        player.borrow_mut().get_or_insert_with(open);
    });
}

fn open() -> gtk::MediaFile {
    crate::ui::load_stylesheet();
    let media = match themed() {
        Some(path) => gtk::MediaFile::for_filename(path),
        None => gtk::MediaFile::for_resource(&format!("{RESOURCE_PREFIX}/sounds/capture.oga")),
    };
    media.connect_error_notify(|media| {
        if let Some(error) = media.error() {
            eprintln!("kiekje: the capture sound did not play: {error}");
        }
    });
    media
}

fn themed() -> Option<PathBuf> {
    let mut dirs = vec![glib::user_data_dir().join("sounds")];
    dirs.extend(
        glib::system_data_dirs()
            .into_iter()
            .map(|dir| dir.join("sounds")),
    );
    let theme = gtk::Settings::default()
        .and_then(|settings| settings.gtk_sound_theme_name())
        .map(|name| name.to_string());
    let mut themes: Vec<&str> = theme.iter().map(String::as_str).collect();
    if !themes.contains(&FALLBACK_THEME) {
        themes.push(FALLBACK_THEME);
    }
    find(&dirs, &themes, |path| path.is_file())
}

fn find(dirs: &[PathBuf], themes: &[&str], exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    themes.iter().find_map(|theme| {
        NAMES.iter().find_map(|name| {
            dirs.iter().find_map(|dir| {
                EXTENSIONS
                    .iter()
                    .map(|extension| {
                        dir.join(theme)
                            .join("stereo")
                            .join(format!("{name}.{extension}"))
                    })
                    .find(|path| exists(path))
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/home/me/.local/share/sounds"),
            PathBuf::from("/usr/share/sounds"),
        ]
    }

    #[test]
    fn prefers_the_screen_capture_sound_of_the_user_theme() {
        let found = find(&dirs(), &["ocean", "freedesktop"], |path| {
            path == Path::new("/usr/share/sounds/ocean/stereo/screen-capture.oga")
                || path == Path::new("/usr/share/sounds/freedesktop/stereo/screen-capture.oga")
        });
        assert_eq!(
            found,
            Some(PathBuf::from(
                "/usr/share/sounds/ocean/stereo/screen-capture.oga"
            ))
        );
    }

    #[test]
    fn falls_back_to_the_camera_shutter_sound() {
        let found = find(&dirs(), &["freedesktop"], |path| {
            path == Path::new("/home/me/.local/share/sounds/freedesktop/stereo/camera-shutter.wav")
        });
        assert_eq!(
            found,
            Some(PathBuf::from(
                "/home/me/.local/share/sounds/freedesktop/stereo/camera-shutter.wav"
            ))
        );
    }

    #[test]
    fn finds_nothing_without_a_sound_theme() {
        assert_eq!(find(&dirs(), &["freedesktop"], |_| false), None);
    }
}
