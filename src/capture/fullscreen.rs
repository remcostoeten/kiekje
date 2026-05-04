use super::CaptureResult;
use crate::platform::capture::CaptureBackend;
use anyhow::{Context, Result};

pub fn capture<B: CaptureBackend>(backend: &B) -> Result<CaptureResult> {
    let png_data = backend
        .capture_fullscreen()
        .context("grim failed to capture fullscreen")?;
    Ok(CaptureResult { png_data })
}
