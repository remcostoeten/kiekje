use crate::clipboard;
use crate::settings::config::{CaptureMode, Settings};
use crate::storage::save;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn save_capture(png_data: &[u8], settings: &Settings, mode: CaptureMode) -> Result<PathBuf> {
    save::save_capture(png_data, settings, mode)
}

pub fn save_capture_to_path(png_data: &[u8], path: &Path) -> Result<()> {
    save::save_capture_to_path(png_data, path)
}

pub fn suggested_save_filename(settings: &Settings, mode: CaptureMode) -> String {
    save::suggested_save_path(settings, mode)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kiekje.png")
        .to_string()
}

pub fn copy_png(png_data: &[u8]) -> Result<()> {
    clipboard::copy_png(png_data)
}

pub fn maybe_open_saved_path(path: &Path, settings: &Settings) -> Result<()> {
    if !settings.open_after_save {
        return Ok(());
    }

    match Command::new("xdg-open").arg(path).spawn() {
        Ok(_) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{maybe_open_saved_path, suggested_save_filename};
    use crate::settings::config::{CaptureMode, Settings};
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn suggests_filename_from_storage_rules() {
        let settings = Settings {
            default_save_location: PathBuf::from("/tmp/shots"),
            filename_template: "kiekje-{mode}".to_string(),
            ..Settings::default()
        };

        let filename = suggested_save_filename(&settings, CaptureMode::Region);

        assert_eq!(filename, "kiekje-region.png");
    }

    #[test]
    fn skips_open_when_setting_is_disabled() {
        let settings = Settings {
            open_after_save: false,
            ..Settings::default()
        };

        let result = maybe_open_saved_path(Path::new("/tmp/kiekje.png"), &settings);

        assert!(result.is_ok());
    }
}
