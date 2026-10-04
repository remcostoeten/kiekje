use std::fs;
use std::path::{Path, PathBuf};

use gtk::prelude::*;

use crate::app::{APP_ID, gdk_monitors, monitor_info};
use crate::backends::detect::{self, Environment, LAYER_SHELL};
use crate::backends::{Capabilities, backend, candidates};
use crate::capture::{BackendChoice, BackendKind};
use crate::config::{self, Config};
use crate::desktop_file;
use crate::overlay;
use crate::style::{Style, pad};

const BINARY: &str = env!("CARGO_PKG_NAME");
const LABEL_WIDTH: usize = 18;
const BACKEND_WIDTH: usize = 24;
const ROLE_WIDTH: usize = 12;
const PORTALS: [&str; 3] = ["Screenshot", "ScreenCast", "GlobalShortcuts"];
const BACKENDS: [BackendKind; 4] = [
    BackendKind::Kwin,
    BackendKind::ExtImageCopy,
    BackendKind::WlrScreencopy,
    BackendKind::Portal,
];

enum Overlay {
    LayerShell,
    LayerShellUnusable,
    Fullscreen,
}

enum KwinAccess {
    Ready,
    Stale(PathBuf),
    Missing,
}

enum Severity {
    Warning,
    Error,
}

struct Problem {
    severity: Severity,
    message: String,
    fix: Option<String>,
}

struct Monitor {
    connector: String,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    scale: f64,
}

struct Bus {
    daemon_running: bool,
    portals: Vec<(&'static str, Option<u32>)>,
}

struct Report {
    session_type: String,
    desktop: String,
    overlay: Option<Overlay>,
    monitors: Vec<Monitor>,
    bus: Result<Bus, String>,
    available: Vec<(BackendKind, Capabilities)>,
    kwin_present: bool,
    kwin_access: Option<KwinAccess>,
    config_path: PathBuf,
    config: Result<Option<Config>, String>,
}

/// Prints the capabilities this session exposes, followed by every problem and its fix.
///
/// # Examples
///
/// ```ignore
/// doctor::run();
/// ```
pub fn run() {
    print!("{}", render(&collect(), &Style::stdout()));
}

fn collect() -> Report {
    let variable = |name: &str| std::env::var(name).unwrap_or_else(|_| "unset".into());
    let gtk_ready = gtk::init().is_ok();
    let environment = detect::probe();

    let overlay = gtk_ready.then(|| overlay_mode(&environment));
    let monitors = if gtk_ready {
        gdk_monitors()
            .iter()
            .map(|monitor| {
                let info = monitor_info(monitor);
                Monitor {
                    connector: info.connector,
                    width: info.logical.width,
                    height: info.logical.height,
                    x: info.logical.x,
                    y: info.logical.y,
                    scale: monitor.scale(),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let bus = detect::session_bus()
        .map(|connection| Bus {
            daemon_running: detect::name_has_owner(&connection, APP_ID),
            portals: PORTALS
                .iter()
                .map(|interface| (*interface, detect::portal_version(&connection, interface)))
                .collect(),
        })
        .map_err(|error| error.to_string());

    let config_path = config::path();
    let available = candidates(BackendChoice::Auto, &environment)
        .into_iter()
        .map(|kind| (kind, backend(kind).capabilities()))
        .collect();

    Report {
        session_type: variable("XDG_SESSION_TYPE"),
        desktop: variable("XDG_CURRENT_DESKTOP"),
        overlay,
        monitors,
        bus,
        available,
        kwin_present: environment.has_kwin(),
        kwin_access: environment.has_kwin().then(kwin_access),
        config: config::load_from(&config_path).map_err(|error| {
            error
                .to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        }),
        config_path,
    }
}

fn overlay_mode(environment: &Environment) -> Overlay {
    if overlay::uses_layer_shell() {
        Overlay::LayerShell
    } else if environment.has_global(LAYER_SHELL) {
        Overlay::LayerShellUnusable
    } else {
        Overlay::Fullscreen
    }
}

fn kwin_access() -> KwinAccess {
    let path = desktop_file::installed_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return KwinAccess::Missing;
    };
    let current = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let exec_matches = contents.lines().any(|line| {
        line.strip_prefix("Exec=")
            .is_some_and(|exec| exec.trim_start_matches('"').starts_with(&current))
    });
    if exec_matches {
        KwinAccess::Ready
    } else {
        KwinAccess::Stale(path)
    }
}

fn unavailable_reason(kind: BackendKind, report: &Report) -> &'static str {
    match kind {
        BackendKind::Kwin => "KWin is not the compositor of this session",
        BackendKind::ExtImageCopy | BackendKind::WlrScreencopy => {
            "this compositor does not offer it"
        }
        BackendKind::Portal if report.bus.is_err() => "no session bus",
        BackendKind::Portal => "no Screenshot portal",
    }
}

fn traits(capabilities: &Capabilities) -> String {
    [
        if capabilities.cursor {
            "can include the pointer"
        } else {
            "no pointer"
        },
        if capabilities.native_resolution {
            "full resolution"
        } else {
            "may be scaled down"
        },
        if capabilities.sandbox_compatible {
            "works in Flatpak"
        } else {
            "not in Flatpak"
        },
    ]
    .join(", ")
}

fn problems(report: &Report) -> Vec<Problem> {
    let mut found = Vec::new();
    let reinstall = Some(format!("run {BINARY} install-desktop-file"));

    if report.overlay.is_none() {
        found.push(Problem {
            severity: Severity::Error,
            message: "GTK could not initialize, so there is no overlay".into(),
            fix: Some("run doctor from inside a graphical session".into()),
        });
    }
    if let Some(Overlay::LayerShellUnusable) = report.overlay {
        found.push(Problem {
            severity: Severity::Warning,
            message: "The compositor offers layer-shell but GTK cannot use it, so the overlay uses fullscreen windows".into(),
            fix: None,
        });
    }
    match &report.bus {
        Err(error) => found.push(Problem {
            severity: Severity::Error,
            message: format!("The session bus is unavailable: {error}"),
            fix: None,
        }),
        Ok(bus) => {
            let shortcuts = bus
                .portals
                .iter()
                .any(|(name, version)| *name == "GlobalShortcuts" && version.is_some());
            if !shortcuts {
                found.push(Problem {
                    severity: Severity::Warning,
                    message: "No GlobalShortcuts portal, so Print and Shift+Print are not bound"
                        .into(),
                    fix: None,
                });
            }
        }
    }
    if report.available.is_empty() {
        found.push(Problem {
            severity: Severity::Error,
            message: "No capture backend is available".into(),
            fix: Some("install xdg-desktop-portal with a backend for this desktop".into()),
        });
    }
    match &report.kwin_access {
        Some(KwinAccess::Missing) => found.push(Problem {
            severity: Severity::Warning,
            message: "KWin only allows ScreenShot2 for an app with a desktop file".into(),
            fix: reinstall,
        }),
        Some(KwinAccess::Stale(path)) => found.push(Problem {
            severity: Severity::Warning,
            message: format!(
                "{} points at another binary, so KWin may refuse ScreenShot2",
                tilde(path)
            ),
            fix: reinstall,
        }),
        Some(KwinAccess::Ready) | None => {}
    }
    if let Err(error) = &report.config {
        found.push(Problem {
            severity: Severity::Warning,
            message: format!("The config can't be used, so defaults apply: {error}"),
            fix: Some(format!(
                "fix {} or delete it to get the defaults back",
                tilde(&report.config_path)
            )),
        });
    }
    found
}

fn tilde(path: &Path) -> String {
    let home = gtk::glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

fn field(style: &Style, label: &str, value: &str) -> String {
    format!("  {}{value}\n", style.dim(&pad(label, LABEL_WIDTH)))
}

fn render(report: &Report, style: &Style) -> String {
    let mut text = format!(
        "{}  {}\n\n",
        style.accent(&format!("{BINARY} doctor")),
        style.dim(&format!("v{}", env!("CARGO_PKG_VERSION"))),
    );

    text.push_str(&format!("{}\n", style.bold("Session")));
    text.push_str(&field(style, "type", &report.session_type));
    text.push_str(&field(style, "desktop", &report.desktop));
    let overlay = match report.overlay {
        Some(Overlay::LayerShell) => style.good("layer-shell"),
        Some(Overlay::LayerShellUnusable | Overlay::Fullscreen) => "fullscreen windows".into(),
        None => style.error("unavailable"),
    };
    text.push_str(&field(style, "overlay", &overlay));
    if let Ok(bus) = &report.bus {
        let daemon = if bus.daemon_running {
            style.good("running")
        } else {
            style.dim("not running")
        };
        text.push_str(&field(style, "daemon", &daemon));
    }
    if report.kwin_present {
        let access = match report.kwin_access {
            Some(KwinAccess::Ready) => style.good("desktop file installed"),
            Some(KwinAccess::Stale(_)) => style.warn("desktop file outdated"),
            Some(KwinAccess::Missing) | None => style.warn("no desktop file"),
        };
        text.push_str(&field(style, "kwin access", &access));
    }
    text.push('\n');

    text.push_str(&format!("{}\n", style.bold("Storage")));
    let path = tilde(&report.config_path);
    let (status, effective) = match &report.config {
        Ok(Some(config)) => (path, config.clone()),
        Ok(None) => (
            format!("{path} {}", style.dim("(not written yet)")),
            Config::default(),
        ),
        Err(_) => (
            format!("{path} {}", style.warn("(invalid)")),
            Config::default(),
        ),
    };
    text.push_str(&field(style, "config", &status));
    text.push_str(&field(
        style,
        "folder",
        &tilde(&effective.storage.captures_dir()),
    ));
    text.push_str(&field(
        style,
        "file name",
        &format!("{}.png", effective.storage.filename),
    ));
    text.push('\n');

    text.push_str(&format!("{}\n", style.bold("Monitors")));
    if report.monitors.is_empty() {
        text.push_str(&format!("  {}\n", style.dim("none found")));
    }
    for monitor in &report.monitors {
        let geometry = format!(
            "{}x{} at {},{}, scale {}",
            monitor.width, monitor.height, monitor.x, monitor.y, monitor.scale
        );
        text.push_str(&field(style, &monitor.connector, &geometry));
    }
    text.push('\n');

    if let Ok(bus) = &report.bus {
        text.push_str(&format!("{}\n", style.bold("Portals")));
        for (name, version) in &bus.portals {
            let value = version.map_or_else(
                || style.error("unavailable"),
                |version| format!("v{version}"),
            );
            text.push_str(&field(style, name, &value));
        }
        text.push('\n');
    }

    text.push_str(&format!("{}\n", style.bold("Capture backends")));
    for (index, (kind, capabilities)) in report.available.iter().enumerate() {
        let role = if index == 0 { "used first" } else { "fallback" };
        text.push_str(&format!(
            "  {} {}{}{}\n",
            style.good("●"),
            style.bold(&pad(&kind.to_string(), BACKEND_WIDTH)),
            pad(role, ROLE_WIDTH),
            style.dim(&traits(capabilities)),
        ));
    }
    for kind in BACKENDS.into_iter().filter(|kind| {
        report
            .available
            .iter()
            .all(|(available, _)| available != kind)
    }) {
        text.push_str(&format!(
            "  {} {}{}\n",
            style.dim("○"),
            style.dim(&pad(&kind.to_string(), BACKEND_WIDTH)),
            style.dim(unavailable_reason(kind, report)),
        ));
    }
    if !report.available.is_empty() {
        text.push_str(&format!(
            "  {}\n",
            style.dim("If the first one fails, the next one down takes over.")
        ));
    }
    text.push('\n');

    let found = problems(report);
    if found.is_empty() {
        text.push_str(&format!("{}\n\n", style.good("No problems found.")));
        return text;
    }
    text.push_str(&format!("{}\n", style.bold("Problems")));
    for problem in found {
        let mark = match problem.severity {
            Severity::Warning => style.warn("!"),
            Severity::Error => style.error("✗"),
        };
        text.push_str(&format!("  {mark} {}\n", problem.message));
        if let Some(fix) = problem.fix {
            text.push_str(&format!("    {}\n", style.dim(&fix)));
        }
    }
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style { color: false };

    fn report() -> Report {
        Report {
            session_type: "wayland".into(),
            desktop: "KDE".into(),
            overlay: Some(Overlay::LayerShell),
            monitors: vec![Monitor {
                connector: "DP-2".into(),
                width: 3840,
                height: 2160,
                x: 0,
                y: 0,
                scale: 1.0,
            }],
            bus: Ok(Bus {
                daemon_running: false,
                portals: PORTALS.iter().map(|name| (*name, Some(2))).collect(),
            }),
            available: vec![(BackendKind::Kwin, backend(BackendKind::Kwin).capabilities())],
            kwin_present: true,
            kwin_access: Some(KwinAccess::Ready),
            config_path: PathBuf::from("/etc/kiekje/config.toml"),
            config: Ok(None),
        }
    }

    #[test]
    fn lists_unavailable_backends_with_a_reason() {
        let text = render(&report(), &PLAIN);
        assert!(text.contains(
            "● kwin-screenshot2        used first  can include the pointer, full resolution, not in Flatpak"
        ));
        assert!(text.contains("○ wlr-screencopy          this compositor does not offer it"));
        assert!(text.contains("○ xdg-desktop-portal      no Screenshot portal"));
    }

    #[test]
    fn reports_no_problems_for_a_ready_session() {
        assert!(render(&report(), &PLAIN).contains("No problems found."));
    }

    #[test]
    fn names_the_fix_for_a_stale_desktop_file() {
        let mut stale = report();
        stale.kwin_access = Some(KwinAccess::Stale(PathBuf::from("/tmp/kiekje.desktop")));
        let text = render(&stale, &PLAIN);
        assert!(text.contains("! /tmp/kiekje.desktop points at another binary"));
        assert!(text.contains(&format!("run {BINARY} install-desktop-file")));
    }

    #[test]
    fn shows_the_default_storage_when_no_config_exists() {
        let text = render(&report(), &PLAIN);
        assert!(text.contains("/etc/kiekje/config.toml (not written yet)"));
        assert!(text.contains("kiekje-{mode}-{date}-{time}.png"));
    }

    #[test]
    fn warns_about_an_invalid_config() {
        let mut invalid = report();
        invalid.config = Err("unknown token `{weekday}`".into());
        let text = render(&invalid, &PLAIN);
        assert!(text.contains("/etc/kiekje/config.toml (invalid)"));
        assert!(text.contains("! The config can't be used, so defaults apply: unknown token"));
        assert!(text.contains("fix /etc/kiekje/config.toml or delete it"));
    }

    #[test]
    fn flags_a_session_without_backends_as_an_error() {
        let mut empty = report();
        empty.available.clear();
        empty.kwin_present = false;
        empty.kwin_access = None;
        assert!(render(&empty, &PLAIN).contains("✗ No capture backend is available"));
    }
}
