use std::collections::HashMap;
use std::io::Read;
use std::os::fd::AsFd;
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

use zbus::blocking::{Connection, Proxy, proxy::Builder};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{Fd, OwnedValue, Value};

use crate::backends::{Capabilities, CaptureBackend};
use crate::capture::{CaptureError, Frame, MonitorInfo, PixelFormat, check_layout};

const DESTINATION: &str = "org.kde.KWin";
const PATH: &str = "/org/kde/KWin/ScreenShot2";
const INTERFACE: &str = "org.kde.KWin.ScreenShot2";
const TIMEOUT: Duration = Duration::from_secs(10);
const PIPE_BYTES: usize = 1 << 20;

static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);
static FRAME_BYTES: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);

fn connection() -> Result<Connection, CaptureError> {
    let mut cached = CONNECTION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(connection) = cached.as_ref() {
        return Ok(connection.clone());
    }
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(TIMEOUT)
        .build()?;
    *cached = Some(connection.clone());
    Ok(connection)
}

fn forget_connection() {
    *CONNECTION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

fn expected_bytes(connector: &str) -> usize {
    FRAME_BYTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .and_then(|sizes| sizes.get(connector).copied())
        .unwrap_or(0)
}

fn remember_bytes(connector: &str, bytes: usize) {
    FRAME_BYTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(connector.to_owned(), bytes);
}

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
    let connection = connection()?;
    let handles: Vec<_> = monitors
        .iter()
        .cloned()
        .map(|monitor| {
            let connection = connection.clone();
            thread::spawn(move || capture_screen(&connection, monitor, include_cursor))
        })
        .collect();

    let frames: Result<Vec<Frame>, CaptureError> = handles
        .into_iter()
        .map(|handle| {
            handle.join().unwrap_or_else(|_| {
                Err(CaptureError::Unavailable("capture thread panicked".into()))
            })
        })
        .collect();
    if matches!(&frames, Err(CaptureError::Dbus(_) | CaptureError::Io(_))) {
        forget_connection();
    }
    frames
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
    // A bigger pipe means fewer read calls for a frame of tens of megabytes; the default still works if this fails.
    let _ = rustix::pipe::fcntl_setpipe_size(&writer, PIPE_BYTES);
    let expected = expected_bytes(&monitor.connector);
    let (sender, pixels) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut data = Vec::with_capacity(expected);
        // The receiver is gone only when the capture already timed out.
        let _ = sender.send(reader.read_to_end(&mut data).map(|_| data));
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
    let data = pixels.recv_timeout(TIMEOUT).map_err(|error| match error {
        mpsc::RecvTimeoutError::Timeout => CaptureError::Timeout,
        mpsc::RecvTimeoutError::Disconnected => {
            CaptureError::Unavailable("pipe reader stopped".into())
        }
    })??;

    let width = read_u32(&results, "width")?;
    let height = read_u32(&results, "height")?;
    let stride = read_u32(&results, "stride")? as usize;
    let format_code = read_u32(&results, "format")?;
    let format = PixelFormat::from_qimage(format_code)
        .ok_or(CaptureError::UnsupportedFormat(format_code))?;

    check_layout(width, height, stride, data.len())?;
    remember_bytes(&monitor.connector, data.len());

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
        zbus::Error::MethodError(name, _, _)
            if ["UnknownMethod", "UnknownInterface", "UnknownObject"]
                .iter()
                .any(|missing| name.as_str().ends_with(missing)) =>
        {
            CaptureError::Unavailable("this KWin has no ScreenShot2".into())
        }
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("InvalidScreen") => {
            CaptureError::Unavailable("KWin does not know this monitor".into())
        }
        _ => CaptureError::Dbus(error),
    }
}
