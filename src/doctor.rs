use std::fs;

use gtk::prelude::*;

use crate::app::{gdk_monitors, monitor_info};
use crate::backends::detect::{self, LAYER_SHELL};
use crate::backends::{backend, candidates};
use crate::capture::BackendChoice;
use crate::desktop_file;
use crate::overlay;

/// Prints the capabilities this session exposes and what is missing.
///
/// # Examples
///
/// ```ignore
/// doctor::run();
/// ```
pub fn run() {
    let variable = |name: &str| std::env::var(name).unwrap_or_else(|_| "unset".into());
    println!(
        "session      {} / {}",
        variable("XDG_SESSION_TYPE"),
        variable("XDG_CURRENT_DESKTOP")
    );

    if gtk::init().is_err() {
        println!("gtk          could not initialize");
        return;
    }
    let environment = detect::probe();
    println!(
        "overlay      {}",
        if overlay::uses_layer_shell() {
            "layer-shell"
        } else if environment.has_global(LAYER_SHELL) {
            "fullscreen windows (the compositor offers layer-shell but GTK cannot use it)"
        } else {
            "fullscreen windows (no layer-shell)"
        }
    );
    for monitor in gdk_monitors() {
        let info = monitor_info(&monitor);
        println!(
            "monitor      {} {}x{}{:+}{:+} scale {}",
            info.connector,
            info.logical.width,
            info.logical.height,
            info.logical.x,
            info.logical.y,
            monitor.scale()
        );
    }

    match detect::session_bus() {
        Ok(connection) => {
            for interface in ["Screenshot", "ScreenCast", "GlobalShortcuts"] {
                let version = detect::portal_version(&connection, interface)
                    .map_or_else(|| "unavailable".into(), |version| format!("v{version}"));
                println!("portal       {interface:<16}{version}");
            }
        }
        Err(error) => println!("dbus         session bus unavailable: {error}"),
    }

    let order = candidates(BackendChoice::Auto, &environment);
    if order.is_empty() {
        println!("capture      no backend available");
    }
    for kind in order {
        let capabilities = backend(kind).capabilities();
        println!(
            "capture      {kind:<24}cursor {} native {} sandbox {}",
            yes_no(capabilities.cursor),
            yes_no(capabilities.native_resolution),
            yes_no(capabilities.sandbox_compatible),
        );
    }

    if environment.has_kwin() {
        println!("kwin         {}", kwin_access_status());
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
fn kwin_access_status() -> String {
    let path = desktop_file::installed_path();
    let Ok(contents) = fs::read_to_string(&path) else {
        return "ScreenShot2 needs a desktop file: run `kiekje-next install-desktop-file`".into();
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
        format!(
            "desktop file installed for this binary ({})",
            path.display()
        )
    } else {
        format!(
            "desktop file at {} points at another binary; run `kiekje-next install-desktop-file` again",
            path.display()
        )
    }
}
