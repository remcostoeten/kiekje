mod about;
mod danger;
mod general;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::app::APP_ID;
use crate::config::{self, Config, ConfigError, Notifications, Storage};
use crate::shortcuts::{
    Modifiers, Recorded, ShortcutAction, ShortcutHost, Shortcuts, Slot, Trigger, hyprland, record,
};
use crate::storage;
use crate::ui;

pub struct Model {
    pub shortcuts: Shortcuts,
    pub host: ShortcutHost,
    pub bound: Vec<(ShortcutAction, String)>,
    pub storage: Storage,
    pub notifications: Notifications,
}

type SavedCallback = Rc<dyn Fn(&Shortcuts)>;

struct Editor {
    host: ShortcutHost,
    shortcuts: RefCell<Shortcuts>,
    recording: Cell<Option<ShortcutAction>>,
    saving: Cell<bool>,
    buttons: HashMap<&'static str, gtk::Button>,
    problem: gtk::Label,
    on_saved: SavedCallback,
}

/// Opens the settings window, or brings the open one to the front.
///
/// Saving and notification changes are written to the config right away. On Hyprland each shortcut can be recorded and is saved to the config and the managed bind
/// file. Where the desktop owns the keys, the rows show its bindings and say where to change them.
///
/// # Examples
///
/// ```ignore
/// let (window, handle) = settings::show(&app, model, save, configure, reload_gallery);
/// ```
pub fn show(
    app: &gtk::Application,
    model: Model,
    on_saved: impl Fn(&Shortcuts) + 'static,
    on_configure: impl Fn() + 'static,
    on_cleared: impl Fn() + 'static,
) -> (gtk::Window, Handle) {
    ui::load_stylesheet();
    let window = gtk::Window::builder()
        .application(app)
        .title("Kiekje settings")
        .default_width(560)
        .default_height(640)
        .css_classes(["settings-window"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(["panel", "settings"])
        .build();
    let general = general::append(&content, &model.storage, model.notifications);
    content.append(&caps("Shortcuts"));
    content.append(&note(&host_note(&model.host)));

    let editable = model.host == ShortcutHost::Hyprland;
    let mut buttons = HashMap::new();
    let rows = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build();
    let mut resets = Vec::new();
    for action in ShortcutAction::ALL {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();
        row.append(
            &gtk::Label::builder()
                .label(action.description())
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );
        let trigger = gtk::Button::builder()
            .label(trigger_text(&model, action))
            .sensitive(editable)
            .css_classes(["trigger"])
            .build();
        row.append(&trigger);
        let reset = gtk::Button::builder()
            .label("Reset")
            .visible(editable)
            .css_classes(["ghost"])
            .build();
        row.append(&reset);
        rows.append(&row);
        buttons.insert(action.id(), trigger);
        resets.push((action, reset));
    }
    content.append(&rows);

    let problem = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["problem"])
        .build();
    content.append(&problem);
    if let ShortcutHost::Desktop {
        configurable: true, ..
    } = model.host
    {
        let configure = gtk::Button::builder()
            .label("Change in desktop settings")
            .halign(gtk::Align::Start)
            .build();
        configure.connect_clicked(move |_| on_configure());
        content.append(&configure);
    }
    if editable {
        content.append(&note(
            "Click a shortcut, then press the new keys. Esc cancels, Backspace turns it off.",
        ));
    }
    window.set_child(Some(
        &gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    ));

    let editor = Rc::new(Editor {
        host: model.host,
        shortcuts: RefCell::new(model.shortcuts),
        recording: Cell::new(None),
        saving: Cell::new(false),
        buttons,
        problem,
        on_saved: Rc::new(on_saved),
    });
    for action in ShortcutAction::ALL {
        if let Some(button) = editor.buttons.get(action.id()) {
            let weak = Rc::downgrade(&editor);
            button.connect_clicked(move |_| {
                if let Some(editor) = weak.upgrade() {
                    editor.start(action);
                }
            });
        }
    }
    for (action, reset) in resets {
        let weak = Rc::downgrade(&editor);
        reset.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                let slot = Trigger::parse(action.default_trigger()).map_or(Slot::Off, Slot::Key);
                editor.save(action, slot);
            }
        });
    }

    let reset_editor = Rc::downgrade(&editor);
    let on_done: danger::DoneCallback = Rc::new(move |done| match done {
        danger::Danger::ResetSettings => {
            let defaults = Config::default();
            if let Some(general) = general.upgrade() {
                general.reset(&defaults);
            }
            if let Some(editor) = reset_editor.upgrade() {
                editor.shortcuts.replace(defaults.shortcuts);
                editor.stop();
                (editor.on_saved)(&editor.shortcuts.borrow());
            }
        }
        danger::Danger::ClearGallery | danger::Danger::ClearThumbnails => on_cleared(),
    });
    about::append(&content);
    danger::append(&content, editor.host == ShortcutHost::Hyprland, on_done);

    let handle = Handle(Rc::downgrade(&editor));
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |controller, keyval, keycode, state| {
        let Some(action) = editor.recording.get() else {
            return glib::Propagation::Proceed;
        };
        let key = key_name(controller, keyval, keycode);
        match record(&key, modifiers(state)) {
            Recorded::Waiting => {}
            Recorded::Canceled => editor.stop(),
            Recorded::Cleared => editor.save(action, Slot::Off),
            Recorded::Done(Ok(trigger)) => editor.save(action, Slot::Key(trigger)),
            Recorded::Done(Err(error)) => editor.complain(&error.to_string()),
        }
        glib::Propagation::Stop
    });
    window.add_controller(keys);

    window.present();
    (window, handle)
}

pub struct Handle(Weak<Editor>);

impl Handle {
    /// Shows the triggers the desktop reports, after it changed them.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.show_bound(&[(ShortcutAction::CaptureRegion, "Print".into())]);
    /// ```
    pub fn show_bound(&self, bound: &[(ShortcutAction, String)]) {
        let Some(editor) = self.0.upgrade() else {
            return;
        };
        for action in ShortcutAction::ALL {
            if let Some(button) = editor.buttons.get(action.id()) {
                button.set_label(&bound_text(bound, action));
            }
        }
    }
}

impl Editor {
    fn start(&self, action: ShortcutAction) {
        if self.saving.get() {
            return;
        }
        self.stop();
        self.recording.set(Some(action));
        self.complain("");
        if let Some(button) = self.buttons.get(action.id()) {
            button.set_label("Press keys");
            button.add_css_class("recording");
        }
    }

    fn stop(&self) {
        self.recording.set(None);
        let shortcuts = self.shortcuts.borrow();
        for action in ShortcutAction::ALL {
            if let Some(button) = self.buttons.get(action.id()) {
                button.set_label(&slot_text(shortcuts.slot(action)));
                button.remove_css_class("recording");
            }
        }
    }

    fn complain(&self, message: &str) {
        self.problem.set_label(message);
        self.problem.set_visible(!message.is_empty());
    }

    fn save(self: &Rc<Self>, action: ShortcutAction, slot: Slot) {
        if self.saving.replace(true) {
            return;
        }
        self.recording.set(None);
        let hyprland = self.host == ShortcutHost::Hyprland;
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let saved = gio::spawn_blocking(move || store(action, slot, hyprland)).await;
            editor.saving.set(false);
            match saved {
                Ok(Ok(shortcuts)) => {
                    editor.shortcuts.replace(shortcuts);
                    editor.stop();
                    editor.complain("");
                    (editor.on_saved)(&editor.shortcuts.borrow());
                }
                Ok(Err(message)) => {
                    editor.stop();
                    editor.complain(&message);
                }
                Err(_) => {
                    editor.stop();
                    editor.complain("The settings worker stopped; nothing was saved.");
                }
            }
        });
    }
}

fn store(action: ShortcutAction, slot: Slot, hyprland: bool) -> Result<Shortcuts, String> {
    let shortcuts =
        config::set_shortcut(&config::path(), action, slot).map_err(|error| match error {
            ConfigError::Shortcuts { source, .. } => {
                let owner = if source.first == action {
                    source.second
                } else {
                    source.first
                };
                format!(
                    "{} is already set for {}.",
                    source.trigger.label(),
                    owner.description()
                )
            }
            other => other.to_string(),
        })?;
    if hyprland {
        let include = hyprland::include_path();
        hyprland::write_include(&include, APP_ID, &shortcuts).map_err(|error| {
            format!(
                "Saved to the config, but {} could not be written: {error}",
                storage::tilde(&include)
            )
        })?;
    }
    Ok(shortcuts)
}

fn host_note(host: &ShortcutHost) -> String {
    match host {
        ShortcutHost::Hyprland => format!(
            "Saved to {}, which Hyprland reloads by itself.",
            storage::tilde(&hyprland::include_path())
        ),
        ShortcutHost::Desktop {
            name,
            configurable: true,
        } => format!("Your desktop owns these keys through {name}. Change them in its dialog."),
        ShortcutHost::Desktop {
            name,
            configurable: false,
        } => format!(
            "Your desktop owns these keys through {name}. Kiekje suggested them once; change them in the desktop's shortcut settings."
        ),
        ShortcutHost::Off(reason) => format!("Global shortcuts are off: {reason}"),
        ShortcutHost::Starting => "Global shortcuts are still starting.".to_owned(),
    }
}

fn trigger_text(model: &Model, action: ShortcutAction) -> String {
    if let ShortcutHost::Desktop { .. } = model.host {
        return bound_text(&model.bound, action);
    }
    slot_text(model.shortcuts.slot(action))
}

fn bound_text(bound: &[(ShortcutAction, String)], action: ShortcutAction) -> String {
    bound
        .iter()
        .find(|(bound, _)| *bound == action)
        .map_or_else(|| "Not set".to_owned(), |(_, label)| label.clone())
}

fn slot_text(slot: &Slot) -> String {
    slot.trigger()
        .map_or_else(|| "Off".to_owned(), Trigger::label)
}

fn modifiers(state: gdk::ModifierType) -> Modifiers {
    Modifiers {
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        logo: state.intersects(gdk::ModifierType::SUPER_MASK | gdk::ModifierType::META_MASK),
    }
}

fn key_name(controller: &gtk::EventControllerKey, keyval: gdk::Key, keycode: u32) -> String {
    let group = i32::try_from(controller.group()).unwrap_or(0);
    let unshifted = controller
        .widget()
        .and_then(|widget| {
            widget
                .display()
                .translate_key(keycode, gdk::ModifierType::empty(), group)
        })
        .map_or(keyval, |(key, ..)| key);
    unshifted
        .to_lower()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_default()
}

fn caps(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["caps"])
        .build()
}

fn note(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["note"])
        .build()
}
