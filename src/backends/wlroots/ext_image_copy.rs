use wayland_client::globals::GlobalList;
use wayland_client::{Connection, QueueHandle, WEnum};
use wayland_protocols::ext::image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::{
    self, ExtImageCopyCaptureFrameV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{
    ExtImageCopyCaptureManagerV1, Options,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::{
    self, ExtImageCopyCaptureSessionV1,
};

use super::{Ignore, State, Status, missing_global};
use crate::capture::CaptureError;
use smithay_client_toolkit::dispatch2::Dispatch2;

struct SessionData(usize);

struct FrameData(usize);

pub(super) fn request(
    globals: &GlobalList,
    handle: &QueueHandle<State>,
    state: &mut State,
    include_cursor: bool,
) -> Result<(), CaptureError> {
    let sources: ExtOutputImageCaptureSourceManagerV1 = globals
        .bind(handle, 1..=1, Ignore)
        .map_err(|_| missing_global("ext_output_image_capture_source_manager_v1"))?;
    let manager: ExtImageCopyCaptureManagerV1 = globals
        .bind(handle, 1..=1, Ignore)
        .map_err(|_| missing_global("ext_image_copy_capture_manager_v1"))?;
    let options = if include_cursor {
        Options::PaintCursors
    } else {
        Options::empty()
    };
    for (index, target) in state.targets.iter().enumerate() {
        let source = sources.create_source(&target.output, handle, Ignore);
        manager.create_session(&source, options, handle, SessionData(index));
    }
    Ok(())
}

impl Dispatch2<ExtImageCopyCaptureSessionV1, State> for SessionData {
    fn event(
        &self,
        state: &mut State,
        session: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &Connection,
        handle: &QueueHandle<State>,
    ) {
        let index = self.0;
        let target = &mut state.targets[index];
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                target.offered_size = Some((width, height));
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat {
                format: WEnum::Value(format),
            } => target.offered_formats.push(format),
            ext_image_copy_capture_session_v1::Event::Done if target.buffer.is_none() => {
                let Some((buffer, layout)) = state.allocate(index, handle) else {
                    return;
                };
                let frame = session.create_frame(handle, FrameData(index));
                frame.attach_buffer(&buffer);
                frame.damage_buffer(0, 0, layout.width as i32, layout.height as i32);
                frame.capture();
            }
            ext_image_copy_capture_session_v1::Event::Stopped
                if matches!(target.status, Status::Pending) =>
            {
                state.fail(index, "the capture session stopped");
            }
            _ => {}
        }
    }
}

impl Dispatch2<ExtImageCopyCaptureFrameV1, State> for FrameData {
    fn event(
        &self,
        state: &mut State,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        let index = self.0;
        match event {
            ext_image_copy_capture_frame_v1::Event::Transform {
                transform: WEnum::Value(transform),
            } => state.targets[index].transform = transform,
            ext_image_copy_capture_frame_v1::Event::Ready => {
                state.targets[index].status = Status::Ready;
            }
            ext_image_copy_capture_frame_v1::Event::Failed { reason } => {
                state.fail(index, format!("the frame failed: {reason:?}"));
            }
            _ => {}
        }
    }
}
