use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::library::{self, Capture, Library, thumbs};
use crate::ui;

const CARD_WIDTH: i32 = 220;
const CARD_HEIGHT: i32 = 140;
const TEXTURE_LIMIT: usize = 240;

struct Gallery {
    paths: gtk::StringList,
    details: RefCell<HashMap<String, Capture>>,
    textures: RefCell<Textures>,
    loading: Cell<bool>,
    stack: gtk::Stack,
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
/// Thumbnails load off the main thread as cards scroll into view. The list reloads whenever the
/// window becomes active, so captures taken while it is open show up on return.
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

    let paths = gtk::StringList::new(&[]);
    let stack = gtk::Stack::builder().vexpand(true).build();
    stack.add_named(&loading_state(), Some("loading"));
    stack.add_named(
        &empty_state("No captures yet. Saved captures show up here."),
        Some("empty"),
    );
    stack.add_named(
        &empty_state("The library could not be read. Details are in the log."),
        Some("failed"),
    );
    content.append(&stack);
    window.set_child(Some(&content));

    let gallery = Rc::new(Gallery {
        paths,
        details: RefCell::new(HashMap::new()),
        textures: RefCell::new(Textures::default()),
        loading: Cell::new(false),
        stack,
        count,
    });
    let grid = grid(&gallery);
    gallery.stack.add_named(
        &gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid)
            .build(),
        Some("grid"),
    );
    gallery.stack.set_visible_child_name("loading");

    let reload = gallery.clone();
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            reload.reload();
        }
    });
    let keys = gtk::EventControllerKey::new();
    let closing = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
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
                Ok(Ok(captures)) => gallery.fill(captures),
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

    fn fill(&self, captures: Vec<Capture>) {
        let texts: Vec<String> = captures
            .iter()
            .map(|capture| capture.path.to_string_lossy().into_owned())
            .collect();
        let current: Vec<String> = (0..self.paths.n_items())
            .filter_map(|position| self.paths.string(position))
            .map(String::from)
            .collect();
        self.count.set_label(&count_text(captures.len()));
        self.stack
            .set_visible_child_name(if captures.is_empty() { "empty" } else { "grid" });
        if current == texts {
            return;
        }
        self.details.replace(
            texts
                .iter()
                .cloned()
                .zip(captures)
                .collect::<HashMap<_, _>>(),
        );
        let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
        self.paths.splice(0, self.paths.n_items(), &borrowed);
    }

    fn thumbnail(self: &Rc<Self>, item: &gtk::ListItem, picture: &gtk::Picture, path: String) {
        if let Some(texture) = self.textures.borrow().get(&path) {
            show_texture(picture, &texture);
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
            if bound_path(&item).as_deref() == Some(path.as_str()) {
                show_texture(&picture, &texture);
            }
        });
    }
}

fn grid(gallery: &Rc<Gallery>) -> gtk::GridView {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        if let Some(item) = object.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&card()));
        }
    });
    let binding = Rc::downgrade(gallery);
    factory.connect_bind(move |_, object| {
        let (Some(item), Some(gallery)) =
            (object.downcast_ref::<gtk::ListItem>(), binding.upgrade())
        else {
            return;
        };
        let Some(path) = bound_path(item) else {
            return;
        };
        let Some((picture, name, meta)) = card_parts(item) else {
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

    let selection = gtk::SingleSelection::builder()
        .model(&gallery.paths)
        .autoselect(false)
        .can_unselect(true)
        .build();
    let grid = gtk::GridView::builder()
        .model(&selection)
        .factory(&factory)
        .max_columns(8)
        .min_columns(1)
        .single_click_activate(false)
        .css_classes(["gallery-grid"])
        .build();
    let opening = gallery.paths.downgrade();
    grid.connect_activate(move |_, position| {
        let Some(path) = opening.upgrade().and_then(|paths| paths.string(position)) else {
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
    });
    grid
}

fn card() -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .css_classes(["card"])
        .build();
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Cover)
        .width_request(CARD_WIDTH)
        .height_request(CARD_HEIGHT)
        .css_classes(["skeleton"])
        .build();
    card.append(&picture);
    card.append(
        &gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(1)
            .hexpand(true)
            .build(),
    );
    card.append(
        &gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["shortcut"])
            .build(),
    );
    card
}

fn card_parts(item: &gtk::ListItem) -> Option<(gtk::Picture, gtk::Label, gtk::Label)> {
    let card = item.child()?;
    let picture = card.first_child()?;
    let name = picture.next_sibling()?;
    let meta = name.next_sibling()?;
    Some((
        picture.downcast().ok()?,
        name.downcast().ok()?,
        meta.downcast().ok()?,
    ))
}

fn bound_path(item: &gtk::ListItem) -> Option<String> {
    item.item()
        .and_downcast::<gtk::StringObject>()
        .map(|object| object.string().into())
}

fn show_texture(picture: &gtk::Picture, texture: &gdk::Texture) {
    picture.remove_css_class("skeleton");
    picture.set_paintable(Some(texture));
}

fn loading_state() -> gtk::Box {
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(8)
        .can_focus(false)
        .valign(gtk::Align::Start)
        .build();
    for _ in 0..8 {
        let skeleton = gtk::Box::builder()
            .width_request(CARD_WIDTH)
            .height_request(CARD_HEIGHT)
            .css_classes(["skeleton"])
            .build();
        grid.append(&skeleton);
    }
    let state = gtk::Box::new(gtk::Orientation::Vertical, 0);
    state.append(&grid);
    state
}

fn empty_state(text: &str) -> gtk::Box {
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

fn count_text(count: usize) -> String {
    match count {
        1 => "1 capture".to_owned(),
        count => format!("{count} captures"),
    }
}

fn meta_text(capture: &Capture) -> String {
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

    #[test]
    fn counts_captures_in_words() {
        assert_eq!(count_text(0), "0 captures");
        assert_eq!(count_text(1), "1 capture");
        assert_eq!(count_text(42), "42 captures");
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
