use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::Model;
use super::general::{Changes, Page, send};
use crate::capture::Availability;
use crate::config::Config;
use crate::ocr::{self, Language};
use crate::ui::widgets::{Group, group, note, row, switch, tagged_row};

const ABOUT: &str = "Pick Text in the overlay's top bar or press G until it is chosen, then select a region. Ctrl+T does the same while drawing. Tesseract reads the text on this computer and nothing is sent anywhere.";
const NAMES: [(&str, &str); 14] = [
    ("eng", "English"),
    ("nld", "Dutch"),
    ("deu", "German"),
    ("fra", "French"),
    ("spa", "Spanish"),
    ("ita", "Italian"),
    ("por", "Portuguese"),
    ("pol", "Polish"),
    ("swe", "Swedish"),
    ("dan", "Danish"),
    ("nor", "Norwegian"),
    ("fry", "Frisian"),
    ("afr", "Afrikaans"),
    ("tur", "Turkish"),
];

pub(super) struct Text {
    quiet: Cell<bool>,
    group: Group,
    chosen: RefCell<Vec<Language>>,
    switches: RefCell<Vec<(Language, gtk::Switch)>>,
    status: gtk::Label,
}

/// Builds the Text group of the Capture page: how text capture works and a switch per language.
///
/// The installed languages are read from Tesseract off the main thread and added when known.
///
/// # Examples
///
/// ```ignore
/// let text = text::append(&pages.capture, &model, &changes);
/// ```
pub(super) fn append(page: &gtk::Box, model: &Model, changes: &Changes) -> Rc<Text> {
    let group = group(page, "Text");
    let status = note("");
    match model.text_capture {
        Availability::Available => {
            group.add(row(
                "Copy text from a region",
                Some(ABOUT),
                None::<&gtk::Widget>,
            ));
            status.set_label("Looking up the installed languages…");
        }
        Availability::Unavailable(reason) => {
            group.add(row(
                "Copy text from a region",
                Some(&format!(
                    "{reason}. Install tesseract and tesseract-data-eng, then restart Kiekje."
                )),
                None::<&gtk::Widget>,
            ));
            status.set_visible(false);
        }
    }
    group.footer(&status);
    let text = Rc::new(Text {
        quiet: Cell::new(false),
        group,
        chosen: RefCell::new(model.text.languages.clone()),
        switches: RefCell::new(Vec::new()),
        status,
    });
    if model.text_capture.is_available() {
        load_languages(&text, changes);
    }
    text
}

fn load_languages(text: &Rc<Text>, changes: &Changes) {
    let weak = Rc::downgrade(text);
    let changes = changes.clone();
    glib::spawn_future_local(async move {
        let installed = gio::spawn_blocking(ocr::installed_languages).await;
        let Some(text) = weak.upgrade() else {
            return;
        };
        match installed {
            Ok(Ok(installed)) => text.show_languages(&installed, &changes),
            Ok(Err(error)) => text.status.set_label(&error.to_string()),
            Err(_) => error!("the language lookup worker stopped"),
        }
    });
}

impl Text {
    fn show_languages(self: &Rc<Self>, installed: &[Language], changes: &Changes) {
        let chosen = self.chosen.borrow().clone();
        let mut listed: Vec<Language> = installed.to_vec();
        listed.extend(
            chosen
                .iter()
                .filter(|language| !installed.contains(language))
                .cloned(),
        );
        for language in listed {
            let present = installed.contains(&language);
            let toggle = switch(chosen.contains(&language));
            let subtitle = (!present).then_some("No data installed, so text is read without it.");
            self.group.add(tagged_row(
                display_name(&language),
                language.as_str(),
                subtitle,
                Some(&toggle),
            ));
            self.connect(&toggle, changes);
            self.switches.borrow_mut().push((language, toggle));
        }
        self.status.set_label(
            "Read in every language that is on. Fewer languages read faster and guess less.",
        );
    }

    fn connect(self: &Rc<Self>, toggle: &gtk::Switch, changes: &Changes) {
        let weak = Rc::downgrade(self);
        let changes = changes.clone();
        toggle.connect_active_notify(move |toggle| {
            let Some(text) = weak.upgrade() else {
                return;
            };
            if text.quiet.get() {
                return;
            }
            let languages: Vec<Language> = text
                .switches
                .borrow()
                .iter()
                .filter(|(_, toggle)| toggle.is_active())
                .map(|(language, _)| language.clone())
                .collect();
            if languages.is_empty() {
                text.quiet.set(true);
                toggle.set_active(true);
                text.quiet.set(false);
                text.status.set_label("Keep at least one language on.");
                return;
            }
            text.chosen.replace(languages.clone());
            send(
                &changes,
                Page::Capture,
                Box::new(move |config| config.text.languages = languages),
            );
        });
    }

    pub(super) fn reset(&self, config: &Config) {
        self.quiet.set(true);
        self.chosen.replace(config.text.languages.clone());
        for (language, toggle) in self.switches.borrow().iter() {
            toggle.set_active(config.text.languages.contains(language));
        }
        self.quiet.set(false);
    }
}

fn display_name(language: &Language) -> &str {
    NAMES
        .iter()
        .find(|(code, _)| *code == language.as_str())
        .map_or(language.as_str(), |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_known_languages_and_shows_the_code_for_the_rest() {
        assert_eq!(display_name(&Language::english()), "English");
        let klingon = Language::try_from("tlh".to_owned()).unwrap();
        assert_eq!(display_name(&klingon), "tlh");
    }
}
