use super::CaptureResult;
use crate::platform::linux::{grim, hyprctl};
use anyhow::{Context, Result};

pub fn capture() -> Result<CaptureResult> {
    let geometry = hyprctl::active_window_geometry()
        .context("failed to resolve active window geometry from Hyprland")?;
    let png_data =
        grim::capture_region(&geometry).context("grim failed to capture active window region")?;
    Ok(CaptureResult { png_data })
}
