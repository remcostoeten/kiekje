use crate::settings::config::{CaptureMode, Settings};
use anyhow::Result;
use std::env;
use std::fmt::{self, Display, Formatter};
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

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
    let plain = doctor_report_with_path(env::var_os("PATH"));
    if doctor_colors_enabled() {
        colorize_doctor_report(&plain)
    } else {
        plain
    }
}

pub fn portal_startup_warning() -> Option<String> {
    let mut missing = Vec::new();
    let mut hints = Vec::new();

    match portal_state(env::var_os("PATH")) {
        PortalState::Available(interfaces) => {
            for interface in [
                PortalInterface::Settings,
                PortalInterface::Inhibit,
                PortalInterface::Screenshot,
            ] {
                if !interfaces.contains(interface.name()) {
                    missing.push(interface.label());
                    if let Some(hint) = portal_hint(interface) {
                        hints.push(hint);
                    }
                }
            }
        }
        PortalState::BrokerUnavailable(message) => {
            return Some(format!(
                "Desktop portal broker unavailable: {message}\nInstall/start xdg-desktop-portal for this session."
            ));
        }
        PortalState::IntrospectionUnavailable(_) => return None,
    }

    if missing.is_empty() {
        return None;
    }

    hints.sort();
    hints.dedup();

    let mut body = format!(
        "Desktop portal interfaces missing: {}.\nGTK may print portal warnings during startup.",
        missing.join(", ")
    );

    if !hints.is_empty() {
        body.push_str("\nSuggested fix: ");
        body.push_str(&hints.join(" "));
    }

    Some(body)
}

#[derive(Debug, Clone)]
pub struct PortalRepairResult {
    pub attempted: bool,
    pub repaired: bool,
    pub message: String,
}

pub fn auto_repair_portals() -> PortalRepairResult {
    repair_portals_with_path(env::var_os("PATH"))
}

pub fn repair_portals() -> PortalRepairResult {
    repair_portals_with_path(env::var_os("PATH"))
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

    out.push('\n');
    out.push_str("Desktop Portal Report\n");
    out.push_str("---------------------\n");
    append_portal_report(&mut out, path);

    out
}

fn append_portal_report(out: &mut String, path: Option<std::ffi::OsString>) {
    match portal_state(path) {
        PortalState::Available(interfaces) => {
            for interface in [
                PortalInterface::Settings,
                PortalInterface::Inhibit,
                PortalInterface::Screenshot,
            ] {
                if interfaces.contains(interface.name()) {
                    out.push_str(&format!(
                        "[OK]   {:10} - portal interface available\n",
                        interface.label()
                    ));
                } else {
                    out.push_str(&format!(
                        "[MISS] {:10} - portal interface unavailable\n",
                        interface.label()
                    ));
                    if let Some(hint) = portal_hint(interface) {
                        out.push_str(&format!("       Hint: {hint}\n"));
                    }
                }
            }
        }
        PortalState::BrokerUnavailable(message) => {
            out.push_str(&format!("[MISS] broker      - {message}\n"));
            out.push_str(
                "       Hint: start an xdg-desktop-portal service for your desktop session.\n",
            );
        }
        PortalState::IntrospectionUnavailable(message) => {
            out.push_str(&format!("[WARN] introspect  - {message}\n"));
            out.push_str(
                "       Hint: install GLib's `gdbus` tool to inspect portal interfaces.\n",
            );
        }
    }
}

fn doctor_colors_enabled() -> bool {
    std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none()
}

fn colorize_doctor_report(report: &str) -> String {
    let mut out = String::new();

    for line in report.lines() {
        let colored = if line == "Kiekje Doctor Report" || line == "Desktop Portal Report" {
            format!("\x1b[1;36m{line}\x1b[0m")
        } else if line.starts_with("[OK]") {
            format!("\x1b[32m{}\x1b[0m", line.replacen("[OK]", "[OK]", 1))
        } else if line.starts_with("[MISS]") {
            format!("\x1b[33m{}\x1b[0m", line.replacen("[MISS]", "[MISS]", 1))
        } else if line.starts_with("[WARN]") {
            format!("\x1b[35m{}\x1b[0m", line.replacen("[WARN]", "[WARN]", 1))
        } else if line.trim_start().starts_with("Install:")
            || line.trim_start().starts_with("Hint:")
        {
            format!("\x1b[2m{line}\x1b[0m")
        } else {
            line.to_string()
        };

        out.push_str(&colored);
        out.push('\n');
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortalInterface {
    Settings,
    Inhibit,
    Screenshot,
}

#[derive(Debug, Clone)]
struct PortalRepairPlan {
    services: Vec<&'static str>,
}

impl PortalRepairPlan {
    fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

impl PortalInterface {
    fn name(self) -> &'static str {
        match self {
            PortalInterface::Settings => "org.freedesktop.portal.Settings",
            PortalInterface::Inhibit => "org.freedesktop.portal.Inhibit",
            PortalInterface::Screenshot => "org.freedesktop.portal.Screenshot",
        }
    }

    fn label(self) -> &'static str {
        match self {
            PortalInterface::Settings => "Settings",
            PortalInterface::Inhibit => "Inhibit",
            PortalInterface::Screenshot => "Screenshot",
        }
    }
}

enum PortalState {
    Available(String),
    BrokerUnavailable(String),
    IntrospectionUnavailable(String),
}

fn portal_state(path: Option<std::ffi::OsString>) -> PortalState {
    if !is_available_with_path("gdbus", path) {
        return PortalState::IntrospectionUnavailable("`gdbus` not found in PATH".to_string());
    }

    let output = Command::new("gdbus")
        .args([
            "introspect",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            PortalState::Available(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(output) => PortalState::BrokerUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ),
        Err(err) => PortalState::BrokerUnavailable(err.to_string()),
    }
}

fn repair_portals_with_path(path: Option<std::ffi::OsString>) -> PortalRepairResult {
    let initial_state = portal_state(path.clone());
    let initial_missing = missing_portal_interfaces(&initial_state);

    if initial_missing.is_empty() {
        return PortalRepairResult {
            attempted: false,
            repaired: true,
            message: "Desktop portal interfaces are already available.".to_string(),
        };
    }

    let plan = plan_portal_repair(path.clone(), &initial_state);
    if plan.is_empty() {
        let mut message = portal_problem_summary(&initial_state);
        let hints = portal_repair_hints(&initial_missing);
        if !hints.is_empty() {
            message.push('\n');
            message.push_str("Suggested fix: ");
            message.push_str(&hints.join(" "));
        }

        return PortalRepairResult {
            attempted: false,
            repaired: false,
            message,
        };
    }

    if let Err(err) = restart_user_services(path.clone(), &plan.services) {
        return PortalRepairResult {
            attempted: true,
            repaired: false,
            message: format!(
                "Automatic portal repair failed while restarting {}: {}",
                plan.services.join(", "),
                err
            ),
        };
    }

    thread::sleep(Duration::from_millis(900));
    let repaired_state = portal_state(path);
    let repaired_missing = missing_portal_interfaces(&repaired_state);
    if repaired_missing.is_empty() {
        return PortalRepairResult {
            attempted: true,
            repaired: true,
            message: format!(
                "Restarted {} and restored the required desktop portal interfaces.",
                plan.services.join(", ")
            ),
        };
    }

    let mut message = format!(
        "Restarted {}, but some desktop portal interfaces are still missing.",
        plan.services.join(", ")
    );
    let hints = portal_repair_hints(&repaired_missing);
    if !hints.is_empty() {
        message.push('\n');
        message.push_str("Suggested fix: ");
        message.push_str(&hints.join(" "));
    }

    PortalRepairResult {
        attempted: true,
        repaired: false,
        message,
    }
}

fn missing_portal_interfaces(state: &PortalState) -> Vec<PortalInterface> {
    match state {
        PortalState::Available(interfaces) => [
            PortalInterface::Settings,
            PortalInterface::Inhibit,
            PortalInterface::Screenshot,
        ]
        .into_iter()
        .filter(|interface| !interfaces.contains(interface.name()))
        .collect(),
        PortalState::BrokerUnavailable(_) => vec![
            PortalInterface::Settings,
            PortalInterface::Inhibit,
            PortalInterface::Screenshot,
        ],
        PortalState::IntrospectionUnavailable(_) => Vec::new(),
    }
}

fn plan_portal_repair(path: Option<std::ffi::OsString>, state: &PortalState) -> PortalRepairPlan {
    if matches!(state, PortalState::IntrospectionUnavailable(_))
        || !is_available_with_path("systemctl", path)
    {
        return PortalRepairPlan {
            services: Vec::new(),
        };
    }

    let missing = missing_portal_interfaces(state);
    if missing.is_empty() {
        return PortalRepairPlan {
            services: Vec::new(),
        };
    }

    let mut services = vec!["xdg-desktop-portal.service"];
    if missing.iter().any(|interface| {
        matches!(
            interface,
            PortalInterface::Settings | PortalInterface::Inhibit
        )
    }) && portal_backend_installed("gtk")
    {
        services.push("xdg-desktop-portal-gtk.service");
    }

    if missing.contains(&PortalInterface::Screenshot) && portal_backend_installed("hyprland") {
        services.push("xdg-desktop-portal-hyprland.service");
    }

    services.sort_unstable();
    services.dedup();

    PortalRepairPlan { services }
}

fn restart_user_services(
    path: Option<std::ffi::OsString>,
    services: &[&str],
) -> std::result::Result<(), String> {
    let output = command_with_path("systemctl", path)?
        .arg("--user")
        .arg("restart")
        .args(services)
        .output()
        .map_err(|err| err.to_string())?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("systemctl exited with status {}", output.status))
    } else {
        Err(stderr)
    }
}

fn portal_problem_summary(state: &PortalState) -> String {
    match state {
        PortalState::Available(interfaces) => {
            let missing: Vec<&str> = [
                PortalInterface::Settings,
                PortalInterface::Inhibit,
                PortalInterface::Screenshot,
            ]
            .into_iter()
            .filter(|interface| !interfaces.contains(interface.name()))
            .map(|interface| interface.label())
            .collect();

            format!(
                "Desktop portal interfaces missing: {}.\nGTK may print portal warnings during startup.",
                missing.join(", ")
            )
        }
        PortalState::BrokerUnavailable(message) => format!(
            "Desktop portal broker unavailable: {message}\nInstall/start xdg-desktop-portal for this session."
        ),
        PortalState::IntrospectionUnavailable(message) => {
            format!("Desktop portal introspection unavailable: {message}")
        }
    }
}

fn portal_repair_hints(missing: &[PortalInterface]) -> Vec<String> {
    let mut hints = Vec::new();
    for interface in missing {
        if let Some(hint) = portal_hint(*interface) {
            hints.push(hint);
        }
    }
    hints.sort();
    hints.dedup();
    hints
}

fn portal_hint(interface: PortalInterface) -> Option<String> {
    match interface {
        PortalInterface::Settings | PortalInterface::Inhibit => {
            if !portal_backend_installed("gtk") {
                return install_gtk_portal_hint();
            }

            Some("restart the user portal services or log out and back in".to_string())
        }
        PortalInterface::Screenshot => {
            if portal_backend_installed("hyprland") {
                Some(
                    "the Hyprland portal backend is installed, but the broker is not exporting Screenshot; restart `xdg-desktop-portal` and `xdg-desktop-portal-hyprland` or re-login".to_string(),
                )
            } else {
                Some(
                    "install a screenshot-capable portal backend for your desktop session"
                        .to_string(),
                )
            }
        }
    }
}

fn portal_backend_installed(name: &str) -> bool {
    let path = format!("/usr/share/xdg-desktop-portal/portals/{name}.portal");
    Path::new(&path).exists()
}

fn install_gtk_portal_hint() -> Option<String> {
    let package = "xdg-desktop-portal-gtk";
    let path = env::var_os("PATH");

    if is_available_with_path("pacman", path.clone()) {
        return Some(format!(
            "install `{package}` with `sudo pacman -S {package}`"
        ));
    }
    if is_available_with_path("apt-get", path.clone()) {
        return Some(format!(
            "install `{package}` with `sudo apt-get install -y {package}`"
        ));
    }
    if is_available_with_path("dnf", path.clone()) {
        return Some(format!(
            "install `{package}` with `sudo dnf install -y {package}`"
        ));
    }
    if is_available_with_path("zypper", path) {
        return Some(format!(
            "install `{package}` with `sudo zypper install {package}`"
        ));
    }

    Some(format!("install the `{package}` backend"))
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

fn command_with_path(
    tool: &str,
    path: Option<std::ffi::OsString>,
) -> std::result::Result<Command, String> {
    if tool.contains('/') {
        if Path::new(tool).is_file() {
            return Ok(Command::new(tool));
        }
        return Err(format!("{tool} not found"));
    }

    let Some(path) = path else {
        return Err(format!("{tool} not found in PATH"));
    };

    for dir in env::split_paths(&path) {
        let candidate = dir.join(tool);
        if candidate.is_file() {
            return Ok(Command::new(candidate));
        }
    }

    Err(format!("{tool} not found in PATH"))
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
    use super::{
        check_capture_requirements_with_path, doctor_report_with_path, missing_portal_interfaces,
        plan_portal_repair, PortalInterface, PortalState,
    };
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
        assert!(report.contains("Desktop Portal Report"));
        assert!(report.contains("[WARN] introspect"));
    }

    #[test]
    fn missing_portal_interfaces_detects_absent_members() {
        let missing = missing_portal_interfaces(&PortalState::Available(
            "org.freedesktop.portal.Settings".to_string(),
        ));
        assert_eq!(
            missing,
            vec![PortalInterface::Inhibit, PortalInterface::Screenshot]
        );
    }

    #[test]
    fn repair_plan_is_empty_without_systemctl_in_path() {
        let plan = plan_portal_repair(Some("".into()), &PortalState::Available(String::new()));
        assert!(plan.services.is_empty());
    }
}
