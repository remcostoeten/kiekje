use std::rc::Rc;
use std::thread;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use super::flow::log_timing;
use super::notice::notify;
use super::save::store;
use super::{Request, Session};
use crate::capture::PixelFormat;
use crate::config::{self};
use crate::editor::{self, Finish};
use crate::notify::Notice;
use crate::output::Image;
use crate::pin;

type Render = Box<dyn FnOnce() -> Option<Image> + Send>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Placement {
    #[default]
    Configured,
    Captures,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Filing {
    pub(super) placement: Placement,
    pub(super) annotated: bool,
}

pub(super) fn deliver(render: Render, request: Request, filing: Filing, session: Rc<Session>) {
    if request.edit {
        annotate(render, request, session);
        return;
    }
    let clipboard = session.clipboard.clone();
    let snapshot = session.trigger_config.take();
    let display = gdk::Display::default().is_some();
    if clipboard.is_none() && !display && !request.save {
        warn!("no display, clipboard not set");
        finish_delivery(
            &session,
            Notice::CaptureFailed {
                reason: "no display to put the image on the clipboard".into(),
            },
        );
        return;
    }

    let (sender, receiver) = async_channel::bounded(2);
    thread::spawn(move || {
        let Some(image) = render() else {
            // The receiver only disappears when the app is quitting.
            let _ = sender.send_blocking(Delivery::Done(empty_notice()));
            return;
        };
        let notice = match clipboard {
            Some(clipboard) => store(image, request, filing, Some(&clipboard), false, snapshot),
            None => copy_off_thread(image, request, filing, display, &sender, snapshot),
        };
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking(Delivery::Done(notice));
    });
    glib::spawn_future_local(async move {
        loop {
            match receiver.recv().await {
                Ok(Delivery::Pixels(pixels)) => copy_with_gtk(pixels),
                Ok(Delivery::Done(notice)) => {
                    finish_delivery(&session, notice);
                    return;
                }
                Err(_) => {
                    error!("delivery worker stopped");
                    session.busy.set(false);
                    return;
                }
            }
        }
    });
}

enum Delivery {
    Pixels(Pixels),
    Done(Notice),
}

struct Pixels {
    width: u32,
    height: u32,
    stride: usize,
    format: PixelFormat,
    bytes: glib::Bytes,
}

impl Pixels {
    fn copied(image: &Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            stride: image.stride,
            format: image.format,
            bytes: glib::Bytes::from(&image.data),
        }
    }

    fn owned(image: Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            stride: image.stride,
            format: image.format,
            bytes: glib::Bytes::from_owned(image.data),
        }
    }
}

fn copy_off_thread(
    image: Image,
    request: Request,
    filing: Filing,
    display: bool,
    sender: &async_channel::Sender<Delivery>,
    config: Option<config::Config>,
) -> Notice {
    if !display {
        return store(image, request, filing, None, false, config);
    }
    if request.save {
        // The receiver only disappears when the app is quitting.
        let _ = sender.send_blocking(Delivery::Pixels(Pixels::copied(&image)));
        return store(image, request, filing, None, true, config);
    }
    let (width, height) = (image.width, image.height);
    // The receiver only disappears when the app is quitting.
    let _ = sender.send_blocking(Delivery::Pixels(Pixels::owned(image)));
    Notice::Copied { width, height }
}

pub(super) fn annotate(render: Render, request: Request, session: Rc<Session>) {
    glib::spawn_future_local(async move {
        let rendered = gio::spawn_blocking(render).await;
        session.busy.set(false);
        let image = match rendered {
            Ok(Some(image)) => image,
            Ok(None) => {
                notify(&session, empty_notice());
                return;
            }
            Err(_) => {
                error!("the render worker stopped");
                return;
            }
        };
        let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() else {
            warn!("no running app to open the editor in");
            return;
        };
        log_timing(
            "trigger -> editor opening",
            request.trigger_us,
            glib::monotonic_time(),
        );
        editor::open(
            &app,
            editor::Source::Capture {
                image,
                mode: request.mode,
            },
        );
    });
}

pub(super) fn finish_edit(
    image: Image,
    finish: Finish,
    annotated: bool,
    request: Request,
    session: Rc<Session>,
) {
    let request = Request {
        edit: false,
        ..request
    };
    let filing = Filing {
        placement: Placement::Configured,
        annotated,
    };
    match finish {
        Finish::Accept => deliver(Box::new(move || Some(image)), request, filing, session),
        Finish::Copy => deliver(
            Box::new(move || Some(image)),
            Request {
                save: false,
                ..request
            },
            filing,
            session,
        ),
        Finish::Save => save_only(
            image,
            Request {
                save: true,
                ..request
            },
            Filing {
                placement: Placement::Captures,
                annotated,
            },
            session,
        ),
        Finish::SaveAs => session.busy.set(false),
        Finish::Pin => {
            if let Some(app) = gio::Application::default().and_downcast::<gtk::Application>() {
                pin::open_image(&app, image, None);
            }
            session.busy.set(false);
        }
    }
}

fn save_only(image: Image, request: Request, filing: Filing, session: Rc<Session>) {
    glib::spawn_future_local(async move {
        match gio::spawn_blocking(move || store(image, request, filing, None, false, None)).await {
            Ok(notice) => finish_delivery(&session, notice),
            Err(_) => {
                error!("the save worker stopped");
                session.busy.set(false);
            }
        }
    });
}

fn empty_notice() -> Notice {
    warn!("nothing to deliver");
    Notice::CaptureFailed {
        reason: "the capture had no image to deliver".into(),
    }
}

pub(super) fn finish_delivery(session: &Rc<Session>, notice: Notice) {
    if let (Notice::Saved { .. } | Notice::Copied { .. }, Some(gallery)) =
        (&notice, session.panels.gallery_handle.borrow().as_ref())
    {
        gallery.reload();
    }
    if matches!(notice, Notice::Saved { .. } | Notice::Copied { .. })
        && let Some(tray) = session.tray.borrow().as_ref()
    {
        tray.flash();
    }
    notify(session, notice);
    session.busy.set(false);
}

fn copy_with_gtk(pixels: Pixels) {
    let Some(display) = gdk::Display::default() else {
        warn!("no display, clipboard not set");
        return;
    };
    let texture = gdk::MemoryTexture::new(
        pixels.width as i32,
        pixels.height as i32,
        pixels.format.memory_format(),
        &pixels.bytes,
        pixels.stride,
    );
    display.clipboard().set_texture(&texture);
    info!(
        "copied {}x{} to the GTK clipboard",
        pixels.width, pixels.height
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureMode, PixelFormat};

    #[test]
    fn hands_copy_only_pixels_to_the_main_thread_without_copying() {
        let data = vec![7u8; 4 * 3 * 2];
        let source = data.as_ptr();
        let image = Image {
            width: 3,
            height: 2,
            stride: 12,
            format: PixelFormat::Bgra,
            data,
        };
        let request = Request {
            save: false,
            ..Request::saved(CaptureMode::Region)
        };
        let (sender, receiver) = async_channel::bounded(2);
        let notice = copy_off_thread(image, request, Filing::default(), true, &sender, None);
        assert!(matches!(
            notice,
            Notice::Copied {
                width: 3,
                height: 2
            }
        ));
        let Ok(Delivery::Pixels(pixels)) = receiver.try_recv() else {
            panic!("expected pixels before the notice");
        };
        assert_eq!((pixels.width, pixels.height, pixels.stride), (3, 2, 12));
        assert_eq!(pixels.bytes.as_ref().as_ptr(), source);
    }
}
