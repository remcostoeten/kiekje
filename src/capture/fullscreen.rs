use super::CaptureResult;
use crate::platform::linux::grim;
use anyhow::{Context, Result};

pub fn capture() -> Result<CaptureResult> {
    let png_data = grim::capture_fullscreen().context("grim failed to capture fullscreen")?;
    Ok(CaptureResult { png_data })
}
