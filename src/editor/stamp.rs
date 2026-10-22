use gtk::prelude::*;

use super::Editor;
use super::document::{Point, Shape, Stroke, Tool};

const CURSOR_STEP: f64 = 10.0;
const GHOST_OPACITY: f64 = 0.5;

impl Editor {
    pub(super) fn is_stamp_tool(&self) -> bool {
        matches!(self.tool.get(), Tool::Check | Tool::Cross | Tool::Emoji)
    }

    pub(super) fn resizes_on_scroll(&self) -> bool {
        self.is_stamp_tool() || self.is_stamp_selected()
    }

    fn is_stamp_selected(&self) -> bool {
        let Some(index) = self.selected.get() else {
            return false;
        };
        matches!(
            self.document.borrow().shapes().get(index),
            Some(Shape::Mark { .. } | Shape::Emoji { .. })
        )
    }

    pub(super) fn track_stamp(&self, point: Point) {
        self.stamp_at.set(Some(point));
        if self.is_stamp_tool() {
            self.canvas.queue_draw();
        }
    }

    pub(super) fn stamp_point(&self) -> Point {
        if let Some(at) = self.stamp_at.get() {
            return at;
        }
        let (width, height) = self.size();
        match self.document.borrow().crop() {
            Some(area) => Point {
                x: area.x + area.width / 2.0,
                y: area.y + area.height / 2.0,
            },
            None => Point {
                x: width / 2.0,
                y: height / 2.0,
            },
        }
    }

    pub(super) fn move_stamp_cursor(&self, dx: f64, dy: f64) -> bool {
        if !self.is_stamp_tool() {
            return false;
        }
        let at = self.stamp_point();
        let (width, height) = self.size();
        self.stamp_at.set(Some(Point {
            x: (at.x + dx * CURSOR_STEP).clamp(0.0, width),
            y: (at.y + dy * CURSOR_STEP).clamp(0.0, height),
        }));
        self.canvas.queue_draw();
        true
    }

    pub(super) fn place_stamp(&self) -> bool {
        if !self.is_stamp_tool() {
            return false;
        }
        self.draft.replace(vec![self.stamp_point()]);
        self.commit();
        true
    }

    pub(super) fn ghost_shape(&self) -> Option<Shape> {
        if !self.is_stamp_tool() || !self.draft.borrow().is_empty() {
            return None;
        }
        let stroke = self.stroke();
        let faded = Stroke {
            opacity: stroke.opacity * GHOST_OPACITY,
            ..stroke
        };
        self.tool
            .get()
            .shape(&[self.stamp_point()], faded, 0, &self.emoji.borrow())
    }
}
