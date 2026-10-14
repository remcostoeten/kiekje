mod about;
mod clipboard;
mod danger;
mod drawing;
mod general;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::app::APP_ID;
use crate::config::{self, Capture, Config, ConfigError, General, Notifications, Overlay, Storage};
use crate::editor::DrawingKeys;
use crate::shortcuts::{
    Modifiers, Recorded, ShortcutAction, ShortcutHost, Shortcuts, Slot, Trigger, hyprland, kde,
    record,
};
use crate::storage;
use crate::ui;
use crate::ui::widgets::{credit, group, note, problem, row};

pub struct Model {
    pub shortcuts: Shortcuts,
    pub host: ShortcutHost,
    pub bound: Vec<(ShortcutAction, String)>,
    pub storage: Storage,
    pub notifications: Notifications,
    pub capture: Capture,
    pub overlay: Overlay,
    pub general: General,
    pub drawing_keys: DrawingKeys,
}

type SavedCallback = Rc<dyn Fn(&Shortcuts)>;

const WINDOW_WIDTH: i32 = 780;
const WINDOW_HEIGHT: i32 = 600;

pub(super) struct Pages {
    capture: gtk::Box,
    shortcuts: gtk::Box,
    saving: gtk::Box,
    general: gtk::Box,
    about: gtk::Box,
}

impl Pages {
    fn new(stack: &gtk::Stack) -> Self {
        Self {
            capture: page(stack, "capture", "Capture"),
            shortcuts: page(stack, "shortcuts", "Shortcuts"),
            saving: page(stack, "saving", "Saving"),
            general: page(stack, "general", "General"),
            about: page(stack, "about", "About"),
        }
    }
}

fn page(stack: &gtk::Stack, name: &str, title: &str) -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .css_classes(["page"])
        .build();
    page.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title"])
            .build(),
    );
    let scroller = gtk::ScrolledWindow::builder()
        .child(&page)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .css_classes(["main"])
        .build();
    stack.add_titled(&scroller, Some(name), title);
    page
}

fn sidebar(stack: &gtk::Stack) -> gtk::Box {
    let sidebar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(["sidebar"])
        .build();
    sidebar.append(
        &gtk::Label::builder()
            .label("Settings")
            .xalign(0.0)
            .css_classes(["caps", "brand"])
            .build(),
    );
    let mut leader: Option<gtk::ToggleButton> = None;
    for (name, title) in [
        ("capture", "Capture"),
        ("shortcuts", "Shortcuts"),
        ("saving", "Saving"),
        ("general", "General"),
        ("about", "About"),
    ] {
        let button = gtk::ToggleButton::builder()
            .child(&gtk::Label::builder().label(title).xalign(0.0).build())
            .active(leader.is_none())
            .css_classes(["nav"])
            .build();
        match &leader {
            Some(first) => button.set_group(Some(first)),
            None => leader = Some(button.clone()),
        }
        let stack = stack.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                stack.set_visible_child_name(name);
            }
        });
        sidebar.append(&button);
    }
    sidebar.append(
        &gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .build(),
    );
    sidebar.append(
        &gtk::Button::builder()
            .child(&gtk::Label::builder().label("Gallery").xalign(0.0).build())
            .action_name("app.gallery")
            .css_classes(["nav"])
            .build(),
    );
    sidebar.append(&credit());
    sidebar
}

fn default_slot(action: ShortcutAction) -> Slot {
    Trigger::parse(action.default_trigger()).map_or(Slot::Off, Slot::Key)
}

struct Editor {
    host: ShortcutHost,
    shortcuts: RefCell<Shortcuts>,
    bound: RefCell<Vec<(ShortcutAction, String)>>,
    recording: Cell<Option<ShortcutAction>>,
    saving: Cell<bool>,
    buttons: HashMap<&'static str, gtk::Button>,
    resets: HashMap<&'static str, gtk::Button>,
    problem: gtk::Label,
    on_saved: SavedCallback,
}

/// Opens the settings window, or brings the open one to the front.
///
/// Saving and notification changes are written to the config right away. On Hyprland and KDE
/// each shortcut can be recorded; it is saved to the config and to the Hyprland bind file or
/// KDE's global shortcuts. Where another desktop owns the keys, the rows show its bindings and
/// say where to change them.
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
        .default_width(WINDOW_WIDTH)
        .default_height(WINDOW_HEIGHT)
        .css_classes(["settings-window"])
        .build();

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["panel", "settings"])
        .build();
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(120)
        .hexpand(true)
        .vexpand(true)
        .build();
    root.append(&sidebar(&stack));
    root.append(&stack);
    window.set_child(Some(&root));

    let pages = Pages::new(&stack);
    let general = general::append(
        &pages,
        &model.storage,
        model.notifications,
        model.capture,
        model.overlay,
        model.general,
    );
    clipboard::append(&pages.general);

    let keys_group = group(&pages.shortcuts, "Keys");
    let editable = model.host.editable();
    let mut buttons = HashMap::new();
    let mut resets = Vec::new();
    for action in ShortcutAction::ALL {
        let trigger = gtk::Button::builder()
            .label(trigger_text(
                &model.host,
                &model.shortcuts,
                &model.bound,
                action,
            ))
            .sensitive(editable)
            .css_classes(["trigger"])
            .build();
        let reset = gtk::Button::builder()
            .label("Reset")
            .visible(editable)
            .css_classes(["ghost", "optional"])
            .build();
        let controls = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        controls.append(&trigger);
        controls.append(&reset);
        keys_group.add(row(action.description(), None, Some(&controls)));
        buttons.insert(action.id(), trigger);
        resets.push((action, reset));
    }
    keys_group.footer(&note(&host_note(&model.host)));
    if editable {
        keys_group.footer(&note(
            "Click a shortcut, then press the new keys. Esc cancels, Backspace turns it off.",
        ));
    }
    let problem = problem();
    keys_group.footer(&problem);
    if let ShortcutHost::Desktop {
        configurable: true, ..
    } = model.host
    {
        let configure = gtk::Button::builder()
            .label("Change in desktop settings")
            .halign(gtk::Align::Start)
            .build();
        configure.connect_clicked(move |_| on_configure());
        pages.shortcuts.append(&configure);
    }
    let drawing = drawing::append(&pages.shortcuts, &window, model.drawing_keys);

    let editor = Rc::new(Editor {
        host: model.host,
        shortcuts: RefCell::new(model.shortcuts),
        bound: RefCell::new(model.bound),
        recording: Cell::new(None),
        saving: Cell::new(false),
        buttons,
        resets: resets
            .iter()
            .map(|(action, reset)| (action.id(), reset.clone()))
            .collect(),
        problem,
        on_saved: Rc::new(on_saved),
    });
    editor.refresh();
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
                editor.save(action, default_slot(action));
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
            if let Some(drawing) = drawing.upgrade() {
                drawing.reset(defaults.drawing.keys);
            }
            if let Some(editor) = reset_editor.upgrade() {
                editor.shortcuts.replace(defaults.shortcuts);
                editor.stop();
                (editor.on_saved)(&editor.shortcuts.borrow());
            }
        }
        danger::Danger::ClearGallery
        | danger::Danger::ClearColours
        | danger::Danger::ClearThumbnails => on_cleared(),
    });
    about::append(&pages.about);
    danger::append(&pages.about, &editor.host, on_done);

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
        editor.bound.replace(bound.to_vec());
        editor.refresh();
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
        for button in self.buttons.values() {
            button.remove_css_class("recording");
        }
        self.refresh();
    }

    fn refresh(&self) {
        let shortcuts = self.shortcuts.borrow();
        let bound = self.bound.borrow();
        for action in ShortcutAction::ALL {
            if self.recording.get() == Some(action) {
                continue;
            }
            if let Some(button) = self.buttons.get(action.id()) {
                button.set_label(&trigger_text(&self.host, &shortcuts, &bound, action));
            }
            if let Some(reset) = self.resets.get(action.id()) {
                let changed = *shortcuts.slot(action) != default_slot(action);
                reset.set_sensitive(changed);
                reset.set_can_target(changed);
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
        let host = self.host.clone();
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let saved = gio::spawn_blocking(move || store(action, slot, &host)).await;
            editor.saving.set(false);
            match saved {
                Ok(Ok(shortcuts)) => {
                    let label = slot_text(shortcuts.slot(action));
                    editor
                        .bound
                        .borrow_mut()
                        .retain(|(bound, _)| *bound != action);
                    editor.bound.borrow_mut().push((action, label));
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

fn store(action: ShortcutAction, slot: Slot, host: &ShortcutHost) -> Result<Shortcuts, String> {
    let on_kde = *host == ShortcutHost::Kde;
    if on_kde && let Some(trigger) = slot.trigger() {
        kde::check(APP_ID, trigger).map_err(|error| format!("{error}."))?;
    }
    let trigger = slot.trigger().cloned();
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
    if on_kde {
        kde::set(APP_ID, action, trigger.as_ref()).map_err(|error| {
            format!("Saved to the config, but KDE kept the old shortcut: {error}")
        })?;
    }
    if *host == ShortcutHost::Hyprland {
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
        ShortcutHost::Kde => {
            "Saved to KDE's global shortcuts, the same list System Settings shows.".to_owned()
        }
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

fn trigger_text(
    host: &ShortcutHost,
    shortcuts: &Shortcuts,
    bound: &[(ShortcutAction, String)],
    action: ShortcutAction,
) -> String {
    match host {
        ShortcutHost::Desktop { .. } | ShortcutHost::Kde => bound_text(bound, action),
        _ => slot_text(shortcuts.slot(action)),
    }
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
