use std::collections::HashMap;
use std::io::Read;
use std::os::fd::AsFd;
use std::thread;

use zbus::blocking::{Connection, Proxy, proxy::Builder};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{Fd, OwnedValue, Value};

use crate::backends::{Capabilities, CaptureBackend};
use crate::capture::{CaptureError, Frame, MonitorInfo, PixelFormat};

const DESTINATION: &str = "org.kde.KWin";
const PATH: &str = "/org/kde/KWin/ScreenShot2";
const INTERFACE: &str = "org.kde.KWin.ScreenShot2";

pub struct Kwin;

impl CaptureBackend for Kwin {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            cursor: true,
            native_resolution: true,
            sandbox_compatible: false,
        }
    }

    fn capture(
        &self,
        monitors: &[MonitorInfo],
        include_cursor: bool,
    ) -> Result<Vec<Frame>, CaptureError> {
        capture_monitors(monitors, include_cursor)
    }
}

fn capture_monitors(
    monitors: &[MonitorInfo],
    include_cursor: bool,
) -> Result<Vec<Frame>, CaptureError> {
    let connection = Connection::session()?;
    let handles: Vec<_> = monitors
        .iter()
        .cloned()
        .map(|monitor| {
            let connection = connection.clone();
            thread::spawn(move || capture_screen(&connection, monitor, include_cursor))
        })
        .collect();

    handles
        .into_iter()
        .map(|handle| {
            handle.join().unwrap_or_else(|_| {
                Err(CaptureError::Unavailable("capture thread panicked".into()))
            })
        })
        .collect()
}

fn capture_screen(
    connection: &Connection,
    monitor: MonitorInfo,
    include_cursor: bool,
) -> Result<Frame, CaptureError> {
    let proxy: Proxy = Builder::new(connection)
        .destination(DESTINATION)?
        .path(PATH)?
        .interface(INTERFACE)?
        .cache_properties(CacheProperties::No)
        .build()?;

    let (mut reader, writer) = std::io::pipe()?;
    let pixels = thread::spawn(move || {
        let mut data = Vec::new();
        reader.read_to_end(&mut data).map(|_| data)
    });

    let options: HashMap<&str, Value> = HashMap::from([
        ("native-resolution", Value::from(true)),
        ("include-cursor", Value::from(include_cursor)),
    ]);
    let reply = proxy.call::<_, _, HashMap<String, OwnedValue>>(
        "CaptureScreen",
        &(
            monitor.connector.as_str(),
            options,
            Fd::from(writer.as_fd()),
        ),
    );
    drop(writer);

    let results = reply.map_err(map_dbus_error)?;
    let data = pixels
        .join()
        .map_err(|_| CaptureError::Unavailable("pipe reader panicked".into()))??;

    let width = read_u32(&results, "width")?;
    let height = read_u32(&results, "height")?;
    let stride = read_u32(&results, "stride")? as usize;
    let format_code = read_u32(&results, "format")?;
    let format = PixelFormat::from_qimage(format_code)
        .ok_or(CaptureError::UnsupportedFormat(format_code))?;

    if data.len() < stride * height as usize {
        return Err(CaptureError::Decode(format!(
            "KWin sent {} bytes, expected {}",
            data.len(),
            stride * height as usize
        )));
    }

    Ok(Frame {
        monitor,
        width,
        height,
        stride,
        format,
        data,
    })
}

fn read_u32(results: &HashMap<String, OwnedValue>, key: &str) -> Result<u32, CaptureError> {
    results
        .get(key)
        .and_then(|value| value.downcast_ref::<u32>().ok())
        .ok_or_else(|| CaptureError::Decode(format!("KWin reply is missing `{key}`")))
}

fn map_dbus_error(error: zbus::Error) -> CaptureError {
    match &error {
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("NoAuthorized") => {
            CaptureError::Unauthorized
        }
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("ServiceUnknown") => {
            CaptureError::Unavailable("KWin is not running".into())
        }
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("InvalidScreen") => {
            CaptureError::Unavailable("KWin does not know this monitor".into())
        }
        _ => CaptureError::Dbus(error),
    }
}
