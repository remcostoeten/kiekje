use smithay_client_toolkit::dispatch2::Dispatch2;
use wayland_client::globals::GlobalList;
use wayland_client::{Connection, Proxy, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    self, Flags, ZwlrScreencopyFrameV1,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use super::{Ignore, State, Status, missing_global};
use crate::capture::CaptureError;

const BUFFER_DONE_SINCE: u32 = 3;

struct FrameData(usize);

pub(super) fn request(
    globals: &GlobalList,
    handle: &QueueHandle<State>,
    state: &mut State,
    include_cursor: bool,
) -> Result<(), CaptureError> {
    let manager: ZwlrScreencopyManagerV1 = globals
        .bind(handle, 1..=BUFFER_DONE_SINCE, Ignore)
        .map_err(|_| missing_global("zwlr_screencopy_manager_v1"))?;
    for (index, target) in state.targets.iter().enumerate() {
        manager.capture_output(
            i32::from(include_cursor),
            &target.output,
            handle,
            FrameData(index),
        );
    }
    Ok(())
}

impl Dispatch2<ZwlrScreencopyFrameV1, State> for FrameData {
    fn event(
        &self,
        state: &mut State,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &Connection,
        handle: &QueueHandle<State>,
    ) {
        let index = self.0;
        let target = &mut state.targets[index];
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                if let WEnum::Value(format) = format {
                    target.offered_formats.push(format);
                }
                target.offered_size = Some((width, height));
                target.offered_stride = Some(stride);
                if frame.version() < BUFFER_DONE_SINCE {
                    copy(state, index, frame, handle);
                }
            }
            zwlr_screencopy_frame_v1::Event::Flags {
                flags: WEnum::Value(flags),
            } => target.y_invert = flags.contains(Flags::YInvert),
            zwlr_screencopy_frame_v1::Event::BufferDone => copy(state, index, frame, handle),
            zwlr_screencopy_frame_v1::Event::Ready { .. } => target.status = Status::Ready,
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.fail(index, "the compositor could not copy the output");
            }
            _ => {}
        }
    }
}

fn copy(
    state: &mut State,
    index: usize,
    frame: &ZwlrScreencopyFrameV1,
    handle: &QueueHandle<State>,
) {
    if state.targets[index].buffer.is_some() {
        return;
    }
    if let Some((buffer, _)) = state.allocate(index, handle) {
        frame.copy(&buffer);
    }
}
