use cheese_core::capture::{backend_for_current_session, SessionKind};
use cheese_core::editor::EditorState;
use cheese_core::model::{Annotation, AnnotationKind, ScreenshotImage};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Default)]
struct CliOptions {
    output: Option<PathBuf>,
    copy: bool,
    annotate: bool,
    region: Option<(u32, u32, u32, u32)>,
    self_test: bool,
    preview: bool,
}

fn parse_args() -> CliOptions {
    let mut opts = CliOptions::default();
    let mut args = env::args().skip(1).peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" | "-o" => {
                if let Some(path) = args.next() {
                    opts.output = Some(PathBuf::from(path));
                }
            }
            "--copy" | "-c" => opts.copy = true,
            "--annotate" | "-a" => opts.annotate = true,
            "--self-test" => opts.self_test = true,
            "--preview" | "-p" => opts.preview = true,
            "--region" | "-r" => {
                if let Some(value) = args.next() {
                    opts.region = parse_region(&value);
                }
            }
            _ => {}
        }
    }

    opts
}

fn parse_region(value: &str) -> Option<(u32, u32, u32, u32)> {
    let (origin, size) = value.trim().split_once(' ')?;
    let (x, y) = origin.split_once(',')?;
    let (width, height) = size.split_once('x')?;

    Some((
        x.parse().ok()?,
        y.parse().ok()?,
        width.parse().ok()?,
        height.parse().ok()?,
    ))
}

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

fn capture_selection(session: SessionKind) -> (u32, u32, u32, u32) {
    if matches!(session, SessionKind::Wayland) {
        let output = Command::new("slurp")
            .output()
            .expect("slurp should be available on this machine");
        let geometry = String::from_utf8_lossy(&output.stdout);
        return parse_slurp_geometry(&geometry).unwrap_or((0, 0, 1280, 720));
    }

    (0, 0, 1280, 720)
}

fn capture_png(
    backend_name: &str,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    output_path: &PathBuf,
) -> Result<(), String> {
    if backend_name == "wayland" {
        let geometry = format!("{x},{y} {width}x{height}");
        let status = Command::new("grim")
            .args(["-g", &geometry])
            .arg(output_path)
            .status()
            .map_err(|err| format!("failed to run grim: {err}"))?;

        if !status.success() {
            return Err(format!("grim failed with status {status}"));
        }

        return Ok(());
    }

    Err(format!("unsupported backend: {backend_name}"))
}

fn flatten_with_magick(
    input_path: &PathBuf,
    output_path: &PathBuf,
    annotations: &[Annotation],
) -> Result<(), String> {
    let mut cmd = Command::new("magick");
    cmd.arg(input_path);

    for annotation in annotations {
        match annotation.kind {
            AnnotationKind::Rectangle | AnnotationKind::Highlight => {
                let color = if matches!(annotation.kind, AnnotationKind::Highlight) {
                    "rgba(255, 220, 0, 0.25)"
                } else {
                    "rgba(255, 64, 64, 0.85)"
                };
                cmd.args([
                    "-stroke",
                    color,
                    "-fill",
                    "none",
                    "-strokewidth",
                    &annotation.stroke_width.to_string(),
                    "-draw",
                    &format!(
                        "rectangle {},{} {},{}",
                        annotation.bounds.x,
                        annotation.bounds.y,
                        annotation.bounds.x + annotation.bounds.width,
                        annotation.bounds.y + annotation.bounds.height
                    ),
                ]);
            }
            AnnotationKind::Arrow => {
                cmd.args([
                    "-stroke",
                    "rgba(255, 64, 64, 0.95)",
                    "-fill",
                    "none",
                    "-strokewidth",
                    &annotation.stroke_width.to_string(),
                    "-draw",
                    &format!(
                        "line {},{} {},{}",
                        annotation.bounds.x,
                        annotation.bounds.y,
                        annotation.bounds.x + annotation.bounds.width,
                        annotation.bounds.y + annotation.bounds.height
                    ),
                ]);
            }
            AnnotationKind::Text => {
                let text = annotation.text.as_deref().unwrap_or("");
                cmd.args([
                    "-fill",
                    "rgba(255, 255, 255, 0.95)",
                    "-stroke",
                    "rgba(0, 0, 0, 0.8)",
                    "-strokewidth",
                    "1",
                    "-annotate",
                    &format!("+{},{}", annotation.bounds.x, annotation.bounds.y),
                    text,
                ]);
            }
            AnnotationKind::Pen => {}
        }
    }

    cmd.arg(output_path);

    let status = cmd
        .status()
        .map_err(|err| format!("failed to run magick: {err}"))?;

    if !status.success() {
        return Err(format!("magick failed with status {status}"));
    }

    Ok(())
}

fn write_demo_png(output_path: &PathBuf, width: u32, height: u32) -> Result<(), String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(format!("P6\n{} {}\n255\n", width, height).as_bytes());

    for y in 0..height {
        for x in 0..width {
            let is_checker = ((x / 20) + (y / 20)) % 2 == 0;
            let (r, g, b) = if is_checker { (80, 140, 220) } else { (30, 30, 40) };
            bytes.extend_from_slice(&[r, g, b]);
        }
    }

    fs::write(output_path, bytes).map_err(|err| format!("failed to write demo image: {err}"))
}

fn copy_png_to_clipboard(path: &PathBuf) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|err| format!("failed to read png: {err}"))?;
    let mut child = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run wl-copy: {err}"))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&bytes)
            .map_err(|err| format!("failed to write clipboard payload: {err}"))?;
    }

    let status = child
        .wait()
        .map_err(|err| format!("failed to wait for wl-copy: {err}"))?;

    if !status.success() {
        return Err(format!("wl-copy failed with status {status}"));
    }

    Ok(())
}

fn main() {
    let opts = parse_args();
    let backend = backend_for_current_session();
    let session = match cheese_core::capture::detect_session_kind() {
        SessionKind::Wayland => "wayland",
        SessionKind::X11 => "x11",
        SessionKind::Unknown => "unknown",
    };

    let (x, y, width, height) = opts
        .region
        .unwrap_or_else(|| capture_selection(cheese_core::capture::detect_session_kind()));
    if opts.self_test {
        let output_path = opts.output.unwrap_or_else(|| {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            env::temp_dir().join(format!("cheese-{stamp}.ppm"))
        });
        if let Err(err) = write_demo_png(&output_path, 320, 180) {
            eprintln!("self-test failed: {err}");
            std::process::exit(1);
        }
        println!("self-test written: {}", output_path.display());
        return;
    }

    let output_path = opts.output.unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        env::temp_dir().join(format!("cheese-{stamp}.png"))
    });
    let capture_path = output_path.with_extension("capture.png");

    if let Err(err) = capture_png(backend.name(), x, y, width, height, &output_path) {
        eprintln!("capture failed on {session}: {err}");
        std::process::exit(1);
    }

    let image = fs::read(&output_path)
        .ok()
        .map(|bytes| ScreenshotImage::from_bytes(width, height, bytes))
        .unwrap_or_else(|| ScreenshotImage::new(width, height));

    let mut editor = EditorState::with_image(image);
    if opts.annotate {
        editor.add_annotation(Annotation::rectangle(32, 32, 240, 120));
        editor.add_annotation(Annotation::arrow(80, 80, 180, 100));
        editor.add_annotation(Annotation::text(36, 170, 220, 40, "Cheese"));
    }

    if let Some(flattened) = editor.flattened() {
        if let Err(err) = fs::copy(&output_path, &capture_path) {
            eprintln!("failed to prepare capture copy: {err}");
            std::process::exit(1);
        }
        if let Err(err) = flatten_with_magick(&capture_path, &output_path, &editor.annotations) {
            eprintln!("annotation flatten failed: {err}");
            std::process::exit(1);
        }
        println!(
            "captured {}x{} on {} via {}",
            flattened.image.width,
            flattened.image.height,
            session,
            backend.name()
        );
    }

    println!("saved: {}", output_path.display());

    if opts.copy {
        if let Err(err) = copy_png_to_clipboard(&output_path) {
            eprintln!("clipboard copy failed: {err}");
            std::process::exit(1);
        }
        println!("copied to clipboard");
    }

    if opts.preview {
        let _ = Command::new("xdg-open").arg(&output_path).spawn();
    }
}
