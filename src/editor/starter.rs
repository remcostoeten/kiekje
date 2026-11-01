use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use super::document::{Color, DEFAULT_EMOJI, Tool};
use super::{DrawingKeys, Remembered, inline, style_adjustments, toolbar};
use crate::config;

struct State {
    keys: DrawingKeys,
    color: Cell<Color>,
    size: gtk::Adjustment,
    opacity: gtk::Adjustment,
    tools: RefCell<Vec<(Tool, gtk::ToggleButton)>>,
    swatches: RefCell<Vec<(Color, gtk::ToggleButton)>>,
    remembered: Remembered,
}

impl State {
    fn drawing(&self) -> config::Drawing {
        config::Drawing {
            color: self.color.get(),
            size: self.size.value().round() as u32,
            opacity: self.opacity.value().round() as u32,
            keys: self.keys,
        }
    }

    fn choose(&self, color: Color) {
        if self.color.replace(color) == color {
            return;
        }
        for (each, button) in self.swatches.borrow().iter() {
            if *each == color {
                button.set_active(true);
            }
        }
        self.remembered.keep(self.drawing());
    }
}

/// The drawing palette the capture overlay shows before anything is selected.
///
/// Each bar looks like the inline editor's palette, with Region picked because a drag selects
/// one. The colour, size and opacity chosen on it are saved and handed to the editor.
#[derive(Clone)]
pub struct Starter(Rc<State>);

impl Starter {
    /// Prepares the palettes for one overlay from the saved drawing style.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let starter = Starter::new(config.drawing);
    /// ```
    pub fn new(drawing: config::Drawing) -> Self {
        let [size, opacity] = style_adjustments(&drawing);
        let state = Rc::new(State {
            keys: drawing.keys,
            color: Cell::new(drawing.color),
            size,
            opacity,
            tools: RefCell::new(Vec::new()),
            swatches: RefCell::new(Vec::new()),
            remembered: Remembered::new(drawing),
        });
        for adjustment in [&state.size, &state.opacity] {
            let weak = Rc::downgrade(&state);
            adjustment.connect_value_changed(move |_| {
                if let Some(state) = weak.upgrade() {
                    state.remembered.keep(state.drawing());
                }
            });
        }
        Self(state)
    }

    /// Builds one palette bar; picking any tool but Region calls `start` with it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// surface.add_overlay(&starter.bar(|tool| begin_drawing(tool)));
    /// ```
    pub fn bar(&self, start: impl Fn(Tool) + 'static) -> gtk::Box {
        let state = &self.0;
        let start = Rc::new(start);
        let emoji = toolbar::emoji_action(DEFAULT_EMOJI);
        let picked = start.clone();
        emoji.connect_clicked(move |_| picked(Tool::Emoji));
        let tools = toolbar::tool_row(&state.keys, true, Tool::Crop, &emoji, move |tool| {
            if tool != Tool::Crop {
                start(tool);
            }
        });
        let history = toolbar::history_buttons();
        for button in &history {
            button.set_sensitive(false);
        }
        let weak = Rc::downgrade(state);
        let styles = toolbar::style_row(
            &state.keys,
            state.color.get(),
            &state.size,
            &state.opacity,
            move |color| {
                if let Some(state) = weak.upgrade() {
                    state.choose(color);
                }
            },
        );
        state.tools.borrow_mut().extend(tools.buttons);
        state.swatches.borrow_mut().extend(styles.buttons);
        let [undo, redo, clear] = &history;
        let bar = inline::bar();
        bar.append(&toolbar::assemble(
            &tools.row,
            [undo, redo, clear],
            &styles.row,
        ));
        inline::keep_focus(bar.upcast_ref());
        bar
    }

    /// Picks Region again on every bar, for when the overlay goes back to selecting.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// starter.reset();
    /// ```
    pub fn reset(&self) {
        for (tool, button) in self.0.tools.borrow().iter() {
            if *tool == Tool::Crop {
                button.set_active(true);
            }
        }
    }

    /// The drawing style picked so far, for the editor to open with.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// Inline::open(frames, index, mode, starter.drawing(), suspend, on_end).await?;
    /// ```
    pub fn drawing(&self) -> config::Drawing {
        self.0.drawing()
    }
}
