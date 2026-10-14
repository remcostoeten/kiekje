use std::f64::consts::TAU;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};

use super::Editor;
use super::canvas::View;
use super::document::Color;

const SHOWN_FOR: Duration = Duration::from_millis(700);
const MONO: &str = "Geist Mono";
const LABEL_SIZE: f64 = 11.0;
const OFFSET: f64 = 18.0;
const PADDING: f64 = 7.0;
const DOT: f64 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dial {
    Size,
    Opacity,
    Color,
}

impl Dial {
    fn for_state(state: gdk::ModifierType) -> Option<Self> {
        if state.contains(gdk::ModifierType::CONTROL_MASK) {
            Some(Self::Size)
        } else if state.contains(gdk::ModifierType::SHIFT_MASK) {
            Some(Self::Opacity)
        } else if state.contains(gdk::ModifierType::ALT_MASK) {
            Some(Self::Color)
        } else {
            None
        }
    }
}

pub(super) struct Hud {
    dial: Dial,
    hiding: glib::SourceId,
}

pub(super) fn attach(editor: &Rc<Editor>) {
    let scroll = gtk::EventControllerScroll::new(
        gtk::EventControllerScrollFlags::BOTH_AXES | gtk::EventControllerScrollFlags::DISCRETE,
    );
    let target = Rc::downgrade(editor);
    scroll.connect_scroll(move |controller, dx, dy| {
        let (Some(editor), Some(dial)) = (
            target.upgrade(),
            Dial::for_state(controller.current_event_state()),
        ) else {
            return glib::Propagation::Proceed;
        };
        let delta = if dy == 0.0 { dx } else { dy };
        let steps = -delta.round() as i32;
        if steps != 0 {
            editor.turn(dial, steps);
        }
        glib::Propagation::Stop
    });
    editor.canvas.add_controller(scroll);
}

impl Editor {
    fn turn(self: &Rc<Self>, dial: Dial, steps: i32) {
        let nudge = |adjustment: &gtk::Adjustment| {
            adjustment
                .set_value(adjustment.value() + f64::from(steps) * adjustment.step_increment());
        };
        match dial {
            Dial::Size => nudge(&self.size),
            Dial::Opacity => nudge(&self.opacity),
            Dial::Color => self.set_color(self.color.get().shifted(steps)),
        }
        self.show_hud(dial);
    }

    pub(super) fn color_key(&self, key: gdk::Key, shift: bool) -> Option<Color> {
        if shift {
            return None;
        }
        key.to_unicode().and_then(Color::numbered)
    }

    pub(super) fn show_hud(self: &Rc<Self>, dial: Dial) {
        let target = Rc::downgrade(self);
        let hiding = glib::timeout_add_local_once(SHOWN_FOR, move || {
            if let Some(editor) = target.upgrade() {
                editor.hud.take();
                editor.canvas.queue_draw();
            }
        });
        if let Some(previous) = self.hud.replace(Some(Hud { dial, hiding })) {
            previous.hiding.remove();
        }
        self.canvas.queue_draw();
    }

    pub(super) fn paint_hud(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let dial = self.hud.borrow().as_ref().map(|hud| hud.dial);
        let (Some(dial), Some((x, y))) = (dial, self.pointer.get()) else {
            return Ok(());
        };
        let stroke = self.stroke();
        let (red, green, blue) = stroke.color.rgb();
        if dial == Dial::Size {
            cr.arc(x, y, (stroke.width * view.zoom() / 2.0).max(0.5), 0.0, TAU);
            cr.set_source_rgba(red, green, blue, stroke.opacity);
            cr.fill_preserve()?;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.7);
            cr.set_line_width(1.0);
            cr.stroke()?;
        }

        let drawing = self.drawing();
        let label = match dial {
            Dial::Size => format!("{} PX", drawing.size),
            Dial::Opacity => format!("{}%", drawing.opacity),
            Dial::Color => drawing.color.label().to_uppercase(),
        };
        cr.select_font_face(MONO, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(LABEL_SIZE);
        let text = cr.text_extents(&label)?;
        let line = cr.font_extents()?;
        let width = PADDING * 3.0 + DOT + text.x_advance();
        let height = PADDING * 2.0 + line.height();
        let room = (
            f64::from(self.canvas.width()) - width,
            f64::from(self.canvas.height()) - height,
        );
        let left = (x + OFFSET).min(room.0).max(0.0).round();
        let top = (y + OFFSET).min(room.1).max(0.0).round();

        cr.rectangle(left + 0.5, top + 0.5, width - 1.0, height - 1.0);
        cr.set_source_rgba(0.07, 0.07, 0.07, 0.92);
        cr.fill_preserve()?;
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
        cr.set_line_width(1.0);
        cr.stroke()?;

        cr.rectangle(left + PADDING, top + (height - DOT) / 2.0, DOT, DOT);
        cr.set_source_rgba(red, green, blue, stroke.opacity);
        cr.fill()?;

        cr.set_source_rgb(0.95, 0.95, 0.95);
        cr.move_to(left + PADDING * 2.0 + DOT, top + PADDING + line.ascent());
        cr.show_text(&label)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_dial_from_the_held_modifier() {
        assert_eq!(
            Dial::for_state(gdk::ModifierType::CONTROL_MASK),
            Some(Dial::Size)
        );
        assert_eq!(
            Dial::for_state(gdk::ModifierType::SHIFT_MASK),
            Some(Dial::Opacity)
        );
        assert_eq!(
            Dial::for_state(gdk::ModifierType::ALT_MASK),
            Some(Dial::Color)
        );
        assert_eq!(Dial::for_state(gdk::ModifierType::empty()), None);
    }
}
