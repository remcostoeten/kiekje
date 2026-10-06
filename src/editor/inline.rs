use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib};

use super::document::Point;
use super::pixels::LoadError;
use super::render::{self, Layers};
use super::{Editor, Host, Input, SaveError, prepare, toolbar};
use crate::capture::{CaptureMode, LogicalRect};
use crate::output::Image;
use crate::ui;

const GAP: i32 = 8;
const MARGIN: i32 = 8;

/// What the user picked to end inline editing with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Accept,
    Copy,
    Save,
    SaveAs,
}

/// How inline editing ended.
pub enum Ending {
    Canceled,
    Finished { image: Image, finish: Finish },
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("the capture could not be prepared: {0}")]
    Load(#[from] LoadError),
    #[error("the capture could not be shown: {0}")]
    Draw(#[from] cairo::Error),
    #[error("the loading worker stopped")]
    Worker,
}

/// An editor that sits on the capture overlay, over the selection it edits.
#[derive(Clone)]
pub struct Inline(Rc<Editor>);

impl Inline {
    /// Prepares an inline editor for a freshly rendered selection.
    ///
    /// Pixels are prepared off the main thread. `on_end` runs once, when the user accepts,
    /// copies, saves or cancels; a finished edit carries the flattened image.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let inline = Inline::open(image, CaptureMode::Region, |ending| {}).await?;
    /// ```
    pub async fn open(
        image: Image,
        mode: CaptureMode,
        on_end: impl FnOnce(Ending) + 'static,
    ) -> Result<Self, OpenError> {
        let prepared = gio::spawn_blocking(move || prepare(Input::Image(image)))
            .await
            .map_err(|_| OpenError::Worker)??;
        let layers = Layers::new(prepared.base, prepared.hidden)?;
        let host = Host::Inline(RefCell::new(Some(Box::new(on_end))));
        Ok(Self(Editor::new(
            host,
            mode,
            None,
            prepared.picture,
            layers,
        )))
    }

    /// Places the canvas over `selection` and the tool and action bars around it.
    ///
    /// `selection` is in the coordinates of `layer`, which covers a surface of `surface` size.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// inline.mount(&fixed, LogicalRect { x: 40, y: 40, width: 640, height: 360 }, (1920, 1080));
    /// ```
    pub fn mount(&self, layer: &gtk::Fixed, selection: LogicalRect, surface: (i32, i32)) {
        ui::load_stylesheet();
        let editor = &self.0;
        layer.add_css_class("editor");
        layer.add_css_class("inline");
        editor
            .canvas
            .set_size_request(selection.width, selection.height);
        layer.put(
            &editor.canvas,
            f64::from(selection.x),
            f64::from(selection.y),
        );

        editor.entry.set_visible(false);
        layer.put(&editor.entry, 0.0, 0.0);
        editor.layer.replace(Some((
            layer.clone(),
            Point {
                x: f64::from(selection.x),
                y: f64::from(selection.y),
            },
        )));

        let tools = tool_bar(editor);
        let actions = action_bar(editor, selection);
        keep_focus(tools.upcast_ref());
        keep_focus(actions.upcast_ref());
        layer.put(&tools, 0.0, 0.0);
        layer.put(&actions, 0.0, 0.0);
        let (top, bottom) = place_bars(selection, surface, natural(&tools), natural(&actions));
        layer.move_(&tools, f64::from(top.0), f64::from(top.1));
        layer.move_(&actions, f64::from(bottom.0), f64::from(bottom.1));

        editor.sync();
        editor.canvas.grab_focus();
    }

    /// Handles a key pressed anywhere on the overlay.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// keys.connect_key_pressed(move |_, key, _, state| inline.key(key, state));
    /// ```
    pub fn key(&self, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        self.0.key(key, state)
    }
}

impl Editor {
    pub(super) fn conclude(self: &Rc<Self>, finish: Finish) {
        let Some((picture, shapes, crop)) = self.begin() else {
            return;
        };
        let editor = self.clone();
        glib::spawn_future_local(async move {
            let flattened =
                gio::spawn_blocking(move || render::flatten(&picture, &shapes, crop)).await;
            editor.busy.set(false);
            match flattened {
                Ok(Ok(image)) => {
                    if let Host::Inline(ending) = &editor.host
                        && let Some(end) = ending.take()
                    {
                        end(Ending::Finished { image, finish });
                    }
                }
                Ok(Err(error)) => {
                    editor.say(&format!("Not finished: {}", SaveError::from(error)), true);
                }
                Err(_) => editor.say("The render worker stopped", true),
            }
        });
    }
}

fn tool_bar(editor: &Rc<Editor>) -> gtk::Box {
    let bar = bar();
    let top = toolbar::row();
    toolbar::history(editor, &top);
    top.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let tools = toolbar::tools(editor);
    top.append(&tools);
    bar.append(&top);
    bar.append(&toolbar::styles(editor));
    bar
}

fn action_bar(editor: &Rc<Editor>, selection: LogicalRect) -> gtk::Box {
    let bar = bar();
    let row = toolbar::row();
    let (width, height) = (editor.picture.width, editor.picture.height);
    let size = gtk::Label::builder()
        .label(format!("{width} × {height}"))
        .tooltip_text(format!(
            "{} × {} on screen",
            selection.width, selection.height
        ))
        .css_classes(["action-label"])
        .build();
    row.append(&size);
    for (label, tooltip, primary, run) in [
        (
            "Accept",
            "Copy and save (Enter)",
            true,
            accept as fn(&Rc<Editor>),
        ),
        ("Cancel", "Discard this capture (Esc)", false, cancel),
        ("Save", "Save to the captures folder (Ctrl+S)", false, save),
        (
            "Save as",
            "Choose where to save (Ctrl+Shift+S)",
            false,
            save_as,
        ),
        ("Copy", "Copy to the clipboard (Ctrl+C)", false, copy),
    ] {
        let button = toolbar::action(label, tooltip);
        if primary {
            button.add_css_class("primary");
        }
        toolbar::on_click(editor, &button, run);
        row.append(&button);
    }
    bar.append(&row);
    editor.status.set_visible(false);
    editor.status.set_wrap(true);
    editor.status.set_max_width_chars(60);
    bar.append(&editor.status);
    bar
}

fn accept(editor: &Rc<Editor>) {
    editor.conclude(Finish::Accept);
}

fn cancel(editor: &Rc<Editor>) {
    editor.dismiss();
}

fn save(editor: &Rc<Editor>) {
    editor.conclude(Finish::Save);
}

fn save_as(editor: &Rc<Editor>) {
    editor.conclude(Finish::SaveAs);
}

fn copy(editor: &Rc<Editor>) {
    editor.conclude(Finish::Copy);
}

fn bar() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .css_classes(["panel", "inline-bar"])
        .build()
}

fn keep_focus(widget: &gtk::Widget) {
    widget.set_focus_on_click(false);
    let mut child = widget.first_child();
    while let Some(current) = child {
        keep_focus(&current);
        child = current.next_sibling();
    }
}

fn natural(widget: &impl IsA<gtk::Widget>) -> (i32, i32) {
    let (_, width, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
    let (_, height, _, _) = widget.measure(gtk::Orientation::Vertical, width);
    (width, height)
}

fn place_bars(
    selection: LogicalRect,
    surface: (i32, i32),
    tools: (i32, i32),
    actions: (i32, i32),
) -> ((i32, i32), (i32, i32)) {
    let centered = |width: i32| {
        fit(
            selection.x + (selection.width - width) / 2,
            MARGIN,
            surface.0 - width - MARGIN,
        )
    };
    let above = selection.y - GAP - tools.1;
    let top = if above >= MARGIN {
        above
    } else {
        selection.y + GAP
    };
    let below = selection.bottom() + GAP;
    let bottom = if below + actions.1 <= surface.1 - MARGIN {
        below
    } else {
        selection.bottom() - GAP - actions.1
    };
    (
        (
            centered(tools.0),
            fit(top, MARGIN, surface.1 - tools.1 - MARGIN),
        ),
        (
            centered(actions.0),
            fit(bottom, MARGIN, surface.1 - actions.1 - MARGIN),
        ),
    )
}

fn fit(value: i32, low: i32, high: i32) -> i32 {
    value.min(high).max(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: (i32, i32) = (1920, 1080);

    fn rect(x: i32, y: i32, width: i32, height: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn puts_tools_above_and_actions_below_a_centered_selection() {
        let (tools, actions) = place_bars(rect(400, 300, 800, 400), SURFACE, (600, 80), (500, 40));
        assert_eq!(tools, (500, 300 - GAP - 80));
        assert_eq!(actions, (550, 700 + GAP));
    }

    #[test]
    fn moves_the_bars_inside_a_full_screen_selection() {
        let (tools, actions) = place_bars(rect(0, 0, 1920, 1080), SURFACE, (600, 80), (500, 40));
        assert_eq!(tools.1, GAP);
        assert_eq!(actions.1, 1080 - GAP - 40);
    }

    #[test]
    fn keeps_bars_on_screen_near_an_edge() {
        let (tools, actions) = place_bars(rect(1800, 200, 100, 100), SURFACE, (600, 80), (500, 40));
        assert_eq!(tools.0, 1920 - 600 - MARGIN);
        assert_eq!(actions.0, 1920 - 500 - MARGIN);
        let (tools, _) = place_bars(rect(10, 200, 100, 100), SURFACE, (600, 80), (500, 40));
        assert_eq!(tools.0, MARGIN);
    }

    #[test]
    fn prefers_the_left_margin_when_a_bar_is_wider_than_the_screen() {
        let (tools, _) = place_bars(rect(100, 200, 100, 100), (400, 1080), (600, 80), (500, 40));
        assert_eq!(tools.0, MARGIN);
    }
}
