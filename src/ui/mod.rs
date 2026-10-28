pub mod files;
pub mod fonts;
pub mod gallery;
pub mod settings;
pub mod widgets;

#[cfg(test)]
#[path = "../../build/css.rs"]
mod build_css;

use std::cell::{Cell, RefCell};

use gtk::{gdk, gio, glib};

use crate::config::{self, Config, Theme};

pub const RESOURCE_PREFIX: &str = "/dev/remcostoeten/Kiekje";

thread_local! {
    static LOADED: Cell<bool> = const { Cell::new(false) };
    static THEME: Cell<Theme> = const { Cell::new(Theme::Dark) };
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
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
        .unwrap_or_else(|error| warn!("bundled resources are missing: {error}"));
    let Some(display) = gdk::Display::default() else {
        warn!("no display, the stylesheet is not applied");
        return;
    };
    gtk::IconTheme::for_display(&display).add_resource_path(&format!("{RESOURCE_PREFIX}/icons"));
    let settings = gtk::Settings::for_display(&display);
    settings.set_gtk_theme_name(Some("Default"));
    let provider = gtk::CssProvider::new();
    provider.connect_parsing_error(|_, section, error| {
        warn!("style.css {}: {error}", section.to_str());
    });
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    PROVIDER.with(|slot| slot.replace(Some(provider)));
    apply_theme();
}

/// Switches every open and future window to `theme`, before or after the stylesheet is loaded.
///
/// Call it on the GTK main thread, for example when the saved config changes.
///
/// # Examples
///
/// ```ignore
/// ui::set_theme(config.general.theme);
/// ```
pub fn set_theme(theme: Theme) {
    if THEME.with(|current| current.replace(theme)) != theme {
        apply_theme();
    }
}

/// Applies the theme saved in the config file without blocking the main loop.
///
/// # Examples
///
/// ```ignore
/// ui::apply_saved_theme();
/// ```
pub fn apply_saved_theme() {
    glib::spawn_future_local(async {
        match gio::spawn_blocking(config::load).await {
            Ok(config) => set_theme(config.general.theme),
            Err(_) => error!("the config worker stopped"),
        }
    });
}

/// Flips the saved theme between dark and light and applies the result.
///
/// The change is written to the config file, so it survives a restart.
///
/// # Examples
///
/// ```ignore
/// ui::toggle_theme();
/// ```
pub fn toggle_theme() {
    glib::spawn_future_local(async {
        let saved = gio::spawn_blocking(|| {
            config::update(&config::path(), |config: &mut Config| {
                config.general.theme = config.general.theme.toggled();
            })
        })
        .await;
        match saved {
            Ok(Ok(config)) => set_theme(config.general.theme),
            Ok(Err(error)) => warn!("theme not saved: {error}"),
            Err(_) => error!("the config worker stopped"),
        }
    });
}

fn apply_theme() {
    let theme = THEME.with(Cell::get);
    let Some(display) = gdk::Display::default() else {
        return;
    };
    gtk::Settings::for_display(&display)
        .set_gtk_application_prefer_dark_theme(theme == Theme::Dark);
    let sheet = match theme {
        Theme::Dark => "style.css",
        Theme::Light => "style-light.css",
    };
    PROVIDER.with(|slot| {
        if let Some(provider) = slot.borrow().as_ref() {
            provider.load_from_resource(&format!("{RESOURCE_PREFIX}/{sheet}"));
        }
    });
}
