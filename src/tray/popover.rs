use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::monitors::{gdk_monitors, monitor_info};
use crate::capture::{Availability, CaptureMode, LogicalRect};
use crate::fuzzy::Needle;
use crate::library::{self, Library, thumbs};
use crate::overlay;
use crate::ui;
use crate::ui::widgets::{keycap, keycaps};

const RECENT_COUNT: usize = 4;
const THUMBNAIL_WIDTH: i32 = 70;
const THUMBNAIL_HEIGHT: i32 = 44;
const MARGIN: i32 = 8;
const PANEL_WIDTH: i32 = 328;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Capture(CaptureMode),
    Annotate,
    Record,
    OpenCaptures,
    Settings,
    Gallery,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horizontal {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub monitor: usize,
    pub vertical: Vertical,
    pub horizontal: Horizontal,
}

pub struct Content {
    pub shortcuts: Vec<(CaptureMode, String)>,
    pub window: Availability,
    pub recording: bool,
}

type PickCallback = Rc<dyn Fn(Pick)>;

/// Picks the monitor and corner the popover anchors to from where the tray icon was clicked.
///
/// The corner nearest the click wins, so a panel at the bottom gets a popover that grows up.
/// Without a usable click, or one outside every monitor, it uses the first monitor's top right.
///
/// # Examples
///
/// ```ignore
/// let screen = LogicalRect { x: 0, y: 0, width: 1920, height: 1080 };
/// let placement = place(Some((1900, 1070)), &[screen]);
/// assert_eq!(placement.vertical, Vertical::Bottom);
/// ```
pub fn place(click: Option<(i32, i32)>, monitors: &[LogicalRect]) -> Placement {
    let fallback = Placement {
        monitor: 0,
        vertical: Vertical::Top,
        horizontal: Horizontal::Right,
    };
    let Some((x, y)) = click.filter(|&point| point != (0, 0)) else {
        return fallback;
    };
    let Some((monitor, rect)) = monitors
        .iter()
        .enumerate()
        .find(|(_, rect)| rect.contains_point(x, y))
    else {
        return fallback;
    };
    Placement {
        monitor,
        vertical: if y - rect.y < rect.bottom() - y {
            Vertical::Top
        } else {
            Vertical::Bottom
        },
        horizontal: if x - rect.x < rect.right() - x {
            Horizontal::Left
        } else {
            Horizontal::Right
        },
    }
}

/// Opens the styled tray popover and calls `on_pick` with what the user chose.
///
/// The popover closes itself after a pick, on Escape and on a click outside the panel. It
/// is a transparent layer-shell surface over the monitor where the compositor has one and
/// an undecorated window elsewhere.
///
/// # Examples
///
/// ```ignore
/// let window = popover::show(&app, Some((x, y)), content, |pick| println!("{pick:?}"));
/// ```
pub fn show(
    app: &gtk::Application,
    click: Option<(i32, i32)>,
    content: Content,
    on_pick: impl Fn(Pick) + 'static,
) -> gtk::Window {
    ui::load_stylesheet();
    let window = gtk::Window::builder()
        .application(app)
        .decorated(false)
        .title("Kiekje")
        .css_classes(["kiekje-popover"])
        .build();
    let monitors = gdk_monitors();
    let rects: Vec<LogicalRect> = monitors
        .iter()
        .map(|monitor| monitor_info(monitor).logical)
        .collect();
    let placement = place(click, &rects);

    let on_pick: PickCallback = Rc::new(on_pick);
    let picked = Rc::new(RefCell::new(None::<Pick>));
    let Built {
        panel,
        recent,
        entry,
        palette,
    } = build(&window, &content, &on_pick, &picked);
    window.set_child(Some(&panel));
    if overlay::uses_layer_shell() {
        cover(&window, monitors.get(placement.monitor));
        position(&panel, placement);
        close_on_outside_click(&window, &panel);
    } else {
        window.set_resizable(false);
    }

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let key_window = window.downgrade();
    let key_entry = entry.clone();
    let key_palette = palette.clone();
    keys.connect_key_pressed(move |_, key, _, _| match key {
        gdk::Key::Down => {
            key_palette.step(true);
            glib::Propagation::Stop
        }
        gdk::Key::Up => {
            key_palette.step(false);
            glib::Propagation::Stop
        }
        gdk::Key::Escape => {
            if key_entry.text().is_empty() {
                if let Some(window) = key_window.upgrade() {
                    window.close();
                }
            } else {
                key_entry.set_text("");
            }
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    window.add_controller(keys);

    window.connect_is_active_notify(|window| {
        if !window.is_active() && window.is_visible() {
            window.close();
        }
    });
    let close_pick = on_pick.clone();
    window.connect_close_request(move |_| {
        if let Some(pick) = picked.borrow_mut().take() {
            close_pick(pick);
        }
        glib::Propagation::Proceed
    });

    window.present();
    entry.grab_focus();
    load_recent(&recent);
    window
}

fn cover(window: &gtk::Window, monitor: Option<&gdk::Monitor>) {
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace(Some("kiekje-popover"));
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    if let Some(monitor) = monitor {
        window.set_monitor(Some(monitor));
    }
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
}

fn position(panel: &gtk::Box, placement: Placement) {
    panel.set_valign(match placement.vertical {
        Vertical::Top => gtk::Align::Start,
        Vertical::Bottom => gtk::Align::End,
    });
    panel.set_halign(match placement.horizontal {
        Horizontal::Left => gtk::Align::Start,
        Horizontal::Right => gtk::Align::End,
    });
    panel.set_margin_top(MARGIN);
    panel.set_margin_bottom(MARGIN);
    panel.set_margin_start(MARGIN);
    panel.set_margin_end(MARGIN);
}

fn close_on_outside_click(window: &gtk::Window, panel: &gtk::Box) {
    let click = gtk::GestureClick::new();
    click.set_button(0);
    let panel = panel.downgrade();
    click.connect_pressed(move |gesture, _, x, y| {
        let Some(window) = gesture.widget().and_downcast::<gtk::Window>() else {
            return;
        };
        let Some(panel) = panel.upgrade() else {
            return;
        };
        let inside = window
            .pick(x, y, gtk::PickFlags::DEFAULT)
            .is_some_and(|target| target == panel || target.is_ancestor(&panel));
        if !inside {
            window.close();
        }
    });
    window.add_controller(click);
}

struct Built {
    panel: gtk::Box,
    recent: gtk::Box,
    entry: gtk::Entry,
    palette: Rc<Palette>,
}

struct ActionRow {
    button: gtk::Button,
    title: &'static str,
    section: usize,
}

struct Palette {
    rows: Vec<ActionRow>,
    headers: Vec<gtk::Label>,
    no_match: gtk::Label,
    selected: Cell<Option<usize>>,
}

impl Palette {
    fn select(&self, index: Option<usize>) {
        for row in &self.rows {
            row.button.remove_css_class("selected");
        }
        if let Some(row) = index.and_then(|index| self.rows.get(index)) {
            row.button.add_css_class("selected");
        }
        self.selected.set(index);
    }

    fn pickable(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.button.is_visible() && row.button.is_sensitive())
            .map(|(index, _)| index)
            .collect()
    }

    fn step(&self, forward: bool) {
        let pickable = self.pickable();
        let Some(last) = pickable.len().checked_sub(1) else {
            return;
        };
        let at = self
            .selected
            .get()
            .and_then(|current| pickable.iter().position(|&index| index == current));
        let next = match (at, forward) {
            (Some(at), true) => (at + 1).min(last),
            (Some(at), false) => at.saturating_sub(1),
            (None, true) => 0,
            (None, false) => last,
        };
        self.select(Some(pickable[next]));
    }

    fn filter(&self, query: &str) {
        let needle = Needle::new(query);
        let everything = query.trim().is_empty();
        for row in &self.rows {
            row.button
                .set_visible(everything || needle.matches(row.title));
        }
        for (section, header) in self.headers.iter().enumerate() {
            header.set_visible(
                self.rows
                    .iter()
                    .any(|row| row.section == section && row.button.is_visible()),
            );
        }
        self.no_match
            .set_visible(self.rows.iter().all(|row| !row.button.is_visible()));
        self.select(self.pickable().first().copied());
    }

    fn activate(&self) {
        let picked = self.selected.get().and_then(|index| self.rows.get(index));
        if let Some(row) = picked
            && row.button.is_sensitive()
        {
            row.button.emit_clicked();
        }
    }
}

fn build(
    window: &gtk::Window,
    content: &Content,
    on_pick: &PickCallback,
    picked: &Rc<RefCell<Option<Pick>>>,
) -> Built {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(PANEL_WIDTH)
        .css_classes(["panel", "popover-panel"])
        .build();

    let filter = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["pop-filter"])
        .build();
    filter.append(&icon("kiekje-search-symbolic"));
    let entry = gtk::Entry::builder()
        .placeholder_text("Search actions")
        .has_frame(false)
        .hexpand(true)
        .build();
    entry.update_property(&[gtk::accessible::Property::Label("Search actions")]);
    filter.append(&entry);
    filter.append(&keycap("Esc"));
    panel.append(&filter);

    let list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(1)
        .css_classes(["pop-list"])
        .build();
    let mut rows = Vec::new();
    let mut headers = Vec::new();

    let capture_header = section_title("Capture");
    list.append(&capture_header);
    headers.push(capture_header);
    let shortcut_of = |mode: CaptureMode| {
        content
            .shortcuts
            .iter()
            .find(|(bound, _)| *bound == mode)
            .map(|(_, trigger)| trigger.as_str())
    };
    let capture_actions = [
        (
            "Capture region",
            "kiekje-region-symbolic",
            shortcut_of(CaptureMode::Region),
            Pick::Capture(CaptureMode::Region),
        ),
        (
            "Annotate region",
            "kiekje-pen-symbolic",
            None,
            Pick::Annotate,
        ),
        (
            "Capture every screen",
            "kiekje-screens-symbolic",
            shortcut_of(CaptureMode::Fullscreen),
            Pick::Capture(CaptureMode::Fullscreen),
        ),
        (
            "Capture window",
            "kiekje-window-symbolic",
            shortcut_of(CaptureMode::Window),
            Pick::Capture(CaptureMode::Window),
        ),
        (
            "Repeat last region",
            "kiekje-repeat-symbolic",
            shortcut_of(CaptureMode::Repeat),
            Pick::Capture(CaptureMode::Repeat),
        ),
    ];
    for (title, icon_name, shortcut, pick) in capture_actions {
        let button = action(icon_name, title, shortcut);
        connect_pick(&button, window, picked, pick);
        if let (Pick::Capture(CaptureMode::Window), Availability::Unavailable(reason)) =
            (pick, content.window)
        {
            button.set_sensitive(false);
            button.set_tooltip_text(Some(reason));
        }
        list.append(&button);
        rows.push(ActionRow {
            button,
            title,
            section: 0,
        });
    }

    let record_header = section_title("Record");
    list.append(&record_header);
    headers.push(record_header);
    let (record_title, record_icon) = if content.recording {
        ("Stop recording", "kiekje-stop-symbolic")
    } else {
        ("Record screen", "kiekje-record-symbolic")
    };
    let record = action(record_icon, record_title, None);
    if content.recording {
        record.add_css_class("recording");
    }
    connect_pick(&record, window, picked, Pick::Record);
    list.append(&record);
    rows.push(ActionRow {
        button: record,
        title: record_title,
        section: 1,
    });

    let no_match = gtk::Label::builder()
        .label("No matching actions")
        .visible(false)
        .css_classes(["no-match"])
        .build();
    list.append(&no_match);
    panel.append(&list);

    let palette = Rc::new(Palette {
        rows,
        headers,
        no_match,
        selected: Cell::new(None),
    });
    for (index, row) in palette.rows.iter().enumerate() {
        let motion = gtk::EventControllerMotion::new();
        let hovered = palette.clone();
        motion.connect_enter(move |_, _, _| hovered.select(Some(index)));
        row.button.add_controller(motion);
    }
    palette.select(palette.pickable().first().copied());
    let changed = palette.clone();
    entry.connect_changed(move |entry| changed.filter(&entry.text()));
    let activated = palette.clone();
    entry.connect_activate(move |_| activated.activate());

    let recent_block = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["recent"])
        .build();
    let recent_header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    let recent_label = section_title("Recent");
    recent_label.set_hexpand(true);
    recent_header.append(&recent_label);
    let gallery = ghost("kiekje-open-symbolic", "Gallery");
    gallery.add_css_class("compact");
    connect_pick(&gallery, window, picked, Pick::Gallery);
    recent_header.append(&gallery);
    recent_block.append(&recent_header);
    let recent = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .homogeneous(true)
        .build();
    recent_block.append(&recent_strip(&recent));
    panel.append(&recent_block);

    let footer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .css_classes(["pop-foot"])
        .build();
    let folder = ghost("kiekje-folder-symbolic", "Folder");
    connect_pick(&folder, window, picked, Pick::OpenCaptures);
    footer.append(&folder);
    let settings = ghost("kiekje-sliders-symbolic", "Settings");
    connect_pick(&settings, window, picked, Pick::Settings);
    footer.append(&settings);
    footer.append(&gtk::Box::builder().hexpand(true).build());
    let quit = ghost("kiekje-power-symbolic", "Quit");
    quit.add_css_class("quit");
    let quit_pick = on_pick.clone();
    let quit_window = window.downgrade();
    quit.connect_clicked(move |_| {
        if let Some(window) = quit_window.upgrade() {
            window.close();
        }
        quit_pick(Pick::Quit);
    });
    footer.append(&quit);
    panel.append(&footer);
    panel.append(&hints());

    Built {
        panel,
        recent,
        entry,
        palette,
    }
}

fn connect_pick(
    button: &gtk::Button,
    window: &gtk::Window,
    picked: &Rc<RefCell<Option<Pick>>>,
    pick: Pick,
) {
    let window = window.downgrade();
    let picked = picked.clone();
    button.connect_clicked(move |_| {
        picked.replace(Some(pick));
        if let Some(window) = window.upgrade() {
            window.close();
        }
    });
}

fn section_title(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["section-title"])
        .build()
}

fn icon(name: &str) -> gtk::Image {
    gtk::Image::builder().icon_name(name).pixel_size(16).build()
}

fn hints() -> gtk::Box {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .css_classes(["pop-hints"])
        .build();
    for (keys, what) in [
        (&["↑", "↓"][..], "move"),
        (&["↵"][..], "run"),
        (&["Esc"][..], "close"),
    ] {
        let group = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(5)
            .build();
        for key in keys {
            group.append(&keycap(key));
        }
        group.append(&gtk::Label::new(Some(what)));
        bar.append(&group);
    }
    bar
}

fn action(icon_name: &str, label: &str, shortcut: Option<&str>) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    row.append(&icon(icon_name));
    row.append(
        &gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["action-label"])
            .build(),
    );
    if let Some(shortcut) = shortcut {
        row.append(&keycaps(shortcut));
    }
    let button = gtk::Button::builder()
        .child(&row)
        .css_classes(["act"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn ghost(icon_name: &str, label: &str) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    row.append(&icon(icon_name));
    row.append(&gtk::Label::new(Some(label)));
    let button = gtk::Button::builder()
        .child(&row)
        .css_classes(["ghost"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button
}

fn recent_strip(row: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(THUMBNAIL_HEIGHT + 2)
        .child(row)
        .build()
}

fn load_recent(row: &gtk::Box) {
    let row = row.downgrade();
    glib::spawn_future_local(async move {
        let loaded = gio::spawn_blocking(move || {
            let cache = thumbs::dir();
            recent_captures()
                .into_iter()
                .filter_map(|path| match thumbs::texture(&cache, &path) {
                    Ok(texture) => Some((path, texture)),
                    Err(error) => {
                        warn!("no thumbnail: {error}");
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .await;
        let Some(row) = row.upgrade() else {
            return;
        };
        let thumbnails = loaded.unwrap_or_else(|_| {
            error!("the thumbnail worker stopped");
            Vec::new()
        });
        if thumbnails.is_empty() {
            row.append(
                &gtk::Label::builder()
                    .label("No captures yet")
                    .hexpand(true)
                    .css_classes(["empty-recent"])
                    .build(),
            );
            return;
        }
        let shown = thumbnails.len();
        for (path, texture) in thumbnails {
            row.append(&thumbnail_button(path, &texture));
        }
        for _ in shown..RECENT_COUNT {
            row.append(&gtk::Box::new(gtk::Orientation::Horizontal, 0));
        }
    });
}

fn thumbnail_button(path: PathBuf, texture: &gdk::Texture) -> gtk::Button {
    let picture = gtk::Picture::for_paintable(texture);
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_size_request(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let button = gtk::Button::builder()
        .child(&picture)
        .tooltip_text(name.as_str())
        .css_classes(["thumbnail"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Open recent capture {name}"
    ))]);
    button.connect_clicked(move |button| {
        let uri = gio::File::for_path(&path).uri();
        if let Some(window) = button.root().and_downcast::<gtk::Window>() {
            window.close();
        }
        glib::spawn_future_local(async move {
            if let Err(error) =
                gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
            {
                error!("could not open {uri}: {error}");
            }
        });
    });
    button
}

fn recent_captures() -> Vec<PathBuf> {
    match Library::open(&library::path()).and_then(|library| library.recent(RECENT_COUNT)) {
        Ok(captures) => captures
            .into_iter()
            .map(|capture| capture.path)
            .filter(|path| path.is_file())
            .collect(),
        Err(error) => {
            warn!("recent captures unavailable: {error}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn screen(x: i32, y: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width: 1920,
            height: 1080,
        }
    }

    #[test]
    fn anchors_to_the_corner_nearest_the_click() {
        let monitors = [screen(0, 0), screen(1920, 0)];
        assert_eq!(
            place(Some((3800, 1070)), &monitors),
            Placement {
                monitor: 1,
                vertical: Vertical::Bottom,
                horizontal: Horizontal::Right
            }
        );
        assert_eq!(
            place(Some((20, 10)), &monitors),
            Placement {
                monitor: 0,
                vertical: Vertical::Top,
                horizontal: Horizontal::Left
            }
        );
    }

    #[test]
    fn falls_back_to_the_top_right_without_a_usable_click() {
        let fallback = Placement {
            monitor: 0,
            vertical: Vertical::Top,
            horizontal: Horizontal::Right,
        };
        let monitors = [screen(-1920, 0), screen(0, 0)];
        assert_eq!(place(None, &monitors), fallback);
        assert_eq!(place(Some((0, 0)), &monitors), fallback);
        assert_eq!(place(Some((9000, 9000)), &monitors), fallback);
    }
}
