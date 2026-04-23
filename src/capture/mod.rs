mod fullscreen;
mod region;
mod window;

use crate::platform::linux::{grim, hyprctl};
use crate::settings::config::CaptureMode;
use anyhow::Result;

#[derive(Clone)]
pub struct CaptureResult {
    pub png_data: Vec<u8>,
}

pub trait CaptureBackend {
    fn capture_fullscreen(&self) -> Result<Vec<u8>>;
    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>>;
    fn active_window_geometry(&self) -> Result<String>;
}

struct LinuxCaptureBackend;

impl CaptureBackend for LinuxCaptureBackend {
    fn capture_fullscreen(&self) -> Result<Vec<u8>> {
        grim::capture_fullscreen()
    }

    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>> {
        grim::capture_region(geometry)
    }

    fn active_window_geometry(&self) -> Result<String> {
        hyprctl::active_window_geometry()
    }
}

pub fn capture(mode: CaptureMode) -> Result<CaptureResult> {
    capture_with_backend(mode, &LinuxCaptureBackend)
}

fn capture_with_backend(mode: CaptureMode, backend: &dyn CaptureBackend) -> Result<CaptureResult> {
    match mode {
        CaptureMode::Region => region::capture(backend),
        CaptureMode::Fullscreen => fullscreen::capture(backend),
        CaptureMode::Window => window::capture(backend),
    }
}

#[cfg(test)]
mod tests {
    use super::{capture_with_backend, CaptureBackend};
    use crate::settings::config::CaptureMode;
    use anyhow::{bail, Result};
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeBackend {
        calls: RefCell<Vec<&'static str>>,
    }

    impl CaptureBackend for FakeBackend {
        fn capture_fullscreen(&self) -> Result<Vec<u8>> {
            self.calls.borrow_mut().push("capture_fullscreen");
            Ok(vec![1, 2, 3])
        }

        fn capture_region(&self, geometry: &str) -> Result<Vec<u8>> {
            self.calls.borrow_mut().push("capture_region");
            if geometry.is_empty() {
                bail!("geometry should not be empty");
            }
            Ok(vec![4, 5, 6])
        }

        fn active_window_geometry(&self) -> Result<String> {
            self.calls.borrow_mut().push("active_window_geometry");
            Ok("10,20 30x40".to_string())
        }
    }

    #[test]
    fn fullscreen_capture_uses_backend() {
        let backend = FakeBackend::default();

        let capture = capture_with_backend(CaptureMode::Fullscreen, &backend).unwrap();

        assert_eq!(capture.png_data, vec![1, 2, 3]);
        assert_eq!(backend.calls.borrow().as_slice(), &["capture_fullscreen"]);
    }

    #[test]
    fn window_capture_uses_geometry_then_region_backend_calls() {
        let backend = FakeBackend::default();

        let capture = capture_with_backend(CaptureMode::Window, &backend).unwrap();

        assert_eq!(capture.png_data, vec![4, 5, 6]);
        assert_eq!(
            backend.calls.borrow().as_slice(),
            &["active_window_geometry", "capture_region"]
        );
    }
}
