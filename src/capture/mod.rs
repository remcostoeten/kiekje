mod fullscreen;
mod region;
mod window;

use crate::settings::config::CaptureMode;
use anyhow::Result;

#[derive(Clone)]
pub struct CaptureResult {
    pub png_data: Vec<u8>,
}

pub fn capture(mode: CaptureMode) -> Result<CaptureResult> {
    match mode {
        CaptureMode::Region => region::capture(),
        CaptureMode::Fullscreen => fullscreen::capture(),
        CaptureMode::Window => window::capture(),
    }
}
