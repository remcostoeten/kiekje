use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};
use thiserror::Error;

use crate::config::{self, Config, ConfigError};
use crate::ids::APP_ID;
use crate::library::{self, Library, LibraryError, thumbs};
use crate::shortcuts::kde::KdeError;
use crate::shortcuts::{ShortcutHost, Shortcuts, hyprland, kde};
use crate::storage;
use crate::ui::widgets::{group, problem, row};

const ARMED_FOR: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Danger {
    ResetSettings,
    ClearGallery,
    ClearColours,
    ClearThumbnails,
}

impl Danger {
    const ALL: [Self; 4] = [
        Self::ResetSettings,
        Self::ClearGallery,
        Self::ClearColours,
        Self::ClearThumbnails,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ResetSettings => "Reset settings",
            Self::ClearGallery => "Clear gallery",
            Self::ClearColours => "Clear colours",
            Self::ClearThumbnails => "Clear thumbnails",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::ResetSettings => "Reset",
            Self::ClearGallery | Self::ClearColours | Self::ClearThumbnails => "Clear",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::ResetSettings => {
                "Puts the folder, file name, notifications and shortcuts back to their defaults."
            }
            Self::ClearGallery => {
                "Forgets every capture in the gallery and deletes unsaved ones. Saved files stay on disk."
            }
            Self::ClearColours => "Forgets every colour copied from the magnifier.",
            Self::ClearThumbnails => "Deletes cached thumbnails. They are made again when needed.",
        }
    }
}

#[derive(Debug, Error)]
enum DangerError {
    #[error("Settings were not reset: {0}")]
    ResetConfig(ConfigError),
    #[error(
        "Settings were reset, but {} could not be written: {source}",
        storage::tilde(path)
    )]
    HyprlandInclude { path: PathBuf, source: io::Error },
    #[error("Settings were reset, but KDE kept the old shortcuts: {0}")]
    KdeShortcuts(KdeError),
    #[error("The gallery was not cleared: {0}")]
    ClearGallery(LibraryError),
    #[error("The gallery was cleared, but unsaved captures were not deleted: {0}")]
    DeleteUnsaved(io::Error),
    #[error("The colours were not cleared: {0}")]
    ClearColours(LibraryError),
    #[error("Thumbnails were not cleared: {0}")]
    ClearThumbnails(io::Error),
}

pub(super) type DoneCallback = Rc<dyn Fn(Danger)>;

pub(super) fn append(page: &gtk::Box, host: &ShortcutHost, on_done: DoneCallback) {
    let zone = group(page, "Danger zone");
    let result = problem();
    for danger in Danger::ALL {
        let button = gtk::Button::builder()
            .label(danger.verb())
            .css_classes(["danger"])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(danger.label())]);
        let result = result.clone();
        let on_done = on_done.clone();
        let host = host.clone();
        button.connect_clicked(move |button| {
            if !button.has_css_class("armed") {
                arm(button, danger);
                return;
            }
            disarm(button, danger);
            button.set_sensitive(false);
            let result = result.clone();
            let on_done = on_done.clone();
            let button = button.clone();
            let host = host.clone();
            glib::spawn_future_local(async move {
                let outcome = gio::spawn_blocking(move || run(danger, &host)).await;
                button.set_sensitive(true);
                let message = match outcome {
                    Ok(Ok(message)) => {
                        on_done(danger);
                        message
                    }
                    Ok(Err(error)) => error.to_string(),
                    Err(_) => "The settings worker stopped; nothing was changed.".to_owned(),
                };
                result.set_label(&message);
                result.set_visible(true);
            });
        });
        zone.add(row(danger.label(), Some(danger.detail()), Some(&button)));
    }
    zone.footer(&result);
}

fn arm(button: &gtk::Button, danger: Danger) {
    button.add_css_class("armed");
    button.set_label("Confirm");
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Confirm {}",
        danger.label().to_lowercase()
    ))]);
    button.set_tooltip_text(Some(&format!(
        "Click again to {}",
        danger.label().to_lowercase()
    )));
    let weak = button.downgrade();
    glib::timeout_add_local_once(ARMED_FOR, move || {
        if let Some(button) = weak.upgrade() {
            disarm(&button, danger);
        }
    });
}

fn disarm(button: &gtk::Button, danger: Danger) {
    button.remove_css_class("armed");
    button.set_label(danger.verb());
    button.update_property(&[gtk::accessible::Property::Label(danger.label())]);
    button.set_tooltip_text(None);
}

fn run(danger: Danger, host: &ShortcutHost) -> Result<String, DangerError> {
    match danger {
        Danger::ResetSettings => {
            config::save_to(&config::path(), &Config::default())
                .map_err(DangerError::ResetConfig)?;
            match host {
                ShortcutHost::Hyprland => {
                    let include = hyprland::include_path();
                    hyprland::write_include(&include, APP_ID, &Shortcuts::default()).map_err(
                        |source| DangerError::HyprlandInclude {
                            path: include.clone(),
                            source,
                        },
                    )?;
                }
                ShortcutHost::Kde => {
                    let skipped = kde::set_all(APP_ID, &Shortcuts::default())
                        .map_err(DangerError::KdeShortcuts)?;
                    if !skipped.is_empty() {
                        let reasons: Vec<String> =
                            skipped.iter().map(ToString::to_string).collect();
                        return Ok(format!(
                            "Settings are back to their defaults. Some shortcuts kept their keys: {}.",
                            reasons.join("; ")
                        ));
                    }
                }
                ShortcutHost::Desktop { .. } | ShortcutHost::Off(_) | ShortcutHost::Starting => {}
            }
            Ok("Settings are back to their defaults.".to_owned())
        }
        Danger::ClearGallery => {
            let forgotten = Library::open(&library::path())
                .and_then(|library| library.clear())
                .map_err(DangerError::ClearGallery)?;
            let deleted = storage::delete_files_in(&storage::unsaved_dir())
                .map_err(DangerError::DeleteUnsaved)?;
            Ok(format!(
                "The gallery forgot {} and deleted {}.",
                count(forgotten, "capture"),
                count(deleted, "unsaved capture")
            ))
        }
        Danger::ClearColours => {
            let forgotten = Library::open(&library::path())
                .and_then(|library| library.clear_colours())
                .map_err(DangerError::ClearColours)?;
            Ok(format!("Forgot {}.", count(forgotten, "colour")))
        }
        Danger::ClearThumbnails => {
            let removed =
                thumbs::prune(&thumbs::dir(), &[]).map_err(DangerError::ClearThumbnails)?;
            Ok(format!("Deleted {}.", count(removed, "cached thumbnail")))
        }
    }
}

fn count(amount: usize, noun: &str) -> String {
    match amount {
        1 => format!("1 {noun}"),
        _ => format!("{amount} {noun}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_one_and_many() {
        assert_eq!(count(1, "capture"), "1 capture");
        assert_eq!(count(0, "capture"), "0 captures");
        assert_eq!(count(86, "cached thumbnail"), "86 cached thumbnails");
    }
}
