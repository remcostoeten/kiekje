use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    #[default]
    Region,
    Fullscreen,
    Window,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub delay_ms: u64,
    pub default_save_location: PathBuf,
    pub copy_to_clipboard: bool,
    pub open_editor: bool,
    pub default_capture_mode: CaptureMode,
    pub auto_save: bool,
    pub filename_template: String,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            delay_ms: 0,
            default_save_location: home.join("Pictures").join("Screenshots"),
            copy_to_clipboard: true,
            open_editor: true,
            default_capture_mode: CaptureMode::Region,
            auto_save: false,
            filename_template: "screeny-{timestamp}-{mode}.png".to_string(),
        }
    }
}

impl Settings {
    pub fn load_or_default() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read settings file at {}", path.display()))?;
        let settings: Settings = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse settings file at {}", path.display()))?;
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config directory {}", parent.display()))?;
        }
        let body = serde_json::to_string_pretty(self).context("failed to serialize settings")?;
        fs::write(&path, body)
            .with_context(|| format!("failed to write settings file to {}", path.display()))?;
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("screeny").join("config.json"));
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME and XDG_CONFIG_HOME are not set")?;
    Ok(home.join(".config").join("screeny").join("config.json"))
}
