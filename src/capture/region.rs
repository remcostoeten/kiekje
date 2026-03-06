use super::CaptureResult;
use crate::platform::linux::{grim, slurp};
use anyhow::{Context, Result};

pub fn capture() -> Result<CaptureResult> {
    let geometry = slurp::select_region().context("slurp failed to select a region")?;
    let png_data = grim::capture_region(&geometry).context("grim failed to capture selected region")?;
    Ok(CaptureResult { png_data })
}
