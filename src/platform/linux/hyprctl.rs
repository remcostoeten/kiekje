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

    parse_active_window_geometry_from_json(&output.stdout)
}

pub(crate) fn parse_active_window_geometry_from_json(raw: &[u8]) -> Result<String> {
    let window: ActiveWindow =
        serde_json::from_slice(raw).context("failed to parse hyprctl activewindow JSON")?;

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

#[cfg(test)]
mod tests {
    use super::parse_active_window_geometry_from_json;

    #[test]
    fn parses_valid_hyprctl_json() {
        let json = br#"{"at":[100,200],"size":[1280,720]}"#;
        let geom = parse_active_window_geometry_from_json(json).unwrap();
        assert_eq!(geom, "100,200 1280x720");
    }

    #[test]
    fn errors_when_required_fields_missing() {
        let json = br#"{"at":[100,200]}"#;
        let err = parse_active_window_geometry_from_json(json).unwrap_err();
        assert!(
            err.to_string()
                .contains("hyprctl activewindow did not include window size"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn errors_on_invalid_window_size() {
        let json = br#"{"at":[100,200],"size":[0,720]}"#;
        let err = parse_active_window_geometry_from_json(json).unwrap_err();
        assert!(
            err.to_string().contains("active window has invalid size"),
            "unexpected error: {err}"
        );
    }
}
