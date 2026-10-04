use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::{caps, note};
use crate::config;
use crate::library::{self, thumbs};
use crate::storage;
use crate::ui::files;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) fn append(content: &gtk::Box) {
    content.append(&caps("About"));
    let title = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
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
    content.append(&title);
    content.append(&note(
        "Captures the screen at the moment you ask, then lets you pick, mark up and keep the part you need.",
    ));

    let places = [
        ("Config", config::path()),
        ("Library", library::path()),
        ("Thumbnails", thumbs::dir()),
    ];
    for (label, path) in places {
        content.append(&place(label, path));
    }

    let status = note("");
    status.set_visible(false);
    let copy = gtk::Button::with_label("Copy doctor report");
    copy.set_halign(gtk::Align::Start);
    copy.set_tooltip_text(Some("Copies what this session supports, for a bug report"));
    let shown = status.clone();
    copy.connect_clicked(move |button| copy_report(button, &shown));
    content.append(&copy);
    content.append(&status);
}

fn place(label: &str, path: PathBuf) -> gtk::Box {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    row.append(
        &gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .width_chars(10)
            .build(),
    );
    row.append(
        &gtk::Label::builder()
            .label(storage::tilde(&path))
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .css_classes(["value"])
            .build(),
    );
    let show = gtk::Button::builder()
        .label("Show")
        .css_classes(["ghost"])
        .tooltip_text(format!(
            "Show {} in the file manager",
            storage::tilde(&path)
        ))
        .build();
    show.connect_clicked(move |_| {
        glib::spawn_future_local(files::reveal(path.clone()));
    });
    row.append(&show);
    row
}

fn copy_report(button: &gtk::Button, status: &gtk::Label) {
    button.set_sensitive(false);
    let button = button.clone();
    let status = status.clone();
    glib::spawn_future_local(async move {
        let message = match doctor_report().await {
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

async fn doctor_report() -> Result<String, glib::Error> {
    let binary = std::env::current_exe()
        .map_err(|error| glib::Error::new(gio::IOErrorEnum::NotFound, &error.to_string()))?;
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    );
    launcher.setenv("NO_COLOR", "1", true);
    let process = launcher.spawn(&[binary.as_os_str(), "doctor".as_ref()])?;
    let (stdout, _) = process.communicate_utf8_future(None).await?;
    Ok(stdout.map(|text| text.to_string()).unwrap_or_default())
}
