use crate::settings::config::CaptureMode;
use anyhow::Result;

pub trait CaptureBackend {
    fn capture_fullscreen(&self) -> Result<Vec<u8>>;
    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>>;
    fn active_window_geometry(&self) -> Result<String>;
}

pub fn current_backend() -> impl CaptureBackend {
    crate::platform::linux::backend::current_backend()
}

pub fn platform_name() -> &'static str {
    "linux"
}

#[allow(dead_code)]
pub fn backend_name() -> &'static str {
    "grim-hyprland"
}

#[allow(dead_code)]
pub fn mode_supported(mode: CaptureMode) -> bool {
    matches!(
        mode,
        CaptureMode::Region | CaptureMode::Fullscreen | CaptureMode::Window
    )
}
