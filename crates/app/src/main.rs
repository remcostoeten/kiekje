use cheese_core::capture::{backend_for_current_session, SessionKind};
use cheese_core::editor::EditorState;
use cheese_core::model::Annotation;
use std::process::Command;

fn parse_slurp_geometry(geometry: &str) -> Option<(u32, u32, u32, u32)> {
    let (origin, size) = geometry.trim().split_once(' ')?;
    let (x, y) = origin.split_once(',')?;
    let (width, height) = size.split_once('x')?;

    Some((
        x.parse().ok()?,
        y.parse().ok()?,
        width.parse().ok()?,
        height.parse().ok()?,
    ))
}

fn main() {
    let backend = backend_for_current_session();
    let session = match cheese_core::capture::detect_session_kind() {
        SessionKind::Wayland => "wayland",
        SessionKind::X11 => "x11",
        SessionKind::Unknown => "unknown",
    };

    let (x, y, width, height) = if matches!(session, "wayland") {
        let output = Command::new("slurp")
            .output()
            .expect("slurp should be available on this machine");
        let geometry = String::from_utf8_lossy(&output.stdout);
        parse_slurp_geometry(&geometry).unwrap_or((0, 0, 1280, 720))
    } else {
        (0, 0, 1280, 720)
    };

    let image = backend
        .capture_region(x, y, width, height)
        .unwrap_or_else(|err| {
            eprintln!("capture failed on {session}: {err:?}");
            cheese_core::model::ScreenshotImage::new(width, height)
        });

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
