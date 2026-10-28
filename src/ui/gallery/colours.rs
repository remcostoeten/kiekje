use std::cell::Cell;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gio, glib};

use super::{Gallery, cards};
use crate::library::{self, Colour, Library};

const SWATCH: i32 = 28;

pub(super) struct Palette {
    pub(super) page: gtk::Stack,
    list: gtk::ListBox,
    count: Cell<usize>,
}

pub(super) fn palette() -> Palette {
    let page = gtk::Stack::builder()
        .vexpand(true)
        .vhomogeneous(false)
        .build();
    page.add_named(
        &cards::empty_state(
            "No colours yet. Press C in the magnifier while selecting to copy a colour; it is listed here.",
        ),
        Some("empty"),
    );
    page.add_named(
        &cards::empty_state("The colour history could not be read. Details are in the log."),
        Some("failed"),
    );
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .activate_on_single_click(true)
        .valign(gtk::Align::Start)
        .css_classes(["colours"])
        .build();
    list.update_property(&[gtk::accessible::Property::Label("Copied colours")]);
    page.add_named(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build(),
        Some("items"),
    );
    Palette {
        page,
        list,
        count: Cell::new(0),
    }
}

impl Gallery {
    pub(super) fn show_colours(self: &Rc<Self>, on: bool) {
        self.pages
            .set_visible_child_name(if on { "colours" } else { "captures" });
        self.say("", false);
        if on {
            self.count
                .set_label(&colour_count_text(self.palette.count.get()));
            self.reload_colours();
        } else if self.loaded.get() {
            self.refresh();
        }
    }

    pub(super) fn showing_colours(&self) -> bool {
        self.pages.visible_child_name().as_deref() == Some("colours")
    }

    pub(super) fn reload_colours(self: &Rc<Self>) {
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let loaded = gio::spawn_blocking(|| Library::open(&library::path())?.colours()).await;
            match loaded {
                Ok(Ok(colours)) => gallery.fill_colours(&colours),
                Ok(Err(error)) => {
                    error!("gallery could not read the colour history: {error}");
                    gallery.palette.page.set_visible_child_name("failed");
                }
                Err(_) => {
                    error!("the colour history worker stopped");
                    gallery.palette.page.set_visible_child_name("failed");
                }
            }
        });
    }

    fn fill_colours(self: &Rc<Self>, colours: &[Colour]) {
        let list = &self.palette.list;
        list.remove_all();
        for colour in colours {
            list.append(&row(colour, Rc::downgrade(self)));
        }
        self.palette.count.set(colours.len());
        if self.showing_colours() {
            self.count.set_label(&colour_count_text(colours.len()));
        }
        self.palette
            .page
            .set_visible_child_name(if colours.is_empty() { "empty" } else { "items" });
    }

    fn copy_colour(&self, text: &str) {
        self.clipboard().set_text(text);
        self.say(&format!("Copied {text}"), false);
    }

    fn remove_colour(self: &Rc<Self>, colour: Colour) {
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let removed = gio::spawn_blocking(move || {
                Library::open(&library::path())?.remove_colour(colour.id)
            })
            .await;
            match removed {
                Ok(Ok(_)) => gallery.say(&format!("Removed {}", colour.hex()), false),
                Ok(Err(error)) => {
                    gallery.say(
                        &format!("The colour history was not updated: {error}"),
                        true,
                    );
                }
                Err(_) => gallery.say("The colour history worker stopped", true),
            }
            gallery.reload_colours();
        });
    }
}

fn row(colour: &Colour, gallery: Weak<Gallery>) -> gtk::ListBoxRow {
    let colour = *colour;
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    content.append(&swatch(colour.rgb));
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    text.append(
        &gtk::Label::builder()
            .label(colour.hex())
            .xalign(0.0)
            .css_classes(["hex"])
            .build(),
    );
    text.append(
        &gtk::Label::builder()
            .label(format!("{}  {}", colour.css_rgb(), picked_text(colour)))
            .xalign(0.0)
            .css_classes(["shortcut"])
            .build(),
    );
    content.append(&text);
    let hex = colour.hex();
    let css = colour.css_rgb();
    content.append(&button("Copy RGB", &gallery, move |gallery| {
        gallery.copy_colour(&css);
    }));
    content.append(&button("Remove", &gallery, move |gallery| {
        gallery.remove_colour(colour);
    }));
    let row = gtk::ListBoxRow::builder()
        .child(&content)
        .tooltip_text(format!("Copy {hex}"))
        .build();
    row.update_property(&[gtk::accessible::Property::Label(&format!("Copy {hex}"))]);
    row.connect_activate(move |_| {
        if let Some(gallery) = gallery.upgrade() {
            gallery.copy_colour(&hex);
        }
    });
    row
}

fn button(
    label: &str,
    gallery: &Weak<Gallery>,
    on_click: impl Fn(&Rc<Gallery>) + 'static,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .css_classes(["ghost", "compact"])
        .build();
    let gallery = gallery.clone();
    button.connect_clicked(move |_| {
        if let Some(gallery) = gallery.upgrade() {
            on_click(&gallery);
        }
    });
    button
}

fn swatch(rgb: [u8; 3]) -> gtk::DrawingArea {
    let swatch = gtk::DrawingArea::builder()
        .content_width(SWATCH)
        .content_height(SWATCH)
        .valign(gtk::Align::Center)
        .build();
    swatch.set_draw_func(move |_, cr, width, height| {
        let [red, green, blue] = rgb.map(|channel| f64::from(channel) / 255.0);
        cr.set_source_rgb(red, green, blue);
        cr.rectangle(0.5, 0.5, f64::from(width) - 1.0, f64::from(height) - 1.0);
        let filled = cr.fill_preserve();
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
        cr.set_line_width(1.0);
        if let Err(error) = filled.and_then(|()| cr.stroke()) {
            warn!("a colour swatch was not drawn: {error}");
        }
    });
    swatch
}

fn picked_text(colour: Colour) -> String {
    glib::DateTime::from_unix_local(colour.picked_us / 1_000_000)
        .and_then(|date| date.format("%Y-%m-%d %H:%M"))
        .map(String::from)
        .unwrap_or_default()
}

fn colour_count_text(count: usize) -> String {
    match count {
        1 => "1 colour".to_owned(),
        _ => format!("{count} colours"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_colours_in_words() {
        assert_eq!(colour_count_text(0), "0 colours");
        assert_eq!(colour_count_text(1), "1 colour");
        assert_eq!(colour_count_text(12), "12 colours");
    }
}
