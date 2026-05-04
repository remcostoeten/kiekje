use anyhow::{bail, Context, Result};
use std::process::Command;

pub trait ScreenshotTool {
    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>>;
    fn capture_fullscreen(&self) -> Result<Vec<u8>>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GrimCli;

impl ScreenshotTool for GrimCli {
    fn capture_region(&self, geometry: &str) -> Result<Vec<u8>> {
        let output = Command::new("grim")
            .arg("-g")
            .arg(geometry)
            .arg("-")
            .output()
            .context("failed to execute grim for region capture")?;

        if !output.status.success() {
            bail!(
                "grim region capture failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(output.stdout)
    }

    fn capture_fullscreen(&self) -> Result<Vec<u8>> {
        let output = Command::new("grim")
            .arg("-")
            .output()
            .context("failed to execute grim for fullscreen capture")?;

        if !output.status.success() {
            bail!(
                "grim fullscreen capture failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(output.stdout)
    }
}

pub fn capture_region(geometry: &str) -> Result<Vec<u8>> {
    GrimCli.capture_region(geometry)
}

#[allow(dead_code)]
pub fn capture_fullscreen() -> Result<Vec<u8>> {
    GrimCli.capture_fullscreen()
}
