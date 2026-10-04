use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::{caps, note};
use crate::app::APP_ID;
use crate::config::{self, Config};
use crate::library::{self, Library, thumbs};
use crate::shortcuts::{Shortcuts, hyprland};
use crate::storage;

const ARMED_FOR: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Danger {
    ResetSettings,
    ClearGallery,
    ClearThumbnails,
}

impl Danger {
    const ALL: [Self; 3] = [
        Self::ResetSettings,
        Self::ClearGallery,
        Self::ClearThumbnails,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ResetSettings => "Reset settings",
            Self::ClearGallery => "Clear gallery",
            Self::ClearThumbnails => "Clear thumbnails",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::ResetSettings => {
                "Puts the folder, file name, notifications and shortcuts back to their defaults."
            }
            Self::ClearGallery => "Forgets every capture in the gallery. The files stay on disk.",
            Self::ClearThumbnails => "Deletes cached thumbnails. They are made again when needed.",
        }
    }
}

pub(super) type DoneCallback = Rc<dyn Fn(Danger)>;

pub(super) fn append(content: &gtk::Box, hyprland: bool, on_done: DoneCallback) {
    content.append(&caps("Danger zone"));
    let result = note("");
    result.set_visible(false);
    for danger in Danger::ALL {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();
        let detail = note(danger.detail());
        detail.set_hexpand(true);
        row.append(&detail);
        let button = gtk::Button::builder()
            .label(danger.label())
            .css_classes(["danger"])
            .build();
        let result = result.clone();
        let on_done = on_done.clone();
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
            glib::spawn_future_local(async move {
                let outcome = gio::spawn_blocking(move || run(danger, hyprland)).await;
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
        row.append(&button);
        content.append(&row);
    }
    content.append(&result);
}

fn arm(button: &gtk::Button, danger: Danger) {
    button.add_css_class("armed");
    button.set_label("Click again");
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
    button.set_label(danger.label());
    button.set_tooltip_text(None);
}

fn run(danger: Danger, hyprland: bool) -> Result<String, String> {
    match danger {
        Danger::ResetSettings => {
            config::save_to(&config::path(), &Config::default())
                .map_err(|error| format!("Settings were not reset: {error}"))?;
            if hyprland {
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
