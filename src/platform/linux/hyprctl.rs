use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct ActiveWindow {
    at: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
}

pub fn active_window_geometry() -> Result<String> {
    let output = Command::new("hyprctl")
        .arg("activewindow")
        .arg("-j")
        .output()
        .context("failed to execute hyprctl activewindow -j")?;

    if !output.status.success() {
        bail!(
            "hyprctl activewindow failed (status {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let window: ActiveWindow = serde_json::from_slice(&output.stdout)
        .context("failed to parse hyprctl activewindow JSON")?;

    let (x, y) = window
        .at
        .context("hyprctl activewindow did not include window position")?;
    let (w, h) = window
        .size
        .context("hyprctl activewindow did not include window size")?;

    if w <= 0 || h <= 0 {
        bail!("active window has invalid size: {}x{}", w, h);
    }

    Ok(format!("{},{} {}x{}", x, y, w, h))
}
