use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const BARE_KEYS: [&str; 1] = ["Print"];
const MODIFIER_KEYS: [&str; 16] = [
    "Shift_L",
    "Shift_R",
    "Control_L",
    "Control_R",
    "Alt_L",
    "Alt_R",
    "Meta_L",
    "Meta_R",
    "Super_L",
    "Super_R",
    "Hyper_L",
    "Hyper_R",
    "ISO_Level3_Shift",
    "ISO_Level5_Shift",
    "Caps_Lock",
    "Num_Lock",
];
const RESERVED: [&str; 6] = [
    "ALT+Tab",
    "ALT+SHIFT+Tab",
    "ALT+F4",
    "CTRL+ALT+Delete",
    "CTRL+ALT+BackSpace",
    "LOGO+Tab",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
}

impl Modifiers {
    fn names(self) -> impl Iterator<Item = &'static str> {
        [
            (self.ctrl, "CTRL"),
            (self.alt, "ALT"),
            (self.shift, "SHIFT"),
            (self.logo, "LOGO"),
        ]
        .into_iter()
        .filter_map(|(held, name)| held.then_some(name))
    }

    fn any_besides_shift(self) -> bool {
        self.ctrl || self.alt || self.logo
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Trigger {
    modifiers: Modifiers,
    key: String,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TriggerError {
    #[error("the shortcut is empty")]
    Empty,
    #[error("`{0}` is not a modifier; use CTRL, ALT, SHIFT or LOGO")]
    UnknownModifier(String),
    #[error("`{0}` is not a key name")]
    InvalidKey(String),
    #[error("a shortcut needs a key besides its modifiers")]
    OnlyModifiers,
    #[error("{0} needs Ctrl, Alt or Super, or it would fire while typing")]
    NeedsModifier(String),
    #[error("{0} is reserved by the desktop")]
    Reserved(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recorded {
    Waiting,
    Canceled,
    Cleared,
    Done(Result<Trigger, TriggerError>),
}

impl Trigger {
    /// Builds a trigger from held modifiers and a GDK key name, applying the binding rules.
    ///
    /// Single letters are stored in lower case. A key with no modifier is only allowed for
    /// Print, Shift alone does not count, and desktop combos like Alt+Tab are refused.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let trigger = Trigger::new(Modifiers { logo: true, shift: true, ..Default::default() }, "S")?;
    /// assert_eq!(trigger.to_string(), "SHIFT+LOGO+s");
    /// ```
    pub fn new(modifiers: Modifiers, key: &str) -> Result<Self, TriggerError> {
        if key.is_empty() {
            return Err(TriggerError::OnlyModifiers);
        }
        if MODIFIER_KEYS.contains(&key) {
            return Err(TriggerError::OnlyModifiers);
        }
        if !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(TriggerError::InvalidKey(key.to_owned()));
        }
        let key = if key.chars().count() == 1 {
            key.to_ascii_lowercase()
        } else {
            key.to_owned()
        };
        let trigger = Self { modifiers, key };
        if !modifiers.any_besides_shift() && !BARE_KEYS.contains(&trigger.key.as_str()) {
            return Err(TriggerError::NeedsModifier(trigger.label()));
        }
        if RESERVED.contains(&trigger.to_string().as_str()) {
            return Err(TriggerError::Reserved(trigger.label()));
        }
        Ok(trigger)
    }

    /// Parses a trigger in the XDG shortcuts format, such as `CTRL+SHIFT+s`.
    ///
    /// Modifier names ignore case, and `SUPER`, `META` and `CONTROL` are accepted as aliases.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(Trigger::parse("super+shift+S")?.to_string(), "SHIFT+LOGO+s");
    /// ```
    pub fn parse(source: &str) -> Result<Self, TriggerError> {
        let source = source.trim();
        if source.is_empty() {
            return Err(TriggerError::Empty);
        }
        let (prefix, key) = source.rsplit_once('+').unwrap_or(("", source));
        let mut modifiers = Modifiers::default();
        for name in prefix.split('+').filter(|name| !name.is_empty()) {
            match name.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => modifiers.ctrl = true,
                "ALT" => modifiers.alt = true,
                "SHIFT" => modifiers.shift = true,
                "LOGO" | "SUPER" | "META" => modifiers.logo = true,
                _ => return Err(TriggerError::UnknownModifier(name.to_owned())),
            }
        }
        Self::new(modifiers, key.trim())
    }

    /// The trigger as shown to people, such as `Ctrl+Shift+S` or `Super+Print`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(Trigger::parse("LOGO+s")?.label(), "Super+S");
    /// ```
    pub fn label(&self) -> String {
        let modifiers = [
            (self.modifiers.ctrl, "Ctrl"),
            (self.modifiers.alt, "Alt"),
            (self.modifiers.shift, "Shift"),
            (self.modifiers.logo, "Super"),
        ];
        let mut parts: Vec<String> = modifiers
            .into_iter()
            .filter(|(held, _)| *held)
            .map(|(_, name)| name.to_owned())
            .collect();
        let key = if self.key.chars().count() == 1 {
            self.key.to_ascii_uppercase()
        } else {
            self.key.clone()
        };
        parts.push(key);
        parts.join("+")
    }

    /// The modifiers and key as Hyprland's `bind` takes them, such as `("SUPER SHIFT", "s")`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let (modifiers, key) = trigger.hyprland();
    /// let line = format!("bind = {modifiers}, {key}, global, {APP_ID}:capture-region");
    /// ```
    pub fn hyprland(&self) -> (String, &str) {
        let modifiers = self
            .modifiers
            .names()
            .map(|name| if name == "LOGO" { "SUPER" } else { name })
            .collect::<Vec<_>>()
            .join(" ");
        (modifiers, &self.key)
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for name in self.modifiers.names() {
            write!(formatter, "{name}+")?;
        }
        formatter.write_str(&self.key)
    }
}

/// Turns one key press in the shortcut recorder into the recorder's next state.
///
/// A modifier on its own keeps waiting, Escape cancels and Backspace clears the shortcut,
/// both only without modifiers. Anything else finishes with a trigger or the rule it broke.
///
/// # Examples
///
/// ```ignore
/// match record(&key_name, modifiers) {
///     Recorded::Done(Ok(trigger)) => save(trigger),
///     _ => {}
/// }
/// ```
pub fn record(key: &str, modifiers: Modifiers) -> Recorded {
    if MODIFIER_KEYS.contains(&key) {
        return Recorded::Waiting;
    }
    if modifiers == Modifiers::default() {
        match key {
            "Escape" => return Recorded::Canceled,
            "BackSpace" | "Delete" => return Recorded::Cleared,
            _ => {}
        }
    }
    Recorded::Done(Trigger::new(modifiers, key))
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum Slot {
    Off,
    Key(Trigger),
}

impl Slot {
    pub fn trigger(&self) -> Option<&Trigger> {
        match self {
            Self::Off => None,
            Self::Key(trigger) => Some(trigger),
        }
    }
}

impl TryFrom<String> for Slot {
    type Error = TriggerError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        if source.trim().is_empty() {
            return Ok(Self::Off);
        }
        Trigger::parse(&source).map(Self::Key)
    }
}

impl From<Slot> for String {
    fn from(slot: Slot) -> Self {
        match slot {
            Slot::Off => String::new(),
            Slot::Key(trigger) => trigger.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(ctrl: bool, alt: bool, shift: bool, logo: bool) -> Modifiers {
        Modifiers {
            ctrl,
            alt,
            shift,
            logo,
        }
    }

    #[test]
    fn writes_modifiers_in_xdg_order_with_a_lowercase_letter() {
        let trigger = Trigger::new(held(false, false, true, true), "S").unwrap();
        assert_eq!(trigger.to_string(), "SHIFT+LOGO+s");
        assert_eq!(trigger.label(), "Shift+Super+S");
    }

    #[test]
    fn parses_aliases_in_any_case() {
        assert_eq!(
            Trigger::parse("super+Control+S").unwrap().to_string(),
            "CTRL+LOGO+s"
        );
        assert_eq!(
            Trigger::parse("SHIFT+Print").unwrap().label(),
            "Shift+Print"
        );
    }

    #[test]
    fn allows_print_without_a_modifier_only() {
        assert!(Trigger::parse("Print").is_ok());
        assert_eq!(
            Trigger::parse("s"),
            Err(TriggerError::NeedsModifier("S".into()))
        );
        assert_eq!(
            Trigger::parse("SHIFT+F5"),
            Err(TriggerError::NeedsModifier("Shift+F5".into()))
        );
        assert!(Trigger::parse("ALT+F5").is_ok());
    }

    #[test]
    fn rejects_desktop_combos_and_bad_names() {
        assert_eq!(
            Trigger::parse("ALT+Tab"),
            Err(TriggerError::Reserved("Alt+Tab".into()))
        );
        assert_eq!(
            Trigger::parse("HYPER+s"),
            Err(TriggerError::UnknownModifier("HYPER".into()))
        );
        assert_eq!(
            Trigger::parse("CTRL+Shift_L"),
            Err(TriggerError::OnlyModifiers)
        );
        assert_eq!(
            Trigger::parse("CTRL+a b"),
            Err(TriggerError::InvalidKey("a b".into()))
        );
        assert_eq!(Trigger::parse("  "), Err(TriggerError::Empty));
    }

    #[test]
    fn splits_for_hyprland_with_super() {
        let trigger = Trigger::parse("SHIFT+LOGO+s").unwrap();
        assert_eq!(trigger.hyprland(), ("SHIFT SUPER".to_owned(), "s"));
        let bare = Trigger::parse("Print").unwrap();
        assert_eq!(bare.hyprland(), (String::new(), "Print"));
    }

    #[test]
    fn records_until_a_non_modifier_key() {
        let ctrl = held(true, false, false, false);
        assert_eq!(record("Control_L", ctrl), Recorded::Waiting);
        assert_eq!(
            record("k", ctrl),
            Recorded::Done(Ok(Trigger::parse("CTRL+k").unwrap()))
        );
        assert_eq!(
            record("k", Modifiers::default()),
            Recorded::Done(Err(TriggerError::NeedsModifier("K".into())))
        );
    }

    #[test]
    fn escape_cancels_and_backspace_clears_without_modifiers() {
        assert_eq!(record("Escape", Modifiers::default()), Recorded::Canceled);
        assert_eq!(record("BackSpace", Modifiers::default()), Recorded::Cleared);
        assert_eq!(
            record("BackSpace", held(true, true, false, false)),
            Recorded::Done(Err(TriggerError::Reserved("Ctrl+Alt+BackSpace".into())))
        );
    }

    #[test]
    fn reads_an_empty_slot_as_off() {
        assert_eq!(Slot::try_from(String::new()), Ok(Slot::Off));
        assert_eq!(String::from(Slot::Off), "");
        let slot = Slot::try_from("SHIFT+Print".to_owned()).unwrap();
        assert_eq!(String::from(slot), "SHIFT+Print");
    }
}
