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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxSessionDetails {
    pub session_type: Option<String>,
    pub current_desktop: Option<String>,
    pub desktop_session: Option<String>,
    pub wayland_display: Option<String>,
    pub hyprland_instance_signature: bool,
}

impl LinuxSessionDetails {
    fn detect() -> Self {
        Self {
            session_type: env::var("XDG_SESSION_TYPE").ok(),
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok(),
            desktop_session: env::var("DESKTOP_SESSION").ok(),
            wayland_display: env::var("WAYLAND_DISPLAY").ok(),
            hyprland_instance_signature: env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some(),
        }
    }

    fn is_wayland(&self) -> bool {
        self.session_type
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
            || self.wayland_display.is_some()
    }

    pub fn compositor_label(&self) -> Option<String> {
        if self.hyprland_instance_signature {
            return Some("Hyprland".to_string());
        }

        self.current_desktop
            .as_deref()
            .and_then(parse_compositor_name)
            .or_else(|| {
                self.desktop_session
                    .as_deref()
                    .and_then(parse_compositor_name)
            })
    }

    pub fn session_summary(&self) -> String {
        let session = self.session_type.as_deref().unwrap_or("unknown");
        let display = self.wayland_display.as_deref().unwrap_or("unset");
        format!("{session} (WAYLAND_DISPLAY={display})")
    }

    fn supports_active_window_capture(&self) -> bool {
        self.is_wayland()
            && self
                .compositor_label()
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("Hyprland"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedEnvironmentReason {
    NonWaylandSession,
    ActiveWindowCaptureRequiresHyprland,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedEnvironmentError {
    pub mode: CaptureMode,
    pub reason: UnsupportedEnvironmentReason,
    pub environment: LinuxSessionDetails,
    pub workaround: Option<String>,
}

impl UnsupportedEnvironmentError {
    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            CaptureMode::Region => "region",
            CaptureMode::Fullscreen => "fullscreen",
            CaptureMode::Window => "window",
        }
    }

    pub fn reason_label(&self) -> &'static str {
        match self.reason {
            UnsupportedEnvironmentReason::NonWaylandSession => {
                "grim-based capture requires an active Wayland session"
            }
            UnsupportedEnvironmentReason::ActiveWindowCaptureRequiresHyprland => {
                "active-window capture currently requires Hyprland"
            }
        }
    }
}

impl Display for UnsupportedEnvironmentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported Linux capture environment")
    }
}

impl std::error::Error for UnsupportedEnvironmentError {}

#[derive(Debug, Clone)]
struct Requirement {
    tool: &'static str,
    required_for: &'static str,
    workaround: Option<&'static str>,
}

pub fn check_capture_requirements(mode: CaptureMode, settings: &Settings) -> Result<()> {
    check_capture_requirements_with_path_and_session(
        mode,
        settings,
        env::var_os("PATH"),
        LinuxSessionDetails::detect(),
    )
}

#[allow(dead_code)]
pub(crate) fn check_capture_requirements_with_path(
    mode: CaptureMode,
    settings: &Settings,
    path: Option<std::ffi::OsString>,
) -> Result<()> {
    check_capture_requirements_with_path_and_session(
        mode,
        settings,
        path,
        LinuxSessionDetails::detect(),
    )
}

pub(crate) fn check_capture_requirements_with_path_and_session(
    mode: CaptureMode,
    settings: &Settings,
    path: Option<std::ffi::OsString>,
    session: LinuxSessionDetails,
) -> Result<()> {
    if let Some(err) = capture_environment_error(mode, &session) {
        return Err(err.into());
    }

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
    let plain =
        doctor_report_with_path_and_session(env::var_os("PATH"), LinuxSessionDetails::detect());
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

pub fn repair_portals() -> PortalRepairResult {
    repair_portals_with_path(env::var_os("PATH"))
}

#[allow(dead_code)]
pub(crate) fn doctor_report_with_path(path: Option<std::ffi::OsString>) -> String {
    doctor_report_with_path_and_session(path, LinuxSessionDetails::detect())
}

pub(crate) fn doctor_report_with_path_and_session(
    path: Option<std::ffi::OsString>,
    session: LinuxSessionDetails,
) -> String {
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
    out.push_str("Linux Session Report\n");
    out.push_str("--------------------\n");
    append_session_report(&mut out, &session);

    out.push('\n');
    out.push_str("Desktop Portal Report\n");
    out.push_str("---------------------\n");
    append_portal_report(&mut out, path);

    out
}

fn append_session_report(out: &mut String, session: &LinuxSessionDetails) {
    if session.is_wayland() {
        out.push_str("[OK]   session     - Wayland session detected\n");
    } else {
        out.push_str("[MISS] session     - Wayland session not detected; grim-based capture is unavailable\n");
    }

    let compositor = session
        .compositor_label()
        .unwrap_or_else(|| "unknown".to_string());
    out.push_str(&format!(
        "[{}] compositor  - {}\n",
        if session.supports_active_window_capture() {
            "OK"
        } else {
            "WARN"
        },
        compositor
    ));

    if session.supports_active_window_capture() {
        out.push_str("[OK]   window mode  - active-window capture is supported\n");
    } else {
        out.push_str("[WARN] window mode  - active-window capture currently requires Hyprland\n");
    }
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
            format!("\x1b[32m{line}\x1b[0m")
        } else if line.starts_with("[MISS]") {
            format!("\x1b[33m{line}\x1b[0m")
        } else if line.starts_with("[WARN]") {
            format!("\x1b[35m{line}\x1b[0m")
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

fn capture_environment_error(
    mode: CaptureMode,
    session: &LinuxSessionDetails,
) -> Option<UnsupportedEnvironmentError> {
    if !session.is_wayland() {
        return Some(UnsupportedEnvironmentError {
            mode,
            reason: UnsupportedEnvironmentReason::NonWaylandSession,
            environment: session.clone(),
            workaround: Some("run Kiekje inside a Wayland session".to_string()),
        });
    }

    if matches!(mode, CaptureMode::Window) && !session.supports_active_window_capture() {
        return Some(UnsupportedEnvironmentError {
            mode,
            reason: UnsupportedEnvironmentReason::ActiveWindowCaptureRequiresHyprland,
            environment: session.clone(),
            workaround: Some("use `kiekje fullscreen` or `kiekje region`".to_string()),
        });
    }

    None
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

fn parse_compositor_name(value: &str) -> Option<String> {
    value
        .split(':')
        .map(str::trim)
        .find(|segment| !segment.is_empty())
        .map(normalize_compositor_name)
}

fn normalize_compositor_name(value: &str) -> String {
    if value.eq_ignore_ascii_case("hyprland") {
        "Hyprland".to_string()
    } else {
        value.to_string()
    }
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
        check_capture_requirements_with_path_and_session, doctor_report_with_path_and_session,
        missing_portal_interfaces, plan_portal_repair, LinuxSessionDetails, PortalInterface,
        PortalState, UnsupportedEnvironmentError,
    };
    use crate::settings::config::{CaptureMode, Settings};

    #[test]
    fn reports_mode_specific_missing_dependencies() {
        let settings = Settings {
            copy_to_clipboard: true,
            ..Settings::default()
        };

        let err = check_capture_requirements_with_path_and_session(
            CaptureMode::Window,
            &settings,
            Some("".into()),
            LinuxSessionDetails {
                session_type: Some("wayland".to_string()),
                current_desktop: Some("Hyprland".to_string()),
                desktop_session: Some("hyprland".to_string()),
                wayland_display: Some("wayland-1".to_string()),
                hyprland_instance_signature: true,
            },
        )
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
        let report = doctor_report_with_path_and_session(
            Some("".into()),
            LinuxSessionDetails {
                session_type: Some("wayland".to_string()),
                current_desktop: Some("Hyprland".to_string()),
                desktop_session: Some("hyprland".to_string()),
                wayland_display: Some("wayland-1".to_string()),
                hyprland_instance_signature: true,
            },
        );
        assert!(report.contains("Linux Session Report"));
        assert!(report.contains("[MISS] grim"));
        assert!(report.contains("[MISS] wl-copy"));
        assert!(report.contains("[MISS] hyprctl"));
        assert!(report.contains("[OK]   session"));
        assert!(report.contains("[OK]   window mode"));
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

    #[test]
    fn rejects_x11_sessions_before_tool_checks() {
        let err = check_capture_requirements_with_path_and_session(
            CaptureMode::Region,
            &Settings::default(),
            Some("".into()),
            LinuxSessionDetails {
                session_type: Some("x11".to_string()),
                current_desktop: None,
                desktop_session: None,
                wayland_display: None,
                hyprland_instance_signature: false,
            },
        )
        .unwrap_err();

        let unsupported = err
            .downcast_ref::<UnsupportedEnvironmentError>()
            .expect("expected UnsupportedEnvironmentError");
        assert_eq!(unsupported.mode, CaptureMode::Region);
        assert_eq!(
            unsupported.reason_label(),
            "grim-based capture requires an active Wayland session"
        );
    }

    #[test]
    fn rejects_window_mode_on_non_hyprland_wayland() {
        let err = check_capture_requirements_with_path_and_session(
            CaptureMode::Window,
            &Settings::default(),
            Some("".into()),
            LinuxSessionDetails {
                session_type: Some("wayland".to_string()),
                current_desktop: Some("GNOME".to_string()),
                desktop_session: Some("gnome".to_string()),
                wayland_display: Some("wayland-1".to_string()),
                hyprland_instance_signature: false,
            },
        )
        .unwrap_err();

        let unsupported = err
            .downcast_ref::<UnsupportedEnvironmentError>()
            .expect("expected UnsupportedEnvironmentError");
        assert_eq!(unsupported.mode, CaptureMode::Window);
        assert_eq!(
            unsupported.reason_label(),
            "active-window capture currently requires Hyprland"
        );
        assert_eq!(
            unsupported.environment.compositor_label().as_deref(),
            Some("GNOME")
        );
    }
}
