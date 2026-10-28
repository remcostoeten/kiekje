use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::library::{self, Library};

use super::Editor;
use super::document::Shape;
use super::document::Tool;
use super::emoji_index;
use super::toolbar;

const COLUMNS: u32 = 8;
const MAX_GLYPH_CHARS: usize = 16;
const SHOWN: usize = 64;
const RECENT: usize = COLUMNS as usize;

struct Picker {
    search: gtk::SearchEntry,
    recent: gtk::FlowBox,
    recent_row: gtk::Box,
    grid: gtk::FlowBox,
    caption: gtk::Label,
}

fn first_match(query: &str) -> Option<&'static str> {
    emoji_index::search(query, 1)
        .first()
        .map(|entry| entry.glyph)
}

fn choice(glyph: &str, caption: &gtk::Label) -> gtk::FlowBoxChild {
    let child = gtk::FlowBoxChild::new();
    child.set_child(Some(
        &gtk::Label::builder()
            .label(glyph)
            .css_classes(["emoji-choice"])
            .build(),
    ));
    let label = emoji_index::find(glyph).map_or_else(|| glyph.to_owned(), |entry| entry.label());
    child.set_tooltip_text(Some(&label));
    child.update_property(&[gtk::accessible::Property::Label(&label)]);
    let focused = gtk::EventControllerFocus::new();
    let named = caption.clone();
    let name = label.clone();
    focused.connect_enter(move |_| named.set_label(&name));
    child.add_controller(focused);
    let hovered = gtk::EventControllerMotion::new();
    let named = caption.clone();
    hovered.connect_enter(move |_, _, _| named.set_label(&label));
    child.add_controller(hovered);
    child
}

fn glyph_of(child: &gtk::FlowBoxChild) -> Option<glib::GString> {
    child
        .child()
        .and_downcast::<gtk::Label>()
        .map(|label| label.text())
}

fn focus_first(grid: &gtk::FlowBox) -> bool {
    grid.child_at_index(0)
        .is_some_and(|child| child.grab_focus())
}

impl Picker {
    fn fill(&self, query: &str) {
        let searching = !query.trim().is_empty();
        self.recent_row
            .set_visible(!searching && self.recent.child_at_index(0).is_some());
        self.grid.remove_all();
        let found = emoji_index::search(query, SHOWN);
        for entry in &found {
            self.grid.append(&choice(entry.glyph, &self.caption));
        }
        let caption = match (searching, found.len()) {
            (false, _) => "Type to search in English or Dutch, or an emoticon like :p",
            (true, 0) => "No emoji matches. Paste one to use it anyway",
            (true, 1) => "1 match, Enter picks it",
            (true, _) => "Enter picks the first match",
        };
        self.caption.set_label(caption);
    }

    fn fill_recent(&self, glyphs: &[String]) {
        self.recent.remove_all();
        for glyph in glyphs.iter().take(RECENT) {
            self.recent.append(&choice(glyph, &self.caption));
        }
        self.recent_row.set_visible(!glyphs.is_empty());
    }

    fn connect_keys(&self, editor: &Rc<Editor>) {
        let (search, recent, grid) = (self.search.clone(), self.recent.clone(), self.grid.clone());
        let below = recent.clone();
        let further = grid.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            let moved = key == gtk::gdk::Key::Down
                && ((below.is_visible() && focus_first(&below)) || focus_first(&further));
            if moved {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.search.add_controller(keys);

        let (above, below) = (search.clone(), grid.clone());
        self.recent.connect_keynav_failed(move |_, direction| {
            let moved = match direction {
                gtk::DirectionType::Up | gtk::DirectionType::TabBackward => above.grab_focus(),
                gtk::DirectionType::Down | gtk::DirectionType::TabForward => focus_first(&below),
                _ => false,
            };
            if moved {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        let (above, row) = (search.clone(), recent.clone());
        self.grid.connect_keynav_failed(move |_, direction| {
            let moved = match direction {
                gtk::DirectionType::Up | gtk::DirectionType::TabBackward => {
                    (row.is_visible() && focus_first(&row)) || above.grab_focus()
                }
                _ => false,
            };
            if moved {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });

        for flow in [&self.recent, &self.grid] {
            let chosen = Rc::downgrade(editor);
            flow.connect_child_activated(move |_, child| {
                if let (Some(editor), Some(glyph)) = (chosen.upgrade(), glyph_of(child)) {
                    editor.choose_emoji(&glyph);
                }
            });
        }
        let entered = Rc::downgrade(editor);
        self.search.connect_activate(move |entry| {
            let glyph = typed_glyph(&entry.text())
                .map(str::to_owned)
                .or_else(|| first_match(&entry.text()).map(str::to_owned));
            if let (Some(editor), Some(glyph)) = (entered.upgrade(), glyph) {
                editor.choose_emoji(&glyph);
            }
        });
        let stopped = Rc::downgrade(editor);
        self.search.connect_stop_search(move |_| {
            if let Some(editor) = stopped.upgrade() {
                editor.close_menu();
            }
        });
    }
}

fn typed_glyph(text: &str) -> Option<&str> {
    let text = text.trim();
    let wide = text
        .chars()
        .any(|each| !each.is_ascii() && !each.is_alphanumeric());
    (wide && text.chars().count() <= MAX_GLYPH_CHARS).then_some(text)
}

fn flow(name: &str) -> gtk::FlowBox {
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(COLUMNS)
        .max_children_per_line(COLUMNS)
        .column_spacing(2)
        .row_spacing(2)
        .css_classes(["emoji-grid"])
        .build();
    flow.update_property(&[gtk::accessible::Property::Label(name)]);
    flow
}

impl Editor {
    pub(super) fn emoji_picker(self: &Rc<Self>, typed: &str) -> gtk::Box {
        let picker = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["context-list", "emoji-picker"])
            .build();
        if self.is_inline() {
            picker.add_css_class("panel");
        }
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search in English or Dutch, or paste an emoji")
            .build();
        search.update_property(&[gtk::accessible::Property::Label("Search emoji")]);
        search.set_key_capture_widget(Some(&picker));
        let recent_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .visible(false)
            .build();
        recent_row.append(
            &gtk::Label::builder()
                .label("Recent")
                .xalign(0.0)
                .css_classes(["section-title"])
                .build(),
        );
        let recent = flow("Recent emoji");
        recent_row.append(&recent);
        let grid = flow("Emoji");
        let caption = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["note"])
            .build();
        let parts = Picker {
            search: search.clone(),
            recent,
            recent_row: recent_row.clone(),
            grid: grid.clone(),
            caption: caption.clone(),
        };
        parts.fill_recent(&self.recent_emoji.borrow());
        parts.fill(typed);
        parts.connect_keys(self);
        let parts = Rc::new(parts);
        let changed = parts.clone();
        search.connect_search_changed(move |entry| changed.fill(entry.text().as_str()));
        search.set_text(typed);
        search.set_position(-1);

        picker.append(&search);
        picker.append(&recent_row);
        picker.append(&grid);
        picker.append(&caption);
        picker.append(&self.keep_open_row());
        picker
    }

    fn keep_open_row(self: &Rc<Self>) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        let label = gtk::Label::builder()
            .label("Keep open after picking")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["note"])
            .build();
        let keep = gtk::Switch::builder()
            .active(self.keep_emoji_open.get())
            .valign(gtk::Align::Center)
            .build();
        keep.update_property(&[gtk::accessible::Property::Label("Keep open after picking")]);
        let target = Rc::downgrade(self);
        keep.connect_active_notify(move |keep| {
            if let Some(editor) = target.upgrade() {
                editor.keep_emoji_open.set(keep.is_active());
                editor.pin_menu(keep.is_active());
            }
        });
        row.append(&label);
        row.append(&keep);
        row
    }

    pub(super) fn open_emoji_picker(self: &Rc<Self>) {
        self.open_emoji_picker_with("");
    }

    pub(super) fn open_emoji_picker_with(self: &Rc<Self>, typed: &str) {
        let (x, y) = self.view.get().to_widget(self.stamp_point());
        self.open_emoji_menu(typed, x, y);
    }

    pub(super) fn open_emoji_menu(self: &Rc<Self>, typed: &str, x: f64, y: f64) {
        self.open_menu_with(self.emoji_picker(typed), x, y);
        self.pin_menu(self.keep_emoji_open.get());
    }

    pub(super) fn load_recent_emoji(self: &Rc<Self>) {
        let target = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let loaded = gio::spawn_blocking(|| {
                Library::open(&library::path()).and_then(|library| library.recent_emoji(RECENT))
            })
            .await;
            match (target.upgrade(), loaded) {
                (Some(editor), Ok(Ok(glyphs))) => {
                    editor.recent_emoji.replace(glyphs);
                }
                (_, Ok(Err(error))) => error!("could not read recent emoji: {error}"),
                _ => {}
            }
        });
    }

    fn choose_emoji(self: &Rc<Self>, glyph: &str) {
        let kept = self.menu_pinned.get();
        if !kept {
            self.close_menu();
        }
        self.emoji.replace(glyph.to_owned());
        toolbar::describe_emoji(&self.emoji_button, glyph);
        self.note_recent_emoji(glyph);
        let replaced = self.replace_selected_emoji(glyph);
        if !replaced {
            self.pick(Tool::Emoji);
        }
        self.say(&format!("Emoji set to {glyph}"), false);
        self.canvas.queue_draw();
        if !kept {
            self.canvas.grab_focus();
        }
    }

    fn note_recent_emoji(&self, glyph: &str) {
        let mut recent = self.recent_emoji.borrow_mut();
        recent.retain(|known| known != glyph);
        recent.insert(0, glyph.to_owned());
        recent.truncate(RECENT);
        let glyph = glyph.to_owned();
        let used_us = glib::real_time();
        gio::spawn_blocking(move || {
            let noted = Library::open(&library::path())
                .and_then(|library| library.use_emoji(&glyph, used_us));
            if let Err(error) = noted {
                error!("could not remember the emoji: {error}");
            }
        });
    }
    fn replace_selected_emoji(&self, glyph: &str) -> bool {
        let selected = self.selected.get().and_then(|index| {
            self.document
                .borrow()
                .shapes()
                .get(index)
                .map(|shape| matches!(shape, Shape::Emoji { .. }))
        });
        if selected != Some(true) {
            return false;
        }
        self.replace_selected(|shape| match shape {
            Shape::Emoji {
                at, stroke, angle, ..
            } => Shape::Emoji {
                at: *at,
                glyph: glyph.to_owned(),
                stroke: *stroke,
                angle: *angle,
            },
            other => other.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_best_match_on_enter() {
        assert_eq!(first_match("fire"), Some("🔥"));
        assert_eq!(first_match("vuur"), Some("🔥"));
        assert_eq!(first_match(""), Some("👍"));
        assert_eq!(first_match("nothing like this"), None);
    }

    #[test]
    fn accepts_a_pasted_emoji_but_not_a_word() {
        assert_eq!(typed_glyph(" 🦀 "), Some("🦀"));
        assert_eq!(typed_glyph("👩‍💻"), Some("👩‍💻"));
        assert_eq!(typed_glyph("fire"), None);
        assert_eq!(typed_glyph("café"), None);
        assert_eq!(typed_glyph(""), None);
        assert_eq!(typed_glyph("🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀🦀"), None);
    }
}
