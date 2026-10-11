use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_QUALITY: u8 = 90;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    #[serde(alias = "jpg")]
    Jpeg,
}

impl ImageFormat {
    pub const ALL: [Self; 2] = [Self::Png, Self::Jpeg];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
        }
    }

    /// Reads the format a file name asks for from its extension.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ImageFormat::from_path(Path::new("shot.JPEG")), Some(ImageFormat::Jpeg));
    /// ```
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Quality(u8);

#[derive(Debug, Error, PartialEq, Eq)]
#[error("quality {0} must be between 1 and 100")]
pub struct QualityError(u8);

impl Quality {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 100;

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for Quality {
    fn default() -> Self {
        Self(DEFAULT_QUALITY)
    }
}

impl TryFrom<u8> for Quality {
    type Error = QualityError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(QualityError(value))
        }
    }
}

impl From<Quality> for u8 {
    fn from(quality: Quality) -> Self {
        quality.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_format_from_extension_in_any_case() {
        assert_eq!(
            ImageFormat::from_path(Path::new("a.PNG")),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::from_path(Path::new("a.jpeg")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_path(Path::new("a.Jpg")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(ImageFormat::from_path(Path::new("a.webp")), None);
        assert_eq!(ImageFormat::from_path(Path::new("a")), None);
    }

    #[test]
    fn rejects_quality_outside_one_to_hundred() {
        assert_eq!(Quality::try_from(0), Err(QualityError(0)));
        assert_eq!(Quality::try_from(101), Err(QualityError(101)));
        assert_eq!(Quality::try_from(100).map(Quality::get), Ok(100));
        assert_eq!(Quality::default().get(), DEFAULT_QUALITY);
    }
}
