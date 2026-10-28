use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::Gallery;
use crate::library::{self, Library};
use crate::storage::{self, RenameError};

#[derive(Debug)]
enum Outcome {
    Renamed(PathBuf),
    Refused(RenameError),
    Failed(String),
}

impl Gallery {
    pub(super) fn rename(self: &Rc<Self>, path: PathBuf) {
        let Some(id) = self
            .details
            .borrow()
            .get(path.to_string_lossy().as_ref())
            .map(|capture| capture.id)
        else {
            return;
        };
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let suffix = path
            .extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()))
            .unwrap_or_default();

        let window = gtk::Window::builder()
            .title("Rename the capture")
            .modal(true)
            .resizable(false)
            .default_width(420)
            .css_classes(["settings-window"])
            .build();
        window.set_transient_for(self.status.root().and_downcast::<gtk::Window>().as_ref());
        let focus_gallery = Rc::downgrade(self);
        window.connect_close_request(move |_| {
            if let Some(gallery) = focus_gallery.upgrade() {
                glib::idle_add_local_once(move || gallery.focus_selected());
            }
            glib::Propagation::Proceed
        });
        let body = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .css_classes(["page"])
            .build();
        let frame = gtk::Box::builder()
            .css_classes(["panel", "settings", "gallery"])
            .build();
        frame.append(&body);
        let entry = gtk::Entry::builder()
            .text(&stem)
            .activates_default(true)
            .hexpand(true)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label("New name")]);
        let field = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        field.append(&entry);
        field.append(
            &gtk::Label::builder()
                .label(&suffix)
                .css_classes(["note"])
                .build(),
        );
        body.append(&field);
        let problem = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .visible(false)
            .css_classes(["problem"])
            .build();
        body.append(&problem);
        let buttons = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .build();
        let cancel = gtk::Button::builder()
            .label("Cancel")
            .css_classes(["ghost"])
            .build();
        let accept = gtk::Button::builder()
            .label("Rename")
            .css_classes(["primary"])
            .build();
        buttons.append(&cancel);
        buttons.append(&accept);
        body.append(&buttons);
        window.set_child(Some(&frame));
        window.set_default_widget(Some(&accept));

        let escape = gtk::ShortcutController::new();
        if let Some(trigger) = gtk::ShortcutTrigger::parse_string("Escape") {
            escape.add_shortcut(gtk::Shortcut::new(
                Some(trigger),
                Some(gtk::NamedAction::new("window.close")),
            ));
        }
        window.add_controller(escape);
        let closing = window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(window) = closing.upgrade() {
                window.close();
            }
        });

        let gallery = self.clone();
        let dialog = window.downgrade();
        let field = entry.downgrade();
        accept.connect_clicked(move |button| {
            let (Some(window), Some(entry)) = (dialog.upgrade(), field.upgrade()) else {
                return;
            };
            let stem = entry.text().to_string();
            let source = path.clone();
            let gallery = gallery.clone();
            let problem = problem.clone();
            let button = button.clone();
            button.set_sensitive(false);
            glib::spawn_future_local(async move {
                let outcome = gio::spawn_blocking(move || apply(&source, &stem, id)).await;
                button.set_sensitive(true);
                match outcome {
                    Ok(Outcome::Renamed(target)) => {
                        window.close();
                        gallery.say(&format!("Renamed to {}", file_name(&target)), false);
                        gallery
                            .pending
                            .replace(Some(target.to_string_lossy().into_owned()));
                        gallery.reload();
                    }
                    Ok(Outcome::Refused(error)) => {
                        problem.set_label(&capitalised(&error.to_string()));
                        problem.set_visible(true);
                    }
                    Ok(Outcome::Failed(reason)) => {
                        problem.set_label(&reason);
                        problem.set_visible(true);
                    }
                    Err(_) => {
                        problem.set_label("The rename worker stopped");
                        problem.set_visible(true);
                    }
                }
            });
        });

        window.present();
        entry.grab_focus();
        entry.select_region(0, -1);
    }
}

fn apply(source: &Path, stem: &str, id: i64) -> Outcome {
    let target = match storage::rename(source, stem) {
        Ok(target) => target,
        Err(error) => return Outcome::Refused(error),
    };
    if target == source {
        return Outcome::Renamed(target);
    }
    let Err(error) =
        Library::open(&library::path()).and_then(|library| library.relink(id, &target))
    else {
        return Outcome::Renamed(target);
    };
    match std::fs::rename(&target, source) {
        Ok(()) => Outcome::Failed(format!("Not renamed: {error}")),
        Err(undo) => Outcome::Failed(format!(
            "Renamed to {}, but the gallery was not updated ({error}) and the old name could not be restored ({undo})",
            file_name(&target)
        )),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}
