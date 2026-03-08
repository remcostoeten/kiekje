use crate::settings::config::{CaptureMode, Settings};
use anyhow::Result;
use std::env;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
pub struct MissingDependency {
    pub tool: String,
    pub required_for: String,
    pub install_command: Option<String>,
    pub workaround: Option<String>,
}

#[derive(Debug)]
pub struct MissingDependenciesError {
    pub items: Vec<MissingDependency>,
}

impl Display for MissingDependenciesError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "missing required system dependencies")
    }
}

impl std::error::Error for MissingDependenciesError {}

#[derive(Debug, Clone)]
struct Requirement {
    tool: &'static str,
    required_for: &'static str,
    workaround: Option<&'static str>,
}

pub fn check_capture_requirements(mode: CaptureMode, settings: &Settings) -> Result<()> {
    check_capture_requirements_with_path(mode, settings, env::var_os("PATH"))
}

pub(crate) fn check_capture_requirements_with_path(
    mode: CaptureMode,
    settings: &Settings,
    path: Option<std::ffi::OsString>,
) -> Result<()> {
    let requirements = capture_requirements(mode, settings);
    let mut missing = Vec::new();

    for req in requirements {
        if !is_available_with_path(req.tool, path.clone()) {
            missing.push(MissingDependency {
                tool: req.tool.to_string(),
                required_for: req.required_for.to_string(),
                install_command: install_command_for_with_path(req.tool, path.clone()),
                workaround: req.workaround.map(|w| w.to_string()),
            });
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(MissingDependenciesError { items: missing }.into())
    }
}

pub fn doctor_report() -> String {
    doctor_report_with_path(env::var_os("PATH"))
}

pub(crate) fn doctor_report_with_path(path: Option<std::ffi::OsString>) -> String {
    let checks = [
        ("grim", "capture backend"),
        ("wl-copy", "clipboard copy"),
        ("hyprctl", "Hyprland active-window capture"),
    ];

    let mut out = String::new();
    out.push_str("Kiekje Doctor Report\n");
    out.push_str("====================\n");

    for (tool, purpose) in checks {
        if is_available_with_path(tool, path.clone()) {
            out.push_str(&format!("[OK]   {:8} - {}\n", tool, purpose));
        } else {
            out.push_str(&format!("[MISS] {:8} - {}\n", tool, purpose));
            if let Some(cmd) = install_command_for_with_path(tool, path.clone()) {
                out.push_str(&format!("       Install: {}\n", cmd));
            }
        }
    }

    out
}

fn capture_requirements(mode: CaptureMode, settings: &Settings) -> Vec<Requirement> {
    let mut requirements = vec![Requirement {
        tool: "grim",
        required_for: "all capture modes",
        workaround: None,
    }];

    match mode {
        CaptureMode::Region => {}
        CaptureMode::Fullscreen => {}
        CaptureMode::Window => requirements.push(Requirement {
            tool: "hyprctl",
            required_for: "window capture mode (Hyprland active window)",
            workaround: Some("use `kiekje fullscreen` or `kiekje region`"),
        }),
    }

    if settings.copy_to_clipboard {
        requirements.push(Requirement {
            tool: "wl-copy",
            required_for: "copy_to_clipboard=true",
            workaround: Some("set `copy_to_clipboard` to `false` in config"),
        });
    }

    requirements
}

fn is_available_with_path(tool: &str, path: Option<std::ffi::OsString>) -> bool {
    if tool.contains('/') {
        return std::path::Path::new(tool).is_file();
    }

    let Some(path) = path else {
        return false;
    };

    env::split_paths(&path).any(|dir| dir.join(tool).is_file())
}

fn install_command_for_with_path(tool: &str, path: Option<std::ffi::OsString>) -> Option<String> {
    let package = map_package(tool);

    if is_available_with_path("pacman", path.clone()) {
        return Some(format!("sudo pacman -S {}", package));
    }
    if is_available_with_path("apt-get", path.clone()) {
        return Some(format!("sudo apt-get install -y {}", package));
    }
    if is_available_with_path("dnf", path.clone()) {
        return Some(format!("sudo dnf install -y {}", package));
    }
    if is_available_with_path("zypper", path) {
        return Some(format!("sudo zypper install {}", package));
    }

    None
}

fn map_package(tool: &str) -> String {
    match tool {
        "wl-copy" => "wl-clipboard".to_string(),
        "hyprctl" => "hyprland".to_string(),
        _ => tool.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{check_capture_requirements_with_path, doctor_report_with_path};
    use crate::settings::config::{CaptureMode, Settings};

    #[test]
    fn reports_mode_specific_missing_dependencies() {
        let settings = Settings {
            copy_to_clipboard: true,
            ..Settings::default()
        };

        let err =
            check_capture_requirements_with_path(CaptureMode::Window, &settings, Some("".into()))
                .unwrap_err();
        let missing = err
            .downcast_ref::<super::MissingDependenciesError>()
            .expect("expected MissingDependenciesError");

        let tools: Vec<&str> = missing.items.iter().map(|x| x.tool.as_str()).collect();
        assert!(tools.contains(&"grim"));
        assert!(tools.contains(&"hyprctl"));
        assert!(tools.contains(&"wl-copy"));
        assert!(!tools.contains(&"slurp"));
    }

    #[test]
    fn doctor_report_marks_missing_when_path_is_empty() {
        let report = doctor_report_with_path(Some("".into()));
        assert!(report.contains("[MISS] grim"));
        assert!(report.contains("[MISS] wl-copy"));
        assert!(report.contains("[MISS] hyprctl"));
    }
}
