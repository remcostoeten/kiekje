use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR_NAME: &str = "kiekje";
const LEGACY_CONFIG_DIR_NAME: &str = "screeny";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    #[default]
    Region,
    Fullscreen,
    Window,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub delay_ms: u64,
    pub default_save_location: PathBuf,
    pub copy_to_clipboard: bool,
    pub close_after_copy: bool,
    pub open_after_save: bool,
    pub open_editor: bool,
    pub default_capture_mode: CaptureMode,
    pub auto_save: bool,
    pub tray_autostart: bool,
    pub shortcut_region: String,
    pub shortcut_fullscreen: String,
    pub shortcut_window: String,
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
            close_after_copy: false,
            open_after_save: false,
            open_editor: true,
            default_capture_mode: CaptureMode::Region,
            auto_save: false,
            tray_autostart: false,
            shortcut_region: "SUPER SHIFT, S".to_string(),
            shortcut_fullscreen: "SUPER SHIFT, F".to_string(),
            shortcut_window: "SUPER SHIFT, W".to_string(),
            filename_template: "kiekje-{timestamp}-{mode}.png".to_string(),
        }
    }
}

impl Settings {
    pub fn load_or_default() -> Result<Self> {
        let path = existing_config_path()?.unwrap_or(config_path()?);
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read settings file at {}", path.display()))?;
        match serde_json::from_str(&raw) {
            Ok(settings) => Ok(settings),
            Err(parse_err) => {
                let backup = backup_corrupt_config(&path)?;
                eprintln!(
                    "Warning: invalid settings file at {} ({parse_err}). Backed up to {} and reset to defaults.",
                    path.display(),
                    backup.display()
                );
                let defaults = Self::default();
                defaults.save()?;
                Ok(defaults)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let body = serde_json::to_string_pretty(self).context("failed to serialize settings")?;
        fs::write(&path, body)
            .with_context(|| format!("failed to write settings file to {}", path.display()))?;
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    preferred_config_path_from_env(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn existing_config_path() -> Result<Option<PathBuf>> {
    existing_config_path_from_env(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

pub(crate) fn preferred_config_path_from_env(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf> {
    if let Some(xdg) = xdg_config_home {
        return Ok(PathBuf::from(xdg).join(CONFIG_DIR_NAME).join("config.json"));
    }

    let home = home
        .map(PathBuf::from)
        .context("HOME and XDG_CONFIG_HOME are not set")?;
    Ok(home
        .join(".config")
        .join(CONFIG_DIR_NAME)
        .join("config.json"))
}

pub(crate) fn legacy_config_path_from_env(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<PathBuf> {
    if let Some(xdg) = xdg_config_home {
        return Ok(PathBuf::from(xdg)
            .join(LEGACY_CONFIG_DIR_NAME)
            .join("config.json"));
    }

    let home = home
        .map(PathBuf::from)
        .context("HOME and XDG_CONFIG_HOME are not set")?;
    Ok(home
        .join(".config")
        .join(LEGACY_CONFIG_DIR_NAME)
        .join("config.json"))
}

pub(crate) fn existing_config_path_from_env(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Result<Option<PathBuf>> {
    let preferred = preferred_config_path_from_env(xdg_config_home.clone(), home.clone())?;
    if preferred.exists() {
        return Ok(Some(preferred));
    }

    let legacy = legacy_config_path_from_env(xdg_config_home, home)?;
    if legacy.exists() {
        return Ok(Some(legacy));
    }

    Ok(None)
}

fn backup_corrupt_config(path: &std::path::Path) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("failed to generate backup timestamp")?
        .as_secs();

    let file_name = path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("config.json");
    let backup_name = format!("{file_name}.corrupt-{ts}");
    let backup_path = path.with_file_name(backup_name);

    fs::rename(path, &backup_path).with_context(|| {
        format!(
            "failed to backup corrupt settings file {} to {}",
            path.display(),
            backup_path.display()
        )
    })?;

    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::{
        existing_config_path_from_env, legacy_config_path_from_env, preferred_config_path_from_env,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn uses_xdg_config_home_when_available() {
        let path =
            preferred_config_path_from_env(Some("/tmp/xdg".into()), Some("/tmp/home".into()))
                .unwrap();
        assert_eq!(path, PathBuf::from("/tmp/xdg/kiekje/config.json"));
    }

    #[test]
    fn falls_back_to_home_dot_config() {
        let path = preferred_config_path_from_env(None, Some("/tmp/home".into())).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/home/.config/kiekje/config.json"));
    }

    #[test]
    fn keeps_legacy_path_available_for_upgrade_reads() {
        let path =
            legacy_config_path_from_env(Some("/tmp/xdg".into()), Some("/tmp/home".into())).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/xdg/screeny/config.json"));
    }

    #[test]
    fn errors_when_both_xdg_and_home_are_missing() {
        let err = preferred_config_path_from_env(None, None).unwrap_err();
        assert!(
            err.to_string()
                .contains("HOME and XDG_CONFIG_HOME are not set"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reads_legacy_config_when_new_path_is_missing() {
        let base = std::env::temp_dir().join(format!("kiekje-config-test-{}", std::process::id()));
        let legacy_dir = base.join("screeny");
        fs::create_dir_all(&legacy_dir).unwrap();
        let legacy_path = legacy_dir.join("config.json");
        fs::write(&legacy_path, "{}").unwrap();

        let resolved =
            existing_config_path_from_env(Some(base.clone().into_os_string()), None).unwrap();
        assert_eq!(resolved, Some(legacy_path.clone()));

        let _ = fs::remove_file(legacy_path);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn deserializes_missing_new_fields_from_defaults() {
        let settings: super::Settings = serde_json::from_str(
            r#"{
                "delay_ms": 0,
                "default_save_location": "/tmp/shots",
                "copy_to_clipboard": true,
                "close_after_copy": false,
                "open_after_save": false,
                "open_editor": true,
                "default_capture_mode": "region",
                "auto_save": false,
                "filename_template": "kiekje-{timestamp}-{mode}.png"
            }"#,
        )
        .unwrap();

        assert!(!settings.tray_autostart);
        assert_eq!(settings.shortcut_region, "SUPER SHIFT, S");
        assert_eq!(settings.shortcut_fullscreen, "SUPER SHIFT, F");
        assert_eq!(settings.shortcut_window, "SUPER SHIFT, W");
    }
}
