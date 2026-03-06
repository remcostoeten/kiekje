use crate::settings::config::{CaptureMode, Settings};
use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::PathBuf;

pub fn save_capture(png_data: &[u8], settings: &Settings, mode: CaptureMode) -> Result<PathBuf> {
    let path = render_path(settings, mode);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create save directory {}", parent.display()))?;
    }
    fs::write(&path, png_data)
        .with_context(|| format!("failed to save capture to {}", path.display()))?;
    Ok(path)
}

fn render_path(settings: &Settings, mode: CaptureMode) -> PathBuf {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let mode_str = match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    };

    let mut filename = settings.filename_template.clone();
    filename = filename.replace("{timestamp}", &timestamp);
    filename = filename.replace("{mode}", mode_str);

    if !filename.ends_with(".png") {
        filename.push_str(".png");
    }

    settings.default_save_location.join(filename)
}
