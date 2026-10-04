mod cards;
mod toolbar;

use std::cell::{Cell, RefCell};
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
    all: RefCell<Vec<Capture>>,
    details: RefCell<HashMap<String, Capture>>,
    query: RefCell<Query>,
    textures: RefCell<Textures>,
    loading: Cell<bool>,
    loaded: Cell<bool>,
    stack: gtk::Stack,
    views: gtk::Stack,
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
/// Captures can be searched by name, filtered by kind and age, sorted, and shown as a grid or a
/// list. Thumbnails load off the main thread as items scroll into view. The library is read again
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
        .css_classes(["panel", "settings", "gallery"])
        .build();
    let header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    header.append(
        &gtk::Label::builder()
            .label("Gallery")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["caps"])
            .build(),
    );
    let count = gtk::Label::builder().css_classes(["shortcut"]).build();
    header.append(&count);
    content.append(&header);

    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&cards::loading_state(), Some("loading"));
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

    let gallery = Rc::new(Gallery {
        paths: gtk::StringList::new(&[]),
        all: RefCell::new(Vec::new()),
        details: RefCell::new(HashMap::new()),
        query: RefCell::new(Query::default()),
        textures: RefCell::new(Textures::default()),
        loading: Cell::new(false),
        loaded: Cell::new(false),
        stack,
        views: gtk::Stack::new(),
        count,
    });
    let selection = gtk::SingleSelection::builder()
        .model(&gallery.paths)
        .autoselect(false)
        .can_unselect(true)
        .build();
    gallery.views.add_named(
        &scrolled(&cards::grid(&gallery, &selection)),
        Some(View::Grid.name()),
    );
    gallery.views.add_named(
        &scrolled(&cards::list(&gallery, &selection)),
        Some(View::List.name()),
    );
    gallery.stack.add_named(&gallery.views, Some("items"));
    gallery.stack.set_visible_child_name("loading");

    let changed = Rc::downgrade(&gallery);
    let (bar, search) = toolbar::build(move |change| {
        if let Some(gallery) = changed.upgrade() {
            gallery.apply(change);
        }
    });
    content.append(&bar);
    content.append(&gallery.stack);
    window.set_child(Some(&content));
    search.set_key_capture_widget(Some(&window));

    let reload = gallery.clone();
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            reload.reload();
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
    /// Reads the library again, after a capture was saved while the gallery is open.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// handle.reload();
    /// ```
    pub fn reload(&self) {
        if let Some(gallery) = self.0.upgrade() {
            gallery.reload();
        }
    }
}

impl Gallery {
    fn reload(self: &Rc<Self>) {
        if self.loading.replace(true) {
            return;
        }
        let gallery = self.clone();
        glib::spawn_future_local(async move {
            let loaded = gio::spawn_blocking(|| {
                Library::open(&library::path()).and_then(|library| library.recent(usize::MAX))
            })
            .await;
            gallery.loading.set(false);
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

    fn apply(&self, change: Change) {
        {
            let mut query = self.query.borrow_mut();
            match change {
                Change::Text(text) => query.text = text,
                Change::Kind(kind) => query.kind = kind,
                Change::Period(period) => query.period = period,
                Change::Sort(sort) => query.sort = sort,
                Change::View(view) => {
                    self.views.set_visible_child_name(view.name());
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
        let shown = query::apply(&all, &self.query.borrow(), glib::real_time());
        self.count.set_label(&count_text(shown.len(), all.len()));
        self.stack
            .set_visible_child_name(match (all.len(), shown.len()) {
                (0, _) => "empty",
                (_, 0) => "nomatch",
                _ => "items",
            });
        let texts: Vec<String> = shown
            .iter()
            .map(|capture| capture.path.to_string_lossy().into_owned())
            .collect();
        let current: Vec<String> = (0..self.paths.n_items())
            .filter_map(|position| self.paths.string(position))
            .map(String::from)
            .collect();
        if current == texts {
            return;
        }
        self.details.replace(
            texts
                .iter()
                .cloned()
                .zip(shown.into_iter().cloned())
                .collect(),
        );
        drop(all);
        let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
        self.paths.splice(0, self.paths.n_items(), &borrowed);
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
            let made = gio::spawn_blocking(move || thumbs::load(&thumbs::dir(), &source)).await;
            let png = match made {
                Ok(Ok(png)) => png,
                Ok(Err(error)) => {
                    eprintln!("kiekje: no thumbnail: {error}");
                    return;
                }
                Err(_) => {
                    eprintln!("kiekje: the thumbnail worker stopped");
                    return;
                }
            };
            let texture = match gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)) {
                Ok(texture) => texture,
                Err(error) => {
                    eprintln!("kiekje: thumbnail for {path} unreadable: {error}");
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
