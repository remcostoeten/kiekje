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
    let requirements = capture_requirements(mode, settings);
    let mut missing = Vec::new();

    for req in requirements {
        if !is_available(req.tool) {
            missing.push(MissingDependency {
                tool: req.tool.to_string(),
                required_for: req.required_for.to_string(),
                install_command: install_command_for(req.tool),
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
    let checks = [
        ("grim", "capture backend"),
        ("slurp", "region selection"),
        ("wl-copy", "clipboard copy"),
        ("hyprctl", "Hyprland active-window capture"),
    ];

    let mut out = String::new();
    out.push_str("Screeny Doctor Report\n");
    out.push_str("====================\n");

    for (tool, purpose) in checks {
        if is_available(tool) {
            out.push_str(&format!("[OK]   {:8} - {}\n", tool, purpose));
        } else {
            out.push_str(&format!("[MISS] {:8} - {}\n", tool, purpose));
            if let Some(cmd) = install_command_for(tool) {
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
        CaptureMode::Region => requirements.push(Requirement {
            tool: "slurp",
            required_for: "region capture mode",
            workaround: Some("use `capture-app fullscreen`"),
        }),
        CaptureMode::Fullscreen => {}
        CaptureMode::Window => requirements.push(Requirement {
            tool: "hyprctl",
            required_for: "window capture mode (Hyprland active window)",
            workaround: Some("use `capture-app fullscreen` or `capture-app region`"),
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

fn is_available(tool: &str) -> bool {
    if tool.contains('/') {
        return std::path::Path::new(tool).is_file();
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|dir| dir.join(tool).is_file())
}

fn install_command_for(tool: &str) -> Option<String> {
    let package = map_package(tool);

    if is_available("pacman") {
        return Some(format!("sudo pacman -S {}", package));
    }
    if is_available("apt-get") {
        return Some(format!("sudo apt-get install -y {}", package));
    }
    if is_available("dnf") {
        return Some(format!("sudo dnf install -y {}", package));
    }
    if is_available("zypper") {
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
