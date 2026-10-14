use gtk::prelude::*;
use gtk::{gio, glib};

use crate::doctor::clipboard::{self, ClipboardManager};
use crate::ui::widgets::{group, note, row};

pub(super) fn append(page: &gtk::Box) {
    let section = group(page, "Clipboard");
    let found = gtk::Label::builder()
        .label("Looking")
        .xalign(0.0)
        .css_classes(["value"])
        .build();
    section.add(row(
        "Clipboard manager",
        Some("Kiekje holds copied captures while it runs."),
        Some(&found),
    ));
    let explanation = note("");
    explanation.set_visible(false);
    section.footer(&explanation);

    glib::spawn_future_local(async move {
        let detected = gio::spawn_blocking(clipboard::detect).await;
        let (label, text) = match detected {
            Ok(Some(manager)) => (manager.name(), kept(manager)),
            Ok(None) => (
                "None found",
                "No clipboard manager found. Copied captures are lost when Kiekje quits."
                    .to_owned(),
            ),
            Err(_) => (
                "Unknown",
                "The clipboard check stopped, so it is unknown whether copies outlive Kiekje."
                    .to_owned(),
            ),
        };
        found.set_label(label);
        explanation.set_label(&text);
        explanation.set_visible(true);
    });
}

fn kept(manager: ClipboardManager) -> String {
    format!(
        "{} is running and can keep copied captures after Kiekje quits.",
        manager.name()
    )
}
