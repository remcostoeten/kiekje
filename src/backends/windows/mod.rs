pub mod hyprland;
pub mod kwin;

use std::fmt;
use std::io;

use thiserror::Error;

use super::detect::{self, Environment};
use crate::capture::{Availability, LogicalRect};

pub const UNAVAILABLE: &str = "this desktop does not report where its windows are; window capture works on Plasma 6 and Hyprland";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowSourceKind {
    Hyprland,
    Kwin,
}

impl fmt::Display for WindowSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(match self {
            Self::Hyprland => "hyprland-ipc",
            Self::Kwin => "kwin-scripting",
        })
    }
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("no window geometry source in this session")]
    Unavailable,
    #[error("timed out waiting for the window list")]
    Timeout,
    #[error("KWin refused to load the window list script")]
    ScriptRejected,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),
    #[error("could not parse the window list: {0}")]
    Parse(#[from] serde_json::Error),
}

pub trait WindowGeometrySource {
    /// Lists the frames of the windows visible right now, topmost first, in desktop-wide
    /// logical coordinates.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let windows = source(WindowSourceKind::Kwin).windows()?;
    /// ```
    fn windows(&self) -> Result<Vec<LogicalRect>, WindowError>;
}

/// Returns the source that implements `kind`.
///
/// # Examples
///
/// ```ignore
/// let windows = source(WindowSourceKind::Hyprland).windows()?;
/// ```
pub fn source(kind: WindowSourceKind) -> &'static dyn WindowGeometrySource {
    match kind {
        WindowSourceKind::Hyprland => &hyprland::Hyprland,
        WindowSourceKind::Kwin => &kwin::Kwin,
    }
}

/// Picks the window geometry source for a session from the Hyprland socket and the probed
/// compositor, or `None` when the session has no supported source.
///
/// # Examples
///
/// ```ignore
/// let kind = choose(hyprland::socket_path().is_some(), &detect::probe());
/// ```
pub fn choose(hyprland_socket: bool, environment: &Environment) -> Option<WindowSourceKind> {
    if hyprland_socket {
        return Some(WindowSourceKind::Hyprland);
    }
    environment.has_kwin().then_some(WindowSourceKind::Kwin)
}

/// Finds the window geometry source this session supports, or `None` when it has none.
///
/// Blocks on the Wayland registry and D-Bus, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let kind = windows::detect();
/// ```
pub fn detect() -> Option<WindowSourceKind> {
    match hyprland::socket_path() {
        Some(_) => Some(WindowSourceKind::Hyprland),
        None => choose(false, &detect::probe_compositor()),
    }
}

/// Whether window capture works in this session, with the reason when it does not.
///
/// Window capture crops the frozen frame to a window, so it needs a window geometry source.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(availability(None), Availability::Unavailable(UNAVAILABLE));
/// ```
pub fn availability(source: Option<WindowSourceKind>) -> Availability {
    match source {
        Some(_) => Availability::Available,
        None => Availability::Unavailable(UNAVAILABLE),
    }
}

/// Lists the visible windows with the source this session supports.
///
/// Blocks on IPC or D-Bus, so call it off the GTK main thread, at trigger time next to the
/// capture so the rectangles match the frozen frame.
///
/// # Examples
///
/// ```ignore
/// let (kind, windows) = windows::snapshot()?;
/// ```
pub fn snapshot() -> Result<(WindowSourceKind, Vec<LogicalRect>), WindowError> {
    let kind = detect().ok_or(WindowError::Unavailable)?;
    source(kind).windows().map(|windows| (kind, windows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::detect::{LAYER_SHELL, PLASMA_SHELL, WLR_SCREENCOPY};

    fn environment(globals: &[&str], kwin: bool) -> Environment {
        Environment {
            wayland_globals: globals.iter().map(|name| (*name).to_owned()).collect(),
            kwin_on_bus: kwin,
            portal_screenshot: Some(2),
        }
    }

    #[test]
    fn uses_the_hyprland_socket_when_it_exists() {
        assert_eq!(
            choose(true, &environment(&[LAYER_SHELL, WLR_SCREENCOPY], false)),
            Some(WindowSourceKind::Hyprland)
        );
    }

    #[test]
    fn uses_kwin_scripting_on_plasma() {
        assert_eq!(
            choose(false, &environment(&[LAYER_SHELL, PLASMA_SHELL], true)),
            Some(WindowSourceKind::Kwin)
        );
    }

    #[test]
    fn reports_window_capture_unavailable_without_a_source() {
        assert_eq!(
            availability(Some(WindowSourceKind::Kwin)),
            Availability::Available
        );
        assert_eq!(availability(None), Availability::Unavailable(UNAVAILABLE));
    }

    #[test]
    fn reports_no_source_on_other_desktops() {
        assert_eq!(choose(false, &environment(&[], false)), None);
        assert_eq!(choose(false, &environment(&[WLR_SCREENCOPY], true)), None);
    }
}
