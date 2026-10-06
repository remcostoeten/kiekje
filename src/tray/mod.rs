mod icon;
pub mod popover;
mod sni;

use thiserror::Error;

use crate::capture::CaptureMode;

pub use sni::SniTray;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Capture { mode: CaptureMode },
    Annotate,
    OpenMenu { x: i32, y: i32 },
    OpenCaptures,
    OpenGallery,
    OpenSettings,
    Quit,
}

#[derive(Debug, Error)]
pub enum TrayError {
    #[error("no tray host is running: {0}")]
    Unavailable(String),
    #[error("D-Bus error: {0}")]
    Dbus(String),
}

/// Shows a tray icon and forwards what the user picks as commands.
///
/// `start` blocks on D-Bus, so it runs off the GTK main thread. The host never builds widgets;
/// the returned guard keeps the icon alive and removes it on `stop`.
///
/// # Examples
///
/// ```ignore
/// let guard = SniTray.start(sender)?;
/// ```
pub trait TrayHost {
    type Guard: TrayGuard + Send + 'static;

    fn name(&self) -> &'static str;

    fn start(self, commands: async_channel::Sender<TrayCommand>) -> Result<Self::Guard, TrayError>;
}

pub trait TrayGuard {
    fn stop(&self);
}
