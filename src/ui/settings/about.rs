use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::config;
use crate::library::{self, thumbs};
use crate::storage;
use crate::ui::files;
use crate::ui::widgets::{group, note, path_row, row};

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
