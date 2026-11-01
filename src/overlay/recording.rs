use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::capture::LogicalRect;
use crate::record::Area;

const PILL_SIZE: i32 = 132;
const FADE_SECONDS: f64 = 0.9;
const FADE_DEPTH: f64 = 0.7;
const FADE_TICK: Duration = Duration::from_millis(16);
const DIM: f64 = 0.35;
const GAP: f64 = 2.0;
const LINE: f64 = 1.0;
const BORDER_ALPHA: f64 = 0.75;
const DASH: [f64; 2] = [6.0, 5.0];
const ACCENT: (f64, f64, f64) = (254.0 / 255.0, 81.0 / 255.0, 1.0 / 255.0);

const CLEAR_BACKGROUND: &str = "window.recording-frame { background: transparent; \
     background-color: transparent; background-image: none; }";

thread_local! {
    static CLEARED: Cell<bool> = const { Cell::new(false) };
}

/// Where on which monitor the recording frame is drawn, in the monitor's logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub connector: String,
    pub region: (f64, f64, f64, f64),
}

/// A click-through layer surface that dims a monitor except for the region being recorded.
///
/// Dropping it closes the surface.
pub struct RecordingFrame {
    window: gtk::Window,
}

impl Drop for RecordingFrame {
    fn drop(&mut self) {
        self.window.destroy();
    }
}

/// Converts a recording area in frame pixels to the region it covers in the monitor's logical pixels.
///
/// # Examples
///
/// ```ignore
/// let marker = marker_for(&area, "DP-1", monitor.logical);
/// ```
pub fn marker_for(area: &Area, connector: &str, monitor: LogicalRect) -> Marker {
    let scale_x = f64::from(monitor.width) / f64::from(area.frame.0);
    let scale_y = f64::from(monitor.height) / f64::from(area.frame.1);
    Marker {
        connector: connector.to_owned(),
        region: (
            f64::from(area.left) * scale_x,
            f64::from(area.top) * scale_y,
            f64::from(area.width) * scale_x,
            f64::from(area.height) * scale_y,
        ),
    }
}

/// Dims the recorded monitor and outlines the recorded region with a dashed border.
///
/// The border sits two pixels outside the region so the recording never contains it. Returns
/// `None` where overlays are not layer surfaces, because a fullscreen window would take input
/// and end up in the recording.
///
/// # Examples
///
/// ```ignore
/// let frame = show(&marker);
/// drop(frame);
/// ```
pub fn show(marker: &Marker) -> Option<RecordingFrame> {
    if !super::uses_layer_shell() {
        return None;
    }
    let (display, monitor) = find_monitor(Some(&marker.connector))?;
    let window = layer_window(&display, &monitor, "kiekje-recording-frame");
    let region = marker.region;
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_can_target(false);
    area.set_draw_func(move |_, cr, width, height| {
        if let Err(error) = draw(cr, f64::from(width), f64::from(height), region) {
            warn!("could not draw the recording frame: {error}");
        }
    });
    window.set_child(Some(&area));
    window.present();
    Some(RecordingFrame { window })
}

/// A transparent, click-through countdown number shown before a recording starts.
///
/// Dropping it closes the surface.
pub struct CountdownBadge {
    window: gtk::Window,
    label: gtk::Label,
    pill: gtk::Box,
    fade: Rc<Cell<Option<glib::SourceId>>>,
}

impl Drop for CountdownBadge {
    fn drop(&mut self) {
        if let Some(source) = self.fade.take() {
            source.remove();
        }
        self.window.destroy();
    }
}

impl CountdownBadge {
    /// Shows `number` and fades it out over the following second.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// badge.set(2);
    /// ```
    pub fn set(&self, number: u32) {
        if let Some(source) = self.fade.take() {
            source.remove();
        }
        self.label.set_label(&number.to_string());
        self.pill.set_opacity(1.0);
        let started = Instant::now();
        let pill = self.pill.clone();
        let fade = self.fade.clone();
        let source = glib::timeout_add_local(FADE_TICK, move || {
            let progress = (started.elapsed().as_secs_f64() / FADE_SECONDS).min(1.0);
            pill.set_opacity(1.0 - FADE_DEPTH * ease_out(progress));
            if progress < 1.0 {
                return glib::ControlFlow::Continue;
            }
            fade.take();
            glib::ControlFlow::Break
        });
        self.fade.set(Some(source));
    }
}

fn ease_out(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(3)
}

/// Shows a countdown number centred on `center`, or on the monitor when there is no region.
///
/// Returns `None` where overlays are not layer surfaces.
///
/// # Examples
///
/// ```ignore
/// let badge = show_countdown(Some(&marker));
/// badge.set(3);
/// ```
pub fn show_countdown(marker: Option<&Marker>) -> Option<CountdownBadge> {
    if !super::uses_layer_shell() {
        return None;
    }
    crate::ui::load_stylesheet();
    let Some((display, monitor)) = find_monitor(marker.map(|marker| marker.connector.as_str()))
    else {
        warn!("no monitor to show the countdown on");
        return None;
    };
    let bounds = monitor.geometry();
    let (center_x, center_y) = marker.map_or(
        (
            f64::from(bounds.width()) / 2.0,
            f64::from(bounds.height()) / 2.0,
        ),
        |marker| {
            let (x, y, width, height) = marker.region;
            (x + width / 2.0, y + height / 2.0)
        },
    );
    let window = layer_window(&display, &monitor, "kiekje-recording-count");
    let label = gtk::Label::new(None);
    let pill = gtk::Box::builder()
        .css_classes(["recording-count"])
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(offset(center_x, PILL_SIZE))
        .margin_top(offset(center_y, PILL_SIZE))
        .width_request(PILL_SIZE)
        .height_request(PILL_SIZE)
        .build();
    label.set_hexpand(true);
    label.set_vexpand(true);
    pill.append(&label);
    pill.set_can_target(false);
    window.set_child(Some(&pill));
    window.present();
    Some(CountdownBadge {
        window,
        label,
        pill,
        fade: Rc::new(Cell::new(None)),
    })
}

fn offset(center: f64, size: i32) -> i32 {
    (center - f64::from(size) / 2.0).max(0.0).round() as i32
}

fn find_monitor(connector: Option<&str>) -> Option<(gdk::Display, gdk::Monitor)> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    let monitor = (0..monitors.n_items())
        .filter_map(|index| {
            monitors
                .item(index)
                .and_then(|item| item.downcast::<gdk::Monitor>().ok())
        })
        .find(|monitor| {
            monitor.is_valid()
                && connector
                    .is_none_or(|wanted| monitor.connector().is_some_and(|name| name == wanted))
        })?;
    Some((display, monitor))
}

fn layer_window(display: &gdk::Display, monitor: &gdk::Monitor, namespace: &str) -> gtk::Window {
    clear_background(display);
    let window = gtk::Window::builder().decorated(false).build();
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace(Some(namespace));
    window.set_monitor(Some(monitor));
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::None);
    window.add_css_class("recording-frame");
    window.connect_realize(|window| {
        if let Some(surface) = window.surface() {
            surface.set_input_region(Some(&cairo::Region::create()));
        }
    });
    window
}

fn clear_background(display: &gdk::Display) {
    if CLEARED.with(|cleared| cleared.replace(true)) {
        return;
    }
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CLEAR_BACKGROUND);
    // The theme's window background beats the app stylesheet, so only the user priority wins.
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

fn draw(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    (x, y, region_width, region_height): (f64, f64, f64, f64),
) -> Result<(), cairo::Error> {
    cr.set_source_rgba(0.0, 0.0, 0.0, DIM);
    cr.set_fill_rule(cairo::FillRule::EvenOdd);
    cr.rectangle(0.0, 0.0, width, height);
    cr.rectangle(x, y, region_width, region_height);
    cr.fill()?;
    let (red, green, blue) = ACCENT;
    cr.set_source_rgba(red, green, blue, BORDER_ALPHA);
    cr.set_line_width(LINE);
    cr.set_dash(&DASH, 0.0);
    let inset = GAP + LINE / 2.0;
    cr.rectangle(
        x - inset,
        y - inset,
        region_width + inset * 2.0,
        region_height + inset * 2.0,
    );
    cr.stroke()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: u32, top: u32, width: u32, height: u32) -> Area {
        Area {
            left,
            top,
            width,
            height,
            frame: (3840, 2160),
            origin: (0, 0),
        }
    }

    #[test]
    fn maps_frame_pixels_to_logical_pixels_on_a_scaled_monitor() {
        let monitor = LogicalRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let marker = marker_for(&area(200, 100, 400, 300), "DP-1", monitor);
        assert_eq!(marker.region, (100.0, 50.0, 200.0, 150.0));
        assert_eq!(marker.connector, "DP-1");
    }

    #[test]
    fn keeps_pixels_on_an_unscaled_monitor() {
        let monitor = LogicalRect {
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
        };
        let marker = marker_for(&area(10, 20, 30, 40), "DP-1", monitor);
        assert_eq!(marker.region, (10.0, 20.0, 30.0, 40.0));
    }
}
