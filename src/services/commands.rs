//! Serializable command surface for future Tauri IPC bindings.
#![allow(dead_code)]

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::services::app::{AppError, AppResult};
use crate::services::{capture as capture_service, diagnostics, settings as settings_service};
use crate::settings::config::{CaptureMode, Settings};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommandRequest {
    pub mode: CaptureMode,
    #[serde(default)]
    pub settings: Option<Settings>,
    #[serde(default)]
    pub include_png: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommandResponse {
    pub mode: CaptureMode,
    pub png_base64: Option<String>,
    pub png_byte_len: usize,
    pub copied_to_clipboard: bool,
    pub saved_path: Option<String>,
    pub backend: CaptureBackendInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBackendInfo {
    pub platform: String,
    pub backend: String,
    pub region_supported: bool,
    pub fullscreen_supported: bool,
    pub window_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub title: String,
    pub message: String,
    pub missing_dependencies: Vec<MissingDependencyInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MissingDependencyInfo {
    pub tool: String,
    pub required_for: String,
    pub install_command: Option<String>,
    pub workaround: Option<String>,
}

pub type CommandResult<T> = Result<T, CommandError>;

pub fn capture(request: CaptureCommandRequest) -> CommandResult<CaptureCommandResponse> {
    capture_inner(request).map_err(CommandError::from)
}

pub fn load_settings() -> CommandResult<Settings> {
    settings_service::load_or_default().map_err(CommandError::from)
}

pub fn save_settings(settings: Settings) -> CommandResult<()> {
    settings_service::save(&settings).map_err(CommandError::from)
}

pub fn default_settings() -> Settings {
    settings_service::default_settings()
}

pub fn doctor_report() -> String {
    diagnostics::doctor_report()
}

pub fn capture_backend_info() -> CaptureBackendInfo {
    CaptureBackendInfo {
        platform: "linux".to_string(),
        backend: "grim-hyprland".to_string(),
        region_supported: true,
        fullscreen_supported: true,
        window_supported: true,
    }
}

fn capture_inner(request: CaptureCommandRequest) -> AppResult<CaptureCommandResponse> {
    let settings = match request.settings {
        Some(settings) => settings,
        None => settings_service::load_or_default()?,
    };
    let include_png = request.include_png.unwrap_or(true);
    let execution = capture_service::run(request.mode, &settings)?;
    let png_base64 = include_png.then(|| STANDARD.encode(&execution.capture.png_data));

    Ok(CaptureCommandResponse {
        mode: execution.mode,
        png_base64,
        png_byte_len: execution.capture.png_data.len(),
        copied_to_clipboard: execution.copied_to_clipboard,
        saved_path: execution
            .saved_path
            .as_ref()
            .map(|path| path.display().to_string()),
        backend: capture_backend_info(),
    })
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        let missing_dependencies = error
            .missing_dependencies()
            .map(|missing| {
                missing
                    .items
                    .iter()
                    .map(|item| MissingDependencyInfo {
                        tool: item.tool.clone(),
                        required_for: item.required_for.clone(),
                        install_command: item.install_command.clone(),
                        workaround: item.workaround.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            code: error.code().to_string(),
            title: error.title().to_string(),
            message: error.feedback_body(),
            missing_dependencies,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{capture_backend_info, CommandError};
    use crate::diagnostics::{MissingDependenciesError, MissingDependency};
    use crate::services::app::AppError;
    use crate::settings::config::CaptureMode;

    #[test]
    fn reports_backend_capabilities_for_current_platform() {
        let info = capture_backend_info();

        assert!(!info.platform.is_empty());
        assert!(!info.backend.is_empty());
        assert!(info.region_supported);
    }

    #[test]
    fn command_error_preserves_missing_dependency_details() {
        let error = AppError::MissingDependencies(MissingDependenciesError {
            items: vec![MissingDependency {
                tool: "grim".to_string(),
                required_for: "capture backend".to_string(),
                install_command: Some("pacman -S grim".to_string()),
                workaround: None,
            }],
        });

        let command_error = CommandError::from(error);

        assert_eq!(command_error.code, "KIEKJE-E001");
        assert_eq!(command_error.missing_dependencies.len(), 1);
        assert_eq!(command_error.missing_dependencies[0].tool, "grim");
    }
}
