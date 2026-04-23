//! Reusable settings service for UI shells and command surfaces.

use std::path::PathBuf;

use crate::services::app::{AppError, AppResult};
use crate::settings::config::{self, Settings};

/// Stateless access point for persisted application settings.
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsService;

/// Loads persisted settings, or returns defaults when no config exists.
pub fn load_or_default() -> AppResult<Settings> {
    SettingsService::load_or_default()
}

/// Persists the provided settings using the current config backend.
pub fn save(settings: &Settings) -> AppResult<()> {
    SettingsService::save(settings)
}

/// Returns the resolved config file path for the current environment.
#[allow(dead_code)]
pub fn config_path() -> AppResult<PathBuf> {
    SettingsService::config_path()
}

/// Returns the default settings used when no persisted config exists.
#[allow(dead_code)]
pub fn default_settings() -> Settings {
    SettingsService::default_settings()
}

impl SettingsService {
    /// Loads persisted settings, or returns defaults when no config exists.
    ///
    /// Invalid config files are handled by the underlying backend, which backs
    /// them up and restores default settings.
    pub fn load_or_default() -> AppResult<Settings> {
        Settings::load_or_default().map_err(AppError::settings)
    }

    /// Persists the provided settings using the current config backend.
    pub fn save(settings: &Settings) -> AppResult<()> {
        settings.save().map_err(AppError::settings)
    }

    /// Returns the resolved config file path for the current environment.
    #[allow(dead_code)]
    pub fn config_path() -> AppResult<PathBuf> {
        config::config_path().map_err(AppError::settings)
    }

    /// Returns the default settings used when no persisted config exists.
    #[allow(dead_code)]
    pub fn default_settings() -> Settings {
        Settings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{config_path, default_settings, SettingsService};

    #[test]
    fn default_settings_matches_backend_defaults() {
        assert_eq!(
            default_settings().delay_ms,
            crate::settings::config::Settings::default().delay_ms
        );
        assert_eq!(
            default_settings().default_capture_mode,
            crate::settings::config::Settings::default().default_capture_mode
        );
        assert_eq!(
            default_settings().filename_template,
            crate::settings::config::Settings::default().filename_template
        );
    }

    #[test]
    fn config_path_matches_backend_resolution() {
        assert_eq!(
            config_path().unwrap(),
            crate::settings::config::config_path().unwrap()
        );
    }

    #[test]
    fn service_type_and_free_functions_share_the_same_defaults() {
        assert_eq!(
            SettingsService::default_settings().default_save_location,
            default_settings().default_save_location
        );
    }
}
