use cheese_core::capture::{CaptureBackend, StubCaptureBackend};
use cheese_core::editor::EditorState;
use cheese_core::model::Annotation;

fn main() {
    let backend = StubCaptureBackend::default();
    let image = backend
        .capture_region(0, 0, 1280, 720)
        .expect("stub capture should succeed");

    let mut editor = EditorState::with_image(image);
    editor.add_annotation(Annotation::rectangle(32, 32, 240, 120));

    println!(
        "Cheese scaffold ready: backend={}, image={}x{}, annotations={}",
        backend.name(),
        editor.image.as_ref().map(|image| image.width).unwrap_or_default(),
        editor.image.as_ref().map(|image| image.height).unwrap_or_default(),
        editor.annotations.len()
    );

    if let Some(flattened) = editor.flattened() {
        println!(
            "flattened image prepared: {}x{}, annotations={}",
            flattened.image.width,
            flattened.image.height,
            flattened.annotation_count
        );
    }
}
