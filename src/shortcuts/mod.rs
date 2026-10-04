pub mod hyprland;
mod portal;

use gtk::glib;
use thiserror::Error;

pub use portal::{PortalGuard, PortalShortcuts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    CaptureRegion,
    CaptureFullscreen,
}

impl ShortcutAction {
    pub const ALL: [Self; 2] = [Self::CaptureRegion, Self::CaptureFullscreen];

    pub fn id(self) -> &'static str {
        match self {
            Self::CaptureRegion => "capture-region",
            Self::CaptureFullscreen => "capture-fullscreen",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::CaptureRegion => "Capture a region",
            Self::CaptureFullscreen => "Capture every screen",
        }
    }

    /// The trigger suggested to the desktop, in the XDG shortcuts format.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ShortcutAction::CaptureRegion.preferred_trigger(), "Print");
    /// ```
    pub fn preferred_trigger(self) -> &'static str {
        match self {
            Self::CaptureRegion => "Print",
            Self::CaptureFullscreen => "SHIFT+Print",
        }
    }
}

/// The `hyprland.conf` line that sends the preferred trigger to this shortcut.
///
/// Hyprland's portal registers shortcuts without a key, so the user adds this bind.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(
///     hyprland_bind(ShortcutAction::CaptureFullscreen, "dev.example.App"),
///     "bind = SHIFT, Print, global, dev.example.App:capture-fullscreen"
/// );
/// ```
pub fn hyprland_bind(action: ShortcutAction, app_id: &str) -> String {
    let trigger = action.preferred_trigger();
    let (modifiers, key) = trigger.rsplit_once('+').unwrap_or(("", trigger));
    format!(
        "bind = {}, {key}, global, {app_id}:{}",
        modifiers.replace('+', " "),
        action.id()
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub action: ShortcutAction,
    pub trigger: Option<String>,
}

#[derive(Debug, Error)]
pub enum ShortcutError {
    #[error("no GlobalShortcuts portal: {0}")]
    Unavailable(String),
    #[error("the shortcut request was canceled")]
    Canceled,
    #[error("the portal answered with response code {0}")]
    Failed(u32),
    #[error("unexpected reply from the portal: {0}")]
    Reply(String),
    #[error("D-Bus error: {0}")]
    Dbus(#[from] glib::Error),
}

/// Registers global shortcuts with the desktop and reports their activations.
///
/// The returned guard keeps the registration alive; dropping it stops activations.
///
/// # Examples
///
/// ```ignore
/// let guard = registrar.bind(|action| println!("{}", action.id())).await?;
/// ```
pub trait ShortcutRegistrar {
    type Guard;

    fn name(&self) -> &'static str;

    async fn bind(
        &self,
        on_activated: impl Fn(ShortcutAction) + 'static,
    ) -> Result<(Self::Guard, Vec<Binding>), ShortcutError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_action_from_its_id() {
        for action in ShortcutAction::ALL {
            assert_eq!(ShortcutAction::parse(action.id()), Some(action));
        }
        assert_eq!(ShortcutAction::parse("capture-window"), None);
    }

    #[test]
    fn writes_a_hyprland_bind_for_each_preferred_trigger() {
        assert_eq!(
            hyprland_bind(ShortcutAction::CaptureRegion, "dev.example.App"),
            "bind = , Print, global, dev.example.App:capture-region"
        );
        assert_eq!(
            hyprland_bind(ShortcutAction::CaptureFullscreen, "dev.example.App"),
            "bind = SHIFT, Print, global, dev.example.App:capture-fullscreen"
        );
    }
}
