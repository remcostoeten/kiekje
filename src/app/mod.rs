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

pub fn run_launcher(settings: Settings) -> Result<()> {
    tray::run_launcher(settings)
}

pub fn run_tray(settings: Settings) -> Result<()> {
    tray::run_tray(settings)
}

pub fn show_feedback_window(title: &str, body: &str) {
    tray::show_feedback_window(title, body);
}
