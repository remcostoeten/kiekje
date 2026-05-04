use super::CaptureResult;
use crate::platform::capture::CaptureBackend;
use anyhow::{Context, Result};

pub fn capture<B: CaptureBackend>(backend: &B) -> Result<CaptureResult> {
    let geometry = backend
        .active_window_geometry()
        .context("failed to resolve active window geometry from Hyprland")?;
    let png_data = backend
        .capture_region(&geometry)
        .context("grim failed to capture active window region")?;
    Ok(CaptureResult { png_data })
}

#[cfg(test)]
mod tests {
    use super::capture;
    use crate::platform::capture::CaptureBackend;
    use anyhow::{anyhow, Result};
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeBackend {
        calls: RefCell<Vec<&'static str>>,
        geometry_result: Option<String>,
        region_result: Option<Vec<u8>>,
    }

    impl CaptureBackend for FakeBackend {
        fn capture_fullscreen(&self) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        fn capture_region(&self, _geometry: &str) -> Result<Vec<u8>> {
            self.calls.borrow_mut().push("capture_region");
            self.region_result
                .clone()
                .ok_or_else(|| anyhow!("region capture failed"))
        }

        fn active_window_geometry(&self) -> Result<String> {
            self.calls.borrow_mut().push("active_window_geometry");
            self.geometry_result
                .clone()
                .ok_or_else(|| anyhow!("geometry lookup failed"))
        }
    }

    #[test]
    fn uses_geometry_then_region_capture() {
        let backend = FakeBackend {
            geometry_result: Some("5,6 7x8".to_string()),
            region_result: Some(vec![9, 8, 7]),
            ..FakeBackend::default()
        };

        let result = capture(&backend).unwrap();

        assert_eq!(result.png_data, vec![9, 8, 7]);
        assert_eq!(
            backend.calls.borrow().as_slice(),
            &["active_window_geometry", "capture_region"]
        );
    }

    #[test]
    fn returns_geometry_errors_without_region_capture() {
        let backend = FakeBackend {
            geometry_result: None,
            region_result: Some(vec![1, 2, 3]),
            ..FakeBackend::default()
        };

        let err = capture(&backend).unwrap_err();

        assert!(err
            .to_string()
            .contains("failed to resolve active window geometry from Hyprland"));
        assert_eq!(
            backend.calls.borrow().as_slice(),
            &["active_window_geometry"]
        );
    }
}
