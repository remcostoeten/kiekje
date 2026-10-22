use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::Editor;
use super::document::Shape;
use super::document::Tool;
use super::toolbar;

const COLUMNS: u32 = 8;
const MAX_GLYPH_CHARS: usize = 16;

const CHOICES: [(&str, &str); 64] = [
    ("👍", "thumbs up yes good"),
    ("👎", "thumbs down no bad"),
    ("👏", "clap applause"),
    ("🙌", "celebrate hands up"),
    ("🙏", "please thanks pray"),
    ("💪", "strong muscle"),
    ("👀", "eyes look see"),
    ("🤔", "thinking hmm"),
    ("👉", "point right"),
    ("👈", "point left"),
    ("👆", "point up"),
    ("👇", "point down"),
    ("✋", "stop hand"),
    ("🤷", "shrug unsure"),
    ("🙈", "see no evil"),
    ("🤯", "mind blown"),
    ("😀", "smile happy"),
    ("😂", "laugh tears joy"),
    ("😍", "love heart eyes"),
    ("😎", "cool sunglasses"),
    ("😅", "sweat nervous"),
    ("😢", "sad cry"),
    ("😡", "angry mad"),
    ("😱", "scream shock"),
    ("🥳", "party celebrate"),
    ("😴", "sleep tired"),
    ("🤮", "sick gross"),
    ("🎉", "party popper tada"),
    ("🔥", "fire hot"),
    ("⭐", "star favourite"),
    ("✨", "sparkles new"),
    ("💥", "boom collision"),
    ("❤️", "heart love red"),
    ("💔", "broken heart"),
    ("💯", "hundred perfect"),
    ("💡", "idea light bulb"),
    ("⚠️", "warning caution"),
    ("❗", "exclamation important"),
    ("❓", "question"),
    ("✅", "check done yes"),
    ("❌", "cross no wrong"),
    ("⛔", "no entry"),
    ("🚫", "prohibited forbidden"),
    ("🚧", "construction wip"),
    ("🐛", "bug"),
    ("🔧", "wrench fix"),
    ("🔨", "hammer build"),
    ("🚀", "rocket launch ship"),
    ("📌", "pin"),
    ("📎", "paperclip attach"),
    ("🔒", "lock secure"),
    ("🔓", "unlock open"),
    ("🔑", "key"),
    ("⏰", "alarm clock time"),
    ("🎯", "target goal"),
    ("🏁", "finish flag"),
    ("🏆", "trophy win"),
    ("🔍", "search magnify"),
    ("💬", "comment speech"),
    ("📝", "note memo"),
    ("🔄", "refresh repeat"),
    ("➕", "plus add"),
    ("➖", "minus remove"),
    ("➡️", "arrow right next"),
];

fn matches(index: usize, query: &str) -> bool {
    query.is_empty() || CHOICES[index].1.contains(query)
}

fn first_match(query: &str) -> Option<&'static str> {
    (0..CHOICES.len())
        .find(|index| matches(*index, query))
        .map(|index| CHOICES[index].0)
}

fn typed_glyph(text: &str) -> Option<&str> {
    let text = text.trim();
    let wide = text
        .chars()
        .any(|each| !each.is_ascii() && !each.is_alphanumeric());
    (wide && text.chars().count() <= MAX_GLYPH_CHARS).then_some(text)
}

impl Editor {
    pub(super) fn emoji_picker(self: &Rc<Self>) -> gtk::Box {
        let picker = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["context-list", "emoji-picker"])
            .build();
        if self.is_inline() {
            picker.add_css_class("panel");
        }
        let search = gtk::Entry::builder()
            .placeholder_text("Search, or paste an emoji")
            .max_length(32)
            .build();
        search.update_property(&[gtk::accessible::Property::Label("Search emoji")]);
        let grid = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .min_children_per_line(COLUMNS)
            .max_children_per_line(COLUMNS)
            .column_spacing(2)
            .row_spacing(2)
            .css_classes(["emoji-grid"])
            .build();
        for (glyph, name) in CHOICES {
            let child = gtk::FlowBoxChild::new();
            child.set_child(Some(
                &gtk::Label::builder()
                    .label(glyph)
                    .css_classes(["emoji-choice"])
                    .build(),
            ));
            child.set_tooltip_text(Some(name));
            child.update_property(&[gtk::accessible::Property::Label(name)]);
            grid.append(&child);
        }

        let query = Rc::new(RefCell::new(String::new()));
        let shown = query.clone();
        grid.set_filter_func(move |child| {
            usize::try_from(child.index()).is_ok_and(|index| matches(index, &shown.borrow()))
        });
        let narrowed = grid.clone();
        let typed = query.clone();
        search.connect_changed(move |entry| {
            *typed.borrow_mut() = entry.text().trim().to_lowercase();
            narrowed.invalidate_filter();
        });

        let chosen = Rc::downgrade(self);
        grid.connect_child_activated(move |_, child| {
            let glyph = usize::try_from(child.index())
                .ok()
                .and_then(|index| CHOICES.get(index));
            if let (Some(editor), Some((glyph, _))) = (chosen.upgrade(), glyph) {
                editor.choose_emoji(glyph);
            }
        });
        let entered = Rc::downgrade(self);
        let asked = query.clone();
        search.connect_activate(move |entry| {
            let glyph = typed_glyph(&entry.text())
                .map(str::to_owned)
                .or_else(|| first_match(&asked.borrow()).map(str::to_owned));
            if let (Some(editor), Some(glyph)) = (entered.upgrade(), glyph) {
                editor.choose_emoji(&glyph);
            }
        });

        let keys = gtk::EventControllerKey::new();
        let below = grid.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key != gtk::gdk::Key::Down {
                return glib::Propagation::Proceed;
            }
            let first = (0..)
                .map_while(|index| below.child_at_index(index))
                .find(|child| child.is_child_visible());
            match first {
                Some(child) => {
                    child.grab_focus();
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        search.add_controller(keys);

        picker.append(&search);
        picker.append(&grid);
        picker
    }

    pub(super) fn open_emoji_picker(self: &Rc<Self>) {
        let (x, y) = self.view.get().to_widget(self.stamp_point());
        self.open_menu_with(self.emoji_picker(), x, y);
    }

    fn choose_emoji(self: &Rc<Self>, glyph: &str) {
        self.close_menu();
        self.emoji.replace(glyph.to_owned());
        toolbar::describe_emoji(&self.emoji_button, glyph);
        let replaced = self.replace_selected_emoji(glyph);
        if !replaced {
            self.pick(Tool::Emoji);
        }
        self.say(&format!("Emoji set to {glyph}"), false);
        self.canvas.queue_draw();
        self.canvas.grab_focus();
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
            Shape::Emoji { at, stroke, .. } => Shape::Emoji {
                at: *at,
                glyph: glyph.to_owned(),
                stroke: *stroke,
            },
            other => other.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_every_choice_findable_by_name() {
        for (glyph, name) in CHOICES {
            assert!(!glyph.is_empty() && !name.is_empty(), "{glyph}");
        }
        assert_eq!(first_match("fire"), Some("🔥"));
        assert_eq!(first_match("thumbs down"), Some("👎"));
        assert_eq!(first_match("nothing like this"), None);
    }

    #[test]
    fn shows_everything_for_an_empty_search() {
        assert!((0..CHOICES.len()).all(|index| matches(index, "")));
        assert_eq!(first_match(""), Some("👍"));
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
