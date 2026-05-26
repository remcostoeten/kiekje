#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotImage {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationKind {
    Arrow,
    Rectangle,
    Text,
    Pen,
    Highlight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub bounds: Bounds,
    pub text: Option<String>,
    pub stroke_width: u32,
}

impl ScreenshotImage {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = width.saturating_mul(height).saturating_mul(4);
        Self {
            width,
            height,
            bytes: vec![0; pixel_count as usize],
        }
    }

    pub fn from_bytes(width: u32, height: u32, bytes: Vec<u8>) -> Self {
        Self {
            width,
            height,
            bytes,
        }
    }

    pub fn blank_like(&self) -> Self {
        Self::new(self.width, self.height)
    }
}

impl Annotation {
    pub fn rectangle(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            kind: AnnotationKind::Rectangle,
            bounds: Bounds::new(x, y, width, height),
            text: None,
            stroke_width: 2,
        }
    }

    pub fn arrow(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            kind: AnnotationKind::Arrow,
            bounds: Bounds::new(x, y, width, height),
            text: None,
            stroke_width: 2,
        }
    }

    pub fn text(x: i32, y: i32, width: i32, height: i32, text: impl Into<String>) -> Self {
        Self {
            kind: AnnotationKind::Text,
            bounds: Bounds::new(x, y, width, height),
            text: Some(text.into()),
            stroke_width: 2,
        }
    }
}
