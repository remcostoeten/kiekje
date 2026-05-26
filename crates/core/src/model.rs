#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationKind {
    Arrow,
    Rectangle,
    Text,
    Pen,
    Highlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: Option<String>,
}

impl ScreenshotImage {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = width.saturating_mul(height).saturating_mul(4);
        Self {
            width,
            height,
            pixels: vec![0; pixel_count as usize],
        }
    }
}

