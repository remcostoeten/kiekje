use std::cmp::Ordering;
use std::fmt;

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<Identifier>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Identifier {
    Numeric(u64),
    Alphanumeric(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a version like 1.2.3 or 1.2.3-alpha.4")]
pub struct VersionError(String);

impl Version {
    /// Parses a semantic version, with or without a leading `v`; build metadata is dropped.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let tag = Version::parse("v0.1.0-alpha.5")?;
    /// assert!(tag < Version::parse("0.1.0")?);
    /// ```
    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let invalid = || VersionError(text.to_owned());
        let bare = text.strip_prefix('v').unwrap_or(text);
        let bare = bare.split_once('+').map_or(bare, |(version, _)| version);
        let (core, pre) = match bare.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (bare, None),
        };
        let mut numbers = core.split('.').map(number);
        let (Some(Some(major)), Some(Some(minor)), Some(Some(patch)), None) = (
            numbers.next(),
            numbers.next(),
            numbers.next(),
            numbers.next(),
        ) else {
            return Err(invalid());
        };
        let pre = match pre {
            Some(pre) => pre
                .split('.')
                .map(identifier)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(invalid)?,
            None => Vec::new(),
        };
        Ok(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    /// Whether this is a pre-release such as `0.1.0-alpha.5`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(Version::parse("0.1.0-alpha.5")?.is_prerelease());
    /// ```
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

fn number(part: &str) -> Option<u64> {
    let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let leading_zero = part.len() > 1 && part.starts_with('0');
    if digits && !leading_zero {
        part.parse().ok()
    } else {
        None
    }
}

fn identifier(part: &str) -> Option<Identifier> {
    if part.is_empty() {
        return None;
    }
    if part.bytes().all(|byte| byte.is_ascii_digit()) {
        return number(part).map(Identifier::Numeric);
    }
    part.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        .then(|| Identifier::Alphanumeric(part.to_owned()))
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (index, part) in self.pre.iter().enumerate() {
            let separator = if index == 0 { '-' } else { '.' };
            match part {
                Identifier::Numeric(value) => write!(formatter, "{separator}{value}")?,
                Identifier::Alphanumeric(value) => write!(formatter, "{separator}{value}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn compares_numeric_prerelease_parts_as_numbers() {
        assert!(version("0.1.0-alpha.5") < version("0.1.0-alpha.10"));
        assert!(version("0.1.0-alpha.9") < version("0.1.0-alpha.10"));
    }

    #[test]
    fn ranks_a_prerelease_below_its_release() {
        assert!(version("0.1.0-alpha.10") < version("0.1.0"));
        assert!(version("0.1.0") < version("0.1.1-alpha.1"));
    }

    #[test]
    fn follows_semver_precedence_between_prerelease_kinds() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(version(pair[0]) < version(pair[1]), "{pair:?}");
        }
    }

    #[test]
    fn compares_the_core_numbers_before_the_prerelease() {
        assert!(version("0.9.9") < version("0.10.0-alpha.1"));
        assert!(version("1.2.3-rc.1") > version("1.2.2"));
    }

    #[test]
    fn reads_a_tag_with_a_v_and_ignores_build_metadata() {
        assert_eq!(version("v0.1.0-alpha.5"), version("0.1.0-alpha.5"));
        assert_eq!(version("1.0.0+build.7"), version("1.0.0"));
        assert_eq!(version("v0.1.0-alpha.5").to_string(), "0.1.0-alpha.5");
    }

    #[test]
    fn reads_the_crate_version() {
        assert!(Version::parse(env!("CARGO_PKG_VERSION")).is_ok());
    }

    #[test]
    fn rejects_text_that_is_not_a_version() {
        for text in [
            "",
            "nightly",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-",
            "1.2.3-alpha..1",
            "1.2.3-alpha_1",
        ] {
            assert!(Version::parse(text).is_err(), "{text}");
        }
    }
}
