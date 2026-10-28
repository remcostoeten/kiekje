use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::config::{self, ConfigError};
use crate::editor::{DrawAction, DrawKey, DrawingKeys};
use crate::ui::widgets::{group, note, problem, row};

pub(super) struct Panel {
    keys: RefCell<DrawingKeys>,
    recording: Cell<Option<DrawAction>>,
    saving: Cell<bool>,
    buttons: Vec<(DrawAction, gtk::Button)>,
    resets: Vec<(DrawAction, gtk::Button)>,
    problem: gtk::Label,
}

/// Adds the Drawing keys group to the Shortcuts page and listens for keys while one records.
///
/// Each drawing action gets one key pressed without modifiers. Keys are saved to the config
/// right away and apply to the next editor that opens.
///
/// # Examples
///
/// ```ignore
/// let panel = drawing::append(&pages.shortcuts, &window, config.drawing.keys);
/// ```
pub(super) fn append(page: &gtk::Box, window: &gtk::Window, keys: DrawingKeys) -> Weak<Panel> {
    let keys_group = group(page, "Drawing keys");
    let mut buttons = Vec::new();
    let mut resets = Vec::new();
    for action in DrawAction::ALL {
        let trigger = gtk::Button::builder()
            .label(keys.get(action).label())
            .css_classes(["trigger"])
            .build();
        trigger.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}: {}",
            action.label(),
            keys.get(action).label()
        ))]);
        let reset = gtk::Button::builder()
            .label("Reset")
            .css_classes(["ghost", "optional"])
            .build();
        reset.update_property(&[gtk::accessible::Property::Label(&format!(
            "Reset {}",
            action.label()
        ))]);
        let controls = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        controls.append(&trigger);
        controls.append(&reset);
        keys_group.add(row(action.label(), None, Some(&controls)));
        buttons.push((action, trigger));
        resets.push((action, reset));
    }
    keys_group.footer(&note(
        "Click a key, then press the new one. Esc cancels, Backspace turns it off. Right-click the canvas for the same actions. Changes apply to the next editor you open.",
    ));
    let problem = problem();
    keys_group.footer(&problem);

    let panel = Rc::new(Panel {
        keys: RefCell::new(keys),
        recording: Cell::new(None),
        saving: Cell::new(false),
        buttons,
        resets,
        problem,
    });
    panel.refresh();
    for (action, button) in &panel.buttons {
        let (action, weak) = (*action, Rc::downgrade(&panel));
        button.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.start(action);
            }
        });
    }
    for (action, reset) in &panel.resets {
        let (action, weak) = (*action, Rc::downgrade(&panel));
        reset.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.save(action, action.default_key());
            }
        });
    }

    let handle = Rc::downgrade(&panel);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, keyval, _, state| {
        let Some(action) = panel.recording.get() else {
            return glib::Propagation::Proceed;
        };
        panel.pressed(action, keyval.to_lower(), state);
        glib::Propagation::Stop
    });
    window.add_controller(keys);
    handle
}

impl Panel {
    pub(super) fn reset(&self, keys: DrawingKeys) {
        self.keys.replace(keys);
        self.stop();
        self.complain("");
    }

    fn pressed(self: &Rc<Self>, action: DrawAction, key: gdk::Key, state: gdk::ModifierType) {
        match key {
            gdk::Key::Escape => return self.stop(),
            gdk::Key::BackSpace | gdk::Key::Delete => return self.save(action, DrawKey::Off),
            _ => {}
        }
        let Some(typed) = key.to_unicode().filter(|typed| !typed.is_control()) else {
            if !is_modifier(key) {
                self.complain("Use a letter, a digit or punctuation.");
            }
            return;
        };
        if state.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        ) {
            self.complain("Drawing keys work without Ctrl, Alt or Super.");
            return;
        }
        match DrawKey::new(typed) {
            Ok(key) => self.save(action, key),
            Err(error) => self.complain(&format!("{error}.")),
        }
    }

    fn start(&self, action: DrawAction) {
        if self.saving.get() {
            return;
        }
        self.stop();
        self.recording.set(Some(action));
        self.complain("");
        if let Some((_, button)) = self.buttons.iter().find(|(each, _)| *each == action) {
            button.set_label("Press a key");
            button.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}: press a key",
                action.label()
            ))]);
            button.add_css_class("recording");
        }
    }

    fn stop(&self) {
        self.recording.set(None);
        for (_, button) in &self.buttons {
            button.remove_css_class("recording");
        }
        self.refresh();
    }

    fn refresh(&self) {
        let keys = self.keys.borrow();
        for (action, button) in &self.buttons {
            if self.recording.get() != Some(*action) {
                let key = keys.get(*action).label();
                button.set_label(&key);
                button.update_property(&[gtk::accessible::Property::Label(&format!(
                    "{}: {key}",
                    action.label()
                ))]);
            }
        }
        for (action, reset) in &self.resets {
            let changed = !keys.is_default(*action);
            reset.set_sensitive(changed);
            reset.set_can_target(changed);
        }
    }

    fn complain(&self, message: &str) {
        self.problem.set_label(message);
        self.problem.set_visible(!message.is_empty());
    }

    fn save(self: &Rc<Self>, action: DrawAction, key: DrawKey) {
        if self.saving.replace(true) {
            return;
        }
        self.recording.set(None);
        let panel = self.clone();
        glib::spawn_future_local(async move {
            let saved =
                gio::spawn_blocking(move || config::set_drawing_key(&config::path(), action, key))
                    .await;
            panel.saving.set(false);
            match saved {
                Ok(Ok(keys)) => {
                    panel.keys.replace(keys);
                    panel.stop();
                    panel.complain("");
                }
                Ok(Err(ConfigError::Keys { source, .. })) => {
                    let owner = if source.first == action {
                        source.second
                    } else {
                        source.first
                    };
                    panel.stop();
                    panel.complain(&format!(
                        "{} is already set for {}.",
                        source.key,
                        owner.label()
                    ));
                }
                Ok(Err(error)) => {
                    panel.stop();
                    panel.complain(&error.to_string());
                }
                Err(_) => {
                    panel.stop();
                    panel.complain("The settings worker stopped; nothing was saved.");
                }
            }
        });
    }
}

fn is_modifier(key: gdk::Key) -> bool {
    let name = key.name().unwrap_or_default();
    ["_L", "_R", "_Shift", "_Lock"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}
