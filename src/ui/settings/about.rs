use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::library::{self, thumbs};
use crate::storage;
use crate::ui::files;
use crate::ui::widgets::{group, note, path_row, row};
use crate::update::{self, Available, Outcome, Version, state};

pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) fn append(page: &gtk::Box) {
    let header = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(["about"])
        .build();
    let title = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    title.append(
        &gtk::Label::builder()
            .label("Kiekje")
            .xalign(0.0)
            .css_classes(["about-name"])
            .build(),
    );
    title.append(
        &gtk::Label::builder()
            .label(VERSION)
            .xalign(0.0)
            .valign(gtk::Align::Baseline)
            .css_classes(["value"])
            .build(),
    );
    header.append(&title);
    header.append(&note(
        "Captures the screen at the moment you ask, then lets you pick, mark up and keep the part you need.",
    ));
    page.append(&header);

    let places = group(page, "On this computer");
    for (label, path) in [
        ("Config", config::path()),
        ("Library", library::path()),
        ("Thumbnails", thumbs::dir()),
    ] {
        places.add(place(label, path));
    }

    append_updates(page);

    let support = group(page, "Support");
    let status = note("");
    status.set_visible(false);
    let copy = gtk::Button::with_label("Copy");
    let shown = status.clone();
    copy.connect_clicked(move |button| copy_report(button, &shown));
    support.add(row(
        "Doctor report",
        Some("What this session supports, for a bug report."),
        Some(&copy),
    ));
    support.footer(&status);
}

fn place(label: &str, path: PathBuf) -> gtk::ListBoxRow {
    let tilde = storage::tilde(&path);
    let show = gtk::Button::builder()
        .label("Show")
        .css_classes(["ghost"])
        .tooltip_text(format!("Show {tilde} in the file manager"))
        .build();
    show.connect_clicked(move |_| {
        glib::spawn_future_local(files::reveal(path.clone()));
    });
    path_row(label, &tilde, Some(&show))
}

fn copy_report(button: &gtk::Button, status: &gtk::Label) {
    button.set_sensitive(false);
    let button = button.clone();
    let status = status.clone();
    glib::spawn_future_local(async move {
        let message = match super::doctor::report().await {
            Ok(report) => {
                button.clipboard().set_text(&report);
                format!(
                    "Copied the doctor report ({} lines).",
                    report.lines().count()
                )
            }
            Err(error) => format!("The doctor report failed: {error}"),
        };
        button.set_sensitive(true);
        status.set_label(&message);
        status.set_visible(true);
    });
}

struct Updates {
    check: gtk::Button,
    open: gtk::Button,
    status: gtk::Label,
    link: RefCell<Option<String>>,
}

fn append_updates(page: &gtk::Box) {
    let releases = group(page, "Updates");
    let check = gtk::Button::with_label("Check now");
    let open = gtk::Button::builder()
        .label("Release page")
        .css_classes(["ghost"])
        .visible(false)
        .build();
    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    controls.append(&open);
    controls.append(&check);
    releases.add(row(
        "Newer release",
        Some("Asks api.github.com once, even when the daily check is off."),
        Some(&controls),
    ));
    let status = note("");
    status.set_visible(false);
    releases.footer(&status);

    let updates = Rc::new(Updates {
        check,
        open,
        status,
        link: RefCell::new(None),
    });
    let shown = updates.clone();
    updates.open.connect_clicked(move |_| {
        if let Some(url) = shown.link.borrow().clone() {
            glib::spawn_future_local(files::open_link(url));
        }
    });
    let checking = updates.clone();
    updates
        .check
        .connect_clicked(move |_| check_now(checking.clone()));

    glib::spawn_future_local(async move {
        if let Ok(Ok(stored)) = gio::spawn_blocking(|| state::load_from(&state::path())).await {
            updates.show_stored(&stored);
        }
    });
}

fn check_now(updates: Rc<Updates>) {
    updates.check.set_sensitive(false);
    updates.say("Checking for updates.");
    glib::spawn_future_local(async move {
        let checked = gio::spawn_blocking(|| update::check_now(&state::path())).await;
        updates.check.set_sensitive(true);
        match checked {
            Ok(Ok(Outcome::Newer(available))) => updates.show_newer(&available),
            Ok(Ok(Outcome::UpToDate)) => {
                updates.hide_link();
                updates.say(&format!("Kiekje {VERSION} is the newest release."));
            }
            Ok(Err(error)) => {
                error!("could not check for updates: {error}");
                updates.say("Could not check for updates.");
            }
            Err(_) => updates.say("Could not check for updates."),
        }
    });
}

impl Updates {
    fn say(&self, message: &str) {
        self.status.set_label(message);
        self.status.set_visible(true);
    }

    fn show_newer(&self, available: &Available) {
        self.link.replace(Some(available.url.clone()));
        self.open.set_visible(true);
        self.say(&format!(
            "Kiekje {} is available. This is {VERSION}.",
            available.version
        ));
    }

    fn hide_link(&self) {
        self.link.replace(None);
        self.open.set_visible(false);
    }

    fn show_stored(&self, stored: &state::State) {
        let newer = stored.available.as_ref().filter(|available| {
            match (Version::parse(&available.version), update::current()) {
                (Ok(found), Ok(current)) => found > current,
                _ => false,
            }
        });
        if let Some(available) = newer {
            self.show_newer(available);
            return;
        }
        let checked = stored
            .last_checked
            .and_then(|seconds| i64::try_from(seconds).ok())
            .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok())
            .and_then(|time| time.format("%Y-%m-%d %H:%M").ok());
        if let Some(checked) = checked {
            self.say(&format!("No newer release at the last check, {checked}."));
        }
    }
}
