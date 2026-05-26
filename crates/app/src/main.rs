use cheese_core::capture::{backend_for_current_session, SessionKind};
use cheese_core::editor::EditorState;
use cheese_core::model::{Annotation, ScreenshotImage};
use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

#[derive(Debug, Default)]
struct CliOptions {
    output: Option<PathBuf>,
    copy: bool,
    annotate: bool,
    region: Option<(u32, u32, u32, u32)>,
    self_test: bool,
    preview: bool,
    edit: bool,
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
            "--edit" | "-e" => opts.edit = true,
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
            .args(["-b", "000000AA", "-s", "ff550066", "-c", "ffffffcc", "-B", "111111cc"])
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

fn editor_html(image_url: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Cheese Preview</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #0d1117;
      --panel: rgba(17, 24, 39, 0.92);
      --line: rgba(255,255,255,0.14);
      --accent: #ff5a5f;
      --text: #e5e7eb;
    }}
    html, body {{ margin: 0; height: 100%; background: radial-gradient(circle at top, #1f2937 0, #0b1020 55%, #06070a 100%); color: var(--text); font-family: system-ui, sans-serif; }}
    body {{ display: grid; grid-template-rows: auto 1fr; }}
    .toolbar {{ display: flex; gap: 8px; padding: 12px; background: var(--panel); border-bottom: 1px solid var(--line); align-items: center; flex-wrap: wrap; }}
    .toolbar button, .toolbar a {{ background: #111827; color: var(--text); border: 1px solid var(--line); padding: 8px 12px; border-radius: 10px; cursor: pointer; text-decoration: none; }}
    .toolbar button.active {{ border-color: var(--accent); box-shadow: 0 0 0 1px rgba(255,90,95,0.25) inset; }}
    .toolbar .spacer {{ flex: 1; }}
    .canvas-wrap {{ display: grid; place-items: center; padding: 18px; overflow: auto; }}
    canvas {{ background: #0b0f14; box-shadow: 0 18px 60px rgba(0,0,0,0.5); max-width: 100%; height: auto; }}
    .hint {{ opacity: 0.7; font-size: 13px; }}
  </style>
</head>
<body>
  <div class="toolbar">
    <button id="capture">Capture region</button>
    <button data-tool="select" class="active">Select</button>
    <button data-tool="rect">Rect</button>
    <button data-tool="arrow">Arrow</button>
    <button data-tool="pen">Pen</button>
    <button data-tool="text">Text</button>
    <button id="undo">Undo</button>
    <button id="save">Save PNG</button>
    <div class="spacer"></div>
    <div class="hint">Drag to place annotations. Press Esc to clear tool. Shift keeps arrows straighter.</div>
  </div>
  <div class="canvas-wrap">
    <canvas id="c"></canvas>
  </div>
  <script>
    const imageUrl = {image_url:?};
    const canvas = document.getElementById('c');
    const ctx = canvas.getContext('2d');
    const img = new Image();
    img.src = imageUrl;
    let tool = 'select';
    const annotations = [];
    let current = null;
    let dragStart = null;
    let penPoints = [];
    let captureMode = true;

    function resize() {{
      canvas.width = img.naturalWidth || 1280;
      canvas.height = img.naturalHeight || 720;
      redraw();
    }}

    function redraw() {{
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      if (img.complete && img.naturalWidth) ctx.drawImage(img, 0, 0);
      for (const a of annotations) drawAnnotation(a);
      if (current) drawAnnotation(current, true);
    }}

    function drawAnnotation(a, preview=false) {{
      ctx.lineWidth = 3;
      ctx.strokeStyle = preview ? 'rgba(255, 215, 0, 0.95)' : 'rgba(255, 90, 95, 0.95)';
      ctx.fillStyle = preview ? 'rgba(255, 215, 0, 0.15)' : 'rgba(255, 90, 95, 0.15)';
      const x = a.x, y = a.y, w = a.w, h = a.h;
      if (a.kind === 'rect') {{
        ctx.strokeRect(x, y, w, h);
      }} else if (a.kind === 'arrow') {{
        ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + w, y + h); ctx.stroke();
        ctx.beginPath(); ctx.arc(x + w, y + h, 5, 0, Math.PI * 2); ctx.fill();
      }} else if (a.kind === 'pen') {{
        if (a.points.length > 1) {{
          ctx.beginPath();
          for (let i = 0; i < a.points.length; i++) {{
            const p = a.points[i];
            if (i === 0) ctx.moveTo(p.x, p.y); else ctx.lineTo(p.x, p.y);
          }}
          ctx.stroke();
        }}
      }} else if (a.kind === 'text') {{
        ctx.font = '20px system-ui';
        ctx.fillStyle = 'rgba(255,255,255,0.95)';
        ctx.fillText(a.text || 'Text', x, y + 20);
      }}
    }}

    function pointerPos(evt) {{
      const rect = canvas.getBoundingClientRect();
      const sx = canvas.width / rect.width;
      const sy = canvas.height / rect.height;
      return {{ x: (evt.clientX - rect.left) * sx, y: (evt.clientY - rect.top) * sy }};
    }}

    async function startCapture() {{
      const res = await fetch('/capture', {{ method: 'POST' }});
      if (!res.ok) {{
        alert('Capture failed');
        return;
      }}
      captureMode = false;
      img.src = imageUrl + '?v=' + Date.now();
      document.getElementById('capture').textContent = 'Re-capture';
    }}

    document.getElementById('capture').onclick = startCapture;

    canvas.addEventListener('pointerdown', evt => {{
      if (captureMode) return;
      if (tool === 'select') return;
      dragStart = pointerPos(evt);
      if (tool === 'pen') {{
        penPoints = [dragStart];
        current = {{ kind: 'pen', points: penPoints }};
      }} else {{
        current = {{ kind: tool, x: dragStart.x, y: dragStart.y, w: 0, h: 0, text: tool === 'text' ? prompt('Text label:') || '' : '' }};
      }}
      canvas.setPointerCapture(evt.pointerId);
    }});
    canvas.addEventListener('pointermove', evt => {{
      if (captureMode) return;
      if (!dragStart || !current || tool === 'text') return;
      const p = pointerPos(evt);
      if (tool === 'pen') {{
        penPoints.push(p);
      }} else {{
        current.w = p.x - dragStart.x;
        current.h = p.y - dragStart.y;
      }}
      redraw();
    }});
    canvas.addEventListener('pointerup', () => {{
      if (captureMode) return;
      if (!current) return;
      annotations.push(current);
      current = null;
      dragStart = null;
      redraw();
    }});
    document.querySelectorAll('[data-tool]').forEach(btn => {{
      btn.addEventListener('click', () => {{
        tool = btn.dataset.tool;
        document.querySelectorAll('[data-tool]').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
      }});
    }});
    document.getElementById('undo').onclick = () => {{ annotations.pop(); redraw(); }};
    document.getElementById('save').onclick = async () => {{
      const blob = await new Promise(resolve => canvas.toBlob(resolve, 'image/png'));
      await fetch('/save', {{ method: 'POST', headers: {{ 'Content-Type': 'image/png' }}, body: blob }});
      alert('Saved to the output file.');
    }};
    window.addEventListener('keydown', evt => {{
      if (evt.key === 'Escape') {{
        tool = 'select';
        document.querySelectorAll('[data-tool]').forEach(b => b.classList.remove('active'));
        document.querySelector('[data-tool="select"]').classList.add('active');
      }}
    }});
    img.onload = () => {{ resize(); }};
    img.onerror = () => {{ resize(); }};
    redraw();
  </script>
</body>
</html>"#,
        image_url = image_url
    )
}

fn handle_connection(
    mut stream: TcpStream,
    image_path: Arc<PathBuf>,
    output_path: Arc<PathBuf>,
    done: mpsc::Sender<()>,
    backend_name: &'static str,
    selection: (u32, u32, u32, u32),
) {
    let mut reader = match stream.try_clone() {
        Ok(clone) => BufReader::new(clone),
        Err(_) => return,
    };
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() {
        return;
    }

    let mut content_length = 0usize;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err() {
            return;
        }
        if header_line == "\r\n" {
            break;
        }
        let lower = header_line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0; content_length];
    if content_length > 0 && reader.read_exact(&mut body).is_err() {
        return;
    }

    if first_line.starts_with("GET / ") {
        let html = editor_html("/image.png");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(response.as_bytes());
    } else if first_line.starts_with("GET /image.png ") {
        match fs::read(&*image_path) {
            Ok(bytes) => {
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
            }
            Err(err) => {
                let msg = format!("image missing: {err}");
                let response = format!(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    msg.len(),
                    msg
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }
    } else if first_line.starts_with("POST /capture ") {
        let (x, y, width, height) = selection;
        if capture_png(backend_name, x, y, width, height, &image_path).is_ok() {
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
            let _ = stream.write_all(response.as_bytes());
            return;
        }
        let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 7\r\nConnection: close\r\n\r\nFailed";
        let _ = stream.write_all(response.as_bytes());
    } else if first_line.starts_with("POST /save ") {
        if fs::write(&*output_path, &body).is_ok() {
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 6\r\nConnection: close\r\n\r\nSaved!";
            let _ = stream.write_all(response.as_bytes());
            let _ = done.send(());
            return;
        }
        let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nError";
        let _ = stream.write_all(response.as_bytes());
    } else {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot found";
        let _ = stream.write_all(response.as_bytes());
    }
}

fn start_preview_server(
    image_path: PathBuf,
    output_path: PathBuf,
    backend_name: &'static str,
    selection: (u32, u32, u32, u32),
) -> Result<(u16, mpsc::Receiver<()>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| format!("failed to bind preview server: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("failed to read preview server address: {err}"))?
        .port();
    let (done_tx, done_rx) = mpsc::channel();
    let image_path = Arc::new(image_path);
    let output_path = Arc::new(output_path);
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(
                    stream,
                    Arc::clone(&image_path),
                    Arc::clone(&output_path),
                    done_tx.clone(),
                    backend_name,
                    selection,
                ),
                Err(_) => break,
            }
        }
    });
    Ok((port, done_rx))
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

    if opts.edit || opts.preview {
        match start_preview_server(
            output_path.clone(),
            output_path.clone(),
            backend.name(),
            (x, y, width, height),
        ) {
            Ok((port, done_rx)) => {
                let url = format!("http://127.0.0.1:{port}/");
                let _ = Command::new("xdg-open").arg(&url).spawn();
                println!("preview: {url}");
                let _ = done_rx.recv();
            }
            Err(err) => {
                eprintln!("failed to create preview editor: {err}");
                std::process::exit(1);
            }
        }
        println!("saved: {}", output_path.display());
        if opts.copy {
            if let Err(err) = copy_png_to_clipboard(&output_path) {
                eprintln!("clipboard copy failed: {err}");
                std::process::exit(1);
            }
            println!("copied to clipboard");
        }
        return;
    }

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
        let _ = flattened;
    }
    println!("saved: {}", output_path.display());
    if opts.copy {
        if let Err(err) = copy_png_to_clipboard(&output_path) {
            eprintln!("clipboard copy failed: {err}");
            std::process::exit(1);
        }
        println!("copied to clipboard");
    }

}
