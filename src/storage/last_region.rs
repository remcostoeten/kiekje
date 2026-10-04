use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use gtk::glib;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::write_atomic;
use crate::capture::LogicalRect;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    region: LogicalRect,
}

#[derive(Debug, Error)]
pub enum LastRegionError {
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{} is not valid: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("could not encode the last region: {0}")]
    Encode(#[from] toml::ser::Error),
}

/// Returns `$XDG_DATA_HOME/kiekje/last-region.toml`.
///
/// # Examples
///
/// ```ignore
/// let region = last_region::load_from(&last_region::path())?;
/// ```
pub fn path() -> PathBuf {
    glib::user_data_dir()
        .join("kiekje")
        .join("last-region.toml")
}

/// Reads the last selected region in desktop logical coordinates, or `None` when there is none.
///
/// # Examples
///
/// ```ignore
/// let region = load_from(&path())?;
/// ```
pub fn load_from(path: &Path) -> Result<Option<LogicalRect>, LastRegionError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(LastRegionError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let stored: Stored = toml::from_str(&text).map_err(|source| LastRegionError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(Some(stored.region))
}

/// Remembers `region` as the last selection, replacing the file atomically.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// save_to(&path(), selection.rect)?;
/// ```
pub fn save_to(path: &Path, region: LogicalRect) -> Result<(), LastRegionError> {
    let text = toml::to_string_pretty(&Stored { region })?;
    write_atomic(path, text.as_bytes()).map_err(|source| LastRegionError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Loads the last region from its default path, logging why when the file can't be used.
///
/// # Examples
///
/// ```ignore
/// let region = last_region::load();
/// ```
pub fn load() -> Option<LogicalRect> {
    load_from(&path()).unwrap_or_else(|error| {
        eprintln!("kiekje: ignoring the last region: {error}");
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("kiekje-last-region-{name}-{}", std::process::id()))
            .join("last-region.toml")
    }

    #[test]
    fn reads_a_missing_file_as_none() {
        assert!(load_from(&scratch("missing")).unwrap().is_none());
    }

    #[test]
    fn round_trips_a_region_with_a_negative_origin() {
        let path = scratch("round-trip");
        let region = LogicalRect {
            x: -1280,
            y: 40,
            width: 640,
            height: 360,
        };
        save_to(&path, region).unwrap();
        assert_eq!(load_from(&path).unwrap(), Some(region));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_a_file_without_a_complete_region() {
        let path = scratch("partial");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[region]\nx = 1\ny = 2\n").unwrap();
        assert!(matches!(
            load_from(&path),
            Err(LastRegionError::Parse { .. })
        ));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
