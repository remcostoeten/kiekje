pub mod region_selector;
pub mod tray;
pub mod window;

use crate::capture::CaptureResult;
use crate::settings::config::{CaptureMode, Settings};
use anyhow::Result;

pub fn run_editor(capture: CaptureResult, settings: Settings, mode: CaptureMode) -> Result<()> {
    tray::init_tray();
    window::run(capture, settings, mode)
}
