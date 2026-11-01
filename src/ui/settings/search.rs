use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::ui::widgets::keycap;

const FLASH: Duration = Duration::from_millis(300);
const RESULTS_PAGE: &str = "results";

pub(super) struct Indexed {
    pub name: &'static str,
    pub title: &'static str,
    pub content: gtk::Box,
}

pub(super) struct NavItem {
    pub name: &'static str,
    pub button: gtk::ToggleButton,
    pub hits: gtk::Label,
}

pub(super) struct Results {
    pub title: gtk::Label,
    pub subtitle: gtk::Label,
    pub list: gtk::Box,
}

struct Entry {
    page: usize,
    row: gtk::Widget,
    title: String,
    subtitle: String,
}

struct Hit<'a> {
    entry: &'a Entry,
    index: usize,
}

pub(super) struct Search {
    widget: gtk::Overlay,
    entry: gtk::Entry,
    focused: Cell<bool>,
    stack: gtk::Stack,
    pages: RefCell<Vec<Indexed>>,
    entries: RefCell<Vec<Entry>>,
    nav: RefCell<Vec<NavItem>>,
    results: RefCell<Option<Results>>,
}

impl Search {
    /// Builds the sidebar search field; call [`Search::index`] once every page exists.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let search = Search::new(&stack);
    /// sidebar.append(search.widget());
    /// ```
    pub(super) fn new(stack: &gtk::Stack) -> Rc<Self> {
        let entry = gtk::Entry::builder()
            .placeholder_text("Search")
            .primary_icon_name("kiekje-search-symbolic")
            .primary_icon_activatable(false)
            .primary_icon_sensitive(false)
            .hexpand(true)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label("Search settings")]);
        let slash = keycap("/");
        slash.set_halign(gtk::Align::End);
        slash.set_margin_end(6);
        slash.set_can_target(false);
        let widget = gtk::Overlay::builder()
            .child(&entry)
            .css_classes(["search"])
            .build();
        widget.add_overlay(&slash);
        let search = Rc::new(Self {
            widget,
            entry,
            focused: Cell::new(false),
            stack: stack.clone(),
            pages: RefCell::new(Vec::new()),
            entries: RefCell::new(Vec::new()),
            nav: RefCell::new(Vec::new()),
            results: RefCell::new(None),
        });
        search.connect(slash);
        search
    }

    pub(super) fn widget(&self) -> &gtk::Overlay {
        &self.widget
    }

    /// Reads every preference row on the pages so the field can find them.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// search.index(pages, nav, results);
    /// ```
    pub(super) fn index(&self, pages: Vec<Indexed>, nav: Vec<NavItem>, results: Results) {
        let mut entries = Vec::new();
        for (page, indexed) in pages.iter().enumerate() {
            for row in rows(indexed.content.upcast_ref()) {
                let title = text_of(&row, "row-title");
                let subtitle = text_of(&row, "subtitle");
                if title.is_empty() {
                    continue;
                }
                entries.push(Entry {
                    page,
                    row,
                    title,
                    subtitle,
                });
            }
        }
        self.pages.replace(pages);
        self.entries.replace(entries);
        self.nav.replace(nav);
        self.results.replace(Some(results));
    }

    /// Moves the keyboard focus into the field, for the `/` shortcut.
    pub(super) fn focus(&self) {
        self.entry.grab_focus();
    }

    fn connect(self: &Rc<Self>, slash: gtk::Label) {
        let weak = Rc::downgrade(self);
        let hint = slash.clone();
        self.entry.connect_changed(move |entry| {
            let Some(search) = weak.upgrade() else {
                return;
            };
            hint.set_visible(!search.focused.get() && entry.text().is_empty());
            search.run(&entry.text());
        });
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        let hide = slash.clone();
        focus.connect_enter(move |_| {
            if let Some(search) = weak.upgrade() {
                search.focused.set(true);
            }
            hide.set_visible(false);
        });
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            if let Some(search) = weak.upgrade() {
                search.focused.set(false);
                slash.set_visible(search.entry.text().is_empty());
            }
        });
        self.entry.add_controller(focus);

        let weak = Rc::downgrade(self);
        self.entry.connect_activate(move |_| {
            if let Some(search) = weak.upgrade() {
                search.open_first();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(search) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Escape => {
                    search.clear();
                    search.focus_current_nav();
                    glib::Propagation::Stop
                }
                gdk::Key::Down => {
                    search.focus_first_result();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.entry.add_controller(keys);
    }

    fn run(self: &Rc<Self>, query: &str) {
        let query = query.trim().to_lowercase();
        let nav = self.nav.borrow();
        if query.is_empty() {
            for item in nav.iter() {
                item.button.remove_css_class("dim");
                item.hits.set_label("");
                item.hits.set_visible(false);
            }
            self.show_current();
            return;
        }
        let entries = self.entries.borrow();
        let hits: Vec<Hit> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| matches(entry, &query))
            .map(|(index, entry)| Hit { entry, index })
            .collect();
        let pages = self.pages.borrow();
        for item in nav.iter() {
            let page = pages.iter().position(|page| page.name == item.name);
            let count = hits
                .iter()
                .filter(|hit| Some(hit.entry.page) == page)
                .count();
            if count == 0 {
                item.button.add_css_class("dim");
            } else {
                item.button.remove_css_class("dim");
            }
            item.hits.set_label(&count.to_string());
            item.hits.set_visible(count > 0);
        }
        self.render(&query, &hits, &pages);
        self.stack.set_visible_child_name(RESULTS_PAGE);
    }

    fn render(self: &Rc<Self>, query: &str, hits: &[Hit], pages: &[Indexed]) {
        let results = self.results.borrow();
        let Some(results) = results.as_ref() else {
            return;
        };
        results.title.set_label(&format!("\u{201c}{query}\u{201d}"));
        results.subtitle.set_label(&lead(hits.len()));
        while let Some(child) = results.list.first_child() {
            results.list.remove(&child);
        }
        if hits.is_empty() {
            results.list.append(
                &gtk::Label::builder()
                    .label("No settings found")
                    .css_classes(["no-results"])
                    .build(),
            );
            return;
        }
        for hit in hits {
            let page = pages.get(hit.entry.page).map_or("", |page| page.title);
            results
                .list
                .append(&result_button(hit, page, query, Rc::downgrade(self)));
        }
    }

    fn clear(&self) {
        if self.entry.text().is_empty() {
            self.show_current();
            return;
        }
        self.entry.set_text("");
    }

    fn show_current(&self) {
        let nav = self.nav.borrow();
        if let Some(item) = nav.iter().find(|item| item.button.is_active()) {
            self.stack.set_visible_child_name(item.name);
        }
    }

    fn focus_current_nav(&self) {
        let nav = self.nav.borrow();
        if let Some(item) = nav.iter().find(|item| item.button.is_active()) {
            item.button.grab_focus();
        }
    }

    fn focus_first_result(&self) {
        let results = self.results.borrow();
        if let Some(first) = results
            .as_ref()
            .and_then(|results| results.list.first_child())
        {
            first.grab_focus();
        }
    }

    fn open_first(&self) {
        let query = self.entry.text().trim().to_lowercase();
        if query.is_empty() {
            return;
        }
        let first = self
            .entries
            .borrow()
            .iter()
            .position(|entry| matches(entry, &query));
        if let Some(index) = first {
            self.open(index);
        }
    }

    fn open(&self, index: usize) {
        let (page, row) = {
            let entries = self.entries.borrow();
            let Some(entry) = entries.get(index) else {
                return;
            };
            (entry.page, entry.row.clone())
        };
        let name = self.pages.borrow().get(page).map(|page| page.name);
        self.entry.set_text("");
        if let Some(name) = name
            && let Some(item) = self.nav.borrow().iter().find(|item| item.name == name)
        {
            item.button.set_active(true);
        }
        reveal(&row);
    }
}

/// Lets `/` anywhere in the window focus the search field, unless text is being typed.
///
/// The controller keeps the search alive for as long as the window exists.
///
/// # Examples
///
/// ```ignore
/// search::install_shortcut(&window, &search);
/// ```
pub(super) fn install_shortcut(window: &gtk::Window, search: Rc<Search>) {
    let keys = gtk::EventControllerKey::new();
    let root = window.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let plain =
            !state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK);
        if key != gdk::Key::slash || !plain || typing(&root) {
            return glib::Propagation::Proceed;
        }
        search.focus();
        glib::Propagation::Stop
    });
    window.add_controller(keys);
}

fn typing(window: &gtk::Window) -> bool {
    GtkWindowExt::focus(window)
        .is_some_and(|focus| focus.is::<gtk::Text>() || focus.is::<gtk::TextView>())
}

fn matches(entry: &Entry, query: &str) -> bool {
    entry.title.to_lowercase().contains(query) || entry.subtitle.to_lowercase().contains(query)
}

fn lead(count: usize) -> String {
    match count {
        0 => "Nothing matches. Try a shorter word.".to_owned(),
        1 => "1 setting matches. Enter or click opens it.".to_owned(),
        count => format!("{count} settings match. Enter or click opens one."),
    }
}

fn result_button(hit: &Hit, page: &str, query: &str, search: Weak<Search>) -> gtk::Button {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    content.append(
        &gtk::Label::builder()
            .label(page)
            .valign(gtk::Align::Center)
            .css_classes(["tag"])
            .build(),
    );
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();
    let title = gtk::Label::builder()
        .use_markup(true)
        .label(mark(&hit.entry.title, query))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["row-title"])
        .build();
    text.append(&title);
    if !hit.entry.subtitle.is_empty() {
        text.append(
            &gtk::Label::builder()
                .use_markup(true)
                .label(mark(&hit.entry.subtitle, query))
                .xalign(0.0)
                .wrap(true)
                .css_classes(["subtitle"])
                .build(),
        );
    }
    content.append(&text);
    content.append(
        &gtk::Image::builder()
            .icon_name("kiekje-open-symbolic")
            .pixel_size(14)
            .valign(gtk::Align::Center)
            .build(),
    );
    let button = gtk::Button::builder()
        .child(&content)
        .css_classes(["result"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "{}, on the {page} page",
        hit.entry.title
    ))]);
    let index = hit.index;
    button.connect_clicked(move |_| {
        if let Some(search) = search.upgrade() {
            search.open(index);
        }
    });
    button
}

fn rows(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(widget) = pending.pop() {
        if widget.has_css_class("pref") || widget.has_css_class("choice") {
            found.push(widget);
            continue;
        }
        let mut child = widget.last_child();
        while let Some(current) = child {
            child = current.prev_sibling();
            pending.push(current);
        }
    }
    found
}

fn text_of(row: &gtk::Widget, class: &str) -> String {
    let mut pending = vec![row.clone()];
    while let Some(widget) = pending.pop() {
        if widget.has_css_class(class)
            && let Some(label) = widget.downcast_ref::<gtk::Label>()
        {
            return label.text().to_string();
        }
        let mut child = widget.last_child();
        while let Some(current) = child {
            child = current.prev_sibling();
            pending.push(current);
        }
    }
    String::new()
}

fn reveal(row: &gtk::Widget) {
    let target = control_in(row).unwrap_or_else(|| row.clone());
    target.grab_focus();
    row.add_css_class("flash");
    let row = row.clone();
    glib::timeout_add_local_once(FLASH, move || row.remove_css_class("flash"));
}

fn control_in(row: &gtk::Widget) -> Option<gtk::Widget> {
    let mut pending = vec![row.clone()];
    while let Some(widget) = pending.pop() {
        if widget.is_sensitive()
            && widget.is_focusable()
            && !widget.is::<gtk::ListBoxRow>()
            && !widget.is::<gtk::Label>()
        {
            return Some(widget);
        }
        let mut child = widget.last_child();
        while let Some(current) = child {
            child = current.prev_sibling();
            pending.push(current);
        }
    }
    None
}

fn mark(text: &str, query: &str) -> String {
    let lower = text.to_lowercase();
    let mut output = String::new();
    let mut rest = 0;
    if !query.is_empty() {
        while let Some(found) = lower[rest..].find(query) {
            let start = rest + found;
            let end = start + query.len();
            if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                break;
            }
            output.push_str(&glib::markup_escape_text(&text[rest..start]));
            output.push_str("<b>");
            output.push_str(&glib::markup_escape_text(&text[start..end]));
            output.push_str("</b>");
            rest = end;
        }
    }
    output.push_str(&glib::markup_escape_text(&text[rest..]));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bolds_every_match_and_escapes_markup() {
        assert_eq!(
            mark("Edit a <region>", "edit"),
            "<b>Edit</b> a &lt;region&gt;"
        );
        assert_eq!(
            mark("hints and Hints", "hint"),
            "<b>hint</b>s and <b>Hint</b>s"
        );
        assert_eq!(mark("plain", ""), "plain");
    }

    #[test]
    fn counts_hits_in_words() {
        assert_eq!(lead(0), "Nothing matches. Try a shorter word.");
        assert_eq!(lead(1), "1 setting matches. Enter or click opens it.");
        assert_eq!(lead(3), "3 settings match. Enter or click opens one.");
    }
}
