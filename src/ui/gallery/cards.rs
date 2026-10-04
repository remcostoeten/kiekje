use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use super::{Gallery, actions};
use crate::library::Capture;
use crate::storage;

const CARD_WIDTH: i32 = 220;
const CARD_HEIGHT: i32 = 140;
const ROW_WIDTH: i32 = 72;
const ROW_HEIGHT: i32 = 48;

#[derive(Clone, Copy)]
enum Shape {
    Card,
    Row,
}

pub(super) fn grid(gallery: &Rc<Gallery>) -> gtk::GridView {
    let grid = gtk::GridView::builder()
        .model(&gallery.selection)
        .factory(&factory(gallery, Shape::Card))
        .max_columns(8)
        .min_columns(1)
        .single_click_activate(false)
        .build();
    grid.connect_activate(|grid, position| open_at(grid.model(), position));
    grid
}

pub(super) fn list(gallery: &Rc<Gallery>) -> gtk::ListView {
    let list = gtk::ListView::builder()
        .model(&gallery.selection)
        .factory(&factory(gallery, Shape::Row))
        .single_click_activate(false)
        .build();
    list.connect_activate(|list, position| open_at(list.model(), position));
    list
}

fn factory(gallery: &Rc<Gallery>, shape: Shape) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    let menus = Rc::downgrade(gallery);
    factory.connect_setup(move |_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let widget = item_widget(shape);
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        let target = item.downgrade();
        let menus = menus.clone();
        click.connect_pressed(move |click, _, x, y| {
            let (Some(item), Some(gallery), Some(widget)) =
                (target.upgrade(), menus.upgrade(), click.widget())
            else {
                return;
            };
            gallery.selection.set_selected(item.position());
            let menus = gallery.menus.borrow();
            let menu = menus
                .iter()
                .find(|menu| menu.parent().is_some_and(|view| widget.is_ancestor(&view)));
            if let Some(menu) = menu {
                actions::show_at(menu, &widget, x, y);
            }
        });
        widget.add_controller(click);
        item.set_child(Some(&widget));
    });
    let binding = Rc::downgrade(gallery);
    factory.connect_bind(move |_, object| {
        let (Some(item), Some(gallery)) =
            (object.downcast_ref::<gtk::ListItem>(), binding.upgrade())
        else {
            return;
        };
        let (Some(path), Some((picture, name, meta))) = (bound_path(item), parts(item)) else {
            return;
        };
        let details = gallery.details.borrow();
        let capture = details.get(&path);
        name.set_label(
            &capture
                .and_then(|capture| capture.path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        meta.set_label(&capture.map(meta_text).unwrap_or_default());
        item.set_accessible_label(&name.label());
        drop(details);
        gallery.thumbnail(item, &picture, path);
    });
    factory
}

fn open_at(model: Option<gtk::SelectionModel>, position: u32) {
    let Some(path) = model
        .and_then(|model| model.item(position))
        .and_downcast::<gtk::StringObject>()
        .map(|object| object.string())
    else {
        return;
    };
    let uri = gio::File::for_path(path.as_str()).uri();
    glib::spawn_future_local(async move {
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
        {
            eprintln!("kiekje: could not open {uri}: {error}");
        }
    });
}

fn item_widget(shape: Shape) -> gtk::Box {
    let (orientation, width, height, class) = match shape {
        Shape::Card => (gtk::Orientation::Vertical, CARD_WIDTH, CARD_HEIGHT, "card"),
        Shape::Row => (gtk::Orientation::Horizontal, ROW_WIDTH, ROW_HEIGHT, "row"),
    };
    let widget = gtk::Box::builder()
        .orientation(orientation)
        .spacing(8)
        .css_classes([class])
        .build();
    widget.append(
        &gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(width)
            .height_request(height)
            .css_classes(["skeleton"])
            .build(),
    );
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .valign(gtk::Align::Center)
        .build();
    text.append(
        &gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(1)
            .hexpand(true)
            .build(),
    );
    text.append(
        &gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["shortcut"])
            .build(),
    );
    widget.append(&text);
    widget
}

fn parts(item: &gtk::ListItem) -> Option<(gtk::Picture, gtk::Label, gtk::Label)> {
    let picture = item.child()?.first_child()?;
    let name = picture.next_sibling()?.first_child()?;
    let meta = name.next_sibling()?;
    Some((
        picture.downcast().ok()?,
        name.downcast().ok()?,
        meta.downcast().ok()?,
    ))
}

pub(super) fn bound_path(item: &gtk::ListItem) -> Option<String> {
    item.item()
        .and_downcast::<gtk::StringObject>()
        .map(|object| object.string().into())
}

pub(super) fn show_texture(picture: &gtk::Picture, texture: &gdk::Texture) {
    picture.remove_css_class("skeleton");
    picture.set_paintable(Some(texture));
}

pub(super) fn loading_state() -> gtk::Box {
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(8)
        .can_focus(false)
        .valign(gtk::Align::Start)
        .build();
    for _ in 0..8 {
        grid.append(
            &gtk::Box::builder()
                .width_request(CARD_WIDTH)
                .height_request(CARD_HEIGHT)
                .css_classes(["skeleton"])
                .build(),
        );
    }
    let state = gtk::Box::new(gtk::Orientation::Vertical, 0);
    state.append(&grid);
    state
}

pub(super) fn empty_state(text: &str) -> gtk::Box {
    let state = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Start)
        .build();
    state.append(
        &gtk::Label::builder()
            .label(text)
            .wrap(true)
            .css_classes(["empty-recent"])
            .build(),
    );
    state
}

fn meta_text(capture: &Capture) -> String {
    if capture.missing {
        let folder = capture
            .path
            .parent()
            .map(storage::tilde)
            .unwrap_or_default();
        return format!("Missing from {folder}");
    }
    let date = glib::DateTime::from_unix_local(capture.created_us / 1_000_000)
        .and_then(|date| date.format("%Y-%m-%d %H:%M"))
        .map(String::from)
        .unwrap_or_default();
    format!(
        "{} × {}  {}  {}",
        capture.width,
        capture.height,
        size_text(capture.bytes),
        date
    )
}

fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_sizes_in_decimal_units() {
        assert_eq!(size_text(0), "0 B");
        assert_eq!(size_text(999), "999 B");
        assert_eq!(size_text(1_000), "1.0 KB");
        assert_eq!(size_text(245_300), "245 KB");
        assert_eq!(size_text(3_450_000), "3.5 MB");
        assert_eq!(size_text(12_000_000_000_000), "12000 GB");
    }
}
