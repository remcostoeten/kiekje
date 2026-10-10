use gtk::cairo;
use gtk::prelude::*;

use super::Editor;
use super::canvas::View;
use super::document::{Area, Color, Edit, Point, Tool};
use super::select::tool_cursor;

const SHADE: f64 = 0.6;
const INLINE_SHADE: f64 = 0.45;
const REACH: f64 = 8.0;
const HANDLE: f64 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Inside,
}

impl Edge {
    fn cursor(self) -> &'static str {
        match self {
            Self::Top | Self::Bottom => "ns-resize",
            Self::Left | Self::Right => "ew-resize",
            Self::TopLeft | Self::BottomRight => "nwse-resize",
            Self::TopRight | Self::BottomLeft => "nesw-resize",
            Self::Inside => "move",
        }
    }
}

pub(super) struct Reshaping {
    edge: Edge,
    start: Point,
    original: Area,
    preview: Area,
}

fn edge_at(area: Area, point: Point, slack: f64) -> Option<Edge> {
    let (right, bottom) = (area.x + area.width, area.y + area.height);
    let inside_x = point.x >= area.x - slack && point.x <= right + slack;
    let inside_y = point.y >= area.y - slack && point.y <= bottom + slack;
    if !(inside_x && inside_y) {
        return None;
    }
    let near = |value: f64, edge: f64| (value - edge).abs() <= slack;
    let left = near(point.x, area.x);
    let right = !left && near(point.x, right);
    let top = near(point.y, area.y);
    let bottom = !top && near(point.y, bottom);
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => Edge::TopLeft,
        (_, true, true, _) => Edge::TopRight,
        (true, _, _, true) => Edge::BottomLeft,
        (_, true, _, true) => Edge::BottomRight,
        (true, ..) => Edge::Left,
        (_, true, ..) => Edge::Right,
        (_, _, true, _) => Edge::Top,
        (_, _, _, true) => Edge::Bottom,
        _ => Edge::Inside,
    })
}

fn reshape(area: Area, edge: Edge, delta: (f64, f64), bounds: (f64, f64)) -> Area {
    if edge == Edge::Inside {
        return Area {
            x: (area.x + delta.0).clamp(0.0, (bounds.0 - area.width).max(0.0)),
            y: (area.y + delta.1).clamp(0.0, (bounds.1 - area.height).max(0.0)),
            ..area
        };
    }
    let (mut left, mut top) = (area.x, area.y);
    let (mut right, mut bottom) = (area.x + area.width, area.y + area.height);
    match edge {
        Edge::Left | Edge::TopLeft | Edge::BottomLeft => left += delta.0,
        Edge::Right | Edge::TopRight | Edge::BottomRight => right += delta.0,
        Edge::Top | Edge::Bottom | Edge::Inside => {}
    }
    match edge {
        Edge::Top | Edge::TopLeft | Edge::TopRight => top += delta.1,
        Edge::Bottom | Edge::BottomLeft | Edge::BottomRight => bottom += delta.1,
        Edge::Left | Edge::Right | Edge::Inside => {}
    }
    Area::between(
        Point {
            x: left.clamp(0.0, bounds.0),
            y: top.clamp(0.0, bounds.1),
        },
        Point {
            x: right.clamp(0.0, bounds.0),
            y: bottom.clamp(0.0, bounds.1),
        },
    )
}

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
                None if self.is_inline() => {
                    self.say("Region cleared, the whole screen is kept", false)
                }
                None => self.say("Crop cleared", false),
            }
        }
        self.place_bars();
        self.canvas.queue_draw();
    }

    fn region_edge(&self, point: Point) -> Option<Edge> {
        if !self.is_inline() {
            return None;
        }
        let area = self.document.borrow().crop()?;
        edge_at(area, point, REACH / self.view.get().zoom())
            .filter(|edge| *edge != Edge::Inside || self.tool.get() == Tool::Crop)
    }

    pub(super) fn grab_region(&self, point: Point) -> bool {
        let Some(edge) = self.region_edge(point) else {
            return false;
        };
        let Some(area) = self.document.borrow().crop() else {
            return false;
        };
        self.reshaping.replace(Some(Reshaping {
            edge,
            start: point,
            original: area,
            preview: area,
        }));
        self.hide_bars();
        true
    }

    pub(super) fn reshape_to(&self, point: Point) -> bool {
        let size = self.size();
        let mut reshaping = self.reshaping.borrow_mut();
        let Some(reshaping) = reshaping.as_mut() else {
            return false;
        };
        let delta = (point.x - reshaping.start.x, point.y - reshaping.start.y);
        reshaping.preview = reshape(reshaping.original, reshaping.edge, delta, size);
        self.canvas.queue_draw();
        true
    }

    pub(super) fn release_region(&self) -> bool {
        let Some(reshaping) = self.reshaping.take() else {
            return false;
        };
        if !reshaping.preview.is_tiny() && reshaping.preview != reshaping.original {
            self.document.borrow_mut().apply(Edit::Crop {
                before: Some(reshaping.original),
                after: Some(reshaping.preview),
            });
            self.edited();
        }
        self.place_bars();
        self.canvas.queue_draw();
        true
    }

    pub(super) fn cancel_reshape(&self) -> bool {
        if self.reshaping.take().is_none() {
            return false;
        }
        self.place_bars();
        self.canvas.queue_draw();
        true
    }

    pub(super) fn hover_region(&self, point: Point) {
        if !self.is_inline() || self.reshaping.borrow().is_some() {
            return;
        }
        let cursor = self
            .region_edge(point)
            .map_or_else(|| tool_cursor(self.tool.get()), Edge::cursor);
        let current = self.canvas.cursor().and_then(|cursor| cursor.name());
        if current.as_deref() != Some(cursor) {
            self.canvas.set_cursor_from_name(Some(cursor));
        }
    }

    pub(super) fn step_region(&self, dx: f64, dy: f64, resize: bool) -> bool {
        let Some(area) = self.document.borrow().crop() else {
            return false;
        };
        let edge = if resize {
            Edge::BottomRight
        } else {
            Edge::Inside
        };
        let stepped = reshape(area, edge, (dx, dy), self.size());
        if stepped != area && !stepped.is_tiny() {
            self.document.borrow_mut().apply(Edit::Crop {
                before: Some(area),
                after: Some(stepped),
            });
            self.edited();
            self.place_bars();
        }
        true
    }

    pub(super) fn paint_crop(&self, cr: &cairo::Context, view: View) -> Result<(), cairo::Error> {
        let drafting = self.tool.get() == Tool::Crop && !self.draft.borrow().is_empty();
        let reshaping = self
            .reshaping
            .borrow()
            .as_ref()
            .map(|reshaping| reshaping.preview);
        let area = if drafting {
            self.crop_draft()
        } else {
            reshaping.or_else(|| self.document.borrow().crop())
        };
        let Some(area) = area else {
            return Ok(());
        };
        let inline = self.is_inline();
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
        cr.set_source_rgba(0.0, 0.0, 0.0, if inline { INLINE_SHADE } else { SHADE });
        cr.fill()?;
        let (red, green, blue) = Color::Accent.rgb();
        cr.set_source_rgb(red, green, blue);
        cr.set_line_width(1.0);
        if !inline {
            cr.set_dash(&[6.0, 4.0], 0.0);
        }
        cr.rectangle(
            x0.round() + 0.5,
            y0.round() + 0.5,
            (x1 - x0).round() - 1.0,
            (y1 - y0).round() - 1.0,
        );
        let stroked = cr.stroke();
        cr.set_dash(&[], 0.0);
        stroked?;
        if !inline || drafting || reshaping.is_some() {
            return Ok(());
        }
        let (middle_x, middle_y) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for (x, y) in [
            (x0, y0),
            (middle_x, y0),
            (x1, y0),
            (x0, middle_y),
            (x1, middle_y),
            (x0, y1),
            (middle_x, y1),
            (x1, y1),
        ] {
            cr.rectangle(
                (x - HANDLE / 2.0).round(),
                (y - HANDLE / 2.0).round(),
                HANDLE,
                HANDLE,
            );
        }
        cr.fill()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: (f64, f64) = (1000.0, 800.0);

    fn area(x: f64, y: f64, width: f64, height: f64) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn finds_corners_before_edges_and_the_inside_last() {
        let region = area(100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            edge_at(region, point(103.0, 98.0), 8.0),
            Some(Edge::TopLeft)
        );
        assert_eq!(
            edge_at(region, point(298.0, 200.0), 8.0),
            Some(Edge::BottomRight)
        );
        assert_eq!(edge_at(region, point(200.0, 104.0), 8.0), Some(Edge::Top));
        assert_eq!(edge_at(region, point(305.0, 150.0), 8.0), Some(Edge::Right));
        assert_eq!(
            edge_at(region, point(200.0, 150.0), 8.0),
            Some(Edge::Inside)
        );
        assert_eq!(edge_at(region, point(50.0, 150.0), 8.0), None);
    }

    #[test]
    fn resizes_from_a_corner_and_keeps_the_opposite_one() {
        let region = area(100.0, 100.0, 200.0, 100.0);
        let resized = reshape(region, Edge::TopLeft, (-20.0, -10.0), BOUNDS);
        assert_eq!(resized, area(80.0, 90.0, 220.0, 110.0));
    }

    #[test]
    fn flips_when_an_edge_is_dragged_past_the_opposite_one() {
        let region = area(100.0, 100.0, 200.0, 100.0);
        let flipped = reshape(region, Edge::Left, (250.0, 0.0), BOUNDS);
        assert_eq!(flipped, area(300.0, 100.0, 50.0, 100.0));
    }

    #[test]
    fn keeps_a_moved_region_inside_the_image() {
        let region = area(100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            reshape(region, Edge::Inside, (-500.0, 900.0), BOUNDS),
            area(0.0, 700.0, 200.0, 100.0)
        );
    }

    #[test]
    fn clamps_a_resize_to_the_image() {
        let region = area(100.0, 100.0, 200.0, 100.0);
        assert_eq!(
            reshape(region, Edge::BottomRight, (5000.0, 5000.0), BOUNDS),
            area(100.0, 100.0, 900.0, 700.0)
        );
    }
}
