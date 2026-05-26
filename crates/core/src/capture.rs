use crate::model::ScreenshotImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Wayland,
    X11,
    Unknown,
}

pub fn detect_session_kind() -> SessionKind {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return SessionKind::Wayland;
    }

    if std::env::var_os("DISPLAY").is_some() {
        return SessionKind::X11;
    }

    SessionKind::Unknown
}

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

#[derive(Debug, Default)]
pub struct WaylandCaptureBackend;

impl CaptureBackend for WaylandCaptureBackend {
    fn name(&self) -> &'static str {
        "wayland"
    }

    fn capture_region(&self, _x: u32, _y: u32, width: u32, height: u32) -> Result<ScreenshotImage, CaptureError> {
        if width == 0 || height == 0 {
            return Err(CaptureError::Failed("capture region must be non-zero".to_string()));
        }

        Err(CaptureError::UnsupportedSession)
    }
}

#[derive(Debug, Default)]
pub struct X11CaptureBackend;

impl CaptureBackend for X11CaptureBackend {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn capture_region(&self, _x: u32, _y: u32, width: u32, height: u32) -> Result<ScreenshotImage, CaptureError> {
        if width == 0 || height == 0 {
            return Err(CaptureError::Failed("capture region must be non-zero".to_string()));
        }

        Err(CaptureError::UnsupportedSession)
    }
}

pub fn backend_for_current_session() -> Box<dyn CaptureBackend> {
    match detect_session_kind() {
        SessionKind::Wayland => Box::new(WaylandCaptureBackend),
        SessionKind::X11 => Box::new(X11CaptureBackend),
        SessionKind::Unknown => Box::new(StubCaptureBackend),
    }
}
