use serde::{Deserialize, Serialize};

const MIN_SIZE: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// Returns the point turned `degrees` clockwise around `pivot`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let turned = point.rotated(90.0, centre);
    /// ```
    pub fn rotated(self, degrees: f64, pivot: Self) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let (dx, dy) = (self.x - pivot.x, self.y - pivot.y);
        Self {
            x: pivot.x + dx * cos - dy * sin,
            y: pivot.y + dx * sin + dy * cos,
        }
    }

    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn snapped(self, start: Self) -> Self {
        let step = std::f64::consts::FRAC_PI_4;
        let angle = ((self.y - start.y).atan2(self.x - start.x) / step).round() * step;
        let length = start.distance(self);
        Self {
            x: start.x + length * angle.cos(),
            y: start.y + length * angle.sin(),
        }
    }

    pub fn squared(self, start: Self) -> Self {
        let (dx, dy) = (self.x - start.x, self.y - start.y);
        let side = dx.abs().max(dy.abs());
        Self {
            x: start.x + side.copysign(dx),
            y: start.y + side.copysign(dy),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Area {
    pub fn between(start: Point, end: Point) -> Self {
        Self {
            x: start.x.min(end.x),
            y: start.y.min(end.y),
            width: (start.x - end.x).abs(),
            height: (start.y - end.y).abs(),
        }
    }

    pub fn is_tiny(self) -> bool {
        self.width < MIN_SIZE || self.height < MIN_SIZE
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    #[default]
    #[serde(rename = "orange")]
    Accent,
    Red,
    Yellow,
    Green,
    Blue,
    White,
    Black,
}

impl Color {
    pub const ALL: [Self; 7] = [
        Self::Accent,
        Self::Red,
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::White,
        Self::Black,
    ];

    pub fn rgb(self) -> (f64, f64, f64) {
        let (red, green, blue) = match self {
            Self::Accent => (0xfe, 0x51, 0x01),
            Self::Red => (0xe5, 0x2b, 0x2b),
            Self::Yellow => (0xff, 0xd6, 0x0a),
            Self::Green => (0x2f, 0xbf, 0x71),
            Self::Blue => (0x2f, 0x80, 0xed),
            Self::White => (0xff, 0xff, 0xff),
            Self::Black => (0x11, 0x11, 0x11),
        };
        (
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0,
        )
    }

    /// Returns the palette colour `steps` places further along, wrapping at both ends.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(Color::Accent.shifted(-1), Color::Black);
    /// ```
    pub fn shifted(self, steps: i32) -> Self {
        let count = Self::ALL.len() as i32;
        let at = Self::ALL
            .iter()
            .position(|color| *color == self)
            .unwrap_or(0) as i32;
        Self::ALL[(at + steps).rem_euclid(count) as usize]
    }

    /// Returns the palette colour that digit key `digit` picks, counting from 1.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(Color::numbered('5'), Some(Color::Blue));
    /// ```
    pub fn numbered(digit: char) -> Option<Self> {
        let index = digit.to_digit(10)?.checked_sub(1)?;
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }

    pub fn is_light(self) -> bool {
        matches!(self, Self::Yellow | Self::White)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Accent => "Orange",
            Self::Red => "Red",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::White => "White",
            Self::Black => "Black",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f64,
    pub opacity: f64,
}

pub const MIN_SIZE_PX: u32 = 1;
pub const MAX_SIZE_PX: u32 = 24;
pub const MIN_OPACITY: u32 = 10;

impl Stroke {
    /// Turns a size in screen pixels into a line width in image pixels.
    ///
    /// Captures above 1080 pixels on their short side are usually taken on scaled displays, so
    /// the width grows with them, up to twice the size at 2160 pixels.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let stroke = Stroke::scaled(Color::Red, 3, 100, (3840, 2160));
    /// assert_eq!(stroke.width, 6.0);
    /// ```
    pub fn scaled(color: Color, size: u32, opacity: u32, image: (u32, u32)) -> Self {
        let unit = unit(image);
        let size = size.clamp(MIN_SIZE_PX, MAX_SIZE_PX);
        Self {
            color,
            width: (f64::from(size) * unit).round().max(1.0),
            opacity: f64::from(opacity.clamp(MIN_OPACITY, 100)) / 100.0,
        }
    }

    /// Turns the line back into the size and opacity the toolbar shows, the reverse of `scaled`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let (size, opacity) = shape_stroke.settings((3840, 2160));
    /// ```
    pub fn settings(self, image: (u32, u32)) -> (u32, u32) {
        let size = (self.width / unit(image))
            .round()
            .clamp(f64::from(MIN_SIZE_PX), f64::from(MAX_SIZE_PX));
        let opacity = (self.opacity * 100.0)
            .round()
            .clamp(f64::from(MIN_OPACITY), 100.0);
        (size as u32, opacity as u32)
    }
}

fn unit(image: (u32, u32)) -> f64 {
    (f64::from(image.0.min(image.1)) / 1080.0).clamp(1.0, 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Check,
    Cross,
}

pub const DEFAULT_EMOJI: &str = "👍";

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rectangle {
        area: Area,
        stroke: Stroke,
    },
    Ellipse {
        area: Area,
        stroke: Stroke,
    },
    Line {
        from: Point,
        to: Point,
        stroke: Stroke,
    },
    Arrow {
        from: Point,
        to: Point,
        stroke: Stroke,
    },
    Pen {
        points: Vec<Point>,
        stroke: Stroke,
    },
    Highlight {
        area: Area,
        color: Color,
    },
    Step {
        at: Point,
        number: u32,
        stroke: Stroke,
    },
    Pixelate {
        area: Area,
    },
    Text {
        at: Point,
        text: String,
        stroke: Stroke,
        angle: f64,
    },
    Mark {
        at: Point,
        kind: MarkKind,
        stroke: Stroke,
        angle: f64,
    },
    Emoji {
        at: Point,
        glyph: String,
        stroke: Stroke,
        angle: f64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    Select,
    #[default]
    Rectangle,
    Ellipse,
    Arrow,
    Line,
    Pen,
    Highlight,
    Step,
    Text,
    Check,
    Cross,
    Emoji,
    Pixelate,
    Crop,
}

impl Tool {
    pub const ALL: [Self; 14] = [
        Self::Select,
        Self::Rectangle,
        Self::Ellipse,
        Self::Arrow,
        Self::Line,
        Self::Pen,
        Self::Highlight,
        Self::Step,
        Self::Text,
        Self::Check,
        Self::Cross,
        Self::Emoji,
        Self::Pixelate,
        Self::Crop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Rectangle => "Box",
            Self::Ellipse => "Ellipse",
            Self::Arrow => "Arrow",
            Self::Line => "Line",
            Self::Pen => "Pen",
            Self::Highlight => "Marker",
            Self::Step => "Step",
            Self::Text => "Text",
            Self::Check => "Check",
            Self::Cross => "Cross",
            Self::Emoji => "Emoji",
            Self::Pixelate => "Pixelate",
            Self::Crop => "Crop",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Select => "v",
            Self::Rectangle => "r",
            Self::Ellipse => "e",
            Self::Arrow => "a",
            Self::Line => "l",
            Self::Pen => "p",
            Self::Highlight => "m",
            Self::Step => "n",
            Self::Text => "t",
            Self::Check => "y",
            Self::Cross => "u",
            Self::Emoji => "i",
            Self::Pixelate => "x",
            Self::Crop => "c",
        }
    }

    /// Turns the points of one drag into a shape, or `None` when the drag was too small to mean one.
    ///
    /// Steps, checks, crosses and emoji need only a click; every other tool needs a drag of a few
    /// pixels.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let shape = Tool::Arrow.shape(&points, stroke, document.next_step());
    /// ```
    pub fn constrain(self, start: Point, end: Point) -> Point {
        match self {
            Self::Line | Self::Arrow | Self::Pen => end.snapped(start),
            Self::Rectangle | Self::Ellipse => end.squared(start),
            _ => end,
        }
    }

    pub fn shape(self, points: &[Point], stroke: Stroke, step: u32, glyph: &str) -> Option<Shape> {
        let (&start, &end) = (points.first()?, points.last()?);
        let area = Area::between(start, end);
        let long_enough = start.distance(end) >= MIN_SIZE;
        match self {
            Self::Rectangle if !area.is_tiny() => Some(Shape::Rectangle { area, stroke }),
            Self::Ellipse if !area.is_tiny() => Some(Shape::Ellipse { area, stroke }),
            Self::Highlight if !area.is_tiny() => Some(Shape::Highlight {
                area,
                color: stroke.color,
            }),
            Self::Pixelate if !area.is_tiny() => Some(Shape::Pixelate { area }),
            Self::Line if long_enough => Some(Shape::Line {
                from: start,
                to: end,
                stroke,
            }),
            Self::Arrow if long_enough => Some(Shape::Arrow {
                from: start,
                to: end,
                stroke,
            }),
            Self::Pen if points.len() > 1 => Some(Shape::Pen {
                points: points.to_vec(),
                stroke,
            }),
            Self::Step => Some(Shape::Step {
                at: end,
                number: step,
                stroke,
            }),
            Self::Check => Some(Shape::Mark {
                at: end,
                kind: MarkKind::Check,
                stroke,
                angle: 0.0,
            }),
            Self::Cross => Some(Shape::Mark {
                at: end,
                kind: MarkKind::Cross,
                stroke,
                angle: 0.0,
            }),
            Self::Emoji => Some(Shape::Emoji {
                at: end,
                glyph: glyph.to_owned(),
                stroke,
                angle: 0.0,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Add(Shape),
    Replace {
        index: usize,
        before: Shape,
        after: Shape,
    },
    Remove {
        index: usize,
        shape: Shape,
    },
    Clear(Vec<Shape>),
    Crop {
        before: Option<Area>,
        after: Option<Area>,
    },
    Batch(Vec<Edit>),
}

impl Edit {
    fn valid_for(&self, count: usize) -> bool {
        match self {
            Self::Replace { index, .. } | Self::Remove { index, .. } => *index < count,
            Self::Batch(edits) => edits.iter().all(|edit| edit.valid_for(count)),
            _ => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct Document {
    shapes: Vec<Shape>,
    crop: Option<Area>,
    done: Vec<Edit>,
    undone: Vec<Edit>,
    saved: usize,
    restyling: Option<usize>,
}

impl Document {
    /// Starts a document whose picture is not on disk yet, so closing it asks first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let document = Document::unsaved();
    /// ```
    pub fn unsaved() -> Self {
        Self {
            saved: usize::MAX,
            ..Self::default()
        }
    }

    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    pub fn crop(&self) -> Option<Area> {
        self.crop
    }

    /// Sets the crop the document opens with, outside the undo history.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// document.start_crop(Area { x: 10.0, y: 10.0, width: 640.0, height: 360.0 });
    /// ```
    pub fn start_crop(&mut self, area: Area) {
        self.crop = Some(area);
    }

    /// Applies `edit` and makes it the newest step to undo, dropping anything left to redo.
    ///
    /// An edit aimed at a shape that no longer exists is dropped instead.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// document.apply(Edit::Add(shape));
    /// ```
    pub fn apply(&mut self, edit: Edit) {
        if !edit.valid_for(self.shapes.len()) {
            eprintln!("kiekje: dropped an edit for a shape that no longer exists");
            return;
        }
        if self.saved > self.done.len() {
            self.saved = usize::MAX;
        }
        self.restyling = None;
        self.perform(&edit);
        self.done.push(edit);
        self.undone.clear();
    }

    /// Replaces the shape at `index` with a restyled copy, folding a run of restyles into one undo step.
    ///
    /// Dragging the size or opacity slider restyles many times; undo goes back to before the drag.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// document.restyle(index, shape.restyled(style));
    /// ```
    pub fn restyle(&mut self, index: usize, after: Shape) {
        let unsaved = self.saved != self.done.len();
        if let (
            Some(last),
            true,
            Some(Edit::Replace {
                index: at,
                after: newest,
                ..
            }),
        ) = (self.restyling, unsaved, self.done.last_mut())
            && last == index
            && *at == index
        {
            *newest = after.clone();
            self.put(index, &after);
            return;
        }
        let Some(before) = self.shapes.get(index).cloned() else {
            return;
        };
        self.apply(Edit::Replace {
            index,
            before,
            after,
        });
        self.restyling = Some(index);
    }

    /// Reverts the newest edit and returns whether there was one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if !document.undo() { status("Nothing to undo") }
    /// ```
    pub fn undo(&mut self) -> bool {
        self.restyling = None;
        let Some(edit) = self.done.pop() else {
            return false;
        };
        self.revert(&edit);
        self.undone.push(edit);
        true
    }

    /// Applies the newest undone edit again and returns whether there was one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// document.redo();
    /// ```
    pub fn redo(&mut self) -> bool {
        self.restyling = None;
        let Some(edit) = self.undone.pop() else {
            return false;
        };
        self.perform(&edit);
        self.done.push(edit);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.done.len();
    }

    pub fn is_modified(&self) -> bool {
        self.saved != self.done.len()
    }

    pub fn next_step(&self) -> u32 {
        self.shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Step { number, .. } => Some(*number),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Returns the index of the topmost shape under `point`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let picked = document.hit(point, 6.0 / zoom);
    /// ```
    pub fn hit(&self, point: Point, slack: f64) -> Option<usize> {
        self.shapes
            .iter()
            .rposition(|shape| shape.hit(point, slack))
    }

    fn perform(&mut self, edit: &Edit) {
        match edit {
            Edit::Add(shape) => self.shapes.push(shape.clone()),
            Edit::Replace { index, after, .. } => self.put(*index, after),
            Edit::Remove { index, .. } => {
                if *index < self.shapes.len() {
                    self.shapes.remove(*index);
                }
            }
            Edit::Clear(_) => self.shapes.clear(),
            Edit::Crop { after, .. } => self.crop = *after,
            Edit::Batch(edits) => {
                for edit in edits {
                    self.perform(edit);
                }
            }
        }
    }

    fn revert(&mut self, edit: &Edit) {
        match edit {
            Edit::Add(_) => {
                self.shapes.pop();
            }
            Edit::Replace { index, before, .. } => self.put(*index, before),
            Edit::Remove { index, shape } => {
                let index = (*index).min(self.shapes.len());
                self.shapes.insert(index, shape.clone());
            }
            Edit::Clear(shapes) => self.shapes = shapes.clone(),
            Edit::Crop { before, .. } => self.crop = *before,
            Edit::Batch(edits) => {
                for edit in edits.iter().rev() {
                    self.revert(edit);
                }
            }
        }
    }

    fn put(&mut self, index: usize, shape: &Shape) {
        if let Some(slot) = self.shapes.get_mut(index) {
            *slot = shape.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn snaps_a_shallow_drag_to_a_horizontal_line() {
        let start = Point { x: 10.0, y: 10.0 };
        let end = Tool::Line.constrain(start, Point { x: 110.0, y: 14.0 });
        assert!((end.y - 10.0).abs() < 1e-9);
        assert!((end.x - 10.0 - 100.0_f64.hypot(4.0)).abs() < 1e-9);
    }

    #[test]
    fn snaps_a_steep_pen_drag_to_a_vertical_line() {
        let start = Point { x: 0.0, y: 0.0 };
        let end = Tool::Pen.constrain(start, Point { x: -3.0, y: -50.0 });
        assert!(end.x.abs() < 1e-9);
        assert!(end.y < 0.0);
    }

    #[test]
    fn snaps_a_near_diagonal_arrow_to_forty_five_degrees() {
        let start = Point { x: 0.0, y: 0.0 };
        let end = Tool::Arrow.constrain(start, Point { x: 40.0, y: 46.0 });
        assert!((end.x - end.y).abs() < 1e-9);
    }

    #[test]
    fn squares_a_rectangle_in_the_drag_direction() {
        let start = Point { x: 50.0, y: 50.0 };
        let end = Tool::Rectangle.constrain(start, Point { x: 20.0, y: 70.0 });
        assert!(near(end, Point { x: 20.0, y: 80.0 }));
    }

    #[test]
    fn leaves_unconstrained_tools_alone() {
        let start = Point { x: 0.0, y: 0.0 };
        let end = Point { x: 13.0, y: 7.0 };
        assert!(near(Tool::Pixelate.constrain(start, end), end));
    }

    const STROKE: Stroke = Stroke {
        color: Color::Accent,
        width: 3.0,
        opacity: 1.0,
    };

    fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn step(number: u32) -> Shape {
        Shape::Step {
            at: point(1.0, 1.0),
            number,
            stroke: STROKE,
        }
    }

    #[test]
    fn normalizes_a_drag_in_any_direction() {
        let area = Area::between(point(40.0, 10.0), point(10.0, 30.0));
        assert_eq!(
            area,
            Area {
                x: 10.0,
                y: 10.0,
                width: 30.0,
                height: 20.0
            }
        );
    }

    #[test]
    fn ignores_drags_too_small_to_mean_a_shape() {
        let click = [point(5.0, 5.0), point(6.0, 6.0)];
        for tool in [
            Tool::Rectangle,
            Tool::Ellipse,
            Tool::Arrow,
            Tool::Line,
            Tool::Highlight,
            Tool::Pixelate,
        ] {
            assert_eq!(tool.shape(&click, STROKE, 1, ""), None, "{tool:?}");
        }
        assert_eq!(Tool::Pen.shape(&click[..1], STROKE, 1, ""), None);
    }

    #[test]
    fn places_a_step_with_a_click() {
        let click = [point(5.0, 5.0)];
        assert_eq!(
            Tool::Step.shape(&click, STROKE, 4, ""),
            Some(Shape::Step {
                at: point(5.0, 5.0),
                number: 4,
                stroke: STROKE
            })
        );
    }

    #[test]
    fn places_checks_crosses_and_emoji_with_a_click() {
        let click = [point(5.0, 5.0)];
        assert_eq!(
            Tool::Check.shape(&click, STROKE, 1, ""),
            Some(Shape::Mark {
                at: point(5.0, 5.0),
                kind: MarkKind::Check,
                stroke: STROKE,
                angle: 0.0
            })
        );
        assert_eq!(
            Tool::Cross.shape(&click, STROKE, 1, ""),
            Some(Shape::Mark {
                at: point(5.0, 5.0),
                kind: MarkKind::Cross,
                stroke: STROKE,
                angle: 0.0
            })
        );
        assert_eq!(
            Tool::Emoji.shape(&click, STROKE, 1, "🔥"),
            Some(Shape::Emoji {
                at: point(5.0, 5.0),
                glyph: "🔥".to_owned(),
                stroke: STROKE,
                angle: 0.0
            })
        );
    }

    #[test]
    fn undoes_a_batch_as_one_step() {
        let mut document = Document::default();
        for index in 0..3 {
            document.apply(Edit::Add(step(index)));
        }
        document.apply(Edit::Batch(vec![
            Edit::Remove {
                index: 2,
                shape: step(2),
            },
            Edit::Remove {
                index: 0,
                shape: step(0),
            },
        ]));
        assert_eq!(document.shapes(), &[step(1)]);
        assert!(document.undo());
        assert_eq!(document.shapes(), &[step(0), step(1), step(2)]);
        assert!(document.redo());
        assert_eq!(document.shapes(), &[step(1)]);
        document.apply(Edit::Batch(vec![Edit::Remove {
            index: 5,
            shape: step(5),
        }]));
        assert_eq!(document.shapes(), &[step(1)]);
    }

    #[test]
    fn keeps_every_pen_point() {
        let points = [point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)];
        let Some(Shape::Pen { points: kept, .. }) = Tool::Pen.shape(&points, STROKE, 1, "") else {
            panic!("expected a pen stroke");
        };
        assert_eq!(kept, points);
    }

    #[test]
    fn undoes_and_redoes_in_order() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        document.apply(Edit::Add(step(2)));
        assert!(document.undo());
        assert_eq!(document.shapes(), [step(1)]);
        assert!(document.redo());
        assert_eq!(document.shapes(), [step(1), step(2)]);
        assert!(!document.redo());
        assert!(document.undo() && document.undo());
        assert!(!document.undo());
        assert!(document.shapes().is_empty());
    }

    #[test]
    fn drops_redo_history_after_a_new_edit() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        document.undo();
        document.apply(Edit::Add(step(7)));
        assert!(!document.can_redo());
        assert_eq!(document.shapes(), [step(7)]);
    }

    #[test]
    fn restores_shapes_after_undoing_a_clear() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        document.apply(Edit::Clear(document.shapes().to_vec()));
        assert!(document.shapes().is_empty());
        document.undo();
        assert_eq!(document.shapes(), [step(1)]);
    }

    #[test]
    fn drops_an_edit_for_a_shape_that_is_gone() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        document.apply(Edit::Remove {
            index: 1,
            shape: step(2),
        });
        assert!(document.undo());
        assert!(document.shapes().is_empty());
        assert!(!document.undo());
    }

    #[test]
    fn undoes_a_replace_and_a_remove_in_place() {
        let mut document = Document::default();
        for number in 1..=3 {
            document.apply(Edit::Add(step(number)));
        }
        document.apply(Edit::Replace {
            index: 1,
            before: step(2),
            after: step(9),
        });
        document.apply(Edit::Remove {
            index: 0,
            shape: step(1),
        });
        assert_eq!(document.shapes(), [step(9), step(3)]);
        document.undo();
        assert_eq!(document.shapes(), [step(1), step(9), step(3)]);
        document.undo();
        assert_eq!(document.shapes(), [step(1), step(2), step(3)]);
    }

    #[test]
    fn picks_the_topmost_shape_under_a_point() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        document.apply(Edit::Add(step(2)));
        assert_eq!(document.hit(point(1.0, 1.0), 0.0), Some(1));
        assert_eq!(document.hit(point(500.0, 500.0), 0.0), None);
    }

    #[test]
    fn undoes_a_crop_back_to_the_previous_one() {
        let first = Area {
            x: 1.0,
            y: 1.0,
            width: 10.0,
            height: 10.0,
        };
        let mut document = Document::default();
        document.apply(Edit::Crop {
            before: None,
            after: Some(first),
        });
        document.apply(Edit::Crop {
            before: Some(first),
            after: None,
        });
        assert_eq!(document.crop(), None);
        document.undo();
        assert_eq!(document.crop(), Some(first));
        document.undo();
        assert_eq!(document.crop(), None);
    }

    #[test]
    fn numbers_steps_after_the_highest() {
        let mut document = Document::default();
        assert_eq!(document.next_step(), 1);
        document.apply(Edit::Add(step(3)));
        assert_eq!(document.next_step(), 4);
    }

    #[test]
    fn tracks_changes_since_the_last_save() {
        let mut document = Document::default();
        assert!(!document.is_modified());
        document.apply(Edit::Add(step(1)));
        assert!(document.is_modified());
        document.mark_saved();
        assert!(!document.is_modified());
        document.undo();
        assert!(document.is_modified());
        document.redo();
        assert!(!document.is_modified());
        document.undo();
        document.apply(Edit::Add(step(2)));
        assert!(document.is_modified());
        document.undo();
        assert!(document.is_modified());
    }

    #[test]
    fn folds_a_run_of_restyles_into_one_undo_step() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        let styled = |width| Shape::Step {
            at: point(1.0, 1.0),
            number: 1,
            stroke: Stroke { width, ..STROKE },
        };
        document.restyle(0, styled(4.0));
        document.restyle(0, styled(5.0));
        document.restyle(0, styled(6.0));
        assert_eq!(document.shapes(), &[styled(6.0)]);
        assert!(document.undo());
        assert_eq!(document.shapes(), &[step(1)]);
        assert!(document.redo());
        assert_eq!(document.shapes(), &[styled(6.0)]);
        document.restyle(0, styled(7.0));
        assert!(document.undo());
        assert_eq!(document.shapes(), &[styled(6.0)]);
    }

    #[test]
    fn keeps_a_restyle_after_saving_as_its_own_step() {
        let mut document = Document::default();
        document.apply(Edit::Add(step(1)));
        let styled = |width| Shape::Step {
            at: point(1.0, 1.0),
            number: 1,
            stroke: Stroke { width, ..STROKE },
        };
        document.restyle(0, styled(4.0));
        document.mark_saved();
        document.restyle(0, styled(5.0));
        assert!(document.is_modified());
        assert!(document.undo());
        assert_eq!(document.shapes(), &[styled(4.0)]);
        assert!(!document.is_modified());
    }

    #[test]
    fn counts_a_fresh_capture_as_unsaved() {
        let mut document = Document::unsaved();
        assert!(document.is_modified());
        document.mark_saved();
        assert!(!document.is_modified());
    }

    #[test]
    fn scales_stroke_width_with_the_image() {
        assert_eq!(Stroke::scaled(Color::Red, 3, 100, (800, 500)).width, 3.0);
        assert_eq!(Stroke::scaled(Color::Red, 3, 100, (1920, 1080)).width, 3.0);
        assert_eq!(Stroke::scaled(Color::Red, 3, 100, (2560, 1620)).width, 5.0);
        assert_eq!(Stroke::scaled(Color::Red, 3, 100, (3840, 2160)).width, 6.0);
        assert_eq!(Stroke::scaled(Color::Red, 3, 100, (7680, 4320)).width, 6.0);
    }

    #[test]
    fn clamps_stroke_size_and_opacity() {
        let thin = Stroke::scaled(Color::Red, 0, 0, (10, 10));
        assert_eq!(thin.width, 1.0);
        assert_eq!(thin.opacity, 0.1);
        let heavy = Stroke::scaled(Color::Red, 99, 400, (1920, 1080));
        assert_eq!(heavy.width, f64::from(MAX_SIZE_PX));
        assert_eq!(heavy.opacity, 1.0);
    }

    #[test]
    fn reads_the_toolbar_settings_back_from_a_stroke() {
        for image in [(1920, 1080), (2560, 1620), (3840, 2160)] {
            for size in [MIN_SIZE_PX, 3, 7, MAX_SIZE_PX] {
                let stroke = Stroke::scaled(Color::Red, size, 40, image);
                let (back, opacity) = stroke.settings(image);
                assert_eq!(opacity, 40);
                assert!(
                    back.abs_diff(size) <= 1,
                    "{size} came back as {back} on {image:?}"
                );
            }
        }
    }

    #[test]
    fn cycles_through_the_palette_both_ways() {
        assert_eq!(Color::Accent.shifted(1), Color::Red);
        assert_eq!(Color::Accent.shifted(-1), Color::Black);
        assert_eq!(Color::Black.shifted(1), Color::Accent);
        assert_eq!(Color::Red.shifted(Color::ALL.len() as i32), Color::Red);
    }

    #[test]
    fn numbers_the_palette_from_one() {
        assert_eq!(Color::numbered('1'), Some(Color::Accent));
        assert_eq!(Color::numbered('7'), Some(Color::Black));
        assert_eq!(Color::numbered('0'), None);
        assert_eq!(Color::numbered('8'), None);
        assert_eq!(Color::numbered('x'), None);
    }
}
