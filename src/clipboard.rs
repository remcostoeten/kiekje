use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;
use thiserror::Error;
use wayland_client::globals::{GlobalError, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{
    ConnectError, Connection, Dispatch, DispatchError, EventQueue, QueueHandle, event_created_child,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
    self, ExtDataControlDeviceV1,
};
use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1;
use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
    self, ExtDataControlSourceV1,
};

const PNG_MIME: &str = "image/png";

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("no Wayland display: {0}")]
    NoDisplay(#[from] ConnectError),
    #[error("could not read the Wayland globals: {0}")]
    Globals(#[from] GlobalError),
    #[error("the compositor does not offer {0}")]
    Unsupported(&'static str),
    #[error("the first Wayland roundtrip failed: {0}")]
    Roundtrip(#[from] DispatchError),
    #[error("could not start the clipboard thread: {0}")]
    Io(#[from] io::Error),
    #[error("the clipboard thread stopped")]
    Stopped,
}

#[derive(Clone)]
pub struct Clipboard {
    images: Sender<Arc<Vec<u8>>>,
    waker: Arc<UnixStream>,
}

impl Clipboard {
    /// Connects to the compositor and starts a thread that owns the clipboard through
    /// `ext-data-control-v1`, which works without a focused surface.
    ///
    /// Fails with [`ClipboardError::Unsupported`] when the compositor has no data control
    /// global, which is the signal to fall back to the GTK clipboard.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let clipboard = Clipboard::start()?;
    /// ```
    pub fn start() -> Result<Self, ClipboardError> {
        let connection = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
        let handle = queue.handle();
        let seat: wl_seat::WlSeat = globals
            .bind(&handle, 1..=1, ())
            .map_err(|_| ClipboardError::Unsupported("wl_seat"))?;
        let manager: ExtDataControlManagerV1 = globals
            .bind(&handle, 1..=1, ())
            .map_err(|_| ClipboardError::Unsupported("ext_data_control_manager_v1"))?;
        let device = manager.get_data_device(&seat, &handle, ());

        let mut state = State { running: true };
        queue.roundtrip(&mut state)?;

        let (waker, wake) = UnixStream::pair()?;
        waker.set_nonblocking(true)?;
        wake.set_nonblocking(true)?;
        let (images, receiver) = mpsc::channel();
        let worker = Worker {
            connection,
            queue,
            state,
            manager,
            device,
            images: receiver,
            wake,
        };
        thread::Builder::new()
            .name("kiekje-clipboard".into())
            .spawn(move || {
                if let Err(error) = worker.serve() {
                    eprintln!("kiekje: clipboard thread stopped: {error}");
                }
            })?;

        Ok(Self {
            images,
            waker: Arc::new(waker),
        })
    }

    /// Makes `png` the clipboard content, offered as `image/png`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// clipboard.set_png(Arc::new(output::encode_rgba(&rgba, ImageFormat::Png, quality)?))?;
    /// ```
    pub fn set_png(&self, png: Arc<Vec<u8>>) -> Result<(), ClipboardError> {
        self.images.send(png).map_err(|_| ClipboardError::Stopped)?;
        match (&*self.waker).write(&[1]) {
            Ok(_) => Ok(()),
            // A full wake buffer already guarantees the thread wakes up.
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(_) => Err(ClipboardError::Stopped),
        }
    }
}

struct State {
    running: bool,
}

struct Worker {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    images: Receiver<Arc<Vec<u8>>>,
    wake: UnixStream,
}

impl Worker {
    fn serve(mut self) -> io::Result<()> {
        loop {
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(io::Error::other)?;
            if !self.state.running {
                return Ok(());
            }
            self.connection.flush().map_err(io::Error::other)?;
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };

            let (wayland_ready, wake_ready) = {
                let mut fds = [
                    PollFd::from_borrowed_fd(guard.connection_fd(), PollFlags::IN),
                    PollFd::new(&self.wake, PollFlags::IN),
                ];
                match poll(&mut fds, None) {
                    Ok(_) => {}
                    Err(Errno::INTR) => continue,
                    Err(error) => return Err(error.into()),
                }
                (!fds[0].revents().is_empty(), !fds[1].revents().is_empty())
            };

            if wayland_ready {
                guard.read().map_err(io::Error::other)?;
            } else {
                drop(guard);
            }
            if wake_ready && !self.take_latest_image()? {
                return Ok(());
            }
        }
    }

    fn take_latest_image(&mut self) -> io::Result<bool> {
        let mut buffer = [0u8; 64];
        loop {
            match self.wake.read(&mut buffer) {
                Ok(0) => return Ok(false),
                Ok(_) => continue,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }

        let mut latest = None;
        loop {
            match self.images.try_recv() {
                Ok(png) => latest = Some(png),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(false),
            }
        }
        if let Some(png) = latest {
            let source = self.manager.create_data_source(&self.queue.handle(), png);
            source.offer(PNG_MIME.into());
            self.device.set_selection(Some(&source));
        }
        Ok(true)
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
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

impl Dispatch<wl_seat::WlSeat, ()> for State {
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

impl Dispatch<ExtDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtDataControlManagerV1,
        _: <ExtDataControlManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        device: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::Selection { id: Some(offer) }
            | ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                offer.destroy()
            }
            ext_data_control_device_v1::Event::Finished => {
                device.destroy();
                state.running = false;
            }
            _ => {}
        }
    }

    event_created_child!(State, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtDataControlOfferV1,
        _: <ExtDataControlOfferV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlSourceV1, Arc<Vec<u8>>> for State {
    fn event(
        _: &mut Self,
        source: &ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        png: &Arc<Vec<u8>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { mime_type, fd } if mime_type == PNG_MIME => {
                let png = png.clone();
                thread::spawn(move || match File::from(fd).write_all(&png) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::BrokenPipe => {}
                    Err(error) => eprintln!("kiekje: clipboard transfer failed: {error}"),
                });
            }
            ext_data_control_source_v1::Event::Cancelled => source.destroy(),
            _ => {}
        }
    }
}
