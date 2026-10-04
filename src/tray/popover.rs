use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::{CaptureMode, LogicalRect};
use crate::library::{self, Library};
use crate::overlay;
use crate::ui;

const RECENT_COUNT: usize = 4;
const THUMBNAIL_WIDTH: i32 = 64;
const THUMBNAIL_HEIGHT: i32 = 44;
const MARGIN: i32 = 8;
const PANEL_WIDTH: i32 = 320;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Capture(CaptureMode),
    OpenCaptures,
    Settings,
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
    let monitors = crate::app::gdk_monitors();
    let rects: Vec<LogicalRect> = monitors
        .iter()
        .map(|monitor| crate::app::monitor_info(monitor).logical)
        .collect();
    let placement = place(click, &rects);

    let on_pick: PickCallback = Rc::new(on_pick);
    let picked = Rc::new(RefCell::new(None::<Pick>));
    let (panel, recent) = panel(&window, &content, &on_pick, &picked);
    window.set_child(Some(&panel));
    if overlay::uses_layer_shell() {
        cover(&window, monitors.get(placement.monitor));
        position(&panel, placement);
        close_on_outside_click(&window, &panel);
    } else {
        window.set_resizable(false);
    }

    let keys = gtk::EventControllerKey::new();
    let key_window = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        if let Some(window) = key_window.upgrade() {
            window.close();
        }
        glib::Propagation::Stop
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

fn panel(
    window: &gtk::Window,
    content: &Content,
    on_pick: &PickCallback,
    picked: &Rc<RefCell<Option<Pick>>>,
) -> (gtk::Box, gtk::Box) {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .width_request(PANEL_WIDTH)
        .css_classes(["panel", "popover-panel"])
        .build();
    panel.append(&caps("Kiekje"));

    let actions = [
        (CaptureMode::Region, "Capture region", "primary"),
        (CaptureMode::Fullscreen, "Capture every screen", "outline"),
        (CaptureMode::Repeat, "Repeat last region", "outline"),
    ];
    for (mode, label, kind) in actions {
        let shortcut = content
            .shortcuts
            .iter()
            .find(|(bound, _)| *bound == mode)
            .map(|(_, trigger)| trigger.as_str());
        let button = action_button(label, shortcut, kind);
        connect_pick(&button, window, picked, Pick::Capture(mode));
        panel.append(&button);
    }

    panel.append(&caps("Recent"));
    let recent = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .homogeneous(true)
        .build();
    panel.append(&recent_strip(&recent));

    panel.append(&rule());
    let footer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .homogeneous(true)
        .build();
    let folder = action_button("Folder", None, "ghost");
    connect_pick(&folder, window, picked, Pick::OpenCaptures);
    footer.append(&folder);
    let settings = action_button("Settings", None, "ghost");
    connect_pick(&settings, window, picked, Pick::Settings);
    footer.append(&settings);
    let quit = action_button("Quit", None, "ghost");
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
    (panel, recent)
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

fn caps(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["caps"])
        .build()
}

fn rule() -> gtk::Separator {
    gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["rule"])
        .build()
}

fn action_button(label: &str, shortcut: Option<&str>, kind: &str) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    row.append(
        &gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["action-label"])
            .build(),
    );
    if let Some(shortcut) = shortcut {
        row.append(
            &gtk::Label::builder()
                .label(shortcut)
                .css_classes(["shortcut"])
                .build(),
        );
    }
    gtk::Button::builder()
        .child(&row)
        .css_classes([kind])
        .build()
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
            recent_captures()
                .into_iter()
                .filter_map(|path| match thumbnail_png(&path) {
                    Ok(png) => Some((path, png)),
                    Err(error) => {
                        eprintln!("kiekje: no thumbnail for {}: {error}", path.display());
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
            eprintln!("kiekje: the thumbnail worker stopped");
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
        let mut shown = 0;
        for (path, png) in thumbnails {
            match gdk::Texture::from_bytes(&glib::Bytes::from_owned(png)) {
                Ok(texture) => {
                    row.append(&thumbnail_button(path, &texture));
                    shown += 1;
                }
                Err(error) => eprintln!(
                    "kiekje: thumbnail for {} unreadable: {error}",
                    path.display()
                ),
            }
        }
        for _ in shown..RECENT_COUNT {
            row.append(&gtk::Box::new(gtk::Orientation::Horizontal, 0));
        }
    });
}

fn thumbnail_png(path: &Path) -> Result<Vec<u8>, glib::Error> {
    Pixbuf::from_file_at_scale(path, THUMBNAIL_WIDTH * 2, THUMBNAIL_HEIGHT * 2, true)?
        .save_to_bufferv("png", &[])
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
    button.connect_clicked(move |button| {
        let uri = gio::File::for_path(&path).uri();
        if let Some(window) = button.root().and_downcast::<gtk::Window>() {
            window.close();
        }
        glib::spawn_future_local(async move {
            if let Err(error) =
                gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
            {
                eprintln!("kiekje: could not open {uri}: {error}");
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
            eprintln!("kiekje: recent captures unavailable: {error}");
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
