use crate::settings::config::{CaptureMode, Settings};
use anyhow::{Error, Result};

pub type MissingDependenciesError = crate::diagnostics::MissingDependenciesError;
pub type PortalRepairResult = crate::diagnostics::PortalRepairResult;

/// Returns the current doctor report for the active environment.
pub fn doctor_report() -> String {
    crate::diagnostics::doctor_report()
}

/// Verifies that the requested capture mode can run with the current settings.
pub fn check_capture_requirements(mode: CaptureMode, settings: &Settings) -> Result<()> {
    crate::diagnostics::check_capture_requirements(mode, settings)
}

/// Returns a portal warning message suitable for startup UI surfaces.
pub fn portal_startup_warning() -> Option<String> {
    crate::diagnostics::portal_startup_warning()
}

/// Attempts to repair missing portal services for the current user session.
pub fn repair_portals() -> PortalRepairResult {
    crate::diagnostics::repair_portals()
}

/// Extracts a typed missing-dependencies payload from a diagnostics error.
pub fn missing_dependencies(error: &Error) -> Option<&MissingDependenciesError> {
    error.downcast_ref::<MissingDependenciesError>()
}
