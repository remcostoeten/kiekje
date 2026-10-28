use std::f64::consts::PI;

use gtk::prelude::*;
use gtk::{gio, glib};

const SITE: &str = "https://remcostoeten.com";
const SOURCE: &str = "https://github.com/remcostoeten/kiekje";
const HEART: i32 = 12;
const BEAT_US: f64 = 1_400_000.0;

/// The sidebar footer: a source code link above a "Made with love" sign-off.
///
/// The heart stays still when the desktop turns animations off.
///
/// # Examples
///
/// ```ignore
/// sidebar.append(&footer());
/// ```
pub fn footer() -> gtk::Box {
    let footer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["credit-footer"])
        .build();
    footer.append(&source());
    footer.append(&credit());
    footer
}

fn source() -> gtk::Button {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    content.append(&word("Source code on GitHub"));
    content.append(
        &gtk::Image::builder()
            .icon_name("kiekje-open-symbolic")
            .pixel_size(12)
            .build(),
    );
    let button = gtk::Button::builder()
        .child(&content)
        .tooltip_text(SOURCE)
        .css_classes(["nav", "credit"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(
        "Source code on GitHub, opens github.com/remcostoeten/kiekje",
    )]);
    open_on_click(&button, SOURCE);
    button
}

fn credit() -> gtk::Button {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    content.append(&word("Made with"));
    content.append(&heart());
    content.append(&word("by Remco Stoeten"));
    let button = gtk::Button::builder()
        .child(&content)
        .tooltip_text(SITE)
        .css_classes(["nav", "credit"])
        .build();
    button.update_property(&[gtk::accessible::Property::Label(
        "Made with love by Remco Stoeten, opens remcostoeten.com",
    )]);
    open_on_click(&button, SITE);
    button
}

fn open_on_click(button: &gtk::Button, url: &'static str) {
    button.connect_clicked(move |button| {
        let window = button.root().and_downcast::<gtk::Window>();
        gtk::UriLauncher::new(url).launch(
            window.as_ref(),
            None::<&gio::Cancellable>,
            move |opened| {
                if let Err(error) = opened {
                    warn!("{url} did not open: {error}");
                }
            },
        );
    });
}

fn word(text: &str) -> gtk::Label {
    gtk::Label::builder().label(text).xalign(0.0).build()
}

fn heart() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(HEART)
        .content_height(HEART)
        .valign(gtk::Align::Center)
        .css_classes(["heart"])
        .build();
    area.set_draw_func(|area, cr, width, height| {
        let scale = if animations_enabled(area) {
            1.0 + 0.18 * beat(glib::monotonic_time())
        } else {
            1.0
        };
        let color = area.color();
        cr.set_source_rgba(
            f64::from(color.red()),
            f64::from(color.green()),
            f64::from(color.blue()),
            f64::from(color.alpha()),
        );
        let (cx, cy) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        cr.translate(cx, cy);
        cr.scale(scale, scale);
        cr.translate(-cx, -cy);
        trace_heart(cr, f64::from(width), f64::from(height));
        if let Err(error) = cr.fill() {
            warn!("the heart was not drawn: {error}");
        }
    });
    area.add_tick_callback(|area, _| {
        if animations_enabled(area) {
            area.queue_draw();
        }
        glib::ControlFlow::Continue
    });
    area
}

fn animations_enabled(widget: &impl IsA<gtk::Widget>) -> bool {
    widget.settings().is_gtk_enable_animations() && widget.is_mapped()
}

// Two quick beats, then rest: a lub-dub shape rather than a steady sine.
fn beat(now_us: i64) -> f64 {
    let phase = (now_us as f64 % BEAT_US) / BEAT_US;
    if phase < 0.22 {
        (phase / 0.22 * PI).sin()
    } else if phase < 0.44 {
        0.55 * ((phase - 0.22) / 0.22 * PI).sin()
    } else {
        0.0
    }
}

fn trace_heart(cr: &gtk::cairo::Context, width: f64, height: f64) {
    let radius = width / 4.0;
    let top = height * 0.32;
    cr.new_path();
    cr.arc(width * 0.28, top, radius, PI, 0.0);
    cr.arc(width * 0.72, top, radius, PI, 0.0);
    cr.line_to(width / 2.0, height * 0.95);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_twice_then_rests() {
        let us = |phase: f64| (phase * BEAT_US) as i64;
        assert!(beat(us(0.11)) > 0.9);
        assert!(beat(us(0.33)) > 0.4 && beat(us(0.33)) < 0.6);
        assert_eq!(beat(us(0.8)), 0.0);
    }
}
