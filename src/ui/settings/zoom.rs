use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::ui::RESOURCE_PREFIX;

const DEFAULT_PERCENT: u32 = 100;
const MIN_PERCENT: u32 = 70;
const MAX_PERCENT: u32 = 200;
const STEP_PERCENT: u32 = 10;
const WINDOW_SELECTOR: &str = "window.settings-window";

#[derive(Clone, Copy)]
enum Step {
    In,
    Out,
    Reset,
}

struct Zoom {
    percent: Cell<u32>,
    provider: gtk::CssProvider,
    sheet: String,
}

impl Zoom {
    fn apply(&self, step: Step) {
        let percent = match step {
            Step::In => (self.percent.get() + STEP_PERCENT).min(MAX_PERCENT),
            Step::Out => self
                .percent
                .get()
                .saturating_sub(STEP_PERCENT)
                .max(MIN_PERCENT),
            Step::Reset => DEFAULT_PERCENT,
        };
        if percent == self.percent.replace(percent) {
            return;
        }
        self.provider
            .load_from_string(&scaled_font_rules(&self.sheet, percent));
    }
}

/// Lets Ctrl+plus, Ctrl+minus and Ctrl+0 scale the text of one settings window.
///
/// The shortcuts are ignored while `is_recording` returns true so a recorder can capture them.
pub(super) fn install(window: &gtk::Window, is_recording: impl Fn() -> bool + 'static) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let Ok(bytes) = gio::resources_lookup_data(
        &format!("{RESOURCE_PREFIX}/style.css"),
        gio::ResourceLookupFlags::NONE,
    ) else {
        return;
    };
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    let zoom = Rc::new(Zoom {
        percent: Cell::new(DEFAULT_PERCENT),
        provider: provider.clone(),
        sheet: String::from_utf8_lossy(&bytes).into_owned(),
    });

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, keyval, _, state| {
        if is_recording() || !state.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        match step_for(keyval) {
            Some(step) => {
                zoom.apply(step);
                glib::Propagation::Stop
            }
            None => glib::Propagation::Proceed,
        }
    });
    window.add_controller(keys);
    window.connect_destroy(move |_| {
        gtk::style_context_remove_provider_for_display(&display, &provider);
    });
}

fn step_for(keyval: gdk::Key) -> Option<Step> {
    match keyval {
        gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => Some(Step::In),
        gdk::Key::minus | gdk::Key::underscore | gdk::Key::KP_Subtract => Some(Step::Out),
        gdk::Key::_0 | gdk::Key::KP_0 => Some(Step::Reset),
        _ => None,
    }
}

fn scaled_font_rules(sheet: &str, percent: u32) -> String {
    if percent == DEFAULT_PERCENT {
        return String::new();
    }
    let mut rules = String::new();
    let mut rest = sheet;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|offset| open + offset) else {
            break;
        };
        let selectors = rest[..open].trim();
        let body = &rest[open + 1..close];
        rest = &rest[close + 1..];
        let Some(size) = font_size_px(body) else {
            continue;
        };
        let scoped = selectors
            .split(',')
            .map(str::trim)
            .filter(|selector| !selector.is_empty() && !selector.starts_with("window"))
            .map(|selector| format!("{WINDOW_SELECTOR} {selector}"))
            .collect::<Vec<_>>()
            .join(", ");
        if scoped.is_empty() {
            continue;
        }
        let scaled = size * f64::from(percent) / 100.0;
        rules.push_str(&format!("{scoped} {{ font-size: {scaled:.1}px; }}\n"));
    }
    rules
}

fn font_size_px(body: &str) -> Option<f64> {
    body.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        if name.trim() != "font-size" {
            return None;
        }
        value.trim().strip_suffix("px")?.trim().parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_font_sizes_under_the_settings_window() {
        let sheet = ".settings .value {\n  color: red;\n  font-size: 12px;\n}\n";
        assert_eq!(
            scaled_font_rules(sheet, 150),
            "window.settings-window .settings .value { font-size: 18.0px; }\n"
        );
    }

    #[test]
    fn scopes_every_selector_in_a_list_and_skips_windows() {
        let sheet = ".a, .b { font-size: 10px; }\nwindow.other { font-size: 10px; }\n";
        assert_eq!(
            scaled_font_rules(sheet, 80),
            "window.settings-window .a, window.settings-window .b { font-size: 8.0px; }\n"
        );
    }

    #[test]
    fn emits_nothing_at_the_default_zoom() {
        assert_eq!(scaled_font_rules(".a { font-size: 10px; }", 100), "");
    }

    #[test]
    fn ignores_rules_without_a_pixel_font_size() {
        assert_eq!(scaled_font_rules(".a { color: red; }", 120), "");
    }
}
