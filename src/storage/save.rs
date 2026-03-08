use crate::settings::config::{CaptureMode, Settings};
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

pub fn save_capture(png_data: &[u8], settings: &Settings, mode: CaptureMode) -> Result<PathBuf> {
    let path = suggested_save_path(settings, mode);
    save_capture_to_path(png_data, &path)?;
    Ok(path)
}

pub fn save_capture_to_path(png_data: &[u8], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create save directory {}", parent.display()))?;
    }
    fs::write(path, png_data)
        .with_context(|| format!("failed to save capture to {}", path.display()))?;
    Ok(())
}

pub fn suggested_save_path(settings: &Settings, mode: CaptureMode) -> PathBuf {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    render_path_with_timestamp(settings, mode, &timestamp)
}

pub(crate) fn render_path_with_timestamp(
    settings: &Settings,
    mode: CaptureMode,
    timestamp: &str,
) -> PathBuf {
    let mode_str = match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    };

    let mut filename = settings.filename_template.clone();
    filename = filename.replace("{timestamp}", timestamp);
    filename = filename.replace("{mode}", mode_str);

    if !filename.ends_with(".png") {
        filename.push_str(".png");
    }

    settings.default_save_location.join(filename)
}

#[cfg(test)]
mod tests {
    use super::{render_path_with_timestamp, save_capture_to_path};
    use crate::settings::config::{CaptureMode, Settings};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn substitutes_timestamp_and_mode_tokens() {
        let settings = Settings {
            default_save_location: PathBuf::from("/tmp/shots"),
            filename_template: "kiekje-{timestamp}-{mode}.png".to_string(),
            ..Settings::default()
        };

        let path =
            render_path_with_timestamp(&settings, CaptureMode::Window, "2026-03-06_13-00-00");
        assert_eq!(
            path,
            PathBuf::from("/tmp/shots/kiekje-2026-03-06_13-00-00-window.png")
        );
    }

    #[test]
    fn appends_png_extension_when_missing() {
        let settings = Settings {
            default_save_location: PathBuf::from("/tmp/shots"),
            filename_template: "shot-{mode}".to_string(),
            ..Settings::default()
        };

        let path =
            render_path_with_timestamp(&settings, CaptureMode::Region, "2026-03-06_13-00-00");
        assert_eq!(path, PathBuf::from("/tmp/shots/shot-region.png"));
    }

    #[test]
    fn saves_to_explicit_path() {
        let base = std::env::temp_dir().join(format!("kiekje-test-{}", std::process::id()));
        let path = base.join("nested").join("image.png");
        let png = [1_u8, 2, 3];

        save_capture_to_path(&png, &path).unwrap();

        assert_eq!(fs::read(&path).unwrap(), png);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&base);
    }
}
