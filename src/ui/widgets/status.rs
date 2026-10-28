use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use crate::ui::files;

#[derive(Clone)]
pub enum Failure {
    Capture(String),
    Save(String),
    Clipboard(String),
    Recording(String),
    PartialRecording { path: PathBuf, reason: String },
    Gif(String),
}

impl Failure {
    fn message(&self) -> String {
        match self {
            Self::Capture(reason) => format!("Capture failed: {reason}"),
            Self::Save(reason) => format!("Screenshot not saved: {reason}"),
            Self::Clipboard(reason) => format!("Clipboard copy failed: {reason}"),
            Self::Recording(reason) => format!("Recording failed: {reason}"),
            Self::PartialRecording { path, reason } => {
                format!(
                    "Recording ended early: {reason}. Partial file kept at {}",
                    path.display()
                )
            }
            Self::Gif(reason) => format!("GIF not made: {reason}"),
        }
    }

    fn file(&self) -> Option<PathBuf> {
        match self {
            Self::PartialRecording { path, .. } => Some(path.clone()),
            _ => None,
        }
    }
}

type Dismissed = Rc<RefCell<Option<Box<dyn Fn()>>>>;

pub struct StatusBar {
    pub widget: gtk::Box,
    message: gtk::Label,
    action: gtk::Button,
    file: Rc<RefCell<Option<PathBuf>>>,
    dismissed: Dismissed,
}

impl StatusBar {
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .visible(false)
            .accessible_role(gtk::AccessibleRole::Alert)
            .css_classes(["failure-banner"])
            .build();
        widget.update_property(&[gtk::accessible::Property::Label("Failure")]);
        let message = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .hexpand(true)
            .focusable(true)
            .build();
        let action = gtk::Button::with_label("Open folder");
        action.set_visible(false);
        let file = Rc::new(RefCell::new(None::<PathBuf>));
        let target = file.clone();
        action.connect_clicked(move |_| {
            if let Some(path) = target.borrow().clone() {
                glib::spawn_future_local(files::reveal(path));
            }
        });
        let dismiss = gtk::Button::with_label("Dismiss");
        let dismissed: Dismissed = Rc::new(RefCell::new(None));
        let bar = widget.clone();
        let after = dismissed.clone();
        let close = move || {
            bar.set_visible(false);
            if let Some(after) = after.borrow().as_ref() {
                after();
            }
        };
        let click = close.clone();
        dismiss.connect_clicked(move |_| click());
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        widget.add_controller(keys);
        widget.append(&message);
        widget.append(&action);
        widget.append(&dismiss);
        Self {
            widget,
            message,
            action,
            file,
            dismissed,
        }
    }

    /// Runs `after` whenever the banner is dismissed, so the owner can restore focus.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// bar.on_dismiss(|| list.grab_focus());
    /// ```
    pub fn on_dismiss(&self, after: impl Fn() + 'static) {
        self.dismissed.replace(Some(Box::new(after)));
    }

    pub fn show(&self, failure: Failure) {
        let text = failure.message();
        self.message.set_label(&text);
        self.widget
            .update_property(&[gtk::accessible::Property::Description(&text)]);
        let file = failure.file();
        self.action.set_visible(file.is_some());
        self.file.replace(file);
        self.widget.set_visible(true);
        self.message.grab_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_the_kept_file_only_for_a_partial_recording() {
        let path = PathBuf::from("/videos/partial.webm");
        let partial = Failure::PartialRecording {
            path: path.clone(),
            reason: "stopped early".into(),
        };
        assert_eq!(partial.file(), Some(path));
        assert!(partial.message().contains("Partial file kept"));
        assert_eq!(Failure::Clipboard("busy".into()).file(), None);
    }
}
