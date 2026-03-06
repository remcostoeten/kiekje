pub mod region_selector;
pub mod tray;
pub mod window;

use crate::capture::CaptureResult;
use crate::settings::config::Settings;
use anyhow::Result;

pub fn run_editor(capture: CaptureResult, settings: Settings) -> Result<()> {
    tray::init_tray();
    window::run(capture, settings)
}
