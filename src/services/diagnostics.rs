use crate::services::app::{AppError, AppResult, PortalRepairOutcome};
use crate::settings::config::{CaptureMode, Settings};

pub type MissingDependenciesError = crate::diagnostics::MissingDependenciesError;

/// Returns the current doctor report for the active environment.
pub fn doctor_report() -> String {
    crate::diagnostics::doctor_report()
}

/// Verifies that the requested capture mode can run with the current settings.
pub fn check_capture_requirements(mode: CaptureMode, settings: &Settings) -> AppResult<()> {
    crate::diagnostics::check_capture_requirements(mode, settings).map_err(AppError::diagnostics)
}

/// Returns a portal warning message suitable for startup UI surfaces.
pub fn portal_startup_warning() -> Option<String> {
    crate::diagnostics::portal_startup_warning()
}

/// Attempts to repair missing portal services for the current user session.
pub fn repair_portals() -> PortalRepairOutcome {
    crate::diagnostics::repair_portals()
}

pub fn missing_dependencies(error: &AppError) -> Option<&MissingDependenciesError> {
    error.missing_dependencies()
}
