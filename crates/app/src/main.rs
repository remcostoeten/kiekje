use cheese_core::capture::{backend_for_current_session, SessionKind};
use cheese_core::editor::EditorState;
use cheese_core::model::Annotation;

fn main() {
    let backend = backend_for_current_session();
    let session = match cheese_core::capture::detect_session_kind() {
        SessionKind::Wayland => "wayland",
        SessionKind::X11 => "x11",
        SessionKind::Unknown => "unknown",
    };

    let image = backend
        .capture_region(0, 0, 1280, 720)
        .unwrap_or_else(|_| cheese_core::model::ScreenshotImage::new(1280, 720));

    let mut editor = EditorState::with_image(image);
    editor.add_annotation(Annotation::rectangle(32, 32, 240, 120));

    println!(
        "Cheese scaffold ready: session={}, backend={}, image={}x{}, annotations={}",
        session,
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
