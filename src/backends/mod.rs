pub mod detect;
pub mod kwin;
pub mod portal;
pub mod windows;
pub mod wlroots;

use std::cell::OnceCell;

use crate::capture::{BackendChoice, BackendKind, Capture, CaptureError, Frame, MonitorInfo};
use detect::{Environment, WLR_SCREENCOPY};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub cursor: bool,
    pub native_resolution: bool,
    pub sandbox_compatible: bool,
}

pub trait CaptureBackend {
    fn capabilities(&self) -> Capabilities;
    fn capture(
        &self,
        monitors: &[MonitorInfo],
        include_cursor: bool,
    ) -> Result<Vec<Frame>, CaptureError>;

    /// Captures only `wanted`, which is part of the desktop laid out by `all`.
    ///
    /// Backends that grab the whole desktop at once use `all` to size what they grab.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let frames = backend.capture_some(&all, &wanted, false)?;
    /// ```
    fn capture_some(
        &self,
        _all: &[MonitorInfo],
        wanted: &[MonitorInfo],
        include_cursor: bool,
    ) -> Result<Vec<Frame>, CaptureError> {
        self.capture(wanted, include_cursor)
    }
}

/// Returns the backend that implements `kind`.
///
/// # Examples
///
/// ```ignore
/// let capabilities = backend(BackendKind::Portal).capabilities();
/// ```
pub fn backend(kind: BackendKind) -> &'static dyn CaptureBackend {
    match kind {
        BackendKind::Kwin => &kwin::Kwin,
        BackendKind::ExtImageCopy => &wlroots::ExtImageCopy,
        BackendKind::WlrScreencopy => &wlroots::Screencopy,
        BackendKind::Portal => &portal::Portal,
    }
}

/// Lists the backends to try for `choice`, most direct first, keeping only what the session
/// exposes.
///
/// # Examples
///
/// ```ignore
/// let order = candidates(BackendChoice::Auto, &detect::probe());
/// ```
pub fn candidates(choice: BackendChoice, environment: &Environment) -> Vec<BackendKind> {
    let available = [
        (BackendKind::Kwin, environment.has_kwin()),
        (BackendKind::ExtImageCopy, environment.has_ext_image_copy()),
        (
            BackendKind::WlrScreencopy,
            environment.has_global(WLR_SCREENCOPY),
        ),
        (BackendKind::Portal, environment.portal_screenshot.is_some()),
    ];
    available
        .into_iter()
        .filter(|(kind, present)| *present && choice.allows(*kind))
        .map(|(kind, _)| kind)
        .collect()
}

/// Captures the `wanted` monitors of the desktop laid out by `all` with the first available
/// backend for `choice`, moving to the next one when a backend is unavailable or refuses access.
///
/// Pass the same slice twice to capture every monitor.
///
/// # Examples
///
/// ```ignore
/// let capture = capture_some(BackendChoice::Auto, &monitors, &monitors, false)?;
/// ```
pub fn capture_some(
    choice: BackendChoice,
    all: &[MonitorInfo],
    wanted: &[MonitorInfo],
    include_cursor: bool,
) -> Result<Capture, CaptureError> {
    let mut skipped = Vec::new();
    let mut found = candidates(choice, &detect::probe_compositor());
    if choice.allows(BackendKind::Portal) {
        found.push(BackendKind::Portal);
    }
    let portal_present = OnceCell::new();
    let has_portal = || {
        *portal_present.get_or_init(|| {
            detect::session_bus()
                .ok()
                .and_then(|connection| detect::portal_version(&connection, "Screenshot"))
                .is_some()
        })
    };
    let mut kinds = found.into_iter().peekable();
    while let Some(kind) = kinds.next() {
        if kind == BackendKind::Portal && !has_portal() {
            continue;
        }
        match backend(kind).capture_some(all, wanted, include_cursor) {
            Ok(frames) => {
                return Ok(Capture {
                    backend: kind,
                    frames,
                    fallback_reason: (!skipped.is_empty()).then(|| skipped.join("; ")),
                    windows: Vec::new(),
                });
            }
            Err(error)
                if kinds
                    .peek()
                    .is_some_and(|next| *next != BackendKind::Portal || has_portal())
                    && error.allows_fallback() =>
            {
                skipped.push(format!("{kind}: {error}"));
            }
            Err(error) => return Err(error),
        }
    }
    Err(CaptureError::Unavailable(format!(
        "no `{}` capture backend in this session; run `kiekje doctor`",
        choice.as_str()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use detect::{
        EXT_IMAGE_COPY_CAPTURE, EXT_OUTPUT_IMAGE_CAPTURE_SOURCE, LAYER_SHELL, PLASMA_SHELL,
    };

    fn environment(globals: &[&str], kwin: bool, portal: bool) -> Environment {
        Environment {
            wayland_globals: globals.iter().map(|name| (*name).to_owned()).collect(),
            kwin_on_bus: kwin,
            portal_screenshot: portal.then_some(2),
        }
    }

    #[test]
    fn prefers_kwin_then_portal_on_plasma() {
        let plasma = environment(&[LAYER_SHELL, PLASMA_SHELL], true, true);
        assert_eq!(
            candidates(BackendChoice::Auto, &plasma),
            [BackendKind::Kwin, BackendKind::Portal]
        );
    }

    #[test]
    fn prefers_ext_image_copy_then_screencopy_on_wlroots() {
        let hyprland = environment(
            &[
                LAYER_SHELL,
                WLR_SCREENCOPY,
                EXT_IMAGE_COPY_CAPTURE,
                EXT_OUTPUT_IMAGE_CAPTURE_SOURCE,
            ],
            false,
            true,
        );
        assert_eq!(
            candidates(BackendChoice::Auto, &hyprland),
            [
                BackendKind::ExtImageCopy,
                BackendKind::WlrScreencopy,
                BackendKind::Portal
            ]
        );
        assert_eq!(
            candidates(BackendChoice::Wlroots, &hyprland),
            [BackendKind::ExtImageCopy, BackendKind::WlrScreencopy]
        );
    }

    #[test]
    fn ignores_kwin_on_the_bus_when_another_compositor_runs() {
        let nested = environment(&[WLR_SCREENCOPY], true, true);
        assert_eq!(
            candidates(BackendChoice::Auto, &nested),
            [BackendKind::WlrScreencopy, BackendKind::Portal]
        );
    }

    #[test]
    fn uses_kwin_on_x11() {
        let x11 = environment(&[], true, true);
        assert_eq!(candidates(BackendChoice::Kwin, &x11), [BackendKind::Kwin]);
    }

    #[test]
    fn needs_both_ext_globals_for_ext_image_copy() {
        let partial = environment(&[EXT_IMAGE_COPY_CAPTURE, WLR_SCREENCOPY], false, false);
        assert_eq!(
            candidates(BackendChoice::Auto, &partial),
            [BackendKind::WlrScreencopy]
        );
    }

    #[test]
    fn uses_only_the_portal_on_gnome() {
        let gnome = environment(&[], false, true);
        assert_eq!(
            candidates(BackendChoice::Auto, &gnome),
            [BackendKind::Portal]
        );
        assert!(candidates(BackendChoice::Kwin, &gnome).is_empty());
    }
}
