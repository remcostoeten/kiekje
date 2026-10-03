mod ext_image_copy;
mod screencopy;
mod transform;

use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shm::raw::RawPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_buffer, wl_output, wl_shm};
use wayland_client::{Connection, EventQueue, Proxy, QueueHandle};

use crate::backends::{Capabilities, CaptureBackend};
use crate::capture::{CaptureError, Frame, MonitorInfo, PixelFormat};

const TIMEOUT: Duration = Duration::from_secs(5);
const PREFERRED_FORMATS: [wl_shm::Format; 4] = [
    wl_shm::Format::Xrgb8888,
    wl_shm::Format::Argb8888,
    wl_shm::Format::Xbgr8888,
    wl_shm::Format::Abgr8888,
];
const CAPABILITIES: Capabilities = Capabilities {
    cursor: true,
    native_resolution: true,
    sandbox_compatible: false,
};

pub struct ExtImageCopy;

pub struct Screencopy;

impl CaptureBackend for ExtImageCopy {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn capture(
        &self,
        monitors: &[MonitorInfo],
        include_cursor: bool,
    ) -> Result<Vec<Frame>, CaptureError> {
        capture_with(Protocol::ExtImageCopy, monitors, include_cursor)
    }
}

impl CaptureBackend for Screencopy {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn capture(
        &self,
        monitors: &[MonitorInfo],
        include_cursor: bool,
    ) -> Result<Vec<Frame>, CaptureError> {
        capture_with(Protocol::Screencopy, monitors, include_cursor)
    }
}

#[derive(Clone, Copy)]
enum Protocol {
    ExtImageCopy,
    Screencopy,
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    shm: Shm,
    targets: Vec<Target>,
}

struct Target {
    monitor: MonitorInfo,
    output: wl_output::WlOutput,
    offered_size: Option<(u32, u32)>,
    offered_stride: Option<u32>,
    offered_formats: Vec<wl_shm::Format>,
    buffer: Option<Buffer>,
    transform: wl_output::Transform,
    y_invert: bool,
    status: Status,
}

struct Buffer {
    pool: RawPool,
    layout: Layout,
}

#[derive(Clone, Copy)]
struct Layout {
    width: u32,
    height: u32,
    stride: u32,
    format: wl_shm::Format,
}

enum Status {
    Pending,
    Ready,
    Failed(String),
}

struct Ignore;

fn capture_with(
    protocol: Protocol,
    monitors: &[MonitorInfo],
    include_cursor: bool,
) -> Result<Vec<Frame>, CaptureError> {
    let connection = Connection::connect_to_env()
        .map_err(|error| CaptureError::Unavailable(format!("no Wayland display: {error}")))?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection).map_err(wayland_error)?;
    let handle = queue.handle();
    let mut state = State {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &handle),
        shm: Shm::bind(&globals, &handle).map_err(|_| missing_global("wl_shm"))?,
        targets: Vec::new(),
    };
    queue.roundtrip(&mut state).map_err(wayland_error)?;
    state.targets = monitors
        .iter()
        .map(|monitor| state.target_for(monitor))
        .collect::<Result<_, _>>()?;

    match protocol {
        Protocol::ExtImageCopy => {
            ext_image_copy::request(&globals, &handle, &mut state, include_cursor)?
        }
        Protocol::Screencopy => screencopy::request(&globals, &handle, &mut state, include_cursor)?,
    }
    dispatch_until_settled(&connection, &mut queue, &mut state)?;
    // Disconnecting releases every capture object, so none are destroyed one by one.
    state.targets.into_iter().map(Target::into_frame).collect()
}

fn dispatch_until_settled(
    connection: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
) -> Result<(), CaptureError> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        queue.dispatch_pending(state).map_err(wayland_error)?;
        if state.settled() {
            return Ok(());
        }
        connection.flush().map_err(wayland_error)?;
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CaptureError::Timeout);
        }
        let timeout = Timespec {
            tv_sec: remaining.as_secs() as _,
            tv_nsec: remaining.subsec_nanos().into(),
        };
        let readable = {
            let mut fds = [PollFd::from_borrowed_fd(
                guard.connection_fd(),
                PollFlags::IN,
            )];
            match poll(&mut fds, Some(&timeout)) {
                Ok(ready) => ready > 0,
                Err(Errno::INTR) => false,
                Err(error) => return Err(CaptureError::Io(error.into())),
            }
        };
        if readable {
            guard.read().map_err(wayland_error)?;
        }
    }
}

impl State {
    fn target_for(&self, monitor: &MonitorInfo) -> Result<Target, CaptureError> {
        let found = self.outputs.outputs().find_map(|output| {
            let info = self.outputs.info(&output)?;
            (info.name.as_deref() == Some(monitor.connector.as_str()))
                .then_some((output, info.transform))
        });
        let (output, transform) = found.ok_or_else(|| {
            CaptureError::Unavailable(format!("no Wayland output named {}", monitor.connector))
        })?;
        Ok(Target {
            monitor: monitor.clone(),
            output,
            offered_size: None,
            offered_stride: None,
            offered_formats: Vec::new(),
            buffer: None,
            transform,
            y_invert: false,
            status: Status::Pending,
        })
    }

    fn settled(&self) -> bool {
        self.targets
            .iter()
            .all(|target| !matches!(target.status, Status::Pending))
    }

    fn fail(&mut self, index: usize, reason: impl Into<String>) {
        self.targets[index].status = Status::Failed(reason.into());
    }

    fn allocate(
        &mut self,
        index: usize,
        handle: &QueueHandle<State>,
    ) -> Option<(wl_buffer::WlBuffer, Layout)> {
        let target = &self.targets[index];
        let format = PREFERRED_FORMATS
            .into_iter()
            .find(|format| target.offered_formats.contains(format));
        let (Some(format), Some((width, height))) = (format, target.offered_size) else {
            self.fail(index, "the compositor offered no supported shm buffer");
            return None;
        };
        if width == 0 || height == 0 {
            self.fail(index, "the compositor offered an empty buffer");
            return None;
        }
        let layout = Layout {
            width,
            height,
            stride: target.offered_stride.unwrap_or(width * 4),
            format,
        };
        let mut pool = match RawPool::new(layout.stride as usize * height as usize, &self.shm) {
            Ok(pool) => pool,
            Err(error) => {
                self.fail(index, format!("could not allocate a shm buffer: {error}"));
                return None;
            }
        };
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            layout.stride as i32,
            format,
            Ignore,
            handle,
        );
        self.targets[index].buffer = Some(Buffer { pool, layout });
        Some((buffer, layout))
    }
}

impl Target {
    fn into_frame(self) -> Result<Frame, CaptureError> {
        let connector = self.monitor.connector.clone();
        match self.status {
            Status::Ready => {}
            Status::Failed(reason) => {
                return Err(CaptureError::Unavailable(format!("{connector}: {reason}")));
            }
            Status::Pending => return Err(CaptureError::Timeout),
        }
        let Some(mut buffer) = self.buffer else {
            return Err(CaptureError::Unavailable(format!(
                "{connector}: ready without a buffer"
            )));
        };
        let layout = buffer.layout;
        let format = pixel_format(layout.format)
            .ok_or(CaptureError::UnsupportedFormat(layout.format.into()))?;
        let image = transform::upright(
            &buffer.pool.mmap()[..],
            layout.width,
            layout.height,
            layout.stride as usize,
            self.transform,
            self.y_invert,
        );
        Ok(Frame {
            monitor: self.monitor,
            width: image.width,
            height: image.height,
            stride: image.stride,
            format,
            data: image.data,
        })
    }
}

fn pixel_format(format: wl_shm::Format) -> Option<PixelFormat> {
    match format {
        wl_shm::Format::Xrgb8888 => Some(PixelFormat::Bgrx),
        wl_shm::Format::Argb8888 => Some(PixelFormat::BgraPremultiplied),
        wl_shm::Format::Xbgr8888 => Some(PixelFormat::Rgbx),
        wl_shm::Format::Abgr8888 => Some(PixelFormat::RgbaPremultiplied),
        _ => None,
    }
}

fn wayland_error(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Wayland(error.to_string())
}

fn missing_global(name: &str) -> CaptureError {
    CaptureError::Unavailable(format!("the compositor does not offer {name}"))
}

impl<I: Proxy> Dispatch2<I, State> for Ignore {
    fn event(&self, _: &mut State, _: &I, _: I::Event, _: &Connection, _: &QueueHandle<State>) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers!(OutputState);
}

delegate_registry!(State);
delegate_dispatch2!(State);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_shm_formats_to_memory_order() {
        assert_eq!(
            pixel_format(wl_shm::Format::Xrgb8888),
            Some(PixelFormat::Bgrx)
        );
        assert_eq!(
            pixel_format(wl_shm::Format::Abgr8888),
            Some(PixelFormat::RgbaPremultiplied)
        );
        assert_eq!(pixel_format(wl_shm::Format::Rgb565), None);
    }
}
