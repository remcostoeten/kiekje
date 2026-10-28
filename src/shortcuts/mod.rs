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
    Region,
    Fullscreen,
    Window,
}

impl ShortcutAction {
    pub const ALL: [Self; 3] = [Self::Region, Self::Fullscreen, Self::Window];

    pub fn id(self) -> &'static str {
        match self {
            Self::Region => "capture-region",
            Self::Fullscreen => "capture-fullscreen",
            Self::Window => "capture-window",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Region => "Capture a region",
            Self::Fullscreen => "Capture every screen",
            Self::Window => "Capture a window",
        }
    }

    /// The trigger an action gets until the user picks another, in the XDG shortcuts format.
    ///
    /// `None` leaves the action without a key until the user records one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ShortcutAction::Region.default_trigger(), Some("Print"));
    /// ```
    pub fn default_trigger(self) -> Option<&'static str> {
        match self {
            Self::Region => Some("Print"),
            Self::Fullscreen => Some("SHIFT+Print"),
            Self::Window => None,
        }
    }

    /// The slot an action gets until the user picks another.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ShortcutAction::Window.default_slot(), Slot::Off);
    /// ```
    pub fn default_slot(self) -> Slot {
        self.default_trigger()
            .and_then(|trigger| Trigger::parse(trigger).ok())
            .map_or(Slot::Off, Slot::Key)
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
    #[serde(skip_serializing_if = "Slot::is_off")]
    pub capture_window: Slot,
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
        Self {
            capture_region: ShortcutAction::Region.default_slot(),
            capture_fullscreen: ShortcutAction::Fullscreen.default_slot(),
            capture_window: ShortcutAction::Window.default_slot(),
        }
    }
}

impl Shortcuts {
    pub fn slot(&self, action: ShortcutAction) -> &Slot {
        match action {
            ShortcutAction::Region => &self.capture_region,
            ShortcutAction::Fullscreen => &self.capture_fullscreen,
            ShortcutAction::Window => &self.capture_window,
        }
    }

    pub fn set(&mut self, action: ShortcutAction, slot: Slot) {
        match action {
            ShortcutAction::Region => self.capture_region = slot,
            ShortcutAction::Fullscreen => self.capture_fullscreen = slot,
            ShortcutAction::Window => self.capture_window = slot,
        }
    }

    /// Finds the first trigger that two actions share.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Some(conflict) = shortcuts.conflict() {
    ///     warn!("{conflict}");
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
///     hyprland_bind(ShortcutAction::Fullscreen, &Trigger::parse("SHIFT+Print")?, "dev.example.App"),
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
        assert_eq!(ShortcutAction::parse("capture-active-window"), None);
    }

    #[test]
    fn leaves_window_capture_without_a_default_key() {
        let shortcuts = Shortcuts::default();
        assert_eq!(ShortcutAction::Window.default_trigger(), None);
        assert_eq!(shortcuts.capture_window, Slot::Off);
        assert_eq!(
            shortcuts.capture_region,
            ShortcutAction::Region.default_slot()
        );
        assert_eq!(shortcuts.conflict(), None);
    }

    #[test]
    fn refuses_a_window_key_that_another_action_uses() {
        let mut shortcuts = Shortcuts::default();
        shortcuts.set(ShortcutAction::Window, shortcuts.capture_fullscreen.clone());
        let conflict = shortcuts.conflict().unwrap();
        assert_eq!(conflict.first, ShortcutAction::Fullscreen);
        assert_eq!(conflict.second, ShortcutAction::Window);
        shortcuts.set(
            ShortcutAction::Window,
            Slot::Key(Trigger::parse("ALT+Print").unwrap()),
        );
        assert_eq!(shortcuts.conflict(), None);
    }

    #[test]
    fn writes_a_hyprland_bind_for_the_window_action_once_it_has_a_key() {
        let trigger = Trigger::parse("ALT+Print").unwrap();
        assert_eq!(
            hyprland_bind(ShortcutAction::Window, &trigger, "dev.example.App"),
            "bind = ALT, Print, global, dev.example.App:capture-window"
        );
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
            bind(ShortcutAction::Region),
            "bind = , Print, global, dev.example.App:capture-region"
        );
        assert_eq!(
            bind(ShortcutAction::Fullscreen),
            "bind = SHIFT, Print, global, dev.example.App:capture-fullscreen"
        );
    }

    #[test]
    fn finds_a_trigger_shared_by_two_actions() {
        let mut shortcuts = Shortcuts::default();
        assert_eq!(shortcuts.conflict(), None);
        shortcuts.set(ShortcutAction::Fullscreen, shortcuts.capture_region.clone());
        let conflict = shortcuts.conflict().unwrap();
        assert_eq!(conflict.first, ShortcutAction::Region);
        assert_eq!(conflict.second, ShortcutAction::Fullscreen);
        shortcuts.set(ShortcutAction::Region, Slot::Off);
        shortcuts.set(ShortcutAction::Fullscreen, Slot::Off);
        assert_eq!(shortcuts.conflict(), None);
    }
}
