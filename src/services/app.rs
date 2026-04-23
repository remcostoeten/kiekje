use crate::diagnostics::{MissingDependenciesError, PortalRepairResult};
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    MissingDependencies(MissingDependenciesError),
    CaptureCanceled,
    Settings { details: String },
    Clipboard { details: String },
    Save { details: String },
    Capture { details: String },
    Diagnostics { details: String },
    Launch { details: String },
    Unexpected { details: String },
}

impl AppError {
    pub fn missing_dependencies(&self) -> Option<&MissingDependenciesError> {
        match self {
            Self::MissingDependencies(missing) => Some(missing),
            _ => None,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingDependencies(_) => "KIEKJE-E001",
            Self::CaptureCanceled => "KIEKJE-E002",
            Self::Settings { .. } => "KIEKJE-E101",
            Self::Clipboard { .. } => "KIEKJE-E102",
            Self::Save { .. } => "KIEKJE-E103",
            Self::Capture { .. } => "KIEKJE-E104",
            Self::Diagnostics { .. } => "KIEKJE-E105",
            Self::Launch { .. } => "KIEKJE-E106",
            Self::Unexpected { .. } => "KIEKJE-E999",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::MissingDependencies(_) => "Missing Dependencies",
            Self::CaptureCanceled => "Capture Canceled",
            Self::Settings { .. } => "Settings Error",
            Self::Clipboard { .. } => "Clipboard Error",
            Self::Save { .. } => "Save Error",
            Self::Capture { .. } => "Capture Failed",
            Self::Diagnostics { .. } => "Diagnostics Error",
            Self::Launch { .. } => "Launch Error",
            Self::Unexpected { .. } => "Unexpected Error",
        }
    }

    pub fn details(&self) -> &str {
        match self {
            Self::MissingDependencies(_) => "",
            Self::CaptureCanceled => "",
            Self::Settings { details }
            | Self::Clipboard { details }
            | Self::Save { details }
            | Self::Capture { details }
            | Self::Diagnostics { details }
            | Self::Launch { details }
            | Self::Unexpected { details } => details,
        }
    }

    pub fn feedback_body(&self) -> String {
        match self {
            Self::MissingDependencies(missing) => format_missing_dependencies(missing),
            Self::CaptureCanceled => {
                "The capture was canceled before a screenshot was produced.".to_string()
            }
            _ => self.details().to_string(),
        }
    }

    pub fn from_capture(err: anyhow::Error) -> Self {
        if let Some(missing) = err.downcast_ref::<MissingDependenciesError>() {
            return Self::MissingDependencies(MissingDependenciesError {
                items: missing.items.clone(),
            });
        }
        if err.to_string().contains("capture canceled") {
            return Self::CaptureCanceled;
        }
        Self::Capture {
            details: format!("{err:#}"),
        }
    }

    pub fn settings(err: anyhow::Error) -> Self {
        Self::Settings {
            details: format!("{err:#}"),
        }
    }

    pub fn clipboard(err: anyhow::Error) -> Self {
        Self::Clipboard {
            details: format!("{err:#}"),
        }
    }

    pub fn save(err: anyhow::Error) -> Self {
        Self::Save {
            details: format!("{err:#}"),
        }
    }

    pub fn diagnostics(err: anyhow::Error) -> Self {
        if let Some(missing) = err.downcast_ref::<MissingDependenciesError>() {
            return Self::MissingDependencies(MissingDependenciesError {
                items: missing.items.clone(),
            });
        }
        Self::Diagnostics {
            details: format!("{err:#}"),
        }
    }

    pub fn launch(err: anyhow::Error) -> Self {
        Self::Launch {
            details: format!("{err:#}"),
        }
    }

    pub fn unexpected(err: anyhow::Error) -> Self {
        Self::Unexpected {
            details: format!("{err:#}"),
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDependencies(_) => write!(f, "missing required dependencies"),
            Self::CaptureCanceled => write!(f, "capture canceled"),
            _ => write!(f, "{}", self.title()),
        }
    }
}

impl StdError for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Unexpected {
            details: value.to_string(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::unexpected(value)
    }
}

fn format_missing_dependencies(missing: &MissingDependenciesError) -> String {
    let mut body = String::from("Kiekje cannot continue because required tools are missing.\n\n");
    for item in &missing.items {
        body.push_str(&format!("- {} ({})\n", item.tool, item.required_for));
        if let Some(cmd) = &item.install_command {
            body.push_str(&format!("  Install: {}\n", cmd));
        }
        if let Some(workaround) = &item.workaround {
            body.push_str(&format!("  Option: {}\n", workaround));
        }
    }
    body.push_str("\nRun `kiekje --doctor` for the full readiness report.");
    body
}

pub type PortalRepairOutcome = PortalRepairResult;
