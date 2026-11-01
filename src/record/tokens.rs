use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use gtk::glib;

use crate::storage::write_atomic;

fn path() -> PathBuf {
    glib::user_data_dir()
        .join("kiekje")
        .join("screencast-tokens.toml")
}

fn read(path: &Path) -> BTreeMap<String, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return BTreeMap::new(),
        Err(error) => {
            warn!("could not read {}: {error}", path.display());
            return BTreeMap::new();
        }
    };
    toml::from_str(&text).unwrap_or_else(|error| {
        warn!("{} is not valid, ignoring it: {error}", path.display());
        BTreeMap::new()
    })
}

fn write(path: &Path, tokens: &BTreeMap<String, String>) {
    let text = match toml::to_string_pretty(tokens) {
        Ok(text) => text,
        Err(error) => {
            error!("could not encode the screen share tokens: {error}");
            return;
        }
    };
    if let Err(error) = write_atomic(path, text.as_bytes()) {
        error!("could not write {}: {error}", path.display());
    }
}

/// Returns the screen share restore token remembered for `key`, if there is one.
///
/// # Examples
///
/// ```ignore
/// let token = tokens::load("monitor-0-0");
/// ```
pub fn load(key: &str) -> Option<String> {
    read(&path()).remove(key)
}

/// Remembers the restore token the portal returned for `key`, replacing the file atomically.
///
/// # Examples
///
/// ```ignore
/// tokens::store("monitor-0-0", "token");
/// ```
pub fn store(key: &str, token: &str) {
    let path = path();
    let mut tokens = read(&path);
    if tokens.get(key).is_some_and(|known| known == token) {
        return;
    }
    tokens.insert(key.to_owned(), token.to_owned());
    write(&path, &tokens);
}

/// Drops the restore token for `key`, so the next recording asks the picker again.
///
/// # Examples
///
/// ```ignore
/// tokens::forget("monitor-0-0");
/// ```
pub fn forget(key: &str) {
    let path = path();
    let mut tokens = read(&path);
    if tokens.remove(key).is_some() {
        write(&path, &tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nothing_from_a_missing_file() {
        assert!(read(Path::new("/nonexistent/kiekje/tokens.toml")).is_empty());
    }

    #[test]
    fn keeps_a_token_per_key_across_a_round_trip() {
        let directory = std::env::temp_dir().join(format!("kiekje-tokens-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("tokens.toml");
        let tokens = BTreeMap::from([
            ("any".to_owned(), "one".to_owned()),
            ("monitor-0-0".to_owned(), "two".to_owned()),
        ]);
        write(&file, &tokens);
        assert_eq!(read(&file), tokens);
        fs::remove_dir_all(&directory).unwrap();
    }
}
