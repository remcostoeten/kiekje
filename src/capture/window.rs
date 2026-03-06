use super::CaptureResult;
use crate::platform::linux::grim;
use anyhow::{Result, anyhow};

pub fn capture() -> Result<CaptureResult> {
    // Placeholder for active-window capture. For Hyprland, future work can query
    // active window geometry through `hyprctl activewindow -j`.
    let png_data = grim::capture_fullscreen().map_err(|e| anyhow!("window capture placeholder failed: {e}"))?;
    Ok(CaptureResult { png_data })
}
