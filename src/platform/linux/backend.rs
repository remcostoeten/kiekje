#![allow(dead_code)]

use super::grim::{GrimCli, ScreenshotTool};
use super::hyprctl::{ActiveWindowGeometrySource, HyprctlCli};
use anyhow::Result;

pub trait LinuxCaptureBackend {
    fn capture_fullscreen(&self) -> Result<Vec<u8>>;
    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>>;
    fn active_window_geometry(&self) -> Result<String>;
}

#[derive(Debug, Clone, Default)]
pub struct GrimHyprlandBackend<S = GrimCli, W = HyprctlCli> {
    screenshot_tool: S,
    active_window_source: W,
}

impl<S, W> GrimHyprlandBackend<S, W> {
    pub fn new(screenshot_tool: S, active_window_source: W) -> Self {
        Self {
            screenshot_tool,
            active_window_source,
        }
    }
}

impl<S, W> LinuxCaptureBackend for GrimHyprlandBackend<S, W>
where
    S: ScreenshotTool,
    W: ActiveWindowGeometrySource,
{
    fn capture_fullscreen(&self) -> Result<Vec<u8>> {
        self.screenshot_tool.capture_fullscreen()
    }

    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>> {
        self.screenshot_tool.capture_region(geometry)
    }

    fn active_window_geometry(&self) -> Result<String> {
        self.active_window_source.active_window_geometry()
    }
}

#[cfg(test)]
mod tests {
    use super::{GrimHyprlandBackend, LinuxCaptureBackend};
    use crate::platform::linux::grim::ScreenshotTool;
    use crate::platform::linux::hyprctl::ActiveWindowGeometrySource;
    use anyhow::Result;

    struct FakeScreenshotTool;

    impl ScreenshotTool for FakeScreenshotTool {
        fn capture_region(&self, geometry: &str) -> Result<Vec<u8>> {
            Ok(format!("region:{geometry}").into_bytes())
        }

        fn capture_fullscreen(&self) -> Result<Vec<u8>> {
            Ok(b"fullscreen".to_vec())
        }
    }

    struct FakeActiveWindowSource;

    impl ActiveWindowGeometrySource for FakeActiveWindowSource {
        fn active_window_geometry(&self) -> Result<String> {
            Ok("10,20 300x400".to_string())
        }
    }

    #[test]
    fn delegates_fullscreen_capture_to_screenshot_tool() {
        let backend = GrimHyprlandBackend::new(FakeScreenshotTool, FakeActiveWindowSource);

        let png = backend.capture_fullscreen().unwrap();

        assert_eq!(png, b"fullscreen");
    }

    #[test]
    fn delegates_region_capture_to_screenshot_tool() {
        let backend = GrimHyprlandBackend::new(FakeScreenshotTool, FakeActiveWindowSource);

        let png = backend.capture_region("1,2 3x4").unwrap();

        assert_eq!(png, b"region:1,2 3x4");
    }

    #[test]
    fn delegates_active_window_geometry_to_window_source() {
        let backend = GrimHyprlandBackend::new(FakeScreenshotTool, FakeActiveWindowSource);

        let geometry = backend.active_window_geometry().unwrap();

        assert_eq!(geometry, "10,20 300x400");
    }
}
