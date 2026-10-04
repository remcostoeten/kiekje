use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::canvas::View;
use super::document::{Area, Color, Edit, Point, Tool};

const SHADE: f64 = 0.6;

impl Editor {
    fn crop_draft(&self) -> Option<Area> {
        let draft = self.draft.borrow();
        let (&start, &end) = (draft.first()?, draft.last()?);
        Some(Area::between(start, end))
    }

    pub(super) fn commit_crop(&self) {
        let area = self.crop_draft().filter(|area| !area.is_tiny());
        self.draft.borrow_mut().clear();
        let before = self.document.borrow().crop();
        if area != before {
            self.document.borrow_mut().apply(Edit::Crop {
                before,
                after: area,
            });
            self.edited();
            match area {
                Some(area) => self.say(
                    &format!(
                        "Cropped to {}x{}. Copy and Save keep only this part.",
                        area.width.round(),
                        area.height.round()
                    ),
                    false,
                ),
                None => self.say("Crop cleared", false),
            }
        }
        self.canvas.queue_draw();
    }

    pub(super) fn paint_crop(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let dragging = self.tool.get() == Tool::Crop && !self.draft.borrow().is_empty();
        let area = if dragging {
            self.crop_draft()
        } else {
            self.document.borrow().crop()
        };
        let Some(area) = area else {
            return Ok(());
        };
        let (width, height) = self.size();
        let (left, top) = view.to_widget(Point { x: 0.0, y: 0.0 });
        let (right, bottom) = view.to_widget(Point {
            x: width,
            y: height,
        });
        let (x0, y0) = view.to_widget(Point {
            x: area.x,
            y: area.y,
        });
        let (x1, y1) = view.to_widget(Point {
            x: area.x + area.width,
            y: area.y + area.height,
        });
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        cr.rectangle(left, top, right - left, bottom - top);
        cr.rectangle(x0, y0, x1 - x0, y1 - y0);
        cr.set_source_rgba(0.0, 0.0, 0.0, SHADE);
        cr.fill()?;
        let (red, green, blue) = Color::Accent.rgb();
        cr.set_source_rgb(red, green, blue);
        cr.set_line_width(1.0);
        cr.set_dash(&[6.0, 4.0], 0.0);
        cr.rectangle(
            x0.round() + 0.5,
            y0.round() + 0.5,
            (x1 - x0).round() - 1.0,
            (y1 - y0).round() - 1.0,
        );
        let stroked = cr.stroke();
        cr.set_dash(&[], 0.0);
        stroked
    }
}
