mod actions;
mod cards;
mod colours;
mod rename;
mod toolbar;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::library::query::{self, Query};
use crate::library::{self, Capture, Library, thumbs};
use crate::ui;

use toolbar::{Change, View};

const TEXTURE_LIMIT: usize = 240;

struct Gallery {
    paths: gtk::StringList,
    selection: gtk::SingleSelection,
    menus: RefCell<Vec<gtk::PopoverMenu>>,
    actions: OnceCell<gio::SimpleActionGroup>,
    missing: OnceCell<gtk::ToggleButton>,
    status: gtk::Label,
    all: RefCell<Vec<Capture>>,
    details: RefCell<HashMap<String, Capture>>,
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

#[derive(Default)]
struct Textures {
    loaded: HashMap<String, gdk::Texture>,
    order: VecDeque<String>,
}

impl Textures {
    fn get(&self, path: &str) -> Option<gdk::Texture> {
        self.loaded.get(path).cloned()
    }

    fn put(&mut self, path: String, texture: gdk::Texture) {
        if self.loaded.insert(path.clone(), texture).is_none() {
            self.order.push_back(path);
        }
        while self.order.len() > TEXTURE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.loaded.remove(&oldest);
            }
        }
    }
}

/// Opens the gallery window with every saved capture that is still on disk, newest first.
///
/// A sidebar switches between all captures, missing ones and the copied colours, tabs filter by kind, and the search
/// row filters by name and age, sorts, and switches between a grid and a list. Thumbnails load off the main thread as items scroll into view. The library is read again
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
        .css_classes(["caps", "count"])
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
        .visible(false)
        .css_classes(["note"])
        .build();
    let gallery = Rc::new(Gallery {
        paths,
        selection,
        menus: RefCell::new(Vec::new()),
        actions: OnceCell::new(),
        missing: OnceCell::new(),
        status,
        all: RefCell::new(Vec::new()),
        details: RefCell::new(HashMap::new()),
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
    captures.append(&gallery.stack);
    gallery.pages.add_named(&captures, Some("captures"));
    gallery
        .pages
        .add_named(&gallery.palette.page, Some("colours"));
    content.append(&gallery.pages);
    content.append(&gallery.status);
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

    gallery.reload();
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
}

impl Gallery {
    fn reload(self: &Rc<Self>) {
        if self.loading.replace(true) {
            self.stale.set(true);
            return;
        }
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let loaded = gio::spawn_blocking(|| {
                let mut library = Library::open(&library::path())?;
                library.check_missing()?;
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
                    gallery.all.replace(captures);
                    gallery.refresh();
                }
                Ok(Err(error)) => {
                    eprintln!("kiekje: gallery could not read the library: {error}");
                    gallery.stack.set_visible_child_name("failed");
                }
                Err(_) => {
                    eprintln!("kiekje: the gallery worker stopped");
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
        let texts: Vec<String> = shown
            .iter()
            .map(|capture| capture.path.to_string_lossy().into_owned())
            .collect();
        let current: Vec<String> = (0..self.paths.n_items())
            .filter_map(|position| self.paths.string(position))
            .map(String::from)
            .collect();
        self.details.replace(
            texts
                .iter()
                .cloned()
                .zip(shown.into_iter().cloned())
                .collect(),
        );
        drop(all);
        if current == texts {
            self.sync_actions();
            return;
        }
        let selected = self
            .selection
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|object| object.string());
        let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
        self.paths.splice(0, self.paths.n_items(), &borrowed);
        let kept = selected
            .and_then(|selected| texts.iter().position(|text| *text == selected.as_str()))
            .and_then(|position| u32::try_from(position).ok());
        if let Some(position) = kept {
            self.selection.set_selected(position);
        }
        self.sync_actions();
    }

    fn thumbnail(self: &Rc<Self>, item: &gtk::ListItem, picture: &gtk::Picture, path: String) {
        if let Some(texture) = self.textures.borrow().get(&path) {
            cards::show_texture(picture, &texture);
            return;
        }
        picture.set_paintable(gdk::Paintable::NONE);
        picture.add_css_class("skeleton");
        let gallery = self.clone();
        let item = item.downgrade();
        let picture = picture.downgrade();
        glib::spawn_future_local(async move {
            let source = PathBuf::from(&path);
            let made = gio::spawn_blocking(move || thumbs::texture(&thumbs::dir(), &source)).await;
            let texture = match made {
                Ok(Ok(texture)) => texture,
                Ok(Err(error)) => {
                    eprintln!("kiekje: no thumbnail: {error}");
                    return;
                }
                Err(_) => {
                    eprintln!("kiekje: the thumbnail worker stopped");
                    return;
                }
            };
            gallery
                .textures
                .borrow_mut()
                .put(path.clone(), texture.clone());
            let (Some(item), Some(picture)) = (item.upgrade(), picture.upgrade()) else {
                return;
            };
            if cards::bound_path(&item).as_deref() == Some(path.as_str()) {
                cards::show_texture(&picture, &texture);
            }
        });
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

    #[test]
    fn keeps_only_the_newest_textures() {
        let mut textures = Textures::default();
        let pixel = gdk::MemoryTexture::new(
            1,
            1,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_static(&[0, 0, 0, 255]),
            4,
        );
        for index in 0..=TEXTURE_LIMIT {
            textures.put(index.to_string(), pixel.clone().upcast());
        }
        assert!(textures.get("0").is_none());
        assert!(textures.get(&TEXTURE_LIMIT.to_string()).is_some());
        assert_eq!(textures.loaded.len(), TEXTURE_LIMIT);
        textures.put("1".to_owned(), pixel.upcast());
        assert_eq!(textures.order.len(), TEXTURE_LIMIT);
    }
}
