use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::app::APP_ID;
use crate::config::{self, Config};
use crate::library::{self, Library, thumbs};
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
            Self::ClearGallery => "Forgets every capture in the gallery. The files stay on disk.",
            Self::ClearColours => "Forgets every colour copied from the magnifier.",
            Self::ClearThumbnails => "Deletes cached thumbnails. They are made again when needed.",
        }
    }
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
                    Ok(Err(message)) => message,
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
    button.set_tooltip_text(None);
}

fn run(danger: Danger, host: &ShortcutHost) -> Result<String, String> {
    match danger {
        Danger::ResetSettings => {
            config::save_to(&config::path(), &Config::default())
                .map_err(|error| format!("Settings were not reset: {error}"))?;
            match host {
                ShortcutHost::Hyprland => {
                    let include = hyprland::include_path();
                    hyprland::write_include(&include, APP_ID, &Shortcuts::default()).map_err(
                        |error| {
                            format!(
                                "Settings were reset, but {} could not be written: {error}",
                                storage::tilde(&include)
                            )
                        },
                    )?;
                }
                ShortcutHost::Kde => {
                    let skipped = kde::set_all(APP_ID, &Shortcuts::default()).map_err(|error| {
                        format!("Settings were reset, but KDE kept the old shortcuts: {error}")
                    })?;
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
                .map_err(|error| format!("The gallery was not cleared: {error}"))?;
            Ok(format!(
                "The gallery forgot {}.",
                count(forgotten, "capture")
            ))
        }
        Danger::ClearColours => {
            let forgotten = Library::open(&library::path())
                .and_then(|library| library.clear_colours())
                .map_err(|error| format!("The colours were not cleared: {error}"))?;
            Ok(format!("Forgot {}.", count(forgotten, "colour")))
        }
        Danger::ClearThumbnails => {
            let removed = thumbs::prune(&thumbs::dir(), &[])
                .map_err(|error| format!("Thumbnails were not cleared: {error}"))?;
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
