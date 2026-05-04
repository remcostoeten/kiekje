mod fullscreen;
mod region;
mod window;

use crate::platform::capture::{self as platform_capture, CaptureBackend};
use crate::settings::config::CaptureMode;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct CaptureResult {
    pub png_data: Vec<u8>,
}

pub fn capture(mode: CaptureMode) -> Result<CaptureResult> {
    let backend = platform_capture::current_backend();
    capture_with_backend(mode, &backend)
}

fn capture_with_backend<B: CaptureBackend>(
    mode: CaptureMode,
    backend: &B,
) -> Result<CaptureResult> {
    match mode {
        CaptureMode::Region => region::capture(backend),
        CaptureMode::Fullscreen => fullscreen::capture(backend),
        CaptureMode::Window => window::capture(backend),
    }
}

#[cfg(test)]
mod tests {
    use super::capture_with_backend;
    use crate::platform::capture::CaptureBackend;
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
