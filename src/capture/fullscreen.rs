use super::{CaptureBackend, CaptureResult};
use anyhow::{Context, Result};

pub fn capture(backend: &dyn CaptureBackend) -> Result<CaptureResult> {
    let png_data = backend
        .capture_fullscreen()
        .context("grim failed to capture fullscreen")?;
    Ok(CaptureResult { png_data })
}
