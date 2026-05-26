use crate::model::{Annotation, ScreenshotImage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlattenedImage {
    pub image: ScreenshotImage,
    pub annotation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    pub image: Option<ScreenshotImage>,
    pub annotations: Vec<Annotation>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            image: None,
            annotations: Vec::new(),
        }
    }

    pub fn with_image(image: ScreenshotImage) -> Self {
        Self {
            image: Some(image),
            annotations: Vec::new(),
        }
    }

    pub fn set_image(&mut self, image: ScreenshotImage) {
        self.image = Some(image);
    }

    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    pub fn flattened(&self) -> Option<FlattenedImage> {
        let image = self.image.as_ref()?.blank_like();
        Some(FlattenedImage {
            image,
            annotation_count: self.annotations.len(),
        })
    }
}
