use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::canvas::View;
use super::document::{Color, Point, Shape, Stroke, Tool};

const GHOST_OPACITY: f64 = 0.5;

fn moved_cursor(at: Point, delta: (f64, f64), bounds: (f64, f64)) -> Point {
    Point {
        x: (at.x + delta.0).clamp(0.0, bounds.0),
        y: (at.y + delta.1).clamp(0.0, bounds.1),
    }
}

fn extend_draft(points: &mut Vec<Point>, tool: Tool, point: Point) {
    let Some(&start) = points.first() else {
        return;
    };
    if tool == Tool::Pen {
        if points.last() != Some(&point) {
            points.push(point);
        }
    } else {
        *points = vec![start, point];
    }
}

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
        if self.keyboard_drawing.get() {
            return;
        }
        let was_keyboard = self.keyboard_cursor.replace(false);
        self.stamp_at.set(Some(point));
        if self.is_stamp_tool() || was_keyboard {
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
        let tool = self.tool.get();
        if !self.canvas.has_focus()
            || tool == Tool::Select
            || (tool == Tool::Crop
                && self.document.borrow().crop().is_some()
                && !self.keyboard_drawing.get())
        {
            return false;
        }
        let point = moved_cursor(self.stamp_point(), (dx, dy), self.size());
        self.stamp_at.set(Some(point));
        self.keyboard_cursor.set(true);
        if self.keyboard_drawing.get() {
            extend_draft(&mut self.draft.borrow_mut(), tool, point);
        }
        self.describe_cursor();
        self.canvas.queue_draw();
        true
    }

    pub(super) fn place_stamp(&self) -> bool {
        let tool = self.tool.get();
        if tool == Tool::Select || self.menu.borrow().is_some() {
            return false;
        }
        self.keyboard_cursor.set(true);
        let point = self.stamp_point();
        if tool == Tool::Text {
            self.start_text(point, None);
        } else if self.is_stamp_tool() || tool == Tool::Step {
            self.draft.replace(vec![point]);
            self.commit();
        } else if self.keyboard_drawing.replace(true) {
            self.keyboard_drawing.set(false);
            if tool == Tool::Crop {
                if self.crop_draft().is_some_and(|area| !area.is_tiny()) {
                    self.commit_crop();
                } else {
                    self.cancel_draft();
                }
            } else {
                self.commit();
            }
        } else {
            self.draft.replace(vec![point]);
        }
        self.describe_cursor();
        self.canvas.queue_draw();
        true
    }

    fn describe_cursor(&self) {
        let point = self.stamp_point();
        let message = format!(
            "{} cursor at {:.0}, {:.0}. Arrows move 1 pixel, Shift moves 10. Space places a point; Space or Enter finishes a drawing. Page Up and Page Down select shapes.",
            self.tool.get().label(),
            point.x,
            point.y,
        );
        self.canvas
            .update_property(&[gtk::accessible::Property::Description(&message)]);
        self.say(&message, false);
    }

    pub(super) fn paint_keyboard_cursor(
        &self,
        cr: &cairo::Context,
        view: View,
    ) -> Result<(), cairo::Error> {
        let shown = self.keyboard_cursor.get() || self.canvas.has_visible_focus();
        if !shown || !self.canvas.has_focus() {
            return Ok(());
        }
        let (x, y) = view.to_widget(self.stamp_point());
        let (red, green, blue) = Color::Accent.rgb();
        for (width, color) in [(3.0, (0.04, 0.04, 0.04)), (1.0, (red, green, blue))] {
            cr.set_line_width(width);
            cr.set_source_rgb(color.0, color.1, color.2);
            cr.move_to(x - 7.0, y);
            cr.line_to(x + 7.0, y);
            cr.move_to(x, y - 7.0);
            cr.line_to(x, y + 7.0);
            cr.stroke()?;
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_by_single_pixels_and_clamps_at_image_edges() {
        let at = Point { x: 2.0, y: 9.0 };
        assert_eq!(
            moved_cursor(at, (1.0, 0.0), (10.0, 10.0)),
            Point { x: 3.0, y: 9.0 }
        );
        assert_eq!(
            moved_cursor(at, (-10.0, 10.0), (10.0, 10.0)),
            Point { x: 0.0, y: 10.0 }
        );
    }

    #[test]
    fn keeps_the_first_endpoint_when_adjusting_a_rectangle() {
        let start = Point { x: 1.0, y: 2.0 };
        let end = Point { x: 8.0, y: 9.0 };
        let mut points = vec![start];
        extend_draft(&mut points, Tool::Rectangle, Point { x: 3.0, y: 4.0 });
        extend_draft(&mut points, Tool::Rectangle, end);
        assert_eq!(points, vec![start, end]);
    }

    #[test]
    fn keeps_each_distinct_pen_endpoint() {
        let start = Point { x: 1.0, y: 2.0 };
        let end = Point { x: 8.0, y: 9.0 };
        let mut points = vec![start];
        extend_draft(&mut points, Tool::Pen, end);
        extend_draft(&mut points, Tool::Pen, end);
        assert_eq!(points, vec![start, end]);
    }
}
