use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gio, glib};

use super::{caps, note};
use crate::capture::CaptureMode;
use crate::config::{
    self, AfterSelection, Capture, Config, Folder, Notifications, Overlay, SlowPointer, Storage,
};
use crate::storage::{self, Naming, Template, autostart, template::DEFAULT_TEMPLATE};

type Change = Box<dyn FnOnce(&mut Config) + Send>;
type Changes = async_channel::Sender<Change>;

const PREVIEW_WIDTH: u32 = 1920;
const PREVIEW_HEIGHT: u32 = 1080;

pub(super) struct General {
    quiet: Cell<bool>,
    folder: gtk::Label,
    default_folder: gtk::Button,
    filename: gtk::Entry,
    default_filename: gtk::Button,
    preview: gtk::Label,
    saved_filename: RefCell<String>,
    enabled: gtk::CheckButton,
    sound: gtk::CheckButton,
    edit: gtk::CheckButton,
    magnifier: gtk::CheckButton,
    slow_pointer: gtk::DropDown,
    window_snap: gtk::CheckButton,
    autostart: gtk::CheckButton,
    problem: gtk::Label,
}

pub(super) fn append(
    content: &gtk::Box,
    storage: &Storage,
    notifications: Notifications,
    capture: Capture,
    overlay: Overlay,
) -> Weak<General> {
    let (changes, receiver) = async_channel::unbounded::<Change>();

    content.append(&caps("Startup"));
    let autostart = gtk::CheckButton::builder()
        .label("Start Kiekje at login")
        .sensitive(false)
        .build();
    content.append(&autostart);
    content.append(&note(
        "Keeps the tray and shortcuts ready after you sign in.",
    ));

    content.append(&caps("Capture"));
    let edit = gtk::CheckButton::builder()
        .label("Edit a region on screen before copying and saving it")
        .active(capture.after_selection == AfterSelection::Edit)
        .build();
    content.append(&edit);
    content.append(&note(
        "Enter accepts, Esc cancels. Off: a region is copied and saved when you let go.",
    ));
    let magnifier = gtk::CheckButton::builder()
        .label("Show a magnifier with the pixel colour while selecting; Z toggles it")
        .active(overlay.magnifier)
        .build();
    content.append(&magnifier);
    content.append(&note(
        "Z hides it for one capture, C copies the colour, scrolling zooms and the zoom is kept.",
    ));
    let labels: Vec<&str> = SlowPointer::ALL.iter().map(|speed| speed.label()).collect();
    let slow_pointer = gtk::DropDown::from_strings(&labels);
    slow_pointer.set_selected(slow_index(overlay.slow_pointer));
    slow_pointer.set_sensitive(overlay.magnifier);
    slow_pointer.update_property(&[gtk::accessible::Property::Label("Shift pointer speed")]);
    content.append(&row("Shift", &slow_pointer, &[]));
    content.append(&note(
        "Holding Shift with the magnifier on moves the selection point slower for single pixels.",
    ));
    let window_snap = gtk::CheckButton::builder()
        .label("Highlight the window under the pointer; click to select it")
        .active(overlay.window_snap)
        .build();
    content.append(&window_snap);
    content.append(&note("Works on KDE Plasma 6 and Hyprland."));

    content.append(&caps("Saving"));
    let folder = gtk::Label::builder()
        .label(storage::tilde(&storage.captures_dir()))
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["value"])
        .build();
    let choose = gtk::Button::with_label("Choose");
    let default_folder = ghost("Default", storage.folder.is_some());
    content.append(&row("Folder", &folder, &[&choose, &default_folder]));

    let filename = gtk::Entry::builder()
        .text(storage.filename.to_string())
        .hexpand(true)
        .build();
    filename.update_property(&[gtk::accessible::Property::Label("File name template")]);
    let default_filename = ghost("Default", storage.filename.to_string() != DEFAULT_TEMPLATE);
    content.append(&row("File name", &filename, &[&default_filename]));
    let preview = note("");
    content.append(&preview);
    content.append(&note(
        "Tokens: {mode} {date} {time} {year} {month} {day} {hour} {minute} {second} {width} {height}",
    ));

    content.append(&caps("Notifications"));
    let enabled = gtk::CheckButton::builder()
        .label("Show a notification after each capture")
        .active(notifications.enabled)
        .build();
    let sound = gtk::CheckButton::builder()
        .label("Play a sound with it")
        .active(notifications.sound)
        .sensitive(notifications.enabled)
        .build();
    content.append(&enabled);
    content.append(&sound);

    let problem = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .visible(false)
        .css_classes(["problem"])
        .build();
    content.append(&problem);

    let general = Rc::new(General {
        quiet: Cell::new(false),
        folder,
        default_folder,
        filename,
        default_filename,
        preview,
        saved_filename: RefCell::new(storage.filename.to_string()),
        enabled,
        sound,
        edit,
        magnifier,
        slow_pointer,
        window_snap,
        autostart,
        problem,
    });
    general.show_preview();
    connect(&general, &choose, &changes);
    connect_autostart(&general);
    let handle = Rc::downgrade(&general);

    glib::spawn_future_local(async move {
        while let Ok(change) = receiver.recv().await {
            let saved = gio::spawn_blocking(move || config::update(&config::path(), change)).await;
            match saved {
                Ok(Ok(config)) => general.saved(&config),
                Ok(Err(error)) => general.complain(&format!("Not saved: {error}")),
                Err(_) => general.complain("The settings worker stopped; nothing was saved."),
            }
        }
    });
    handle
}

fn connect(general: &Rc<General>, choose: &gtk::Button, changes: &Changes) {
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    choose.connect_clicked(move |button| pick_folder(button, weak.clone(), sender.clone()));

    let sender = changes.clone();
    general.default_folder.connect_clicked(move |_| {
        send(&sender, Box::new(|config| config.storage.folder = None));
    });

    let weak = Rc::downgrade(general);
    general.filename.connect_changed(move |_| {
        if let Some(general) = weak.upgrade() {
            general.show_preview();
        }
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.filename.connect_activate(move |_| {
        if let Some(general) = weak.upgrade() {
            general.save_filename(&sender);
        }
    });
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    focus.connect_leave(move |_| {
        if let Some(general) = weak.upgrade() {
            general.save_filename(&sender);
        }
    });
    general.filename.add_controller(focus);
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.default_filename.connect_clicked(move |_| {
        if let Some(general) = weak.upgrade() {
            general.filename.set_text(DEFAULT_TEMPLATE);
            general.save_filename(&sender);
        }
    });

    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.enabled.connect_toggled(move |check| {
        let on = check.is_active();
        let Some(general) = weak.upgrade() else {
            return;
        };
        general.sound.set_sensitive(on);
        if general.quiet.get() {
            return;
        }
        send(
            &sender,
            Box::new(move |config| config.notifications.enabled = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.sound.connect_toggled(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Box::new(move |config| config.notifications.sound = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.edit.connect_toggled(move |check| {
        let after = after_selection(check.is_active());
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Box::new(move |config| config.capture.after_selection = after),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.magnifier.connect_toggled(move |check| {
        let on = check.is_active();
        let Some(general) = weak.upgrade() else {
            return;
        };
        general.slow_pointer.set_sensitive(on);
        if general.quiet.get() {
            return;
        }
        send(
            &sender,
            Box::new(move |config| config.overlay.magnifier = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general
        .slow_pointer
        .connect_selected_notify(move |dropdown| {
            let picked = usize::try_from(dropdown.selected())
                .ok()
                .and_then(|index| SlowPointer::ALL.get(index).copied());
            let Some(speed) = picked else {
                return;
            };
            if weak.upgrade().is_none_or(|general| general.quiet.get()) {
                return;
            }
            send(
                &sender,
                Box::new(move |config| config.overlay.slow_pointer = speed),
            );
        });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.window_snap.connect_toggled(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Box::new(move |config| config.overlay.window_snap = on),
        );
    });
}

fn connect_autostart(general: &Rc<General>) {
    let (requests, receiver) = async_channel::unbounded::<bool>();
    let weak = Rc::downgrade(general);
    general.autostart.connect_toggled(move |check| {
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        if requests.try_send(check.is_active()).is_err() {
            eprintln!("kiekje: the autostart worker stopped, the change was not applied");
        }
    });

    let weak = Rc::downgrade(general);
    glib::spawn_future_local(async move {
        let found = gio::spawn_blocking(|| autostart::is_enabled(&autostart::path())).await;
        let Some(general) = weak.upgrade() else {
            return;
        };
        match found {
            Ok(Ok(enabled)) => general.show_autostart(enabled),
            Ok(Err(error)) => general.complain(&format!("Start at login: {error}")),
            Err(_) => general.complain("The settings worker stopped; start at login is unknown."),
        }
        general.autostart.set_sensitive(true);
        drop(general);

        while let Ok(enabled) = receiver.recv().await {
            let applied = gio::spawn_blocking(move || autostart::set(enabled)).await;
            let Some(general) = weak.upgrade() else {
                return;
            };
            match applied {
                Ok(Ok(())) => general.complain(""),
                Ok(Err(error)) => {
                    general.complain(&format!("Start at login not changed: {error}"));
                    general.show_autostart(!enabled);
                }
                Err(_) => {
                    general
                        .complain("The settings worker stopped; start at login was not changed.");
                    general.show_autostart(!enabled);
                }
            }
        }
    });
}

fn after_selection(on: bool) -> AfterSelection {
    if on {
        AfterSelection::Edit
    } else {
        AfterSelection::Finish
    }
}

fn pick_folder(button: &gtk::Button, general: Weak<General>, changes: Changes) {
    let window = button.root().and_downcast::<gtk::Window>();
    let dialog = gtk::FileDialog::builder()
        .title("Choose where captures are saved")
        .modal(true)
        .build();
    dialog.select_folder(window.as_ref(), None::<&gio::Cancellable>, move |chosen| {
        let path = match chosen {
            Ok(file) => file.path(),
            Err(error) if error.matches(gtk::DialogError::Dismissed) => return,
            Err(error) => {
                if let Some(general) = general.upgrade() {
                    general.complain(&format!("The folder picker failed: {error}"));
                }
                return;
            }
        };
        let Some(path) = path else {
            if let Some(general) = general.upgrade() {
                general
                    .complain("That folder is not on this computer, so captures can't go there.");
            }
            return;
        };
        match Folder::from_path(path) {
            Ok(folder) => send(
                &changes,
                Box::new(move |config| config.storage.folder = Some(folder)),
            ),
            Err(error) => {
                if let Some(general) = general.upgrade() {
                    general.complain(&error.to_string());
                }
            }
        }
    });
}

fn send(changes: &Changes, change: Change) {
    if changes.try_send(change).is_err() {
        eprintln!("kiekje: the settings worker stopped, a change was not saved");
    }
}

impl General {
    fn show_preview(&self) {
        match Template::parse(&self.filename.text()) {
            Ok(template) => {
                let name = template.render(&Naming {
                    mode: CaptureMode::Region,
                    stamp: storage::now(),
                    width: PREVIEW_WIDTH,
                    height: PREVIEW_HEIGHT,
                });
                self.preview
                    .set_label(&format!("A region capture now: {name}.png"));
                self.preview.remove_css_class("problem");
                self.preview.add_css_class("note");
            }
            Err(error) => {
                self.preview.set_label(&error.to_string());
                self.preview.remove_css_class("note");
                self.preview.add_css_class("problem");
            }
        }
    }

    fn save_filename(&self, changes: &Changes) {
        let text = self.filename.text().to_string();
        if text == *self.saved_filename.borrow() {
            return;
        }
        let Ok(template) = Template::parse(&text) else {
            return;
        };
        self.saved_filename.replace(text);
        send(
            changes,
            Box::new(move |config| config.storage.filename = template),
        );
    }

    fn saved(&self, config: &Config) {
        self.complain("");
        self.folder
            .set_label(&storage::tilde(&config.storage.captures_dir()));
        self.default_folder
            .set_visible(config.storage.folder.is_some());
        let filename = config.storage.filename.to_string();
        self.default_filename
            .set_visible(filename != DEFAULT_TEMPLATE);
        self.saved_filename.replace(filename);
    }

    pub(super) fn reset(&self, config: &Config) {
        self.quiet.set(true);
        self.saved(config);
        self.filename.set_text(&config.storage.filename.to_string());
        self.enabled.set_active(config.notifications.enabled);
        self.sound.set_active(config.notifications.sound);
        self.edit
            .set_active(config.capture.after_selection == AfterSelection::Edit);
        self.magnifier.set_active(config.overlay.magnifier);
        self.slow_pointer
            .set_selected(slow_index(config.overlay.slow_pointer));
        self.window_snap.set_active(config.overlay.window_snap);
        self.quiet.set(false);
    }

    fn show_autostart(&self, enabled: bool) {
        self.quiet.set(true);
        self.autostart.set_active(enabled);
        self.quiet.set(false);
    }

    fn complain(&self, message: &str) {
        self.problem.set_label(message);
        self.problem.set_visible(!message.is_empty());
    }
}

fn row(label: &str, value: &impl IsA<gtk::Widget>, buttons: &[&gtk::Button]) -> gtk::Box {
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
    row.append(value);
    for button in buttons {
        row.append(*button);
    }
    row
}

fn slow_index(speed: SlowPointer) -> u32 {
    SlowPointer::ALL
        .iter()
        .position(|candidate| *candidate == speed)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

fn ghost(label: &str, visible: bool) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .visible(visible)
        .css_classes(["ghost"])
        .build()
}
