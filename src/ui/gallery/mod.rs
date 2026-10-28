mod actions;
mod cards;
mod colours;
mod loader;
mod rename;
mod toolbar;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::library::query::{self, Query};
use crate::library::{self, Capture, Library};
use crate::ui;
use crate::ui::widgets::{Failure, StatusBar};

use loader::{Textures, Thumbnails};
use toolbar::{Change, View};

struct Gallery {
    paths: gtk::StringList,
    selection: gtk::SingleSelection,
    menus: RefCell<Vec<gtk::PopoverMenu>>,
    actions: OnceCell<gio::SimpleActionGroup>,
    missing: OnceCell<gtk::ToggleButton>,
    status: gtk::Label,
    spinner: gtk::Spinner,
    footer: gtk::Box,
    banner: StatusBar,
    pending: RefCell<Option<String>>,
    all: RefCell<Vec<Rc<Capture>>>,
    details: RefCell<HashMap<Rc<str>, Rc<Capture>>>,
    listed: RefCell<Vec<Rc<str>>>,
    queue: RefCell<Thumbnails>,
    recheck: Cell<bool>,
    query: RefCell<Query>,
    textures: RefCell<Textures>,
    loading: Cell<bool>,
    stale: Cell<bool>,
    loaded: Cell<bool>,
    stack: gtk::Stack,
    views: gtk::Stack,
    pages: gtk::Stack,
    palette: colours::Palette,
    count: gtk::Label,
}

/// Opens the gallery window with every saved capture that is still on disk, newest first.
///
/// A sidebar switches between all captures, missing ones and the copied colours, tabs filter by kind, and the search
/// row filters by name and age, sorts, and switches between a grid and a list. A filter row narrows by dimensions,
/// file size, annotated captures and unsaved ones. Thumbnails load off the main thread as items scroll into view. The library is read again
/// whenever the window becomes active.
///
/// # Examples
///
/// ```ignore
/// let (window, handle) = gallery::show(&app);
/// ```
pub fn show(app: &gtk::Application) -> (gtk::Window, Handle) {
    ui::load_stylesheet();
    let window = gtk::Window::builder()
        .application(app)
        .title("Kiekje gallery")
        .default_width(980)
        .default_height(680)
        .css_classes(["settings-window"])
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .hexpand(true)
        .css_classes(["main"])
        .build();
    let header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    header.append(
        &gtk::Label::builder()
            .label("Gallery")
            .xalign(0.0)
            .css_classes(["title"])
            .build(),
    );
    let count = gtk::Label::builder()
        .valign(gtk::Align::Baseline)
        .css_classes(["section-title", "count"])
        .build();
    header.append(&count);
    content.append(&header);

    let stack = gtk::Stack::builder()
        .vexpand(true)
        .vhomogeneous(false)
        .build();
    stack.add_named(&scrolled(&cards::loading_state()), Some("loading"));
    stack.add_named(
        &cards::empty_state("No captures yet. Saved captures show up here."),
        Some("empty"),
    );
    stack.add_named(
        &cards::empty_state("No captures match. Clear the search or pick other filters."),
        Some("nomatch"),
    );
    stack.add_named(
        &cards::empty_state("The library could not be read. Details are in the log."),
        Some("failed"),
    );

    let paths = gtk::StringList::new(&[]);
    let selection = gtk::SingleSelection::builder()
        .model(&paths)
        .autoselect(false)
        .can_unselect(true)
        .build();
    let status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .hexpand(true)
        .css_classes(["note"])
        .build();
    let spinner = gtk::Spinner::builder()
        .valign(gtk::Align::Center)
        .visible(false)
        .build();
    let footer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .visible(false)
        .css_classes(["status"])
        .build();
    footer.append(&spinner);
    footer.append(&status);
    let banner = StatusBar::new();
    let gallery = Rc::new(Gallery {
        paths,
        selection,
        menus: RefCell::new(Vec::new()),
        actions: OnceCell::new(),
        missing: OnceCell::new(),
        status,
        spinner,
        footer,
        banner,
        pending: RefCell::new(None),
        all: RefCell::new(Vec::new()),
        details: RefCell::new(HashMap::new()),
        listed: RefCell::new(Vec::new()),
        queue: RefCell::new(Thumbnails::default()),
        recheck: Cell::new(false),
        query: RefCell::new(Query::default()),
        textures: RefCell::new(Textures::default()),
        loading: Cell::new(false),
        stale: Cell::new(false),
        loaded: Cell::new(false),
        stack,
        views: gtk::Stack::new(),
        pages: gtk::Stack::builder()
            .vexpand(true)
            .vhomogeneous(false)
            .build(),
        palette: colours::palette(),
        count,
    });
    let restore = Rc::downgrade(&gallery);
    gallery.banner.on_dismiss(move || {
        if let Some(gallery) = restore.upgrade() {
            gallery.focus_selected();
        }
    });
    let grid = cards::grid(&gallery);
    let list = cards::list(&gallery);
    gallery
        .menus
        .replace(vec![actions::attach(&grid), actions::attach(&list)]);
    gallery
        .views
        .add_named(&scrolled(&grid), Some(View::Grid.name()));
    gallery
        .views
        .add_named(&scrolled(&list), Some(View::List.name()));
    actions::install(&window, &gallery);
    gallery.stack.add_named(&gallery.views, Some("items"));
    gallery.stack.set_visible_child_name("loading");

    let changed = Rc::downgrade(&gallery);
    let parts = toolbar::build(move |change| {
        if let Some(gallery) = changed.upgrade() {
            gallery.apply(change);
        }
    });
    let search = parts.search;
    let _ = gallery.missing.set(parts.missing);
    let captures = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    captures.append(&parts.tabs);
    captures.append(&parts.bar);
    captures.append(&parts.filters);
    captures.append(&gallery.stack);
    gallery.pages.add_named(&captures, Some("captures"));
    gallery
        .pages
        .add_named(&gallery.palette.page, Some("colours"));
    content.append(&gallery.pages);
    content.append(&gallery.banner.widget);
    content.append(&gallery.footer);
    let shell = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["panel", "settings", "gallery"])
        .build();
    shell.append(&parts.sidebar);
    shell.append(&content);
    window.set_child(Some(&shell));
    search.set_key_capture_widget(Some(&window));

    let reload = gallery.clone();
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            Handle(Rc::downgrade(&reload)).reload();
        }
    });
    let keys = gtk::EventControllerKey::new();
    let closing = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, state| {
        if key == gdk::Key::f && state.contains(gdk::ModifierType::CONTROL_MASK) {
            search.grab_focus();
            return glib::Propagation::Stop;
        }
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(window) = closing.upgrade() {
            window.close();
        }
        glib::Propagation::Stop
    });
    window.add_controller(keys);

    gallery.load(true);
    window.present();
    (window, Handle(Rc::downgrade(&gallery)))
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .build()
}

pub struct Handle(Weak<Gallery>);

impl Handle {
    /// Reads the library again, after a capture was saved or the library was cleared while the
    /// gallery is open.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.reload();
    /// ```
    pub fn reload(&self) {
        if let Some(gallery) = self.0.upgrade() {
            gallery.reload();
            if gallery.showing_colours() {
                gallery.reload_colours();
            }
        }
    }

    /// Reads the library again and selects and scrolls to `path` once it is listed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.reveal(&gif_path);
    /// ```
    pub fn reveal(&self, path: &std::path::Path) {
        if let Some(gallery) = self.0.upgrade() {
            gallery
                .pending
                .replace(Some(path.to_string_lossy().into_owned()));
            gallery.reload();
        }
    }

    /// Shows `message` in the gallery footer, styled as a problem when `problem` is set.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.say("Saved clip.gif", false);
    /// ```
    pub fn say(&self, message: &str, problem: bool) {
        if let Some(gallery) = self.0.upgrade() {
            gallery.say(message, problem);
        }
    }

    /// Shows an accessible failure banner in the gallery.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.failure(Failure::Capture("No backend".into()));
    /// ```
    pub fn failure(&self, failure: Failure) {
        if let Some(gallery) = self.0.upgrade() {
            gallery.banner.show(failure);
        }
    }

    /// Shows `message` in the gallery footer with a spinner, until the next `say`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.working("Exporting clip.webm as a GIF");
    /// ```
    pub fn working(&self, message: &str) {
        if let Some(gallery) = self.0.upgrade() {
            gallery.working(message);
        }
    }
}

impl Gallery {
    fn reload(self: &Rc<Self>) {
        self.load(false);
    }

    fn load(self: &Rc<Self>, check: bool) {
        if self.loading.replace(true) {
            self.stale.set(true);
            if check {
                self.recheck.set(true);
            }
            return;
        }
        let check = check || self.recheck.take();
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let loaded = gio::spawn_blocking(move || {
                let mut library = Library::open(&library::path())?;
                if check {
                    library.check_missing()?;
                }
                library.all()
            })
            .await;
            gallery.loading.set(false);
            if gallery.stale.replace(false) {
                gallery.reload();
                return;
            }
            match loaded {
                Ok(Ok(captures)) => {
                    gallery.loaded.set(true);
                    gallery
                        .all
                        .replace(captures.into_iter().map(Rc::new).collect());
                    gallery.queue.borrow_mut().forget_failures();
                    gallery.refresh();
                }
                Ok(Err(error)) => {
                    error!("gallery could not read the library: {error}");
                    gallery.stack.set_visible_child_name("failed");
                }
                Err(_) => {
                    error!("the gallery worker stopped");
                    gallery.stack.set_visible_child_name("failed");
                }
            }
        });
    }

    fn apply(self: &Rc<Self>, change: Change) {
        {
            let mut query = self.query.borrow_mut();
            match change {
                Change::Text(text) => query.text = text,
                Change::Kind(kind) => query.kind = kind,
                Change::Period(period) => query.period = period,
                Change::Dimensions(dimensions) => query.dimensions = dimensions,
                Change::FileSize(file_size) => query.file_size = file_size,
                Change::Annotated(annotated) => query.annotated = annotated,
                Change::Unsaved(unsaved) => query.unsaved = unsaved,
                Change::Sort(sort) => query.sort = sort,
                Change::Missing(missing) => query.missing = missing,
                Change::View(view) => {
                    self.views.set_visible_child_name(view.name());
                    return;
                }
                Change::Colours(on) => {
                    drop(query);
                    self.show_colours(on);
                    return;
                }
            }
        }
        if self.loaded.get() {
            self.refresh();
        }
    }

    fn refresh(&self) {
        let all = self.all.borrow();
        let query = self.query.borrow().clone();
        let shown = query::apply(&all, &query, glib::real_time());
        let missing = all.iter().filter(|capture| capture.missing).count();
        let total = if query.missing {
            missing
        } else {
            all.len() - missing
        };
        if !self.showing_colours() {
            self.count.set_label(&count_text(shown.len(), total));
        }
        self.stack
            .set_visible_child_name(match (all.len(), shown.len()) {
                (0, _) => "empty",
                (_, 0) => "nomatch",
                _ => "items",
            });
        if let Some(toggle) = self.missing.get() {
            toolbar::set_nav_label(toggle, &format!("Missing {missing}"));
            toggle.set_visible(missing > 0 || query.missing);
        }
        let texts: Vec<Rc<str>> = shown
            .iter()
            .map(|capture| Rc::from(capture.path.to_string_lossy().as_ref()))
            .collect();
        let current = self.listed.replace(texts.clone());
        let previous = self.details.replace(
            texts
                .iter()
                .cloned()
                .zip(shown.into_iter().cloned())
                .collect(),
        );
        drop(all);
        let pending = self.pending.take();
        if current == texts {
            self.sync_actions();
            self.scroll_to_pending(pending.as_deref(), &texts);
            return;
        }
        let selected = self
            .selection
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|object| object.string());
        let previous_position = self.selection.selected();
        let focused_in_view = self
            .status
            .root()
            .and_downcast::<gtk::Window>()
            .and_then(|window| GtkWindowExt::focus(&window))
            .is_some_and(|focus| focus.is_ancestor(&self.views));
        let borrowed: Vec<&str> = texts.iter().map(|text| &**text).collect();
        let details = self.details.borrow();
        let same = |old: &Rc<str>, new: &Rc<str>| {
            old == new && previous.get(&**new) == details.get(&**new)
        };
        let limit = current.len().min(texts.len());
        let head = current
            .iter()
            .zip(&texts)
            .take_while(|(old, new)| same(old, new))
            .count();
        let tail = current
            .iter()
            .rev()
            .zip(texts.iter().rev())
            .take(limit - head)
            .take_while(|(old, new)| same(old, new))
            .count();
        drop(details);
        let removed = u32::try_from(current.len() - head - tail).unwrap_or(u32::MAX);
        self.paths.splice(
            u32::try_from(head).unwrap_or(u32::MAX),
            removed,
            &borrowed[head..texts.len() - tail],
        );
        let kept = selected
            .as_ref()
            .and_then(|selected| texts.iter().position(|text| &**text == selected.as_str()))
            .and_then(|position| u32::try_from(position).ok());
        if let Some(position) = kept {
            self.selection.set_selected(position);
        } else if selected.is_some() && !texts.is_empty() {
            let last = u32::try_from(texts.len() - 1).unwrap_or(u32::MAX);
            self.selection.set_selected(previous_position.min(last));
            if focused_in_view {
                self.focus_selected();
            }
        }
        self.sync_actions();
        self.scroll_to_pending(pending.as_deref(), &texts);
    }

    fn scroll_to_pending(&self, pending: Option<&str>, texts: &[Rc<str>]) {
        let Some(position) = pending
            .and_then(|pending| texts.iter().position(|text| &**text == pending))
            .and_then(|position| u32::try_from(position).ok())
        else {
            return;
        };
        let flags = gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS;
        for page in [View::Grid, View::List] {
            let Some(view) = self
                .views
                .child_by_name(page.name())
                .and_downcast::<gtk::ScrolledWindow>()
                .and_then(|scrolled| scrolled.child())
            else {
                continue;
            };
            if let Some(grid) = view.downcast_ref::<gtk::GridView>() {
                grid.scroll_to(position, flags, None);
            } else if let Some(list) = view.downcast_ref::<gtk::ListView>() {
                list.scroll_to(position, flags, None);
            }
        }
        self.sync_actions();
    }
}

fn count_text(shown: usize, total: usize) -> String {
    let noun = if total == 1 { "capture" } else { "captures" };
    if shown == total {
        format!("{total} {noun}")
    } else {
        format!("{shown} of {total} {noun}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_captures_and_filtered_results_in_words() {
        assert_eq!(count_text(0, 0), "0 captures");
        assert_eq!(count_text(1, 1), "1 capture");
        assert_eq!(count_text(42, 42), "42 captures");
        assert_eq!(count_text(3, 42), "3 of 42 captures");
        assert_eq!(count_text(0, 1), "0 of 1 capture");
    }
}
