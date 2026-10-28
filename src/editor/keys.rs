use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::document::Tool;

const PUNCTUATION: &str = "[]-=,./;'`\\";
const RESERVED: &str = "dfswz";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawAction {
    Tool(Tool),
    Smaller,
    Larger,
    TurnLeft,
    TurnRight,
}

impl DrawAction {
    pub const ALL: [Self; 18] = [
        Self::Tool(Tool::Select),
        Self::Tool(Tool::Rectangle),
        Self::Tool(Tool::Ellipse),
        Self::Tool(Tool::Arrow),
        Self::Tool(Tool::Line),
        Self::Tool(Tool::Pen),
        Self::Tool(Tool::Highlight),
        Self::Tool(Tool::Step),
        Self::Tool(Tool::Text),
        Self::Tool(Tool::Check),
        Self::Tool(Tool::Cross),
        Self::Tool(Tool::Emoji),
        Self::Tool(Tool::Pixelate),
        Self::Tool(Tool::Crop),
        Self::Smaller,
        Self::Larger,
        Self::TurnLeft,
        Self::TurnRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tool(tool) => tool.label(),
            Self::Smaller => "Thinner lines",
            Self::Larger => "Thicker lines",
            Self::TurnLeft => "Turn left",
            Self::TurnRight => "Turn right",
        }
    }

    pub fn default_key(self) -> DrawKey {
        match self {
            Self::Tool(tool) => tool.key().chars().next().map_or(DrawKey::Off, DrawKey::Key),
            Self::Smaller => DrawKey::Key('['),
            Self::Larger => DrawKey::Key(']'),
            Self::TurnLeft => DrawKey::Key(','),
            Self::TurnRight => DrawKey::Key('.'),
        }
    }
}

/// One key that triggers a drawing action, pressed without modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum DrawKey {
    Key(char),
    Off,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KeyError {
    #[error("{0} can't be a drawing key; use a letter, a digit or punctuation")]
    Invalid(String),
    #[error("{} is taken by the capture overlay", .0.to_ascii_uppercase())]
    Reserved(char),
}

impl DrawKey {
    /// Accepts a lower-case letter, a digit or common punctuation that the overlay does not use.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(DrawKey::new('R'), Ok(DrawKey::Key('r')));
    /// assert!(DrawKey::new('f').is_err());
    /// ```
    pub fn new(key: char) -> Result<Self, KeyError> {
        let key = key.to_ascii_lowercase();
        if !(key.is_ascii_lowercase() || key.is_ascii_digit() || PUNCTUATION.contains(key)) {
            return Err(KeyError::Invalid(key.to_string()));
        }
        if RESERVED.contains(key) {
            return Err(KeyError::Reserved(key));
        }
        Ok(Self::Key(key))
    }

    pub fn label(self) -> String {
        match self {
            Self::Key(key) => key.to_ascii_uppercase().to_string(),
            Self::Off => "Off".to_owned(),
        }
    }
}

impl TryFrom<String> for DrawKey {
    type Error = KeyError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        let mut chars = source.chars();
        match (chars.next(), chars.next()) {
            (None, _) => Ok(Self::Off),
            (Some(key), None) => Self::new(key),
            _ => Err(KeyError::Invalid(source)),
        }
    }
}

impl From<DrawKey> for String {
    fn from(key: DrawKey) -> Self {
        match key {
            DrawKey::Key(key) => key.to_string(),
            DrawKey::Off => Self::new(),
        }
    }
}

impl fmt::Display for DrawKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrawingKeys {
    pub select: DrawKey,
    #[serde(rename = "box")]
    pub rectangle: DrawKey,
    pub ellipse: DrawKey,
    pub arrow: DrawKey,
    pub line: DrawKey,
    pub pen: DrawKey,
    pub marker: DrawKey,
    pub step: DrawKey,
    pub text: DrawKey,
    pub check: DrawKey,
    pub cross: DrawKey,
    pub emoji: DrawKey,
    pub pixelate: DrawKey,
    pub crop: DrawKey,
    pub thinner: DrawKey,
    pub thicker: DrawKey,
    pub turn_left: DrawKey,
    pub turn_right: DrawKey,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{key} is set for both {} and {}", first.label(), second.label())]
pub struct KeyConflict {
    pub key: DrawKey,
    pub first: DrawAction,
    pub second: DrawAction,
}

impl Default for DrawingKeys {
    fn default() -> Self {
        let mut keys = Self {
            select: DrawKey::Off,
            rectangle: DrawKey::Off,
            ellipse: DrawKey::Off,
            arrow: DrawKey::Off,
            line: DrawKey::Off,
            pen: DrawKey::Off,
            marker: DrawKey::Off,
            step: DrawKey::Off,
            text: DrawKey::Off,
            check: DrawKey::Off,
            cross: DrawKey::Off,
            emoji: DrawKey::Off,
            pixelate: DrawKey::Off,
            crop: DrawKey::Off,
            thinner: DrawKey::Off,
            thicker: DrawKey::Off,
            turn_left: DrawKey::Off,
            turn_right: DrawKey::Off,
        };
        for action in DrawAction::ALL {
            keys.set(action, action.default_key());
        }
        keys
    }
}

impl DrawingKeys {
    pub fn get(&self, action: DrawAction) -> DrawKey {
        *self.slot(action)
    }

    pub fn set(&mut self, action: DrawAction, key: DrawKey) {
        *self.slot_mut(action) = key;
    }

    pub fn is_default(&self, action: DrawAction) -> bool {
        self.get(action) == action.default_key()
    }

    /// Returns the action bound to `key`, pressed without modifiers.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(DrawingKeys::default().action('r'), Some(DrawAction::Tool(Tool::Rectangle)));
    /// ```
    pub fn action(&self, key: char) -> Option<DrawAction> {
        let key = DrawKey::Key(key.to_ascii_lowercase());
        DrawAction::ALL
            .into_iter()
            .find(|action| self.get(*action) == key)
    }

    /// Finds the first key that two actions share.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if let Some(conflict) = keys.conflict() { warn!("{conflict}"); }
    /// ```
    pub fn conflict(&self) -> Option<KeyConflict> {
        let bound: Vec<(DrawAction, DrawKey)> = DrawAction::ALL
            .into_iter()
            .map(|action| (action, self.get(action)))
            .filter(|(_, key)| *key != DrawKey::Off)
            .collect();
        bound.iter().enumerate().find_map(|(index, (first, key))| {
            bound[index + 1..]
                .iter()
                .find(|(_, other)| other == key)
                .map(|(second, _)| KeyConflict {
                    key: *key,
                    first: *first,
                    second: *second,
                })
        })
    }

    fn slot(&self, action: DrawAction) -> &DrawKey {
        match action {
            DrawAction::Tool(Tool::Select) => &self.select,
            DrawAction::Tool(Tool::Rectangle) => &self.rectangle,
            DrawAction::Tool(Tool::Ellipse) => &self.ellipse,
            DrawAction::Tool(Tool::Arrow) => &self.arrow,
            DrawAction::Tool(Tool::Line) => &self.line,
            DrawAction::Tool(Tool::Pen) => &self.pen,
            DrawAction::Tool(Tool::Highlight) => &self.marker,
            DrawAction::Tool(Tool::Step) => &self.step,
            DrawAction::Tool(Tool::Text) => &self.text,
            DrawAction::Tool(Tool::Check) => &self.check,
            DrawAction::Tool(Tool::Cross) => &self.cross,
            DrawAction::Tool(Tool::Emoji) => &self.emoji,
            DrawAction::Tool(Tool::Pixelate) => &self.pixelate,
            DrawAction::Tool(Tool::Crop) => &self.crop,
            DrawAction::Smaller => &self.thinner,
            DrawAction::Larger => &self.thicker,
            DrawAction::TurnLeft => &self.turn_left,
            DrawAction::TurnRight => &self.turn_right,
        }
    }

    fn slot_mut(&mut self, action: DrawAction) -> &mut DrawKey {
        match action {
            DrawAction::Tool(Tool::Select) => &mut self.select,
            DrawAction::Tool(Tool::Rectangle) => &mut self.rectangle,
            DrawAction::Tool(Tool::Ellipse) => &mut self.ellipse,
            DrawAction::Tool(Tool::Arrow) => &mut self.arrow,
            DrawAction::Tool(Tool::Line) => &mut self.line,
            DrawAction::Tool(Tool::Pen) => &mut self.pen,
            DrawAction::Tool(Tool::Highlight) => &mut self.marker,
            DrawAction::Tool(Tool::Step) => &mut self.step,
            DrawAction::Tool(Tool::Text) => &mut self.text,
            DrawAction::Tool(Tool::Check) => &mut self.check,
            DrawAction::Tool(Tool::Cross) => &mut self.cross,
            DrawAction::Tool(Tool::Emoji) => &mut self.emoji,
            DrawAction::Tool(Tool::Pixelate) => &mut self.pixelate,
            DrawAction::Tool(Tool::Crop) => &mut self.crop,
            DrawAction::Smaller => &mut self.thinner,
            DrawAction::Larger => &mut self.thicker,
            DrawAction::TurnLeft => &mut self.turn_left,
            DrawAction::TurnRight => &mut self.turn_right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_the_tool_letters_and_brackets() {
        let keys = DrawingKeys::default();
        assert_eq!(keys.action('r'), Some(DrawAction::Tool(Tool::Rectangle)));
        assert_eq!(keys.action('R'), Some(DrawAction::Tool(Tool::Rectangle)));
        assert_eq!(keys.action('['), Some(DrawAction::Smaller));
        assert_eq!(keys.action(']'), Some(DrawAction::Larger));
        assert_eq!(keys.action('q'), None);
        assert_eq!(keys.conflict(), None);
        for action in DrawAction::ALL {
            assert!(keys.is_default(action));
            assert_ne!(keys.get(action), DrawKey::Off);
        }
    }

    #[test]
    fn refuses_overlay_keys_and_odd_characters() {
        assert_eq!(DrawKey::new('B'), Ok(DrawKey::Key('b')));
        assert_eq!(DrawKey::new('7'), Ok(DrawKey::Key('7')));
        assert_eq!(DrawKey::new('f'), Err(KeyError::Reserved('f')));
        assert_eq!(DrawKey::new('S'), Err(KeyError::Reserved('s')));
        assert!(matches!(DrawKey::new('é'), Err(KeyError::Invalid(_))));
        assert!(matches!(DrawKey::new(' '), Err(KeyError::Invalid(_))));
        assert_eq!(DrawKey::try_from(String::new()), Ok(DrawKey::Off));
        assert!(DrawKey::try_from("rr".to_owned()).is_err());
    }

    #[test]
    fn finds_a_key_shared_by_two_actions() {
        let mut keys = DrawingKeys::default();
        keys.set(DrawAction::Tool(Tool::Pen), DrawKey::Key('r'));
        let conflict = keys.conflict().unwrap();
        assert_eq!(conflict.first, DrawAction::Tool(Tool::Rectangle));
        assert_eq!(conflict.second, DrawAction::Tool(Tool::Pen));
        keys.set(DrawAction::Tool(Tool::Rectangle), DrawKey::Off);
        assert_eq!(keys.conflict(), None);
        assert_eq!(keys.action('r'), Some(DrawAction::Tool(Tool::Pen)));
    }
}
