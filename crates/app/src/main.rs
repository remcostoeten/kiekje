use cheese_core::capture::{CaptureBackend, StubCaptureBackend};
use cheese_core::editor::EditorState;
use cheese_core::model::{Annotation, AnnotationKind};

fn main() {
    let backend = StubCaptureBackend::default();
    let image = backend
        .capture_region(0, 0, 1280, 720)
        .expect("stub capture should succeed");

    let mut editor = EditorState::with_image(image);
    editor.add_annotation(Annotation {
        kind: AnnotationKind::Rectangle,
        x: 32,
        y: 32,
        width: 240,
        height: 120,
        text: None,
    });

    println!(
        "Cheese scaffold ready: backend={}, image={}x{}, annotations={}",
        backend.name(),
        editor.image.as_ref().map(|image| image.width).unwrap_or_default(),
        editor.image.as_ref().map(|image| image.height).unwrap_or_default(),
        editor.annotations.len()
    );
}
