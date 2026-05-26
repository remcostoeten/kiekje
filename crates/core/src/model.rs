#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotImage {
    pub width: u32,
    pub height: u32,
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
}

