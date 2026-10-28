use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, glib};

const TICK: Duration = Duration::from_millis(100);
const UNMAP_WAIT: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Waiting(Duration),
    Ready,
    Canceled,
}

impl State {
    fn advance(self, elapsed: Duration) -> Self {
        match self {
            Self::Waiting(deadline) if elapsed >= deadline => Self::Ready,
            state => state,
        }
    }

    fn remaining(self, elapsed: Duration) -> u64 {
        match self {
            Self::Waiting(deadline) => {
                let millis = deadline.saturating_sub(elapsed).as_millis();
                u64::try_from(millis.div_ceil(1000)).unwrap_or(u64::MAX)
            }
            _ => 0,
        }
    }
}

pub(super) struct Countdown {
    window: gtk::Window,
    source: RefCell<Option<glib::SourceId>>,
    state: Cell<State>,
    completed: RefCell<Option<Box<dyn FnOnce()>>>,
    canceled: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl Countdown {
    pub(super) fn start(
        app: &gtk::Application,
        delay: Duration,
        completed: impl FnOnce() + 'static,
        canceled: impl FnOnce() + 'static,
    ) -> Rc<Self> {
        crate::ui::load_stylesheet();
        let window = gtk::Window::builder()
            .application(app)
            .title("Capture countdown")
            .resizable(false)
            .default_width(300)
            .build();
        let panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .css_classes(["panel"])
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let label = gtk::Label::builder()
            .accessible_role(gtk::AccessibleRole::Status)
            .build();
        label.update_property(&[gtk::accessible::Property::Description(
            "Press Escape or activate Cancel to cancel this capture.",
        )]);
        let cancel = gtk::Button::with_label("Cancel");
        panel.append(&label);
        panel.append(&cancel);
        window.set_child(Some(&panel));
        let countdown = Rc::new(Self {
            window,
            source: RefCell::new(None),
            state: Cell::new(State::Waiting(delay)),
            completed: RefCell::new(Some(Box::new(completed))),
            canceled: RefCell::new(Some(Box::new(canceled))),
        });
        let closing = Rc::downgrade(&countdown);
        countdown.window.connect_close_request(move |_| {
            if let Some(countdown) = closing.upgrade() {
                countdown.cancel();
            }
            glib::Propagation::Stop
        });
        let clicked = Rc::downgrade(&countdown);
        cancel.connect_clicked(move |_| {
            if let Some(countdown) = clicked.upgrade() {
                countdown.cancel();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let pressed = Rc::downgrade(&countdown);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(countdown) = pressed.upgrade() {
                countdown.cancel();
            }
            glib::Propagation::Stop
        });
        countdown.window.add_controller(keys);
        let started = Instant::now();
        let mut shown = countdown.state.get().remaining(Duration::ZERO);
        label.set_label(&format!("Capturing in {shown} seconds"));
        let ticking = Rc::downgrade(&countdown);
        let source = glib::timeout_add_local(TICK, move || {
            let Some(countdown) = ticking.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let elapsed = started.elapsed();
            let state = countdown.state.get().advance(elapsed);
            countdown.state.set(state);
            if state == State::Ready {
                countdown.source.take();
                countdown.window.set_visible(false);
                let ready = Rc::downgrade(&countdown);
                let source = glib::timeout_add_local_once(UNMAP_WAIT, move || {
                    if let Some(countdown) = ready.upgrade() {
                        countdown.source.take();
                        if let Some(completed) = countdown.completed.take() {
                            completed();
                        }
                    }
                });
                countdown.source.replace(Some(source));
                return glib::ControlFlow::Break;
            }
            let remaining = state.remaining(elapsed);
            if shown != remaining {
                shown = remaining;
                label.set_label(&format!("Capturing in {shown} seconds"));
            }
            glib::ControlFlow::Continue
        });
        countdown.source.replace(Some(source));
        countdown.window.present();
        cancel.grab_focus();
        countdown
    }

    pub(super) fn cancel(&self) {
        if self.state.replace(State::Canceled) == State::Canceled {
            return;
        }
        if let Some(source) = self.source.take() {
            source.remove();
        }
        self.completed.take();
        self.window.set_visible(false);
        if let Some(canceled) = self.canceled.take() {
            canceled();
        }
    }
}

impl Drop for Countdown {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
        self.window.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_remaining_seconds_up() {
        let state = State::Waiting(Duration::from_millis(2100));
        assert_eq!(state.remaining(Duration::ZERO), 3);
        assert_eq!(state.remaining(Duration::from_millis(100)), 2);
        assert_eq!(state.remaining(Duration::from_millis(2099)), 1);
    }

    #[test]
    fn becomes_ready_at_the_deadline() {
        let state = State::Waiting(Duration::from_secs(3));
        assert_eq!(state.advance(Duration::from_secs(2)), state);
        assert_eq!(state.advance(Duration::from_secs(3)), State::Ready);
        assert_eq!(state.advance(Duration::from_secs(4)), State::Ready);
    }

    #[test]
    fn cancellation_never_becomes_ready() {
        assert_eq!(State::Canceled.advance(Duration::MAX), State::Canceled);
        assert_eq!(State::Canceled.remaining(Duration::ZERO), 0);
    }
}
