use crate::model::ScreenshotImage;

pub trait CaptureBackend {
    fn name(&self) -> &'static str;
    fn capture_region(&self, x: u32, y: u32, width: u32, height: u32) -> Result<ScreenshotImage, CaptureError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    UnsupportedSession,
    Failed(String),
}

#[derive(Debug, Default)]
pub struct StubCaptureBackend;

impl CaptureBackend for StubCaptureBackend {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn capture_region(&self, _x: u32, _y: u32, width: u32, height: u32) -> Result<ScreenshotImage, CaptureError> {
        if width == 0 || height == 0 {
            return Err(CaptureError::Failed("capture region must be non-zero".to_string()));
        }

        Ok(ScreenshotImage::new(width, height))
    }
}
