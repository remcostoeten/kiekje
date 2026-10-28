use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use zbus::blocking::{Proxy, connection, proxy::Builder};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::backends::{Capabilities, CaptureBackend};
use crate::capture::mapping::bounding_box;
use crate::capture::{CaptureError, Frame, MonitorInfo, PixelFormat};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

static TOKEN_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Portal;

impl CaptureBackend for Portal {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            cursor: false,
            native_resolution: false,
            sandbox_compatible: true,
        }
    }

    fn capture(&self, monitors: &[MonitorInfo], _: bool) -> Result<Vec<Frame>, CaptureError> {
        capture_monitors(monitors, monitors)
    }

    fn capture_some(
        &self,
        all: &[MonitorInfo],
        wanted: &[MonitorInfo],
        _: bool,
    ) -> Result<Vec<Frame>, CaptureError> {
        capture_monitors(all, wanted)
    }
}

fn capture_monitors(
    all: &[MonitorInfo],
    wanted: &[MonitorInfo],
) -> Result<Vec<Frame>, CaptureError> {
    let started = SystemTime::now();
    let uri = request_screenshot()?;
    let (path, _) = gtk::glib::filename_from_uri(&uri)
        .map_err(|error| CaptureError::Decode(format!("invalid portal URI {uri}: {error}")))?;

    let image = decode_png(&path);
    remove_if_created_since(&path, started);
    let (width, height, rgba) = image?;
    split_by_monitor(all, wanted, width, height, rgba)
}

fn request_screenshot() -> Result<String, CaptureError> {
    let connection = connection::Builder::session()?
        .method_timeout(RESPONSE_TIMEOUT)
        .build()?;
    let sender = connection
        .unique_name()
        .ok_or_else(|| CaptureError::Unavailable("no D-Bus unique name".into()))?
        .trim_start_matches(':')
        .replace('.', "_");
    let token = format!(
        "kiekje{}_{}",
        std::process::id(),
        TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let request_path = format!("{PORTAL_PATH}/request/{sender}/{token}");

    let request: Proxy = Builder::new(&connection)
        .destination(DESTINATION)?
        .path(request_path)?
        .interface("org.freedesktop.portal.Request")?
        .cache_properties(CacheProperties::No)
        .build()?;
    let mut responses = request.receive_signal("Response")?;

    let screenshot: Proxy = Builder::new(&connection)
        .destination(DESTINATION)?
        .path(PORTAL_PATH)?
        .interface("org.freedesktop.portal.Screenshot")?
        .cache_properties(CacheProperties::No)
        .build()?;
    let options: HashMap<&str, Value> = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("interactive", Value::from(false)),
    ]);
    screenshot
        .call::<_, _, OwnedObjectPath>("Screenshot", &("", options))
        .map_err(map_call_error)?;

    let (sender_channel, receiver) = mpsc::channel();
    thread::spawn(move || {
        let message = responses.next();
        // The receiver is gone after a timeout, so a failed send needs no handling.
        let _ = sender_channel.send(message);
    });
    let message = receiver
        .recv_timeout(RESPONSE_TIMEOUT)
        .map_err(|_| CaptureError::Timeout)?
        .ok_or_else(|| CaptureError::Unavailable("portal closed the request".into()))?;

    let (code, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    match code {
        0 => results
            .get("uri")
            .and_then(|value| value.downcast_ref::<&str>().ok().map(str::to_owned))
            .ok_or_else(|| CaptureError::Decode("portal response has no uri".into())),
        1 => Err(CaptureError::Canceled),
        other => Err(CaptureError::Unavailable(format!(
            "portal returned response code {other}"
        ))),
    }
}

fn map_call_error(error: zbus::Error) -> CaptureError {
    match &error {
        zbus::Error::InputOutput(io) if io.kind() == std::io::ErrorKind::TimedOut => {
            CaptureError::Timeout
        }
        _ => CaptureError::Dbus(error),
    }
}

fn decode_png(path: &Path) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path)?));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder
        .read_info()
        .map_err(|error| CaptureError::Decode(error.to_string()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| CaptureError::Decode("image too large".into()))?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| CaptureError::Decode(error.to_string()))?;
    if info.color_type != png::ColorType::Rgba {
        return Err(CaptureError::Decode(format!(
            "unexpected color type {:?}",
            info.color_type
        )));
    }
    buffer.truncate(info.buffer_size());
    Ok((info.width, info.height, buffer))
}

fn remove_if_created_since(path: &Path, started: SystemTime) {
    let created_by_request = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| modified >= started);
    if created_by_request && let Err(error) = fs::remove_file(path) {
        error!("could not remove portal file {}: {error}", path.display());
    }
}

fn split_by_monitor(
    all: &[MonitorInfo],
    wanted: &[MonitorInfo],
    width: u32,
    height: u32,
    rgba: Vec<u8>,
) -> Result<Vec<Frame>, CaptureError> {
    let bounds =
        bounding_box(all).ok_or_else(|| CaptureError::Unavailable("no monitors".into()))?;
    let scale_x = f64::from(width) / f64::from(bounds.width);
    let scale_y = f64::from(height) / f64::from(bounds.height);
    let source_stride = width as usize * 4;

    let mut whole = Some(rgba);
    let single = wanted.len() == 1;
    wanted
        .iter()
        .map(|monitor| {
            let left = (f64::from(monitor.logical.x - bounds.x) * scale_x).round() as u32;
            let top = (f64::from(monitor.logical.y - bounds.y) * scale_y).round() as u32;
            let frame_width =
                ((f64::from(monitor.logical.width) * scale_x).round() as u32).min(width - left);
            let frame_height =
                ((f64::from(monitor.logical.height) * scale_y).round() as u32).min(height - top);
            let stride = frame_width as usize * 4;
            let covers_image =
                left == 0 && top == 0 && frame_width == width && frame_height == height;
            let data = match whole.take() {
                Some(rgba) if single && covers_image => rgba,
                Some(rgba) => {
                    let data = crop_rows(
                        &rgba,
                        source_stride,
                        (left, top),
                        (frame_width, frame_height),
                    );
                    whole = Some(rgba);
                    data
                }
                None => {
                    return Err(CaptureError::Unavailable(
                        "portal image already used".into(),
                    ));
                }
            };
            Ok(Frame {
                monitor: monitor.clone(),
                width: frame_width,
                height: frame_height,
                stride,
                format: PixelFormat::Rgba,
                data,
            })
        })
        .collect()
}

fn crop_rows(rgba: &[u8], source_stride: usize, at: (u32, u32), size: (u32, u32)) -> Vec<u8> {
    let stride = size.0 as usize * 4;
    let mut data = Vec::with_capacity(stride * size.1 as usize);
    for row in at.1..at.1 + size.1 {
        let start = row as usize * source_stride + at.0 as usize * 4;
        data.extend_from_slice(&rgba[start..start + stride]);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::LogicalRect;

    fn monitor(name: &str, x: i32, y: i32, width: i32, height: i32) -> MonitorInfo {
        MonitorInfo {
            connector: name.into(),
            logical: LogicalRect {
                x,
                y,
                width,
                height,
            },
        }
    }

    #[test]
    fn splits_workspace_image_per_monitor() {
        let monitors = [monitor("A", 0, 0, 2, 1), monitor("B", 2, 0, 1, 1)];
        let rgba: Vec<u8> = (0..3u8)
            .flat_map(|pixel| [pixel, pixel, pixel, 255])
            .collect();
        let frames = split_by_monitor(&monitors, &monitors, 3, 1, rgba).unwrap();
        assert_eq!(frames[0].width, 2);
        assert_eq!(frames[0].data, vec![0, 0, 0, 255, 1, 1, 1, 255]);
        assert_eq!(frames[1].data, vec![2, 2, 2, 255]);
    }

    #[test]
    fn splits_only_the_wanted_monitor_against_the_whole_desktop() {
        let monitors = [monitor("A", 0, 0, 2, 1), monitor("B", 2, 0, 1, 1)];
        let rgba: Vec<u8> = (0..3u8)
            .flat_map(|pixel| [pixel, pixel, pixel, 255])
            .collect();
        let frames = split_by_monitor(&monitors, &monitors[1..], 3, 1, rgba).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].monitor.connector, "B");
        assert_eq!(frames[0].data, vec![2, 2, 2, 255]);
    }

    #[test]
    fn hands_over_the_decoded_image_when_one_monitor_covers_it() {
        let monitors = [monitor("A", 0, 0, 2, 1)];
        let rgba = vec![9u8; 8];
        let frames = split_by_monitor(&monitors, &monitors, 2, 1, rgba).unwrap();
        assert_eq!(frames[0].data, vec![9u8; 8]);
    }
}
