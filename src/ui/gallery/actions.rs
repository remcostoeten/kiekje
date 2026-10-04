use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use super::Gallery;
use crate::library::{self, Library};
use crate::ui::files;

const GROUP: &str = "gallery";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileAction {
    Open,
    Reveal,
    CopyImage,
    CopyPath,
    Trash,
    Forget,
}

impl FileAction {
    const ALL: [Self; 6] = [
        Self::Open,
        Self::Reveal,
        Self::CopyImage,
        Self::CopyPath,
        Self::Trash,
        Self::Forget,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reveal => "reveal",
            Self::CopyImage => "copy-image",
            Self::CopyPath => "copy-path",
            Self::Trash => "trash",
            Self::Forget => "forget",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Reveal => "Show in folder",
            Self::CopyImage => "Copy image",
            Self::CopyPath => "Copy path",
            Self::Trash => "Move to trash",
            Self::Forget => "Remove from gallery",
        }
    }

    fn keys(self) -> Option<&'static str> {
        match self {
            Self::Open => None,
            Self::Reveal => Some("<Ctrl>Return"),
            Self::CopyImage => Some("<Ctrl>c"),
            Self::CopyPath => Some("<Ctrl><Alt>c"),
            Self::Trash => Some("Delete"),
            Self::Forget => None,
        }
    }
}

pub(super) fn install(window: &gtk::Window, gallery: &Rc<Gallery>) {
    let group = gio::SimpleActionGroup::new();
    for action in FileAction::ALL {
        let simple = gio::SimpleAction::new(action.name(), None);
        let target = Rc::downgrade(gallery);
        simple.connect_activate(move |_, _| {
            if let Some(gallery) = target.upgrade() {
                gallery.run(action);
            }
        });
        group.add_action(&simple);
    }
    window.insert_action_group(GROUP, Some(&group));
}

pub(super) fn attach(view: &impl IsA<gtk::Widget>) -> gtk::PopoverMenu {
    let menu = gio::Menu::new();
    let open = gio::Menu::new();
    let copy = gio::Menu::new();
    let remove = gio::Menu::new();
    for action in FileAction::ALL {
        let section = match action {
            FileAction::Open | FileAction::Reveal => &open,
            FileAction::CopyImage | FileAction::CopyPath => &copy,
            FileAction::Trash | FileAction::Forget => &remove,
        };
        section.append(
            Some(action.label()),
            Some(&format!("{GROUP}.{}", action.name())),
        );
    }
    menu.append_section(None, &open);
    menu.append_section(None, &copy);
    menu.append_section(None, &remove);

    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover.set_parent(view);
    let owned = popover.downgrade();
    view.connect_destroy(move |_| {
        if let Some(popover) = owned.upgrade() {
            popover.unparent();
        }
    });

    let shortcuts = gtk::ShortcutController::new();
    for action in FileAction::ALL {
        let Some(trigger) = action.keys().and_then(gtk::ShortcutTrigger::parse_string) else {
            continue;
        };
        let named = gtk::NamedAction::new(&format!("{GROUP}.{}", action.name()));
        shortcuts.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(named)));
    }
    for keys in ["<Shift>F10", "Menu"] {
        let Some(trigger) = gtk::ShortcutTrigger::parse_string(keys) else {
            continue;
        };
        let menu = popover.downgrade();
        let callback = gtk::CallbackAction::new(move |view, _| {
            if let Some(menu) = menu.upgrade() {
                show_at_focus(view, &menu);
            }
            glib::Propagation::Stop
        });
        shortcuts.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(callback)));
    }
    view.add_controller(shortcuts);
    popover
}

pub(super) fn show_at(menu: &gtk::PopoverMenu, item: &gtk::Widget, x: f64, y: f64) {
    let Some(view) = menu.parent() else {
        return;
    };
    let Some(point) = item.compute_point(&view, &gtk::graphene::Point::new(x as f32, y as f32))
    else {
        return;
    };
    menu.set_pointing_to(Some(&gdk::Rectangle::new(
        point.x() as i32,
        point.y() as i32,
        1,
        1,
    )));
    menu.popup();
}

fn show_at_focus(view: &gtk::Widget, menu: &gtk::PopoverMenu) {
    match view.focus_child() {
        Some(child) => {
            let (x, y) = (
                f64::from(child.width()) / 2.0,
                f64::from(child.height()) / 2.0,
            );
            show_at(menu, &child, x, y);
        }
        None => show_at(menu, view, 24.0, 24.0),
    }
}

impl Gallery {
    fn selected(&self) -> Option<PathBuf> {
        self.selection
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|object| PathBuf::from(object.string().as_str()))
    }

    fn run(self: &Rc<Self>, action: FileAction) {
        let Some(path) = self.selected() else {
            return;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let gallery = self.clone();
        match action {
            FileAction::Open => {
                glib::spawn_future_local(files::open(path));
            }
            FileAction::Reveal => {
                glib::spawn_future_local(files::reveal(path));
            }
            FileAction::CopyPath => {
                self.clipboard().set_text(&path.to_string_lossy());
                self.say(&format!("Copied the path of {name}"), false);
            }
            FileAction::CopyImage => {
                glib::spawn_future_local(async move {
                    let loaded =
                        gio::spawn_blocking(move || gdk::Texture::from_filename(&path)).await;
                    match loaded {
                        Ok(Ok(texture)) => {
                            gallery.clipboard().set_texture(&texture);
                            gallery.say(&format!("Copied {name}"), false);
                        }
                        Ok(Err(error)) => {
                            gallery.say(&format!("Could not read {name}: {error}"), true);
                        }
                        Err(_) => gallery.say("The copy worker stopped", true),
                    }
                });
            }
            FileAction::Trash => {
                glib::spawn_future_local(async move {
                    let file = gio::File::for_path(&path);
                    if let Err(error) = file.trash_future(glib::Priority::DEFAULT).await {
                        gallery.say(
                            &format!("Could not move {name} to the trash: {error}"),
                            true,
                        );
                        return;
                    }
                    gallery
                        .forget(path, format!("Moved {name} to the trash"))
                        .await;
                });
            }
            FileAction::Forget => {
                glib::spawn_future_local(async move {
                    let message =
                        format!("Removed {name} from the gallery; the file is still there");
                    gallery.forget(path, message).await;
                });
            }
        }
    }

    async fn forget(self: &Rc<Self>, path: PathBuf, done: String) {
        let removed = gio::spawn_blocking(move || {
            Library::open(&library::path()).and_then(|library| library.remove(&path))
        })
        .await;
        match removed {
            Ok(Ok(_)) => self.say(&done, false),
            Ok(Err(error)) => self.say(&format!("The library was not updated: {error}"), true),
            Err(_) => self.say("The library worker stopped", true),
        }
        self.reload();
    }

    fn clipboard(&self) -> gdk::Clipboard {
        self.status.clipboard()
    }

    fn say(&self, message: &str, problem: bool) {
        self.status.set_label(message);
        self.status.set_visible(!message.is_empty());
        if problem {
            self.status.add_css_class("problem");
        } else {
            self.status.remove_css_class("problem");
        }
    }
}
