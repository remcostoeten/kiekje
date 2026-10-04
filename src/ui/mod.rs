pub mod files;
pub mod fonts;
pub mod gallery;
pub mod settings;

use std::cell::Cell;

use gtk::{gdk, gio};

const RESOURCE_PREFIX: &str = "/dev/remcostoeten/KiekjeNext";

thread_local! {
    static LOADED: Cell<bool> = const { Cell::new(false) };
}

/// Registers the bundled resources and applies the app stylesheet to the default display once.
///
/// Call it on the GTK main thread before building the first styled window.
///
/// # Examples
///
/// ```ignore
/// ui::load_stylesheet();
/// ```
pub fn load_stylesheet() {
    if LOADED.with(|loaded| loaded.replace(true)) {
        return;
    }
    gio::resources_register_include!("kiekje.gresource")
        .unwrap_or_else(|error| eprintln!("kiekje: bundled resources are missing: {error}"));
    let Some(display) = gdk::Display::default() else {
        eprintln!("kiekje: no display, the stylesheet is not applied");
        return;
    };
    gtk::Settings::for_display(&display).set_gtk_application_prefer_dark_theme(true);
    let provider = gtk::CssProvider::new();
    provider.connect_parsing_error(|_, section, error| {
        eprintln!("kiekje: style.css {}: {error}", section.to_str());
    });
    provider.load_from_resource(&format!("{RESOURCE_PREFIX}/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
