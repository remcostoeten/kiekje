use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capture::CaptureMode;

pub const DEFAULT_TEMPLATE: &str = "kiekje-{mode}-{date}-{time}";
const MAX_LENGTH: usize = 200;
// 255 bytes per file name, minus room for a `-1000` duplicate suffix and the extension.
const MAX_BYTES: usize = 255 - 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Mode,
    Date,
    Time,
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Width,
    Height,
}

impl Token {
    fn widest(self) -> usize {
        match self {
            Self::Mode => "fullscreen".len(),
            Self::Date => "2026-10-10".len(),
            Self::Time => "12-00-00".len(),
            Self::Year => 4,
            Self::Month | Self::Day | Self::Hour | Self::Minute | Self::Second => 2,
            Self::Width | Self::Height => u32::MAX.to_string().len(),
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "mode" => Some(Self::Mode),
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "year" => Some(Self::Year),
            "month" => Some(Self::Month),
            "day" => Some(Self::Day),
            "hour" => Some(Self::Hour),
            "minute" => Some(Self::Minute),
            "second" => Some(Self::Second),
            "width" => Some(Self::Width),
            "height" => Some(Self::Height),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Text(String),
    Token(Token),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("the file name template is empty")]
    Empty,
    #[error("the file name template makes names longer than a file name can be")]
    TooLong,
    #[error("unknown token `{{{0}}}` in the file name template")]
    UnknownToken(String),
    #[error("unclosed `{{` in the file name template")]
    Unclosed,
    #[error("stray `}}` in the file name template")]
    StrayBrace,
    #[error("the file name template contains `{0}`, which a file name can't hold")]
    ForbiddenCharacter(char),
    #[error("the file name template can't start with a dot")]
    LeadingDot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub struct Naming {
    pub mode: CaptureMode,
    pub stamp: Stamp,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Template {
    source: String,
    parts: Vec<Part>,
}

impl Template {
    /// Parses a template and rejects anything that could not render to a plain file name.
    ///
    /// Tokens are `{mode}`, `{date}`, `{time}`, `{year}`, `{month}`, `{day}`, `{hour}`,
    /// `{minute}`, `{second}`, `{width}` and `{height}`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let template = Template::parse("shot-{date}")?;
    /// ```
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        if source.trim().is_empty() {
            return Err(TemplateError::Empty);
        }
        if source.chars().count() > MAX_LENGTH {
            return Err(TemplateError::TooLong);
        }
        if source.starts_with('.') {
            return Err(TemplateError::LeadingDot);
        }

        let mut parts = Vec::new();
        let mut text = String::new();
        let mut characters = source.chars();
        while let Some(character) = characters.next() {
            match character {
                '{' => {
                    let mut name = String::new();
                    loop {
                        match characters.next() {
                            Some('}') => break,
                            Some(next) => name.push(next),
                            None => return Err(TemplateError::Unclosed),
                        }
                    }
                    let token = Token::parse(&name).ok_or(TemplateError::UnknownToken(name))?;
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Token(token));
                }
                '}' => return Err(TemplateError::StrayBrace),
                '/' | '\\' | '\0' => return Err(TemplateError::ForbiddenCharacter(character)),
                control if control.is_control() => {
                    return Err(TemplateError::ForbiddenCharacter(control));
                }
                other => text.push(other),
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        let widest: usize = parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => text.len(),
                Part::Token(token) => token.widest(),
            })
            .sum();
        if widest > MAX_BYTES {
            return Err(TemplateError::TooLong);
        }

        Ok(Self {
            source: source.to_owned(),
            parts,
        })
    }

    /// Renders the file name without its extension.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let base = template.render(&naming);
    /// let (path, file) = create_unique(&directory, &base, "png")?;
    /// ```
    pub fn render(&self, naming: &Naming) -> String {
        let stamp = naming.stamp;
        let mut name = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => name.push_str(text),
                Part::Token(Token::Mode) => name.push_str(naming.mode.file_label()),
                Part::Token(Token::Date) => name.push_str(&format!(
                    "{:04}-{:02}-{:02}",
                    stamp.year, stamp.month, stamp.day
                )),
                Part::Token(Token::Time) => name.push_str(&format!(
                    "{:02}-{:02}-{:02}",
                    stamp.hour, stamp.minute, stamp.second
                )),
                Part::Token(Token::Year) => name.push_str(&format!("{:04}", stamp.year)),
                Part::Token(Token::Month) => name.push_str(&format!("{:02}", stamp.month)),
                Part::Token(Token::Day) => name.push_str(&format!("{:02}", stamp.day)),
                Part::Token(Token::Hour) => name.push_str(&format!("{:02}", stamp.hour)),
                Part::Token(Token::Minute) => name.push_str(&format!("{:02}", stamp.minute)),
                Part::Token(Token::Second) => name.push_str(&format!("{:02}", stamp.second)),
                Part::Token(Token::Width) => name.push_str(&naming.width.to_string()),
                Part::Token(Token::Height) => name.push_str(&naming.height.to_string()),
            }
        }
        name
    }
}

impl Default for Template {
    fn default() -> Self {
        Self {
            source: DEFAULT_TEMPLATE.to_owned(),
            parts: vec![
                Part::Text("kiekje-".into()),
                Part::Token(Token::Mode),
                Part::Text("-".into()),
                Part::Token(Token::Date),
                Part::Text("-".into()),
                Part::Token(Token::Time),
            ],
        }
    }
}

impl fmt::Display for Template {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source)
    }
}

impl TryFrom<String> for Template {
    type Error = TemplateError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        Self::parse(&source)
    }
}

impl From<Template> for String {
    fn from(template: Template) -> Self {
        template.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naming() -> Naming {
        Naming {
            mode: CaptureMode::Region,
            stamp: Stamp {
                year: 2026,
                month: 3,
                day: 7,
                hour: 9,
                minute: 5,
                second: 4,
            },
            width: 1280,
            height: 800,
        }
    }

    #[test]
    fn renders_the_default_template() {
        assert_eq!(
            Template::default().render(&naming()),
            "kiekje-region-2026-03-07-09-05-04"
        );
    }

    #[test]
    fn names_a_repeated_region_as_a_region() {
        let repeat = Naming {
            mode: CaptureMode::Repeat,
            ..naming()
        };
        assert_eq!(
            Template::default().render(&repeat),
            "kiekje-region-2026-03-07-09-05-04"
        );
    }

    #[test]
    fn default_matches_its_parsed_source() {
        assert_eq!(
            Template::parse(DEFAULT_TEMPLATE).unwrap(),
            Template::default()
        );
    }

    #[test]
    fn renders_every_token_zero_padded() {
        let template =
            Template::parse("{year}{month}{day}_{hour}{minute}{second} {width}x{height} {mode}")
                .unwrap();
        assert_eq!(
            template.render(&naming()),
            "20260307_090504 1280x800 region"
        );
    }

    #[test]
    fn keeps_literal_text_without_tokens() {
        assert_eq!(
            Template::parse("screenshot").unwrap().render(&naming()),
            "screenshot"
        );
    }

    #[test]
    fn rejects_unknown_tokens() {
        assert_eq!(
            Template::parse("shot-{weekday}"),
            Err(TemplateError::UnknownToken("weekday".into()))
        );
    }

    #[test]
    fn rejects_unbalanced_braces() {
        assert_eq!(Template::parse("shot-{date"), Err(TemplateError::Unclosed));
        assert_eq!(
            Template::parse("shot-date}"),
            Err(TemplateError::StrayBrace)
        );
    }

    #[test]
    fn rejects_path_separators_and_control_characters() {
        assert_eq!(Template::parse("../{date}"), Err(TemplateError::LeadingDot));
        assert_eq!(
            Template::parse("a/{date}"),
            Err(TemplateError::ForbiddenCharacter('/'))
        );
        assert_eq!(
            Template::parse("a\\b"),
            Err(TemplateError::ForbiddenCharacter('\\'))
        );
        assert_eq!(
            Template::parse("a\nb"),
            Err(TemplateError::ForbiddenCharacter('\n'))
        );
    }

    #[test]
    fn rejects_empty_and_overlong_templates() {
        assert_eq!(Template::parse("  "), Err(TemplateError::Empty));
        assert_eq!(
            Template::parse(&"a".repeat(MAX_LENGTH + 1)),
            Err(TemplateError::TooLong)
        );
    }

    #[test]
    fn rejects_templates_that_render_past_a_file_name() {
        assert_eq!(
            Template::parse(&"{date}".repeat(33)),
            Err(TemplateError::TooLong)
        );
        assert_eq!(
            Template::parse(&"\u{732b}".repeat(80)),
            Err(TemplateError::TooLong)
        );
        assert!(Template::parse(&"{date}".repeat(23)).is_ok());
    }

    #[test]
    fn displays_the_source_it_was_parsed_from() {
        let source = "shot {date}";
        assert_eq!(Template::parse(source).unwrap().to_string(), source);
    }
}
