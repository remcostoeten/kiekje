use super::{CaptureBackend, CaptureResult};
use anyhow::{Context, Result};

pub fn capture(backend: &dyn CaptureBackend) -> Result<CaptureResult> {
    let geometry = backend
        .active_window_geometry()
        .context("failed to resolve active window geometry from Hyprland")?;
    let png_data = backend
        .capture_region(&geometry)
        .context("grim failed to capture active window region")?;
    Ok(CaptureResult { png_data })
}
