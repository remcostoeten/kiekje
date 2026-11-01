use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;

use super::Session;
use super::recording::cancel_countdown;
use crate::shortcuts::StopKey;

const DOUBLE_PRESS: Duration = Duration::from_millis(600);
const HOLD: Duration = Duration::from_secs(2);

pub(super) struct Escape {
    last_press: Cell<Option<Instant>>,
    hold: RefCell<Option<glib::SourceId>>,
}

fn is_double_press(last: Option<Instant>, now: Instant) -> bool {
    last.is_some_and(|last| now.saturating_duration_since(last) <= DOUBLE_PRESS)
}

impl Escape {
    pub(super) fn new() -> Self {
        Self {
            last_press: Cell::new(None),
            hold: RefCell::new(None),
        }
    }

    pub(super) fn reset(&self) {
        self.last_press.set(None);
        if let Some(source) = self.hold.take() {
            source.remove();
        }
    }
}

pub(super) fn handle(session: &Rc<Session>, key: StopKey) {
    let escape = &session.recorder.escape;
    if session.recorder.countdown.borrow().is_some() {
        if key == StopKey::Down {
            cancel_countdown(session);
        }
        return;
    }
    if !session.recorder.live.get() {
        escape.reset();
        return;
    }
    match key {
        StopKey::Down => {
            let now = Instant::now();
            if is_double_press(escape.last_press.replace(Some(now)), now) {
                cancel(session);
                return;
            }
            if let Some(source) = escape.hold.take() {
                source.remove();
            }
            let weak = Rc::downgrade(session);
            let source = glib::timeout_add_local_once(HOLD, move || {
                if let Some(session) = weak.upgrade() {
                    session.recorder.escape.hold.take();
                    cancel(&session);
                }
            });
            escape.hold.replace(Some(source));
        }
        StopKey::Up => {
            if let Some(source) = escape.hold.take() {
                source.remove();
            }
        }
    }
}

fn cancel(session: &Rc<Session>) {
    session.recorder.escape.reset();
    if let Some(recording) = session.recorder.active.borrow().as_ref() {
        info!("cancelling the recording");
        recording.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_two_quick_presses_as_a_double_press() {
        let first = Instant::now();
        assert!(is_double_press(
            Some(first),
            first + Duration::from_millis(400)
        ));
        assert!(!is_double_press(
            Some(first),
            first + Duration::from_millis(900)
        ));
        assert!(!is_double_press(None, first));
    }
}
