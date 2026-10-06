pub mod hyprland;
pub mod kde;
mod portal;
mod trigger;

use gtk::glib;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use portal::{PortalGuard, PortalShortcuts};
pub use trigger::{Modifiers, Recorded, Slot, Trigger, record};

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

    /// The trigger an action gets until the user picks another, in the XDG shortcuts format.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ShortcutAction::CaptureRegion.default_trigger(), "Print");
    /// ```
    pub fn default_trigger(self) -> &'static str {
        match self {
            Self::CaptureRegion => "Print",
            Self::CaptureFullscreen => "SHIFT+Print",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutHost {
    Starting,
    Off(String),
    Desktop {
        name: &'static str,
        configurable: bool,
    },
    Kde,
    Hyprland,
}

impl ShortcutHost {
    pub fn editable(&self) -> bool {
        matches!(self, Self::Kde | Self::Hyprland)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Shortcuts {
    pub capture_region: Slot,
    pub capture_fullscreen: Slot,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{} is set for both `{}` and `{}`", trigger.label(), first.id(), second.id())]
pub struct Conflict {
    pub trigger: Trigger,
    pub first: ShortcutAction,
    pub second: ShortcutAction,
}

impl Default for Shortcuts {
    fn default() -> Self {
        let slot = |action: ShortcutAction| {
            Trigger::parse(action.default_trigger()).map_or(Slot::Off, Slot::Key)
        };
        Self {
            capture_region: slot(ShortcutAction::CaptureRegion),
            capture_fullscreen: slot(ShortcutAction::CaptureFullscreen),
        }
    }
}

impl Shortcuts {
    pub fn slot(&self, action: ShortcutAction) -> &Slot {
        match action {
            ShortcutAction::CaptureRegion => &self.capture_region,
            ShortcutAction::CaptureFullscreen => &self.capture_fullscreen,
        }
    }

    pub fn set(&mut self, action: ShortcutAction, slot: Slot) {
        match action {
            ShortcutAction::CaptureRegion => self.capture_region = slot,
            ShortcutAction::CaptureFullscreen => self.capture_fullscreen = slot,
        }
    }

    /// Finds the first trigger that two actions share.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Some(conflict) = shortcuts.conflict() {
    ///     eprintln!("kiekje: {conflict}");
    /// }
    /// ```
    pub fn conflict(&self) -> Option<Conflict> {
        let assigned: Vec<(ShortcutAction, &Trigger)> = ShortcutAction::ALL
            .into_iter()
            .filter_map(|action| self.slot(action).trigger().map(|trigger| (action, trigger)))
            .collect();
        assigned
            .iter()
            .enumerate()
            .find_map(|(index, (first, trigger))| {
                assigned[index + 1..]
                    .iter()
                    .find(|(_, other)| other == trigger)
                    .map(|(second, _)| Conflict {
                        trigger: (*trigger).clone(),
                        first: *first,
                        second: *second,
                    })
            })
    }
}

/// The `hyprland.conf` line that sends `trigger` to this shortcut.
///
/// Hyprland's portal registers shortcuts without a key, so Kiekje writes this bind.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(
///     hyprland_bind(ShortcutAction::CaptureFullscreen, &Trigger::parse("SHIFT+Print")?, "dev.example.App"),
///     "bind = SHIFT, Print, global, dev.example.App:capture-fullscreen"
/// );
/// ```
pub fn hyprland_bind(action: ShortcutAction, trigger: &Trigger, app_id: &str) -> String {
    let (modifiers, key) = trigger.hyprland();
    format!(
        "bind = {modifiers}, {key}, global, {app_id}:{}",
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
    fn writes_a_hyprland_bind_for_each_default_trigger() {
        let shortcuts = Shortcuts::default();
        let bind = |action| {
            hyprland_bind(
                action,
                shortcuts.slot(action).trigger().unwrap(),
                "dev.example.App",
            )
        };
        assert_eq!(
            bind(ShortcutAction::CaptureRegion),
            "bind = , Print, global, dev.example.App:capture-region"
        );
        assert_eq!(
            bind(ShortcutAction::CaptureFullscreen),
            "bind = SHIFT, Print, global, dev.example.App:capture-fullscreen"
        );
    }

    #[test]
    fn finds_a_trigger_shared_by_two_actions() {
        let mut shortcuts = Shortcuts::default();
        assert_eq!(shortcuts.conflict(), None);
        shortcuts.set(
            ShortcutAction::CaptureFullscreen,
            shortcuts.capture_region.clone(),
        );
        let conflict = shortcuts.conflict().unwrap();
        assert_eq!(conflict.first, ShortcutAction::CaptureRegion);
        assert_eq!(conflict.second, ShortcutAction::CaptureFullscreen);
        shortcuts.set(ShortcutAction::CaptureRegion, Slot::Off);
        shortcuts.set(ShortcutAction::CaptureFullscreen, Slot::Off);
        assert_eq!(shortcuts.conflict(), None);
    }
}
