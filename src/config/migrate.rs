use thiserror::Error;
use toml::{Table, Value};

pub const VERSION: i64 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrateError {
    #[error("version {0} is newer than this build reads (up to {VERSION})")]
    Newer(i64),
    #[error("version must be a whole number from 1 to {VERSION}")]
    Invalid,
}

/// Brings a parsed config table up to the current version.
///
/// A file without `version` counts as the current version, because every released build writes it.
///
/// # Examples
///
/// ```ignore
/// let table = migrate(toml::from_str(&text)?)?;
/// ```
pub fn migrate(mut table: Table) -> Result<Table, MigrateError> {
    let version = match table.get("version") {
        None => VERSION,
        Some(Value::Integer(version)) => *version,
        Some(_) => return Err(MigrateError::Invalid),
    };
    match version {
        VERSION => {}
        newer if newer > VERSION => return Err(MigrateError::Newer(newer)),
        _ => return Err(MigrateError::Invalid),
    }
    table.insert("version".into(), Value::Integer(VERSION));
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Table {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn stamps_the_current_version_on_an_unversioned_file() {
        let migrated = migrate(table("[storage]\nfilename = \"a\"")).unwrap();
        assert_eq!(migrated.get("version"), Some(&Value::Integer(VERSION)));
        assert!(migrated.contains_key("storage"));
    }

    #[test]
    fn keeps_a_current_file_unchanged() {
        let current = table("version = 1\n[storage]\nfilename = \"a\"");
        assert_eq!(migrate(current.clone()).unwrap(), current);
    }

    #[test]
    fn refuses_a_file_from_a_newer_build() {
        assert_eq!(migrate(table("version = 2")), Err(MigrateError::Newer(2)));
    }

    #[test]
    fn refuses_a_version_that_is_not_a_positive_integer() {
        assert_eq!(migrate(table("version = 0")), Err(MigrateError::Invalid));
        assert_eq!(
            migrate(table("version = \"1\"")),
            Err(MigrateError::Invalid)
        );
    }
}
