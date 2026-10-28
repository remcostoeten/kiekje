use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use gtk::glib;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::storage::write_atomic;

pub const CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<Available>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notified: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Available {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{} is not valid: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("could not encode the update state: {0}")]
    Encode(#[from] toml::ser::Error),
}

static WRITES: Mutex<()> = Mutex::new(());

/// Returns `$XDG_DATA_HOME/kiekje/update-check.toml`.
///
/// # Examples
///
/// ```ignore
/// let state = state::load_from(&state::path())?;
/// ```
pub fn path() -> PathBuf {
    glib::user_data_dir()
        .join("kiekje")
        .join("update-check.toml")
}

/// Reads the update check state, or the empty state when the file does not exist.
///
/// # Examples
///
/// ```ignore
/// let last = load_from(&path())?.last_checked;
/// ```
pub fn load_from(path: &Path) -> Result<State, StateError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(State::default()),
        Err(source) => {
            return Err(StateError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    toml::from_str(&text).map_err(|source| StateError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Applies `change` to the stored state and writes it back through a temporary file and a rename.
///
/// An unreadable state file is replaced, since it only holds a timestamp and a version. Touches
/// the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// state::update(&state::path(), |state| state.last_checked = Some(now))?;
/// ```
pub fn update(path: &Path, change: impl FnOnce(&mut State)) -> Result<State, StateError> {
    // The guarded value is (), so a panic in another writer leaves nothing inconsistent.
    let _writing = WRITES.lock().unwrap_or_else(PoisonError::into_inner);
    let mut state = load_from(path).unwrap_or_else(|error| {
        warn!("replacing the update state: {error}");
        State::default()
    });
    change(&mut state);
    let text = toml::to_string_pretty(&state)?;
    write_atomic(path, text.as_bytes()).map_err(|source| StateError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(state)
}

/// Whether an automatic check is due, at most once per [`CHECK_INTERVAL_SECONDS`].
///
/// A last check in the future means the clock moved back, so a check is due again.
///
/// # Examples
///
/// ```ignore
/// if is_due(state.last_checked, now) { check()?; }
/// ```
pub fn is_due(last_checked: Option<u64>, now: u64) -> bool {
    match last_checked {
        None => true,
        Some(last) if last > now => true,
        Some(last) => now - last >= CHECK_INTERVAL_SECONDS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_000_000;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("kiekje-update-{name}-{}", std::process::id()))
            .join("update-check.toml")
    }

    #[test]
    fn checks_when_it_never_checked() {
        assert!(is_due(None, NOW));
    }

    #[test]
    fn waits_a_day_between_checks() {
        assert!(!is_due(Some(NOW), NOW));
        assert!(!is_due(Some(NOW - CHECK_INTERVAL_SECONDS + 1), NOW));
        assert!(is_due(Some(NOW - CHECK_INTERVAL_SECONDS), NOW));
        assert!(is_due(Some(NOW - 3 * CHECK_INTERVAL_SECONDS), NOW));
    }

    #[test]
    fn checks_again_when_the_clock_moved_back() {
        assert!(is_due(Some(NOW + 60), NOW));
    }

    #[test]
    fn reads_a_missing_file_as_never_checked() {
        assert_eq!(load_from(&scratch("missing")).unwrap(), State::default());
    }

    #[test]
    fn round_trips_through_an_update() {
        let path = scratch("round-trip");
        let _ = fs::remove_dir_all(path.parent().unwrap());
        update(&path, |state| state.last_checked = Some(NOW)).unwrap();
        let saved = update(&path, |state| {
            state.available = Some(Available {
                version: "0.1.0-alpha.6".into(),
                url: "https://github.com/remcostoeten/kiekje/releases/tag/v0.1.0-alpha.6".into(),
            });
        })
        .unwrap();
        assert_eq!(saved.last_checked, Some(NOW));
        assert_eq!(load_from(&path).unwrap(), saved);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn replaces_an_unreadable_state() {
        let path = scratch("broken");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "last_checked = \"soon\"").unwrap();
        let saved = update(&path, |state| state.last_checked = Some(NOW)).unwrap();
        assert_eq!(saved.last_checked, Some(NOW));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
