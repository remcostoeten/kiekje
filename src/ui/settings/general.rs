use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gio, glib};

use super::Pages;
use crate::capture::CaptureMode;
use crate::config::{
    self, AfterSelection, Capture, Config, Folder, General as GeneralConfig, Notifications,
    Overlay, SlowPointer, Storage,
};
use crate::storage::{
    self, ImageFormat, Naming, Quality, Template, autostart, template::DEFAULT_TEMPLATE,
};
use crate::ui::widgets::{group, note, problem, row, switch};

type Change = Box<dyn FnOnce(&mut Config) + Send>;
type Changes = async_channel::Sender<(Page, Change)>;

const PREVIEW_WIDTH: u32 = 1920;
const PREVIEW_HEIGHT: u32 = 1080;
const ENTRY_WIDTH: i32 = 280;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Capture,
    Saving,
    General,
}

pub(super) struct General {
    quiet: Cell<bool>,
    folder: gtk::Label,
    default_folder: gtk::Button,
    filename: gtk::Entry,
    default_filename: gtk::Button,
    preview: gtk::Label,
    saved_filename: RefCell<String>,
    format: gtk::DropDown,
    quality: gtk::SpinButton,
    quality_row: gtk::ListBoxRow,
    enabled: gtk::Switch,
    sound: gtk::Switch,
    edit: gtk::Switch,
    magnifier: gtk::Switch,
    slow_pointer: gtk::DropDown,
    slow_pointer_row: gtk::ListBoxRow,
    window_snap: gtk::Switch,
    hints: gtk::Switch,
    autostart: gtk::Switch,
    updates: gtk::Switch,
    capture_problem: gtk::Label,
    saving_problem: gtk::Label,
    general_problem: gtk::Label,
}

pub(super) fn append(
    pages: &Pages,
    storage: &Storage,
    notifications: Notifications,
    capture: Capture,
    overlay: Overlay,
    preferences: GeneralConfig,
) -> Weak<General> {
    let (changes, receiver) = async_channel::unbounded::<(Page, Change)>();

    let selection = group(&pages.capture, "Selection");
    let edit = switch(capture.after_selection == AfterSelection::Edit);
    selection.add(row(
        "Edit a region before it is copied and saved",
        Some("Enter accepts, Esc cancels. Off: the region is copied and saved when you let go."),
        Some(&edit),
    ));
    let window_snap = switch(overlay.window_snap);
    selection.add(row(
        "Snap to the window under the pointer",
        Some("Highlights the window; one click selects it. KDE Plasma 6 and Hyprland."),
        Some(&window_snap),
    ));
    let hints = switch(overlay.hints);
    selection.add(row(
        "Show the keyboard hints",
        Some("A bar at the bottom lists the keys: F screen, W window, D draw. ? hides it once."),
        Some(&hints),
    ));

    let loupe = group(&pages.capture, "Magnifier");
    let magnifier = switch(overlay.magnifier);
    loupe.add(row(
        "Show a magnifier with the pixel colour",
        Some("Z hides it for one capture, C copies the colour, scrolling zooms."),
        Some(&magnifier),
    ));
    let labels: Vec<&str> = SlowPointer::ALL.iter().map(|speed| speed.label()).collect();
    let slow_pointer = gtk::DropDown::from_strings(&labels);
    slow_pointer.set_selected(slow_index(overlay.slow_pointer));
    slow_pointer.update_property(&[gtk::accessible::Property::Label("Shift pointer speed")]);
    let slow_pointer_row = loupe.add(row(
        "Pointer speed while holding Shift",
        Some("Slower movement makes single pixels easy to hit."),
        Some(&slow_pointer),
    ));
    slow_pointer_row.set_sensitive(overlay.magnifier);
    let capture_problem = problem();
    loupe.footer(&capture_problem);

    let saving = group(&pages.saving, "Location");
    let folder = gtk::Label::builder()
        .label(storage::tilde(&storage.captures_dir()))
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .css_classes(["value"])
        .build();
    let choose = gtk::Button::with_label("Choose");
    let default_folder = ghost("Default", storage.folder.is_some());
    let folder_controls = controls(&[
        folder.upcast_ref(),
        choose.upcast_ref(),
        default_folder.upcast_ref(),
    ]);
    saving.add(row(
        "Folder",
        Some("Captures are saved here and listed in the gallery."),
        Some(&folder_controls),
    ));

    let filename = gtk::Entry::builder()
        .text(storage.filename.to_string())
        .width_request(ENTRY_WIDTH)
        .build();
    filename.update_property(&[gtk::accessible::Property::Label("File name template")]);
    let default_filename = ghost("Default", storage.filename.to_string() != DEFAULT_TEMPLATE);
    let filename_controls = controls(&[filename.upcast_ref(), default_filename.upcast_ref()]);
    saving.add(row("File name", None, Some(&filename_controls)));
    let preview = note("");
    saving.footer(&preview);
    saving.footer(&note(
        "Tokens: {mode} {date} {time} {year} {month} {day} {hour} {minute} {second} {width} {height}",
    ));

    let kind = group(&pages.saving, "Format");
    let labels: Vec<&str> = ImageFormat::ALL
        .iter()
        .map(|format| format.label())
        .collect();
    let format = gtk::DropDown::from_strings(&labels);
    format.set_selected(format_index(storage.format));
    format.update_property(&[gtk::accessible::Property::Label("Image format")]);
    kind.add(row(
        "Image format",
        Some("PNG keeps every pixel. JPEG files are smaller but blur sharp text a little."),
        Some(&format),
    ));
    let quality =
        gtk::SpinButton::with_range(f64::from(Quality::MIN), f64::from(Quality::MAX), 1.0);
    quality.set_value(f64::from(storage.quality.get()));
    quality.update_property(&[gtk::accessible::Property::Label("JPEG quality")]);
    let quality_row = kind.add(row(
        "JPEG quality",
        Some("Higher keeps more detail and makes larger files."),
        Some(&quality),
    ));
    quality_row.set_sensitive(storage.format == ImageFormat::Jpeg);
    kind.footer(&note(
        "Copying always puts a PNG on the clipboard. Save as follows the extension you type.",
    ));
    let saving_problem = problem();
    kind.footer(&saving_problem);

    let alerts = group(&pages.general, "Notifications");
    let enabled = switch(notifications.enabled);
    alerts.add(row(
        "Notify after each capture",
        Some("Shows the result with actions to open or copy it."),
        Some(&enabled),
    ));
    let sound = switch(notifications.sound);
    alerts.add(row(
        "Play a capture sound",
        Some("Uses the sound theme's shutter sound, or Kiekje's own."),
        Some(&sound),
    ));

    let startup = group(&pages.general, "Startup");
    let autostart = switch(false);
    autostart.set_sensitive(false);
    startup.add(row(
        "Start Kiekje at login",
        Some("Keeps the tray and shortcuts ready after you sign in."),
        Some(&autostart),
    ));
    let general_problem = problem();
    startup.footer(&general_problem);

    let releases = group(&pages.general, "Updates");
    let updates = switch(preferences.check_for_updates);
    releases.add(row(
        "Check for updates",
        Some("Asks api.github.com for a newer release at startup and once a day."),
        Some(&updates),
    ));

    let general = Rc::new(General {
        quiet: Cell::new(false),
        folder,
        default_folder,
        filename,
        default_filename,
        preview,
        saved_filename: RefCell::new(storage.filename.to_string()),
        format,
        quality,
        quality_row,
        enabled,
        sound,
        edit,
        magnifier,
        slow_pointer,
        slow_pointer_row,
        window_snap,
        hints,
        autostart,
        updates,
        capture_problem,
        saving_problem,
        general_problem,
    });
    general.show_preview();
    connect(&general, &choose, &changes);
    connect_autostart(&general);
    let handle = Rc::downgrade(&general);

    glib::spawn_future_local(async move {
        while let Ok((page, change)) = receiver.recv().await {
            let saved = gio::spawn_blocking(move || config::update(&config::path(), change)).await;
            match saved {
                Ok(Ok(config)) => general.saved(&config),
                Ok(Err(error)) => general.complain(page, &format!("Not saved: {error}")),
                Err(_) => general.complain(page, "The settings worker stopped; nothing was saved."),
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
        send(
            &sender,
            Page::Saving,
            Box::new(|config| config.storage.folder = None),
        );
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
    general.enabled.connect_active_notify(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::General,
            Box::new(move |config| config.notifications.enabled = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.sound.connect_active_notify(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::General,
            Box::new(move |config| config.notifications.sound = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.updates.connect_active_notify(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::General,
            Box::new(move |config| config.general.check_for_updates = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.edit.connect_active_notify(move |check| {
        let after = after_selection(check.is_active());
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::Capture,
            Box::new(move |config| config.capture.after_selection = after),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.magnifier.connect_active_notify(move |check| {
        let on = check.is_active();
        let Some(general) = weak.upgrade() else {
            return;
        };
        general.slow_pointer_row.set_sensitive(on);
        if general.quiet.get() {
            return;
        }
        send(
            &sender,
            Page::Capture,
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
                Page::Capture,
                Box::new(move |config| config.overlay.slow_pointer = speed),
            );
        });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.window_snap.connect_active_notify(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::Capture,
            Box::new(move |config| config.overlay.window_snap = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.hints.connect_active_notify(move |check| {
        let on = check.is_active();
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::Capture,
            Box::new(move |config| config.overlay.hints = on),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.format.connect_selected_notify(move |dropdown| {
        let picked = usize::try_from(dropdown.selected())
            .ok()
            .and_then(|index| ImageFormat::ALL.get(index).copied());
        let Some(format) = picked else {
            return;
        };
        let Some(general) = weak.upgrade() else {
            return;
        };
        general
            .quality_row
            .set_sensitive(format == ImageFormat::Jpeg);
        general.show_preview();
        if general.quiet.get() {
            return;
        }
        send(
            &sender,
            Page::Saving,
            Box::new(move |config| config.storage.format = format),
        );
    });
    let weak = Rc::downgrade(general);
    let sender = changes.clone();
    general.quality.connect_value_changed(move |spin| {
        let Ok(quality) = u8::try_from(spin.value_as_int())
            .map_err(|_| ())
            .and_then(|value| Quality::try_from(value).map_err(|_| ()))
        else {
            return;
        };
        if weak.upgrade().is_none_or(|general| general.quiet.get()) {
            return;
        }
        send(
            &sender,
            Page::Saving,
            Box::new(move |config| config.storage.quality = quality),
        );
    });
}

fn connect_autostart(general: &Rc<General>) {
    let (requests, receiver) = async_channel::unbounded::<bool>();
    let weak = Rc::downgrade(general);
    general.autostart.connect_active_notify(move |check| {
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
            Ok(Err(error)) => {
                general.complain(Page::General, &format!("Start at login: {error}"));
            }
            Err(_) => general.complain(
                Page::General,
                "The settings worker stopped; start at login is unknown.",
            ),
        }
        general.autostart.set_sensitive(true);
        drop(general);

        while let Ok(enabled) = receiver.recv().await {
            let applied = gio::spawn_blocking(move || autostart::set(enabled)).await;
            let Some(general) = weak.upgrade() else {
                return;
            };
            match applied {
                Ok(Ok(())) => general.complain(Page::General, ""),
                Ok(Err(error)) => {
                    general.complain(
                        Page::General,
                        &format!("Start at login not changed: {error}"),
                    );
                    general.show_autostart(!enabled);
                }
                Err(_) => {
                    general.complain(
                        Page::General,
                        "The settings worker stopped; start at login was not changed.",
                    );
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
                    general.complain(Page::Saving, &format!("The folder picker failed: {error}"));
                }
                return;
            }
        };
        let Some(path) = path else {
            if let Some(general) = general.upgrade() {
                general.complain(
                    Page::Saving,
                    "That folder is not on this computer, so captures can't go there.",
                );
            }
            return;
        };
        match Folder::from_path(path) {
            Ok(folder) => send(
                &changes,
                Page::Saving,
                Box::new(move |config| config.storage.folder = Some(folder)),
            ),
            Err(error) => {
                if let Some(general) = general.upgrade() {
                    general.complain(Page::Saving, &error.to_string());
                }
            }
        }
    });
}

fn send(changes: &Changes, page: Page, change: Change) {
    if changes.try_send((page, change)).is_err() {
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
                let format = usize::try_from(self.format.selected())
                    .ok()
                    .and_then(|index| ImageFormat::ALL.get(index).copied())
                    .unwrap_or_default();
                self.preview.set_label(&format!(
                    "A region capture now: {name}.{}",
                    format.extension()
                ));
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
            Page::Saving,
            Box::new(move |config| config.storage.filename = template),
        );
    }

    fn saved(&self, config: &Config) {
        for page in [Page::Capture, Page::Saving, Page::General] {
            self.complain(page, "");
        }
        self.folder
            .set_label(&storage::tilde(&config.storage.captures_dir()));
        show_ghost(&self.default_folder, config.storage.folder.is_some());
        let filename = config.storage.filename.to_string();
        show_ghost(&self.default_filename, filename != DEFAULT_TEMPLATE);
        self.saved_filename.replace(filename);
    }

    pub(super) fn reset(&self, config: &Config) {
        self.quiet.set(true);
        self.saved(config);
        self.filename.set_text(&config.storage.filename.to_string());
        self.format
            .set_selected(format_index(config.storage.format));
        self.quality
            .set_value(f64::from(config.storage.quality.get()));
        self.enabled.set_active(config.notifications.enabled);
        self.sound.set_active(config.notifications.sound);
        self.edit
            .set_active(config.capture.after_selection == AfterSelection::Edit);
        self.magnifier.set_active(config.overlay.magnifier);
        self.slow_pointer
            .set_selected(slow_index(config.overlay.slow_pointer));
        self.window_snap.set_active(config.overlay.window_snap);
        self.hints.set_active(config.overlay.hints);
        self.updates.set_active(config.general.check_for_updates);
        self.quiet.set(false);
    }

    fn show_autostart(&self, enabled: bool) {
        self.quiet.set(true);
        self.autostart.set_active(enabled);
        self.quiet.set(false);
    }

    fn complain(&self, page: Page, message: &str) {
        let label = match page {
            Page::Capture => &self.capture_problem,
            Page::Saving => &self.saving_problem,
            Page::General => &self.general_problem,
        };
        label.set_label(message);
        label.set_visible(!message.is_empty());
    }
}

fn controls(widgets: &[&gtk::Widget]) -> gtk::Box {
    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    for widget in widgets {
        controls.append(*widget);
    }
    controls
}

fn slow_index(speed: SlowPointer) -> u32 {
    SlowPointer::ALL
        .iter()
        .position(|candidate| *candidate == speed)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

fn format_index(format: ImageFormat) -> u32 {
    ImageFormat::ALL
        .iter()
        .position(|candidate| *candidate == format)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

fn ghost(label: &str, shown: bool) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .css_classes(["ghost", "optional"])
        .build();
    show_ghost(&button, shown);
    button
}

fn show_ghost(button: &gtk::Button, shown: bool) {
    button.set_sensitive(shown);
    button.set_can_target(shown);
}
