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
  lab_probe show                                   open a fullscreen position pattern
  lab_probe check <png> <frame WxH> <x0,y0,x1,y1>  judge a capture of that pattern
  lab_probe paste <out.png>                        write the image/png clipboard to a file";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["show"] => show(),
        ["check", png, frame, rect] => match check(png, frame, rect) {
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

fn pattern_pixel(x: u32, y: u32, width: u32, height: u32) -> [u8; 3] {
    let channel = |position: u32, size: u32| (position * 255 + (size - 1) / 2) / (size - 1).max(1);
    [channel(x, width) as u8, channel(y, height) as u8, BLUE]
}

fn show() -> ExitCode {
    if let Err(error) = gtk::init() {
        eprintln!("lab_probe: gtk did not start: {error}");
        return ExitCode::FAILURE;
    }
    let (width, height) = (2048, 2048);
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&pattern_pixel(x, y, width, height));
        }
    }
    let texture = gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from_owned(pixels),
        (width * 3) as usize,
    );
    let picture = gtk::Picture::for_paintable(&texture);
    picture.set_content_fit(gtk::ContentFit::Fill);
    let window = gtk::Window::builder()
        .title("kiekje lab pattern")
        .decorated(false)
        .child(&picture)
        .build();
    let main_loop = glib::MainLoop::new(None, false);
    window.connect_close_request({
        let main_loop = main_loop.clone();
        move |_| {
            main_loop.quit();
            glib::Propagation::Proceed
        }
    });
    let frames = std::cell::Cell::new(0);
    window.add_tick_callback(move |_, _| {
        frames.set(frames.get() + 1);
        if frames.get() == 10 {
            println!("lab_probe: pattern shown");
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
    window.fullscreen();
    window.present();
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

struct Capture {
    width: u32,
    height: u32,
    rgb: Vec<[u8; 3]>,
}

fn parse_size(value: &str) -> Result<(u32, u32), String> {
    let (width, height) = value
        .split_once('x')
        .ok_or_else(|| format!("frame size {value} is not WxH"))?;
    let parse = |part: &str| {
        part.parse::<u32>()
            .ok()
            .filter(|size| *size > 0)
            .ok_or_else(|| format!("frame size {value} is not WxH"))
    };
    Ok((parse(width)?, parse(height)?))
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

fn check(path: &str, frame: &str, rect: &str) -> Result<String, String> {
    let capture = read_png(path)?;
    judge(&capture, parse_size(frame)?, parse_rect(rect)?)
}

fn judge(capture: &Capture, frame: (u32, u32), rect: Rect) -> Result<String, String> {
    let expected_width = (rect.x1 - rect.x0) * f64::from(frame.0);
    let expected_height = (rect.y1 - rect.y0) * f64::from(frame.1);
    let (size_slack, position_slack) = if rect == Rect::FULL {
        (0.0, 0.01)
    } else {
        (0.02, 0.02)
    };
    let size = format!("{}x{}", capture.width, capture.height);
    if (f64::from(capture.width) - expected_width).abs() > size_slack * f64::from(frame.0) + 0.5
        || (f64::from(capture.height) - expected_height).abs()
            > size_slack * f64::from(frame.1) + 0.5
    {
        return Err(format!(
            "size {size}, expected {expected_width:.0}x{expected_height:.0}"
        ));
    }

    let mut good = 0;
    let mut wrong_blue = 0;
    let mut worst = (0.0, String::new());
    for row in 0..GRID {
        for column in 0..GRID {
            let u = (f64::from(column) + 0.5) / f64::from(GRID);
            let v = (f64::from(row) + 0.5) / f64::from(GRID);
            let x = ((u * f64::from(capture.width)) as u32).min(capture.width - 1);
            let y = ((v * f64::from(capture.height)) as u32).min(capture.height - 1);
            let [red, green, blue] = capture.rgb[(y * capture.width + x) as usize];
            let want_x = rect.x0 + u * (rect.x1 - rect.x0);
            let want_y = rect.y0 + v * (rect.y1 - rect.y0);
            let got_x = f64::from(red) / 255.0;
            let got_y = f64::from(green) / 255.0;
            let error = (got_x - want_x).abs().max((got_y - want_y).abs());
            let blue_ok = blue.abs_diff(BLUE) <= 12;
            if !blue_ok {
                wrong_blue += 1;
            }
            if error <= position_slack && blue_ok {
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
    let total = GRID * GRID;
    if good * 10 >= total * 9 {
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

    fn render(
        frame: (u32, u32),
        rect: Rect,
        size: (u32, u32),
        transform: fn([u8; 3]) -> [u8; 3],
    ) -> Capture {
        let mut rgb = Vec::new();
        for y in 0..size.1 {
            for x in 0..size.0 {
                let source_x = rect.x0 * f64::from(frame.0) + f64::from(x);
                let source_y = rect.y0 * f64::from(frame.1) + f64::from(y);
                rgb.push(transform(pattern_pixel(
                    source_x as u32,
                    source_y as u32,
                    frame.0,
                    frame.1,
                )));
            }
        }
        Capture {
            width: size.0,
            height: size.1,
            rgb,
        }
    }

    fn same(pixel: [u8; 3]) -> [u8; 3] {
        pixel
    }

    const QUARTER: Rect = Rect {
        x0: 0.25,
        y0: 0.25,
        x1: 0.75,
        y1: 0.75,
    };

    #[test]
    fn passes_an_exact_fullscreen_capture() {
        let capture = render((640, 400), Rect::FULL, (640, 400), same);
        assert!(judge(&capture, (640, 400), Rect::FULL).is_ok());
    }

    #[test]
    fn passes_a_centered_region_one_pixel_off() {
        let capture = render((640, 400), QUARTER, (321, 201), same);
        assert!(judge(&capture, (640, 400), QUARTER).is_ok());
    }

    #[test]
    fn rejects_swapped_red_and_blue() {
        let capture = render((640, 400), Rect::FULL, (640, 400), |[r, g, b]| [b, g, r]);
        let error = judge(&capture, (640, 400), Rect::FULL).unwrap_err();
        assert!(error.contains("red and blue"), "{error}");
    }

    #[test]
    fn rejects_a_horizontally_mirrored_frame() {
        let mut capture = render((640, 400), Rect::FULL, (640, 400), same);
        for row in capture.rgb.as_chunks_mut::<640>().0 {
            row.reverse();
        }
        assert!(judge(&capture, (640, 400), Rect::FULL).is_err());
    }

    #[test]
    fn rejects_a_region_taken_from_the_wrong_place() {
        let shifted = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 0.5,
            y1: 0.5,
        };
        let capture = render((640, 400), shifted, (320, 200), same);
        assert!(judge(&capture, (640, 400), QUARTER).is_err());
    }

    #[test]
    fn rejects_a_logical_size_fullscreen_on_a_scaled_monitor() {
        let capture = render((640, 400), Rect::FULL, (427, 267), same);
        assert!(judge(&capture, (640, 400), Rect::FULL).is_err());
    }
}
