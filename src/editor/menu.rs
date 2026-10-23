use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use super::Editor;
use super::document::Tool;
use super::keys::{DrawAction, DrawKey};

pub(super) enum Menu {
    Popover(gtk::Popover),
    Placed(gtk::Fixed, gtk::Box),
}

#[derive(Clone, Copy)]
enum Item {
    Draw(DrawAction),
    Undo,
    Redo,
    Delete,
}

pub(super) fn attach(editor: &Rc<Editor>) {
    let clicks = gtk::GestureClick::builder()
        .button(gdk::BUTTON_SECONDARY)
        .build();
    let target = Rc::downgrade(editor);
    clicks.connect_pressed(move |gesture, _, x, y| {
        if let Some(editor) = target.upgrade() {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            editor.open_menu(x, y);
        }
    });
    editor.canvas.add_controller(clicks);

    let primary = gtk::GestureClick::builder()
        .button(gdk::BUTTON_PRIMARY)
        .build();
    let target = Rc::downgrade(editor);
    primary.connect_pressed(move |_, _, _, _| {
        if let Some(editor) = target.upgrade()
            && !editor.menu_pinned.get()
        {
            editor.close_menu();
        }
    });
    editor.canvas.add_controller(primary);
}

impl Editor {
    fn open_menu(self: &Rc<Self>, x: f64, y: f64) {
        if self.tool.get() == Tool::Emoji {
            self.open_emoji_menu("", x, y);
        } else {
            self.open_menu_with(self.menu_list(), x, y);
        }
    }

    pub(super) fn open_menu_by_key(self: &Rc<Self>) {
        let (x, y) = if self.is_stamp_tool() {
            self.view.get().to_widget(self.stamp_point())
        } else {
            self.pointer.get().unwrap_or((
                f64::from(self.canvas.width()) / 2.0,
                f64::from(self.canvas.height()) / 2.0,
            ))
        };
        self.open_menu(x, y);
    }

    pub(super) fn open_menu_with(self: &Rc<Self>, list: gtk::Box, x: f64, y: f64) {
        self.close_menu();
        match self.layer.borrow().as_ref() {
            Some((layer, origin)) => {
                let (_, width, _, _) = list.measure(gtk::Orientation::Horizontal, -1);
                let (_, height, _, _) = list.measure(gtk::Orientation::Vertical, width);
                let (room_x, room_y) = (
                    f64::from(self.canvas.width() - width).max(0.0),
                    f64::from(self.canvas.height() - height).max(0.0),
                );
                layer.put(&list, origin.x + x.min(room_x), origin.y + y.min(room_y));
                self.menu
                    .replace(Some(Menu::Placed(layer.clone(), list.clone())));
                if let Some(first) = list.first_child() {
                    first.grab_focus();
                }
            }
            None => {
                let popover = gtk::Popover::builder()
                    .has_arrow(false)
                    .position(gtk::PositionType::Bottom)
                    .halign(gtk::Align::Start)
                    .css_classes(["context-menu"])
                    .child(&list)
                    .build();
                popover.set_parent(&self.canvas);
                popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                let closed = Rc::downgrade(self);
                popover.connect_closed(move |popover| {
                    if let Some(editor) = closed.upgrade() {
                        let mut menu = editor.menu.borrow_mut();
                        if matches!(menu.as_ref(), Some(Menu::Popover(open)) if open == popover) {
                            menu.take();
                        }
                    }
                    let owned = popover.clone();
                    glib::idle_add_local_once(move || owned.unparent());
                });
                popover.popup();
                self.menu.replace(Some(Menu::Popover(popover)));
            }
        }
    }

    pub(super) fn pin_menu(&self, pinned: bool) {
        self.menu_pinned.set(pinned);
        if let Some(Menu::Popover(popover)) = self.menu.borrow().as_ref() {
            popover.set_autohide(!pinned);
        }
    }

    pub(super) fn close_menu(&self) -> bool {
        self.menu_pinned.set(false);
        let Some(menu) = self.menu.take() else {
            return false;
        };
        match menu {
            Menu::Popover(popover) => popover.popdown(),
            Menu::Placed(layer, list) => {
                layer.remove(&list);
                self.canvas.grab_focus();
            }
        }
        true
    }

    fn menu_list(self: &Rc<Self>) -> gtk::Box {
        let list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .css_classes(["context-list"])
            .build();
        if self.is_inline() {
            list.add_css_class("panel");
        }
        for tool in Tool::ALL {
            let (label, key) = match tool {
                Tool::Crop if self.is_inline() => ("Region", "S".to_owned()),
                _ => (
                    tool.label(),
                    key_text(self.keys.get(DrawAction::Tool(tool))),
                ),
            };
            let entry = self.menu_entry(label, &key, Item::Draw(DrawAction::Tool(tool)));
            if tool == self.tool.get() {
                entry.add_css_class("current");
            }
            list.append(&entry);
        }
        list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        for action in [DrawAction::Smaller, DrawAction::Larger] {
            list.append(&self.menu_entry(
                action.label(),
                &key_text(self.keys.get(action)),
                Item::Draw(action),
            ));
        }
        list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let document = self.document.borrow();
        let undo = self.menu_entry("Undo", "Ctrl+Z", Item::Undo);
        undo.set_sensitive(document.can_undo());
        list.append(&undo);
        let redo = self.menu_entry("Redo", "Ctrl+Shift+Z", Item::Redo);
        redo.set_sensitive(document.can_redo());
        list.append(&redo);
        if self.selected.get().is_some() {
            list.append(&self.menu_entry("Delete", "Del", Item::Delete));
        }
        list
    }

    fn menu_entry(self: &Rc<Self>, label: &str, key: &str, item: Item) -> gtk::Button {
        let line = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(24)
            .build();
        line.append(
            &gtk::Label::builder()
                .label(label)
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );
        line.append(
            &gtk::Label::builder()
                .label(key)
                .css_classes(["caps"])
                .build(),
        );
        let button = gtk::Button::builder()
            .child(&line)
            .css_classes(["menu-item"])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        let target = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            if let Some(editor) = target.upgrade() {
                editor.close_menu();
                match item {
                    Item::Draw(action) => editor.run(action),
                    Item::Undo => editor.undo(),
                    Item::Redo => editor.redo(),
                    Item::Delete => {
                        editor.delete_selected();
                    }
                }
            }
        });
        button
    }
}

fn key_text(key: DrawKey) -> String {
    match key {
        DrawKey::Key(_) => key.label(),
        DrawKey::Off => String::new(),
    }
}
