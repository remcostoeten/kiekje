use std::cell::RefCell;
use std::path::{Path, PathBuf};

use gtk::glib;
use gtk::prelude::*;

use crate::ui::RESOURCE_PREFIX;

const NAMES: [&str; 2] = ["screen-capture", "camera-shutter"];
const FALLBACK_THEME: &str = "freedesktop";
const EXTENSIONS: [&str; 3] = ["oga", "ogg", "wav"];

thread_local! {
    static PLAYER: RefCell<Option<gtk::MediaFile>> = const { RefCell::new(None) };
}

/// Plays the capture sound without waiting for it to finish.
///
/// Uses the sound theme's `screen-capture` or `camera-shutter` sound and falls back to the
/// bundled one. Playback goes through GTK's media backend, so a missing backend only logs.
///
/// # Examples
///
/// ```ignore
/// sound::play_capture();
/// ```
pub fn play_capture() {
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
