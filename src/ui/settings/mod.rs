mod about;
mod capture;
mod clipboard;
mod danger;
mod doctor;
mod drawing;
mod general;
mod preview;
mod search;
mod zoom;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use thiserror::Error;

use crate::capture::Availability;
use crate::config::{
    self, Capture, Config, ConfigError, General, Notifications, Overlay, Saving, Storage, Upload,
};
use crate::editor::DrawingKeys;
use crate::ids::APP_ID;
use crate::library::{self, Library};
use crate::shortcuts::kde::KdeError;
use crate::shortcuts::{
    Modifiers, Recorded, ShortcutAction, ShortcutHost, Shortcuts, Slot, hyprland, kde, record,
};
use crate::storage;
use crate::ui;
use crate::ui::widgets::{footer, group, keycaps, note, problem, row};
use search::{Indexed, NavItem, Results, Search};

pub struct Model {
    pub shortcuts: Shortcuts,
    pub host: ShortcutHost,
    pub bound: Vec<(ShortcutAction, String)>,
    pub window_capture: Availability,
    pub storage: Storage,
    pub saving: Saving,
    pub notifications: Notifications,
    pub capture: Capture,
    pub overlay: Overlay,
    pub general: General,
    pub upload: Upload,
    pub drawing_keys: DrawingKeys,
}

type SavedCallback = Rc<dyn Fn(&Shortcuts)>;

const WINDOW_WIDTH: i32 = 960;
const WINDOW_HEIGHT: i32 = 700;
const SIDEBAR_WIDTH: i32 = 228;
const SCROLLED_PAST: f64 = 4.0;
const RESULTS_PAGE: &str = "results";

struct PageSpec {
    name: &'static str,
    title: &'static str,
    subtitle: &'static str,
    icon: &'static str,
}

const PAGE_SPECS: [PageSpec; 6] = [
    PageSpec {
        name: "capture",
        title: "Capture",
        subtitle: "What the overlay does after you press the shortcut. The preview follows your choices.",
        icon: "kiekje-region-symbolic",
    },
    PageSpec {
        name: "shortcuts",
        title: "Shortcuts",
        subtitle: "Global keys that work from anywhere, and the single keys the editor listens for.",
        icon: "kiekje-keyboard-symbolic",
    },
    PageSpec {
        name: "saving",
        title: "Saving",
        subtitle: "Where captures go, what they are called and how long unsaved ones stay.",
        icon: "kiekje-download-symbolic",
    },
    PageSpec {
        name: "general",
        title: "General",
        subtitle: "Appearance, notifications, startup, updates, sharing and the clipboard.",
        icon: "kiekje-sliders-symbolic",
    },
    PageSpec {
        name: "doctor",
        title: "Doctor",
        subtitle: "Every check the doctor runs on this session, with the fix for what is wrong.",
        icon: "kiekje-check-symbolic",
    },
    PageSpec {
        name: "about",
        title: "About",
        subtitle: "Version, where Kiekje keeps its files, support and the danger zone.",
        icon: "kiekje-info-symbolic",
    },
];

pub(super) struct Pages {
    capture: gtk::Box,
    shortcuts: gtk::Box,
    saving: gtk::Box,
    general: gtk::Box,
    doctor: gtk::Box,
    about: gtk::Box,
    indexed: Vec<Indexed>,
}

impl Pages {
    fn new(shell: &Shell) -> Self {
        let indexed: Vec<Indexed> = PAGE_SPECS
            .iter()
            .map(|spec| Indexed {
                name: spec.name,
                title: spec.title,
                content: shell.page(spec.name, spec.title, spec.subtitle).content,
            })
            .collect();
        Self {
            capture: content_of(&indexed, "capture"),
            shortcuts: content_of(&indexed, "shortcuts"),
            saving: content_of(&indexed, "saving"),
            general: content_of(&indexed, "general"),
            doctor: content_of(&indexed, "doctor"),
            about: content_of(&indexed, "about"),
            indexed,
        }
    }
}

fn content_of(indexed: &[Indexed], name: &str) -> gtk::Box {
    indexed
        .iter()
        .find(|page| page.name == name)
        .map(|page| page.content.clone())
        .unwrap_or_else(|| gtk::Box::new(gtk::Orientation::Vertical, 0))
}

struct Page {
    content: gtk::Box,
    title: gtk::Label,
    subtitle: gtk::Label,
}

struct Shell {
    stack: gtk::Stack,
    headerbar: gtk::Box,
    heading: gtk::Label,
}

impl Shell {
    fn new() -> (Self, gtk::WindowHandle) {
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(120)
            .hexpand(true)
            .vexpand(true)
            .build();
        let heading = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["htitle"])
            .build();
        let headerbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["headerbar"])
            .build();
        headerbar.append(&heading);
        headerbar.append(
            &gtk::WindowControls::builder()
                .side(gtk::PackType::End)
                .valign(gtk::Align::Center)
                .css_classes(["wbtns"])
                .build(),
        );
        let handle = gtk::WindowHandle::builder().child(&headerbar).build();
        let shell = Self {
            stack,
            headerbar,
            heading,
        };
        shell.follow_pages();
        (shell, handle)
    }

    fn page(&self, name: &str, title: &str, subtitle: &str) -> Page {
        let head = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .css_classes(["page-head"])
            .build();
        let title_label = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["page-title"])
            .build();
        let subtitle_label = gtk::Label::builder()
            .label(subtitle)
            .xalign(0.0)
            .wrap(true)
            .visible(!subtitle.is_empty())
            .css_classes(["page-subtitle"])
            .build();
        head.append(&title_label);
        head.append(&subtitle_label);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["page"])
            .build();
        content.append(&head);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .css_classes(["scroll"])
            .build();
        self.stack.add_titled(&scroller, Some(name), title);
        let stack = self.stack.clone();
        let headerbar = self.headerbar.clone();
        let owner = name.to_owned();
        scroller
            .vadjustment()
            .connect_value_changed(move |adjustment| {
                if stack.visible_child_name().as_deref() == Some(owner.as_str()) {
                    mark_scrolled(&headerbar, adjustment.value());
                }
            });
        Page {
            content,
            title: title_label,
            subtitle: subtitle_label,
        }
    }

    fn follow_pages(&self) {
        let headerbar = self.headerbar.clone();
        let heading = self.heading.clone();
        self.stack.connect_visible_child_notify(move |stack| {
            let Some(child) = stack.visible_child() else {
                return;
            };
            heading.set_label(&stack.page(&child).title().unwrap_or_default());
            let scrolled = child
                .downcast_ref::<gtk::ScrolledWindow>()
                .map_or(0.0, |scroller| scroller.vadjustment().value());
            mark_scrolled(&headerbar, scrolled);
        });
    }
}

fn mark_scrolled(headerbar: &gtk::Box, offset: f64) {
    if offset > SCROLLED_PAST {
        headerbar.add_css_class("scrolled");
    } else {
        headerbar.remove_css_class("scrolled");
    }
}

fn nav_item(icon_name: &str, title: &str) -> (gtk::ToggleButton, gtk::Label) {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    content.append(
        &gtk::Image::builder()
            .icon_name(icon_name)
            .pixel_size(16)
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["label"])
            .build(),
    );
    let hits = gtk::Label::builder()
        .visible(false)
        .css_classes(["hits"])
        .build();
    content.append(&hits);
    let button = gtk::ToggleButton::builder()
        .child(&content)
        .css_classes(["nav"])
        .build();
    (button, hits)
}

fn side_top() -> gtk::WindowHandle {
    let top = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(9)
        .css_classes(["side-top"])
        .build();
    let mark = gtk::Box::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .css_classes(["side-mark"])
        .build();
    mark.append(
        &gtk::Image::builder()
            .icon_name("kiekje-region-symbolic")
            .pixel_size(14)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build(),
    );
    top.append(&mark);
    top.append(
        &gtk::Label::builder()
            .label("Kiekje")
            .css_classes(["name"])
            .build(),
    );
    top.append(
        &gtk::Label::builder()
            .label("Settings")
            .css_classes(["sub"])
            .build(),
    );
    gtk::WindowHandle::builder().child(&top).build()
}

fn gallery_link() -> gtk::Button {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    content.append(
        &gtk::Image::builder()
            .icon_name("kiekje-gallery-symbolic")
            .pixel_size(16)
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label("Gallery")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["label"])
            .build(),
    );
    let count = gtk::Label::builder().css_classes(["count"]).build();
    content.append(&count);
    content.append(
        &gtk::Image::builder()
            .icon_name("kiekje-open-symbolic")
            .pixel_size(13)
            .css_classes(["ext"])
            .build(),
    );
    let button = gtk::Button::builder()
        .child(&content)
        .action_name("app.gallery")
        .css_classes(["nav", "gallery-link"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label("Open the gallery")]);
    let shown = button.clone();
    glib::spawn_future_local(async move {
        let counted = gio::spawn_blocking(|| {
            Library::open(&library::path()).and_then(|library| library.count())
        })
        .await;
        match counted {
            Ok(Ok(total)) => {
                count.set_label(&total.to_string());
                shown.update_property(&[gtk::accessible::Property::Label(&format!(
                    "Open the gallery, {total} {}",
                    if total == 1 { "capture" } else { "captures" }
                ))]);
            }
            Ok(Err(error)) => debug!("the gallery count is unknown: {error}"),
            Err(_) => debug!("the gallery count worker stopped"),
        }
    });
    button
}

struct Sidebar {
    widget: gtk::Box,
    nav: Vec<NavItem>,
}

fn sidebar(stack: &gtk::Stack, search: &Search) -> Sidebar {
    let widget = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(SIDEBAR_WIDTH)
        .css_classes(["sidebar"])
        .build();
    widget.append(&side_top());
    widget.append(search.widget());
    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(1)
        .css_classes(["nav"])
        .build();
    let mut nav = Vec::new();
    let mut leader: Option<gtk::ToggleButton> = None;
    for spec in &PAGE_SPECS {
        let (button, hits) = nav_item(spec.icon, spec.title);
        button.set_active(leader.is_none());
        match &leader {
            Some(first) => button.set_group(Some(first)),
            None => leader = Some(button.clone()),
        }
        let stack = stack.clone();
        let name = spec.name;
        button.connect_toggled(move |button| {
            if button.is_active() {
                stack.set_visible_child_name(name);
            }
        });
        list.append(&button);
        nav.push(NavItem {
            name: spec.name,
            button,
            hits,
        });
    }
    widget.append(&list);
    widget.append(
        &gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .vexpand(true)
            .build(),
    );
    let foot = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(["side-foot"])
        .build();
    foot.append(&gallery_link());
    foot.append(&footer());
    widget.append(&foot);
    Sidebar { widget, nav }
}

struct Editor {
    host: ShortcutHost,
    shortcuts: RefCell<Shortcuts>,
    bound: RefCell<Vec<(ShortcutAction, String)>>,
    unavailable: Vec<ShortcutAction>,
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
    window.set_titlebar(Some(
        &gtk::Box::builder().css_classes(["titlebar-void"]).build(),
    ));

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["panel", "settings"])
        .build();
    let (shell, headerbar) = Shell::new();
    let search = Search::new(&shell.stack);
    let sidebar = sidebar(&shell.stack, &search);
    let main = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .css_classes(["main"])
        .build();
    main.append(&headerbar);
    main.append(&shell.stack);
    root.append(&sidebar.widget);
    root.append(&main);
    window.set_child(Some(&root));

    let pages = Pages::new(&shell);
    let results = shell.page(RESULTS_PAGE, "Search", "");
    let hits = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["group", "results"])
        .build();
    results.content.append(&hits);
    shell.stack.set_visible_child_name("capture");
    let general = general::append(&pages, &model);
    clipboard::append(&pages.general);

    let keys_group = group(&pages.shortcuts, "Keys");
    let editable = model.host.editable();
    let mut buttons = HashMap::new();
    let mut resets = Vec::new();
    let mut unavailable = Vec::new();
    for action in ShortcutAction::ALL {
        let reason = match (action, model.window_capture) {
            (ShortcutAction::Window, Availability::Unavailable(reason)) => Some(reason),
            _ => None,
        };
        let face = match reason {
            Some(_) => Face::Unavailable,
            None => trigger_face(&model.host, &model.shortcuts, &model.bound, action),
        };
        let trigger = gtk::Button::builder()
            .sensitive(editable && reason.is_none())
            .css_classes(["trigger"])
            .build();
        set_trigger(&trigger, action, &face);
        let reset = gtk::Button::builder()
            .label("Reset")
            .visible(editable && reason.is_none())
            .css_classes(["ghost", "optional"])
            .build();
        reset.update_property(&[gtk::accessible::Property::Label(&format!(
            "Reset {}",
            action.description()
        ))]);
        if reason.is_some() {
            unavailable.push(action);
        }
        let controls = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        controls.append(&trigger);
        controls.append(&reset);
        keys_group.add(row(action.description(), reason, Some(&controls)));
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
        unavailable,
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
                editor.save(action, action.default_slot());
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
    doctor::append(&pages.doctor);
    about::append(&pages.about);
    danger::append(&pages.about, &editor.host, on_done);
    let current = sidebar
        .nav
        .iter()
        .find(|item| item.button.is_active())
        .map(|item| item.button.clone());
    search.index(
        pages.indexed,
        sidebar.nav,
        Results {
            title: results.title,
            subtitle: results.subtitle,
            list: hits,
        },
    );
    search::install_shortcut(&window, search);

    let handle = Handle(Rc::downgrade(&editor));
    let zoom_editor = Rc::downgrade(&editor);
    zoom::install(&window, move || {
        zoom_editor
            .upgrade()
            .is_some_and(|editor| editor.recording.get().is_some())
    });
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
    if let Some(current) = current {
        current.grab_focus();
    }
    (window, handle)
}

pub struct Handle(Weak<Editor>);

impl Handle {
    /// Shows the triggers the desktop reports, after it changed them.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.show_bound(&[(ShortcutAction::Region, "Print".into())]);
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
        if self.saving.get() || self.unavailable.contains(&action) {
            return;
        }
        self.stop();
        self.recording.set(Some(action));
        self.complain("");
        if let Some(button) = self.buttons.get(action.id()) {
            set_trigger(button, action, &Face::Waiting);
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
            if self.recording.get() == Some(action) || self.unavailable.contains(&action) {
                continue;
            }
            if let Some(button) = self.buttons.get(action.id()) {
                let face = trigger_face(&self.host, &shortcuts, &bound, action);
                set_trigger(button, action, &face);
            }
            if let Some(reset) = self.resets.get(action.id()) {
                let changed = *shortcuts.slot(action) != action.default_slot();
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
                    let label = slot_face(shortcuts.slot(action)).text();
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
                Ok(Err(error)) => {
                    editor.stop();
                    editor.complain(&error.to_string());
                }
                Err(_) => {
                    editor.stop();
                    editor.complain("The settings worker stopped; nothing was saved.");
                }
            }
        });
    }
}

#[derive(Debug, Error)]
enum StoreError {
    #[error("{0}.")]
    KdeCheck(KdeError),
    #[error("{trigger} is already set for {owner}.")]
    Conflict {
        trigger: String,
        owner: &'static str,
    },
    #[error(transparent)]
    Config(ConfigError),
    #[error("Saved to the config, but KDE kept the old shortcut: {0}")]
    KdeKept(KdeError),
    #[error(
        "Saved to the config, but {} could not be written: {source}",
        storage::tilde(path)
    )]
    HyprlandInclude { path: PathBuf, source: io::Error },
}

fn store(action: ShortcutAction, slot: Slot, host: &ShortcutHost) -> Result<Shortcuts, StoreError> {
    let on_kde = *host == ShortcutHost::Kde;
    if on_kde && let Some(trigger) = slot.trigger() {
        kde::check(APP_ID, trigger).map_err(StoreError::KdeCheck)?;
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
                StoreError::Conflict {
                    trigger: source.trigger.label(),
                    owner: owner.description(),
                }
            }
            other => StoreError::Config(other),
        })?;
    if on_kde {
        kde::set(APP_ID, action, trigger.as_ref()).map_err(StoreError::KdeKept)?;
    }
    if *host == ShortcutHost::Hyprland {
        let include = hyprland::include_path();
        hyprland::write_include(&include, APP_ID, &shortcuts).map_err(|source| {
            StoreError::HyprlandInclude {
                path: include.clone(),
                source,
            }
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

const NOT_SET: &str = "Not set";
const OFF: &str = "Off";

enum Face {
    Keys(String),
    NotSet,
    Off,
    Unavailable,
    Waiting,
}

impl Face {
    fn text(&self) -> String {
        match self {
            Self::Keys(keys) => keys.clone(),
            Self::NotSet => NOT_SET.to_owned(),
            Self::Off => OFF.to_owned(),
            Self::Unavailable => "Unavailable".to_owned(),
            Self::Waiting => "Press keys".to_owned(),
        }
    }

    fn from_label(label: &str) -> Self {
        match label {
            NOT_SET => Self::NotSet,
            OFF => Self::Off,
            keys => Self::Keys(keys.to_owned()),
        }
    }

    fn widget(&self) -> gtk::Widget {
        let widget: gtk::Widget = match self {
            Self::Keys(keys) => keycaps(keys).upcast(),
            note => gtk::Label::builder()
                .label(note.text())
                .css_classes(["trigger-note"])
                .build()
                .upcast(),
        };
        widget.set_halign(gtk::Align::Center);
        widget
    }
}

fn set_trigger(button: &gtk::Button, action: ShortcutAction, face: &Face) {
    button.set_child(Some(&face.widget()));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}: {}",
        action.description(),
        face.text()
    ))]);
}

fn trigger_face(
    host: &ShortcutHost,
    shortcuts: &Shortcuts,
    bound: &[(ShortcutAction, String)],
    action: ShortcutAction,
) -> Face {
    match host {
        ShortcutHost::Desktop { .. } | ShortcutHost::Kde => bound_face(bound, action),
        _ => slot_face(shortcuts.slot(action)),
    }
}

fn bound_face(bound: &[(ShortcutAction, String)], action: ShortcutAction) -> Face {
    bound
        .iter()
        .find(|(bound, _)| *bound == action)
        .map_or(Face::NotSet, |(_, label)| Face::from_label(label))
}

fn slot_face(slot: &Slot) -> Face {
    slot.trigger()
        .map_or(Face::Off, |trigger| Face::Keys(trigger.label()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_two_status_words_as_notes_and_everything_else_as_keys() {
        assert!(matches!(Face::from_label("Not set"), Face::NotSet));
        assert!(matches!(Face::from_label("Off"), Face::Off));
        assert!(matches!(Face::from_label("Super+S"), Face::Keys(keys) if keys == "Super+S"));
    }

    #[test]
    fn writes_each_face_as_the_text_a_screen_reader_announces() {
        assert_eq!(Face::Keys("Shift+Print".to_owned()).text(), "Shift+Print");
        assert_eq!(Face::NotSet.text(), "Not set");
        assert_eq!(Face::Waiting.text(), "Press keys");
    }
}
