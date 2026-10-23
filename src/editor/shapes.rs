use super::document::{Area, Point, Shape, Stroke};
use super::render;

const ROTATE_REACH: f64 = 24.0;
const SNAP_DEGREES: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Start,
    End,
    Rotate,
}

impl Handle {
    const CORNERS: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopRight,
            Self::BottomRight => Self::TopLeft,
            Self::Start => Self::End,
            Self::End => Self::Start,
            Self::Rotate => Self::Rotate,
        }
    }
}

impl Area {
    fn corner(self, handle: Handle) -> Point {
        let right = self.x + self.width;
        let bottom = self.y + self.height;
        let (x, y) = match handle {
            Handle::TopLeft | Handle::Start => (self.x, self.y),
            Handle::TopRight => (right, self.y),
            Handle::BottomLeft => (self.x, bottom),
            Handle::BottomRight | Handle::End => (right, bottom),
            Handle::Rotate => (self.x + self.width / 2.0, self.y),
        };
        Point { x, y }
    }

    pub(super) fn corners(self) -> [Point; 4] {
        [
            Handle::TopLeft,
            Handle::TopRight,
            Handle::BottomRight,
            Handle::BottomLeft,
        ]
        .map(|handle| self.corner(handle))
    }

    pub(super) fn grown(self, by: f64) -> Self {
        Self {
            x: self.x - by,
            y: self.y - by,
            width: self.width + by * 2.0,
            height: self.height + by * 2.0,
        }
    }

    fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    fn moved(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

impl Point {
    fn moved(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

/// Returns the radius of a numbered step circle drawn with `stroke`.
///
/// # Examples
///
/// ```ignore
/// let radius = step_radius(stroke);
/// ```
pub fn step_radius(stroke: Stroke) -> f64 {
    (stroke.width * 3.0 + 8.0).max(12.0)
}

/// Returns the side, in image pixels, of the square a check, cross or emoji fills.
///
/// # Examples
///
/// ```ignore
/// let side = stamp_size(stroke);
/// ```
pub fn stamp_size(stroke: Stroke) -> f64 {
    step_radius(stroke) * 2.0
}

fn snap_angle(degrees: f64) -> f64 {
    let wrapped = (degrees + 180.0).rem_euclid(360.0) - 180.0;
    let nearest = (wrapped / 90.0).round() * 90.0;
    let snapped = if (wrapped - nearest).abs() <= SNAP_DEGREES {
        nearest
    } else {
        wrapped
    };
    if snapped == 180.0 { -180.0 } else { snapped }
}

/// Returns the font size, in image pixels, for text drawn at line width `width`.
///
/// # Examples
///
/// ```ignore
/// let size = font_size(stroke.width);
/// ```
pub fn font_size(width: f64) -> f64 {
    width * 5.0 + 12.0
}

impl Shape {
    /// Returns the smallest area that holds the shape's geometry, ignoring line width.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let outline = shape.bounds();
    /// ```
    pub fn bounds(&self) -> Area {
        match self {
            Self::Rectangle { area, .. }
            | Self::Ellipse { area, .. }
            | Self::Highlight { area, .. }
            | Self::Pixelate { area } => *area,
            Self::Line { from, to, .. } | Self::Arrow { from, to, .. } => Area::between(*from, *to),
            Self::Pen { points, .. } => {
                let first = points.first().copied().unwrap_or(Point { x: 0.0, y: 0.0 });
                let (low, high) = points.iter().fold((first, first), |(low, high), point| {
                    (
                        Point {
                            x: low.x.min(point.x),
                            y: low.y.min(point.y),
                        },
                        Point {
                            x: high.x.max(point.x),
                            y: high.y.max(point.y),
                        },
                    )
                });
                Area::between(low, high)
            }
            Self::Step { at, stroke, .. } => {
                let radius = step_radius(*stroke);
                Area {
                    x: at.x - radius,
                    y: at.y - radius,
                    width: radius * 2.0,
                    height: radius * 2.0,
                }
            }
            Self::Mark { at, stroke, .. } | Self::Emoji { at, stroke, .. } => {
                let side = stamp_size(*stroke);
                Area {
                    x: at.x - side / 2.0,
                    y: at.y - side / 2.0,
                    width: side,
                    height: side,
                }
            }
            Self::Text {
                at, text, stroke, ..
            } => {
                let (width, height) = render::measure(text, font_size(stroke.width));
                Area {
                    x: at.x,
                    y: at.y,
                    width,
                    height,
                }
            }
        }
    }

    /// Tells whether `point` lies on the shape, allowing `slack` image pixels around its lines.
    ///
    /// Outlines only count near the line, so a click inside an empty box or ellipse falls
    /// through to whatever is beneath it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let picked = shape.hit(point, 6.0 / zoom);
    /// ```
    pub fn hit(&self, point: Point, slack: f64) -> bool {
        match self {
            Self::Rectangle { area, stroke } => {
                let reach = slack + stroke.width / 2.0;
                area.grown(reach).contains(point)
                    && !(area.width > reach * 2.0
                        && area.height > reach * 2.0
                        && area.grown(-reach).contains(point))
            }
            Self::Ellipse { area, stroke } => {
                let (a, b) = (area.width / 2.0, area.height / 2.0);
                if a < 1.0 || b < 1.0 {
                    return area.grown(slack).contains(point);
                }
                let dx = (point.x - area.x - a) / a;
                let dy = (point.y - area.y - b) / b;
                ((dx.hypot(dy) - 1.0) * a.min(b)).abs() <= slack + stroke.width / 2.0
            }
            Self::Highlight { area, .. } | Self::Pixelate { area } => {
                area.grown(slack).contains(point)
            }
            Self::Line { from, to, stroke } | Self::Arrow { from, to, stroke } => {
                segment_distance(point, *from, *to) <= slack + stroke.width / 2.0
            }
            Self::Pen { points, stroke } => {
                let reach = slack + stroke.width / 2.0;
                match points.as_slice() {
                    [only] => only.distance(point) <= reach,
                    _ => points
                        .windows(2)
                        .any(|pair| segment_distance(point, pair[0], pair[1]) <= reach),
                }
            }
            Self::Step { at, stroke, .. } => at.distance(point) <= step_radius(*stroke) + slack,
            Self::Text { angle, .. } | Self::Mark { angle, .. } | Self::Emoji { angle, .. } => {
                let upright = point.rotated(-angle, self.pivot());
                self.bounds().grown(slack).contains(upright)
            }
        }
    }

    /// The point a text or stamp turns around: the centre of its upright bounds.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let centre = shape.pivot();
    /// ```
    pub fn pivot(&self) -> Point {
        let bounds = self.bounds();
        Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        }
    }

    /// The clockwise rotation in degrees of a text or stamp, zero for everything else.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let degrees = shape.angle();
    /// ```
    pub fn angle(&self) -> f64 {
        match self {
            Self::Text { angle, .. } | Self::Mark { angle, .. } | Self::Emoji { angle, .. } => {
                *angle
            }
            _ => 0.0,
        }
    }

    /// Whether the shape can turn: texts and stamps can, outlines and strokes can't.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if shape.turns() { show_rotate_handle() }
    /// ```
    pub fn turns(&self) -> bool {
        matches!(
            self,
            Self::Text { .. } | Self::Mark { .. } | Self::Emoji { .. }
        )
    }

    /// Returns a copy turned to `degrees` clockwise, normalised to a half turn either way.
    ///
    /// Angles within a few degrees of a right angle snap to it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let tilted = shape.turned(15.0);
    /// ```
    pub fn turned(&self, degrees: f64) -> Self {
        let mut shape = self.clone();
        if let Self::Text { angle, .. } | Self::Mark { angle, .. } | Self::Emoji { angle, .. } =
            &mut shape
        {
            *angle = snap_angle(degrees);
        }
        shape
    }

    /// Returns a copy moved by `dx` and `dy` image pixels.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let nudged = shape.moved(1.0, 0.0);
    /// ```
    pub fn moved(&self, dx: f64, dy: f64) -> Self {
        let mut shape = self.clone();
        match &mut shape {
            Self::Rectangle { area, .. }
            | Self::Ellipse { area, .. }
            | Self::Highlight { area, .. }
            | Self::Pixelate { area } => *area = area.moved(dx, dy),
            Self::Line { from, to, .. } | Self::Arrow { from, to, .. } => {
                *from = from.moved(dx, dy);
                *to = to.moved(dx, dy);
            }
            Self::Pen { points, .. } => {
                for point in points.iter_mut() {
                    *point = point.moved(dx, dy);
                }
            }
            Self::Step { at, .. }
            | Self::Text { at, .. }
            | Self::Mark { at, .. }
            | Self::Emoji { at, .. } => *at = at.moved(dx, dy),
        }
        shape
    }

    /// Lists the drag handles of the shape: four corners for boxes, both ends for lines, and a
    /// rotate handle above a text or stamp.
    ///
    /// Pen strokes and steps have none; they can only be moved.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// for (handle, at) in shape.handles() { draw_handle(at) }
    /// ```
    pub fn handles(&self) -> Vec<(Handle, Point)> {
        match self {
            Self::Rectangle { area, .. }
            | Self::Ellipse { area, .. }
            | Self::Highlight { area, .. }
            | Self::Pixelate { area } => Handle::CORNERS
                .into_iter()
                .map(|handle| (handle, area.corner(handle)))
                .collect(),
            Self::Line { from, to, .. } | Self::Arrow { from, to, .. } => {
                vec![(Handle::Start, *from), (Handle::End, *to)]
            }
            Self::Text { .. } | Self::Mark { .. } | Self::Emoji { .. } => {
                let bounds = self.bounds();
                let pivot = self.pivot();
                let above = Point {
                    x: pivot.x,
                    y: bounds.y - ROTATE_REACH,
                };
                vec![(Handle::Rotate, above.rotated(self.angle(), pivot))]
            }
            Self::Pen { .. } | Self::Step { .. } => Vec::new(),
        }
    }

    /// Returns a copy with `handle` dragged to `to` while the opposite handle stays put.
    ///
    /// Dragging the rotate handle turns a text or stamp to face the pointer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let wider = shape.resized(Handle::BottomRight, point);
    /// ```
    pub fn resized(&self, handle: Handle, to: Point) -> Self {
        let mut shape = self.clone();
        match &mut shape {
            Self::Rectangle { area, .. }
            | Self::Ellipse { area, .. }
            | Self::Highlight { area, .. }
            | Self::Pixelate { area } => {
                if Handle::CORNERS.contains(&handle) {
                    *area = Area::between(area.corner(handle.opposite()), to);
                }
            }
            Self::Line { from, to: end, .. } | Self::Arrow { from, to: end, .. } => match handle {
                Handle::Start => *from = to,
                Handle::End => *end = to,
                _ => {}
            },
            Self::Text { .. } | Self::Mark { .. } | Self::Emoji { .. } => {
                if handle == Handle::Rotate {
                    let pivot = shape.pivot();
                    let degrees = (to.y - pivot.y).atan2(to.x - pivot.x).to_degrees() + 90.0;
                    shape = shape.turned(degrees);
                }
            }
            Self::Pen { .. } | Self::Step { .. } => {}
        }
        shape
    }

    /// Returns the line style of a shape that has one.
    ///
    /// Markers and pixelated areas have none.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let opacity = shape.stroke().map_or(1.0, |stroke| stroke.opacity);
    /// ```
    pub fn stroke(&self) -> Option<Stroke> {
        match self {
            Self::Rectangle { stroke, .. }
            | Self::Ellipse { stroke, .. }
            | Self::Line { stroke, .. }
            | Self::Arrow { stroke, .. }
            | Self::Pen { stroke, .. }
            | Self::Step { stroke, .. }
            | Self::Text { stroke, .. }
            | Self::Mark { stroke, .. }
            | Self::Emoji { stroke, .. } => Some(*stroke),
            Self::Highlight { .. } | Self::Pixelate { .. } => None,
        }
    }

    /// Returns a copy drawn with `style`: its colour, width and opacity on a shape with a line.
    ///
    /// Markers only take the colour. Pixelated areas come back unchanged.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let restyled = shape.restyled(editor.stroke());
    /// ```
    pub fn restyled(&self, style: Stroke) -> Self {
        let mut shape = self.clone();
        match &mut shape {
            Self::Rectangle { stroke, .. }
            | Self::Ellipse { stroke, .. }
            | Self::Line { stroke, .. }
            | Self::Arrow { stroke, .. }
            | Self::Pen { stroke, .. }
            | Self::Step { stroke, .. }
            | Self::Text { stroke, .. }
            | Self::Mark { stroke, .. }
            | Self::Emoji { stroke, .. } => *stroke = style,
            Self::Highlight { color, .. } => *color = style.color,
            Self::Pixelate { .. } => {}
        }
        shape
    }
}

fn segment_distance(point: Point, start: Point, end: Point) -> f64 {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let length = dx * dx + dy * dy;
    if length == 0.0 {
        return point.distance(start);
    }
    let along = (((point.x - start.x) * dx + (point.y - start.y) * dy) / length).clamp(0.0, 1.0);
    point.distance(Point {
        x: start.x + along * dx,
        y: start.y + along * dy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::document::{Color, MarkKind};

    const STROKE: Stroke = Stroke {
        color: Color::Accent,
        width: 2.0,
        opacity: 1.0,
    };

    fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn area(x: f64, y: f64, width: f64, height: f64) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn rectangle() -> Shape {
        Shape::Rectangle {
            area: area(10.0, 10.0, 100.0, 50.0),
            stroke: STROKE,
        }
    }

    #[test]
    fn hits_a_box_on_its_outline_but_not_inside() {
        let shape = rectangle();
        assert!(shape.hit(point(10.0, 30.0), 3.0));
        assert!(shape.hit(point(6.5, 30.0), 3.0));
        assert!(!shape.hit(point(60.0, 35.0), 3.0));
        assert!(!shape.hit(point(0.0, 30.0), 3.0));
    }

    #[test]
    fn hits_an_ellipse_near_its_curve() {
        let shape = Shape::Ellipse {
            area: area(0.0, 0.0, 100.0, 50.0),
            stroke: STROKE,
        };
        assert!(shape.hit(point(100.0, 25.0), 2.0));
        assert!(shape.hit(point(50.0, 1.0), 2.0));
        assert!(!shape.hit(point(50.0, 25.0), 2.0));
        assert!(!shape.hit(point(100.0, 0.0), 2.0));
    }

    #[test]
    fn hits_lines_and_pen_strokes_along_their_segments() {
        let line = Shape::Arrow {
            from: point(0.0, 0.0),
            to: point(100.0, 0.0),
            stroke: STROKE,
        };
        assert!(line.hit(point(50.0, 3.0), 2.0));
        assert!(!line.hit(point(50.0, 5.0), 2.0));
        assert!(!line.hit(point(110.0, 0.0), 2.0));
        let pen = Shape::Pen {
            points: vec![point(0.0, 0.0), point(10.0, 10.0), point(20.0, 0.0)],
            stroke: STROKE,
        };
        assert!(pen.hit(point(15.0, 5.0), 1.0));
        assert!(!pen.hit(point(10.0, 0.0), 1.0));
    }

    #[test]
    fn hits_filled_areas_anywhere_inside() {
        let shape = Shape::Pixelate {
            area: area(0.0, 0.0, 10.0, 10.0),
        };
        assert!(shape.hit(point(5.0, 5.0), 0.0));
        assert!(!shape.hit(point(15.0, 5.0), 2.0));
    }

    #[test]
    fn moves_every_point() {
        let pen = Shape::Pen {
            points: vec![point(0.0, 0.0), point(5.0, 5.0)],
            stroke: STROKE,
        };
        assert_eq!(
            pen.moved(2.0, -1.0),
            Shape::Pen {
                points: vec![point(2.0, -1.0), point(7.0, 4.0)],
                stroke: STROKE
            }
        );
        assert_eq!(
            rectangle().moved(1.0, 1.0).bounds(),
            area(11.0, 11.0, 100.0, 50.0)
        );
    }

    #[test]
    fn resizes_from_the_opposite_corner() {
        let shape = rectangle().resized(Handle::TopLeft, point(130.0, 0.0));
        assert_eq!(shape.bounds(), area(110.0, 0.0, 20.0, 60.0));
        let shape = rectangle().resized(Handle::BottomRight, point(60.0, 20.0));
        assert_eq!(shape.bounds(), area(10.0, 10.0, 50.0, 10.0));
    }

    #[test]
    fn moves_one_end_of_a_line() {
        let line = Shape::Line {
            from: point(0.0, 0.0),
            to: point(10.0, 0.0),
            stroke: STROKE,
        };
        assert_eq!(
            line.resized(Handle::Start, point(5.0, 5.0)).handles(),
            [
                (Handle::Start, point(5.0, 5.0)),
                (Handle::End, point(10.0, 0.0))
            ]
        );
    }

    #[test]
    fn offers_no_handles_for_pen_and_steps() {
        let step = Shape::Step {
            at: point(20.0, 20.0),
            number: 1,
            stroke: STROKE,
        };
        assert!(step.handles().is_empty());
        assert_eq!(step.bounds(), area(6.0, 6.0, 28.0, 28.0));
    }

    #[test]
    fn measures_text_to_select_and_move_it() {
        let text = Shape::Text {
            at: point(10.0, 20.0),
            text: "Hello".into(),
            stroke: STROKE,
            angle: 0.0,
        };
        let bounds = text.bounds();
        assert_eq!((bounds.x, bounds.y), (10.0, 20.0));
        assert!(bounds.width > font_size(STROKE.width));
        assert!(bounds.height >= font_size(STROKE.width));
        assert!(text.hit(point(12.0, 25.0), 0.0));
        assert!(!text.hit(point(5.0, 25.0), 0.0));
        assert_eq!(text.handles().len(), 1);
        assert_eq!(text.moved(1.0, 1.0).bounds().x, 11.0);
    }

    #[test]
    fn turns_a_stamp_around_its_centre_and_snaps_near_right_angles() {
        let mark = Shape::Mark {
            at: point(50.0, 40.0),
            kind: MarkKind::Check,
            stroke: STROKE,
            angle: 0.0,
        };
        assert_eq!(mark.pivot(), point(50.0, 40.0));
        let (handle, above) = mark.handles()[0];
        assert_eq!(handle, Handle::Rotate);
        assert_eq!(above.x, 50.0);
        assert!(above.y < mark.bounds().y);
        let right = mark.resized(Handle::Rotate, point(90.0, 40.0));
        assert_eq!(right.angle(), 90.0);
        let nearly = mark.resized(Handle::Rotate, point(90.0, 42.0));
        assert_eq!(nearly.angle(), 90.0);
        let tilted = mark.turned(30.0);
        assert_eq!(tilted.angle(), 30.0);
        assert_eq!(tilted.turned(370.0).angle(), 10.0);
        assert_eq!(mark.turned(-180.0).angle(), -180.0);
        assert!(mark.turns());
        let side = stamp_size(STROKE);
        assert!(tilted.hit(point(50.0, 40.0), 0.0));
        assert!(!tilted.hit(point(50.0 + side, 40.0), 2.0));
        let text = Shape::Text {
            at: point(0.0, 0.0),
            text: "Wide text here".into(),
            stroke: STROKE,
            angle: 90.0,
        };
        let bounds = text.bounds();
        let upright_end = point(bounds.width - 2.0, bounds.height / 2.0);
        assert!(!text.hit(upright_end, 0.0));
        assert!(text.hit(upright_end.rotated(90.0, text.pivot()), 0.0));
    }

    #[test]
    fn centers_a_stamp_on_its_point_and_moves_it_whole() {
        let mark = Shape::Mark {
            at: point(50.0, 40.0),
            kind: MarkKind::Check,
            stroke: STROKE,
            angle: 0.0,
        };
        let side = stamp_size(STROKE);
        assert_eq!(
            mark.bounds(),
            area(50.0 - side / 2.0, 40.0 - side / 2.0, side, side)
        );
        assert!(mark.hit(point(50.0, 40.0), 0.0));
        assert!(!mark.hit(point(50.0 + side, 40.0), 2.0));
        assert_eq!(mark.handles().len(), 1);
        let emoji = Shape::Emoji {
            at: point(10.0, 10.0),
            glyph: "🔥".to_owned(),
            stroke: STROKE,
            angle: 0.0,
        };
        let moved = emoji.moved(5.0, -5.0).bounds();
        assert_eq!(moved.x, 10.0 + 5.0 - side / 2.0);
        assert_eq!(moved.y, 10.0 - 5.0 - side / 2.0);
    }

    #[test]
    fn grows_a_stamp_with_the_line_width() {
        let thin = Stroke {
            width: 1.0,
            ..STROKE
        };
        let thick = Stroke {
            width: 12.0,
            ..STROKE
        };
        assert!(stamp_size(thick) > stamp_size(thin));
    }

    #[test]
    fn restyles_lines_and_fills() {
        let style = Stroke {
            color: Color::Red,
            width: 9.0,
            opacity: 0.5,
        };
        assert_eq!(
            rectangle().restyled(style),
            Shape::Rectangle {
                area: area(10.0, 10.0, 100.0, 50.0),
                stroke: style
            }
        );
        let marker = Shape::Highlight {
            area: area(0.0, 0.0, 1.0, 1.0),
            color: Color::Yellow,
        };
        assert_eq!(
            marker.restyled(Stroke {
                color: Color::Green,
                ..style
            }),
            Shape::Highlight {
                area: area(0.0, 0.0, 1.0, 1.0),
                color: Color::Green
            }
        );
    }
}
