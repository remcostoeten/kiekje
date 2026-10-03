use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::process::ExitCode;

use gtk::prelude::*;
use gtk::{gdk, glib};
use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
    self, ExtDataControlDeviceV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::{
    self, ExtDataControlOfferV1,
};

const PNG_MIME: &str = "image/png";
const BLUE: u8 = 128;
const GRID: u32 = 16;
const UNSUPPORTED: u8 = 3;

const USAGE: &str = "usage:
  lab_probe show                                        cover every monitor with a position pattern
  lab_probe check <png> <frames> <monitor|all> <rect>   judge a capture of that pattern
  lab_probe paste <out.png>                             write the image/png clipboard to a file

  <frames>  the daemon's frame lines as NAME=WxH@LWxLH+X+Y, joined by ';'
  <rect>    x0,y0,x1,y1 as fractions of the monitor, or of the whole desktop for 'all'";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["show"] => show(),
        ["check", png, frames, monitor, rect] => match check(png, frames, monitor, rect) {
            Ok(verdict) => {
                println!("{verdict}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                println!("fail {error}");
                ExitCode::FAILURE
            }
        },
        ["paste", out] => paste(out),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn channel(fraction: f64) -> u8 {
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn pattern_pixel(x: f64, y: f64) -> [u8; 3] {
    [channel(x), channel(y), BLUE]
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Area {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Area {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

fn bounds(areas: impl Iterator<Item = Area>) -> Option<Area> {
    areas.reduce(|a, b| {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Area {
            x,
            y,
            width: (a.x + a.width).max(b.x + b.width) - x,
            height: (a.y + a.height).max(b.y + b.height) - y,
        }
    })
}

fn monitor_area(monitor: &gdk::Monitor) -> Area {
    let geometry = monitor.geometry();
    Area {
        x: f64::from(geometry.x()),
        y: f64::from(geometry.y()),
        width: f64::from(geometry.width()),
        height: f64::from(geometry.height()),
    }
}

fn pattern_texture(area: Area, desktop: Area) -> gdk::MemoryTexture {
    let size = 1024;
    let mut pixels = Vec::with_capacity(size * size * 3);
    for row in 0..size {
        for column in 0..size {
            let x = area.x + (column as f64 + 0.5) / size as f64 * area.width;
            let y = area.y + (row as f64 + 0.5) / size as f64 * area.height;
            pixels.extend_from_slice(&pattern_pixel(
                (x - desktop.x) / desktop.width,
                (y - desktop.y) / desktop.height,
            ));
        }
    }
    gdk::MemoryTexture::new(
        size as i32,
        size as i32,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from_owned(pixels),
        size * 3,
    )
}

fn show() -> ExitCode {
    if let Err(error) = gtk::init() {
        eprintln!("lab_probe: gtk did not start: {error}");
        return ExitCode::FAILURE;
    }
    let Some(display) = gdk::Display::default() else {
        eprintln!("lab_probe: no display");
        return ExitCode::FAILURE;
    };
    let monitors: Vec<gdk::Monitor> = display
        .monitors()
        .iter::<gdk::Monitor>()
        .filter_map(Result::ok)
        .collect();
    let Some(desktop) = bounds(monitors.iter().map(monitor_area)) else {
        eprintln!("lab_probe: no monitors");
        return ExitCode::FAILURE;
    };
    let main_loop = glib::MainLoop::new(None, false);
    let waiting = std::rc::Rc::new(std::cell::Cell::new(monitors.len()));
    for monitor in &monitors {
        let area = monitor_area(monitor);
        println!(
            "lab_probe: monitor {} {}x{}{:+}{:+} scale {}",
            monitor.connector().unwrap_or_default(),
            area.width,
            area.height,
            area.x,
            area.y,
            monitor.scale(),
        );
        let picture = gtk::Picture::for_paintable(&pattern_texture(area, desktop));
        picture.set_content_fit(gtk::ContentFit::Fill);
        let window = gtk::Window::builder()
            .title("kiekje lab pattern")
            .decorated(false)
            .child(&picture)
            .build();
        window.connect_close_request({
            let main_loop = main_loop.clone();
            move |_| {
                main_loop.quit();
                glib::Propagation::Proceed
            }
        });
        let frames = std::cell::Cell::new(0);
        let waiting = waiting.clone();
        window.add_tick_callback(move |_, _| {
            frames.set(frames.get() + 1);
            if frames.get() < 10 {
                return glib::ControlFlow::Continue;
            }
            waiting.set(waiting.get() - 1);
            if waiting.get() == 0 {
                println!("lab_probe: pattern shown");
            }
            glib::ControlFlow::Break
        });
        window.fullscreen_on_monitor(monitor);
        window.present();
    }
    main_loop.run();
    ExitCode::SUCCESS
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    const FULL: Rect = Rect {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };
}

#[derive(Clone, Debug, PartialEq)]
struct FrameInfo {
    name: String,
    width: u32,
    height: u32,
    logical: Area,
}

struct Capture {
    width: u32,
    height: u32,
    rgb: Vec<[u8; 3]>,
}

fn parse_size(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.split_once('x')?;
    let width = width.parse::<u32>().ok().filter(|size| *size > 0)?;
    let height = height.parse::<u32>().ok().filter(|size| *size > 0)?;
    Some((width, height))
}

fn parse_frame(value: &str) -> Option<FrameInfo> {
    let (name, rest) = value.split_once('=')?;
    let (frame, logical) = rest.split_once('@')?;
    let (width, height) = parse_size(frame)?;
    let offsets = logical.find(['+', '-'])?;
    let (logical_width, logical_height) = parse_size(&logical[..offsets])?;
    let offsets = &logical[offsets..];
    let second = offsets[1..].find(['+', '-'])? + 1;
    let x = offsets[..second].parse::<i32>().ok()?;
    let y = offsets[second..].parse::<i32>().ok()?;
    Some(FrameInfo {
        name: name.to_string(),
        width,
        height,
        logical: Area {
            x: f64::from(x),
            y: f64::from(y),
            width: f64::from(logical_width),
            height: f64::from(logical_height),
        },
    })
}

fn parse_frames(value: &str) -> Result<Vec<FrameInfo>, String> {
    let frames: Option<Vec<FrameInfo>> = value
        .split(';')
        .filter(|part| !part.is_empty())
        .map(parse_frame)
        .collect();
    match frames {
        Some(frames) if !frames.is_empty() => Ok(frames),
        _ => Err(format!("frames {value} are not NAME=WxH@LWxLH+X+Y;...")),
    }
}

fn parse_rect(value: &str) -> Result<Rect, String> {
    let parts: Vec<f64> = value
        .split(',')
        .map(|part| part.parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("rect {value} is not x0,y0,x1,y1"))?;
    match parts.as_slice() {
        [x0, y0, x1, y1] if x0 < x1 && y0 < y1 => Ok(Rect {
            x0: *x0,
            y0: *y0,
            x1: *x1,
            y1: *y1,
        }),
        _ => Err(format!("rect {value} is not x0,y0,x1,y1")),
    }
}

fn read_png(path: &str) -> Result<Capture, String> {
    let file = File::open(path).map_err(|error| format!("cannot open {path}: {error}"))?;
    let mut decoder = png::Decoder::new(io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("{path} is not a png: {error}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| format!("{path} is too large"))?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("{path} does not decode: {error}"))?;
    let step = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        other => return Err(format!("{path} has unexpected colour type {other:?}")),
    };
    let rgb = buffer[..info.buffer_size()]
        .chunks_exact(step)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    Ok(Capture {
        width: info.width,
        height: info.height,
        rgb,
    })
}

fn check(path: &str, frames: &str, monitor: &str, rect: &str) -> Result<String, String> {
    let capture = read_png(path)?;
    let frames = parse_frames(frames)?;
    let rect = parse_rect(rect)?;
    let expectation = if monitor == "all" {
        Expectation::desktop(&frames, rect)
    } else {
        let frame = frames
            .iter()
            .find(|frame| frame.name == monitor)
            .ok_or_else(|| format!("no frame for monitor {monitor}"))?;
        Expectation::monitor(&frames, frame, rect)
    };
    judge(&capture, &expectation)
}

struct Expectation {
    width: f64,
    height: f64,
    size_slack: (f64, f64),
    position_slack: f64,
    desktop: Area,
    covered: Vec<Area>,
    source: Area,
}

impl Expectation {
    fn desktop(frames: &[FrameInfo], rect: Rect) -> Self {
        let desktop = bounds(frames.iter().map(|frame| frame.logical)).unwrap_or(Area {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        let ratio = frames
            .iter()
            .map(|frame| f64::from(frame.width) / frame.logical.width.max(1.0))
            .fold(1.0_f64, f64::max);
        let full_width = (desktop.width * ratio).round();
        let full_height = (desktop.height * ratio).round();
        Self {
            width: (rect.x1 - rect.x0) * full_width,
            height: (rect.y1 - rect.y0) * full_height,
            size_slack: if rect == Rect::FULL {
                (0.0, 0.0)
            } else {
                (0.02 * full_width, 0.02 * full_height)
            },
            position_slack: 0.01,
            desktop,
            covered: frames.iter().map(|frame| frame.logical).collect(),
            source: Area {
                x: desktop.x + rect.x0 * desktop.width,
                y: desktop.y + rect.y0 * desktop.height,
                width: (rect.x1 - rect.x0) * desktop.width,
                height: (rect.y1 - rect.y0) * desktop.height,
            },
        }
    }

    fn monitor(frames: &[FrameInfo], frame: &FrameInfo, rect: Rect) -> Self {
        let full = rect == Rect::FULL;
        let desktop = bounds(frames.iter().map(|frame| frame.logical)).unwrap_or(frame.logical);
        let monitor = frame.logical;
        Self {
            width: (rect.x1 - rect.x0) * f64::from(frame.width),
            height: (rect.y1 - rect.y0) * f64::from(frame.height),
            size_slack: if full {
                (0.0, 0.0)
            } else {
                (
                    0.02 * f64::from(frame.width),
                    0.02 * f64::from(frame.height),
                )
            },
            position_slack: if full { 0.01 } else { 0.02 } * monitor.width.max(monitor.height)
                / desktop.width.max(desktop.height),
            desktop,
            covered: vec![monitor],
            source: Area {
                x: monitor.x + rect.x0 * monitor.width,
                y: monitor.y + rect.y0 * monitor.height,
                width: (rect.x1 - rect.x0) * monitor.width,
                height: (rect.y1 - rect.y0) * monitor.height,
            },
        }
    }
}

fn judge(capture: &Capture, expected: &Expectation) -> Result<String, String> {
    let size = format!("{}x{}", capture.width, capture.height);
    if (f64::from(capture.width) - expected.width).abs() > expected.size_slack.0 + 0.5
        || (f64::from(capture.height) - expected.height).abs() > expected.size_slack.1 + 0.5
    {
        return Err(format!(
            "size {size}, expected {:.0}x{:.0}",
            expected.width, expected.height
        ));
    }

    let desktop = expected.desktop;
    let mut good = 0;
    let mut total = 0;
    let mut wrong_blue = 0;
    let mut worst = (0.0, String::new());
    for row in 0..GRID {
        for column in 0..GRID {
            let u = (f64::from(column) + 0.5) / f64::from(GRID);
            let v = (f64::from(row) + 0.5) / f64::from(GRID);
            let logical_x = expected.source.x + u * expected.source.width;
            let logical_y = expected.source.y + v * expected.source.height;
            if !expected
                .covered
                .iter()
                .any(|area| area.contains(logical_x, logical_y))
            {
                continue;
            }
            total += 1;
            let x = ((u * f64::from(capture.width)) as u32).min(capture.width - 1);
            let y = ((v * f64::from(capture.height)) as u32).min(capture.height - 1);
            let [red, green, blue] = capture.rgb[(y * capture.width + x) as usize];
            let want_x = (logical_x - desktop.x) / desktop.width;
            let want_y = (logical_y - desktop.y) / desktop.height;
            let got_x = f64::from(red) / 255.0;
            let got_y = f64::from(green) / 255.0;
            let error = (got_x - want_x).abs().max((got_y - want_y).abs());
            let blue_ok = blue.abs_diff(BLUE) <= 12;
            if !blue_ok {
                wrong_blue += 1;
            }
            if error <= expected.position_slack.max(1.5 / 255.0) && blue_ok {
                good += 1;
            } else if error > worst.0 || worst.1.is_empty() {
                worst = (
                    error,
                    format!(
                        "pixel {x},{y} is rgb {red},{green},{blue}, decodes to {got_x:.3},{got_y:.3}, expected {want_x:.3},{want_y:.3}"
                    ),
                );
            }
        }
    }
    if total > 0 && good * 10 >= total * 9 {
        return Ok(format!("pass {size}, {good}/{total} samples"));
    }
    let hint = if wrong_blue * 2 > total {
        "; blue is not constant, so red and blue are likely swapped"
    } else {
        ""
    };
    Err(format!(
        "{size}, {good}/{total} samples; worst {}{hint}",
        worst.1
    ))
}

#[derive(Default)]
struct Paste {
    mimes: HashMap<ObjectId, Vec<String>>,
    selection: Option<Option<ExtDataControlOfferV1>>,
}

fn paste(out: &str) -> ExitCode {
    match read_clipboard() {
        Ok(Some(png)) => match fs::write(out, &png) {
            Ok(()) => {
                println!("lab_probe: wrote {} clipboard bytes to {out}", png.len());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("lab_probe: cannot write {out}: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(None) => {
            eprintln!("lab_probe: the clipboard has no {PNG_MIME}");
            ExitCode::FAILURE
        }
        Err(PasteError::Unsupported) => {
            eprintln!("lab_probe: the compositor has no ext_data_control_manager_v1");
            ExitCode::from(UNSUPPORTED)
        }
        Err(PasteError::Failed(error)) => {
            eprintln!("lab_probe: reading the clipboard failed: {error}");
            ExitCode::FAILURE
        }
    }
}

enum PasteError {
    Unsupported,
    Failed(String),
}

fn failed(error: impl std::fmt::Display) -> PasteError {
    PasteError::Failed(error.to_string())
}

fn read_clipboard() -> Result<Option<Vec<u8>>, PasteError> {
    let connection = Connection::connect_to_env().map_err(failed)?;
    let (globals, mut queue) = registry_queue_init::<Paste>(&connection).map_err(failed)?;
    let handle = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&handle, 1..=1, ()).map_err(failed)?;
    let manager: ExtDataControlManagerV1 = globals
        .bind(&handle, 1..=1, ())
        .map_err(|_| PasteError::Unsupported)?;
    let device = manager.get_data_device(&seat, &handle, ());
    let mut state = Paste::default();
    while state.selection.is_none() {
        queue.blocking_dispatch(&mut state).map_err(failed)?;
    }
    let Some(Some(offer)) = state.selection.take() else {
        return Ok(None);
    };
    let offers_png = state
        .mimes
        .get(&offer.id())
        .is_some_and(|mimes| mimes.iter().any(|mime| mime == PNG_MIME));
    if !offers_png {
        return Ok(None);
    }
    let (mut reader, writer) = io::pipe().map_err(failed)?;
    offer.receive(PNG_MIME.into(), writer.as_fd());
    connection.flush().map_err(failed)?;
    drop(writer);
    let mut png = Vec::new();
    reader.read_to_end(&mut png).map_err(failed)?;
    offer.destroy();
    device.destroy();
    connection.flush().map_err(failed)?;
    Ok(Some(png))
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Paste {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Paste {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for Paste {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for Paste {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state.mimes.insert(id.id(), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                if state.selection.is_none() {
                    state.selection = Some(id);
                }
            }
            ext_data_control_device_v1::Event::Finished => state.selection = Some(None),
            _ => {}
        }
    }

    event_created_child!(Paste, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Paste {
    fn event(
        state: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.mimes.entry(offer.id()).or_default().push(mime_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: &str) -> FrameInfo {
        parse_frame(value).unwrap()
    }

    fn render(desktop: Area, source: Area, size: (u32, u32)) -> Capture {
        let mut rgb = Vec::new();
        for y in 0..size.1 {
            for x in 0..size.0 {
                let logical_x = source.x + (f64::from(x) + 0.5) / f64::from(size.0) * source.width;
                let logical_y = source.y + (f64::from(y) + 0.5) / f64::from(size.1) * source.height;
                rgb.push(pattern_pixel(
                    (logical_x - desktop.x) / desktop.width,
                    (logical_y - desktop.y) / desktop.height,
                ));
            }
        }
        Capture {
            width: size.0,
            height: size.1,
            rgb,
        }
    }

    fn area(x: f64, y: f64, width: f64, height: f64) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    const QUARTER: Rect = Rect {
        x0: 0.25,
        y0: 0.25,
        x1: 0.75,
        y1: 0.75,
    };

    fn single() -> Vec<FrameInfo> {
        vec![frame("Virtual-1=640x400@640x400+0+0")]
    }

    fn mixed() -> Vec<FrameInfo> {
        vec![
            frame("Virtual-1=1280x800@1280x800+0+0"),
            frame("HEADLESS-1=1920x1200@1280x800-1280+0"),
        ]
    }

    #[test]
    fn parses_a_frame_with_a_negative_offset() {
        assert_eq!(
            frame("HEADLESS-1=1920x1200@1280x800-1280+0"),
            FrameInfo {
                name: "HEADLESS-1".into(),
                width: 1920,
                height: 1200,
                logical: area(-1280.0, 0.0, 1280.0, 800.0),
            }
        );
    }

    #[test]
    fn passes_an_exact_fullscreen_capture() {
        let screen = area(0.0, 0.0, 640.0, 400.0);
        let capture = render(screen, screen, (640, 400));
        assert!(judge(&capture, &Expectation::desktop(&single(), Rect::FULL)).is_ok());
    }

    #[test]
    fn passes_a_centered_region_one_pixel_off() {
        let frames = single();
        let screen = area(0.0, 0.0, 640.0, 400.0);
        let capture = render(screen, area(160.0, 100.0, 321.0, 201.0), (321, 201));
        assert!(
            judge(
                &capture,
                &Expectation::monitor(&frames, &frames[0], QUARTER)
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_swapped_red_and_blue() {
        let screen = area(0.0, 0.0, 640.0, 400.0);
        let mut capture = render(screen, screen, (640, 400));
        for pixel in &mut capture.rgb {
            pixel.swap(0, 2);
        }
        let error = judge(&capture, &Expectation::desktop(&single(), Rect::FULL)).unwrap_err();
        assert!(error.contains("red and blue"), "{error}");
    }

    #[test]
    fn rejects_a_horizontally_mirrored_frame() {
        let screen = area(0.0, 0.0, 640.0, 400.0);
        let mut capture = render(screen, screen, (640, 400));
        for row in capture.rgb.as_chunks_mut::<640>().0 {
            row.reverse();
        }
        assert!(judge(&capture, &Expectation::desktop(&single(), Rect::FULL)).is_err());
    }

    #[test]
    fn rejects_a_region_taken_from_the_wrong_place() {
        let frames = single();
        let screen = area(0.0, 0.0, 640.0, 400.0);
        let capture = render(screen, area(0.0, 0.0, 320.0, 200.0), (320, 200));
        assert!(
            judge(
                &capture,
                &Expectation::monitor(&frames, &frames[0], QUARTER)
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_a_logical_size_fullscreen_on_a_scaled_monitor() {
        let frames = vec![frame("Virtual-1=640x400@427x267+0+0")];
        let screen = area(0.0, 0.0, 427.0, 267.0);
        let capture = render(screen, screen, (427, 267));
        assert!(judge(&capture, &Expectation::desktop(&frames, Rect::FULL)).is_err());
    }

    #[test]
    fn passes_a_mixed_scale_desktop_with_a_negative_origin() {
        let desktop = area(-1280.0, 0.0, 2560.0, 800.0);
        let capture = render(desktop, desktop, (3840, 1200));
        let verdict = judge(&capture, &Expectation::desktop(&mixed(), Rect::FULL));
        assert!(verdict.is_ok(), "{verdict:?}");
    }

    #[test]
    fn skips_samples_outside_every_monitor() {
        let frames = vec![
            frame("A=800x600@800x600+0+0"),
            frame("B=1920x1080@1920x1080+800+0"),
        ];
        let desktop = area(0.0, 0.0, 2720.0, 1080.0);
        let mut capture = render(desktop, desktop, (2720, 1080));
        for (index, pixel) in capture.rgb.iter_mut().enumerate() {
            if index % 2720 < 800 && index / 2720 >= 600 {
                *pixel = [0, 0, 0];
            }
        }
        let verdict = judge(&capture, &Expectation::desktop(&frames, Rect::FULL));
        assert!(verdict.is_ok(), "{verdict:?}");
    }

    #[test]
    fn passes_a_region_on_the_scaled_monitor() {
        let frames = mixed();
        let desktop = area(-1280.0, 0.0, 2560.0, 800.0);
        let capture = render(desktop, area(-960.0, 200.0, 640.0, 400.0), (960, 600));
        let verdict = judge(
            &capture,
            &Expectation::monitor(&frames, &frames[1], QUARTER),
        );
        assert!(verdict.is_ok(), "{verdict:?}");
    }

    #[test]
    fn rejects_a_region_cropped_from_the_other_monitor() {
        let frames = mixed();
        let desktop = area(-1280.0, 0.0, 2560.0, 800.0);
        let capture = render(desktop, area(320.0, 200.0, 640.0, 400.0), (640, 400));
        assert!(
            judge(
                &capture,
                &Expectation::monitor(&frames, &frames[1], QUARTER)
            )
            .is_err()
        );
    }
}
