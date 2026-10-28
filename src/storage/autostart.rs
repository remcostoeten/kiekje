use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use gtk::glib;
use thiserror::Error;

use super::write_atomic;
use crate::desktop_file::quote_exec;
use crate::ids::APP_ID;

const PACKAGED_COMMAND: &str = "kiekje";

#[derive(Debug, Error)]
pub enum AutostartError {
    #[error("could not find the running kiekje binary: {0}")]
    Locate(#[source] io::Error),
    #[error("could not check {}: {source}", path.display())]
    Check { path: PathBuf, source: io::Error },
    #[error("could not write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("could not remove {}: {source}", path.display())]
    Remove { path: PathBuf, source: io::Error },
}

/// Returns `$XDG_CONFIG_HOME/autostart/dev.remcostoeten.Kiekje.desktop`, falling back to
/// `~/.config` when the variable is unset.
///
/// # Examples
///
/// ```ignore
/// let enabled = autostart::is_enabled(&autostart::path())?;
/// ```
pub fn path() -> PathBuf {
    path_in(&glib::user_config_dir())
}

/// Returns the autostart entry path inside the config folder `config_dir`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(
///     path_in(Path::new("/home/me/.config")),
///     Path::new("/home/me/.config/autostart/dev.remcostoeten.Kiekje.desktop"),
/// );
/// ```
pub fn path_in(config_dir: &Path) -> PathBuf {
    config_dir
        .join("autostart")
        .join(format!("{APP_ID}.desktop"))
}

/// Returns the command an autostart entry runs for `executable`.
///
/// A binary under `/usr` comes from a package and is on `PATH`, so the entry names plain
/// `kiekje` and keeps working when the package moves it.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(exec_for(Path::new("/usr/bin/kiekje")), "kiekje");
/// assert_eq!(exec_for(Path::new("/home/me/.local/bin/kiekje")), "/home/me/.local/bin/kiekje");
/// ```
pub fn exec_for(executable: &Path) -> String {
    if executable.starts_with("/usr") {
        return PACKAGED_COMMAND.to_string();
    }
    quote_exec(&executable.to_string_lossy())
}

/// Builds the autostart desktop entry that starts the tray with `exec daemon` at login.
///
/// # Examples
///
/// ```ignore
/// let text = entry("kiekje");
/// ```
pub fn entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kiekje\n\
         Comment=Start the Kiekje tray at login\n\
         Exec={exec} daemon\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-KDE-autostart-phase=2\n\
         X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2\n"
    )
}

/// Reports whether an autostart entry exists at `path`.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let enabled = is_enabled(&path())?;
/// ```
pub fn is_enabled(path: &Path) -> Result<bool, AutostartError> {
    path.try_exists().map_err(|source| AutostartError::Check {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes or removes the autostart entry for the running binary at the default path.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// autostart::set(true)?;
/// ```
pub fn set(enabled: bool) -> Result<(), AutostartError> {
    if !enabled {
        return disable_at(&path());
    }
    let executable = std::env::current_exe()
        .and_then(|executable| executable.canonicalize())
        .map_err(AutostartError::Locate)?;
    enable_at(&path(), &executable)
}

/// Writes the autostart entry for `executable` to `path`, replacing it atomically.
///
/// # Examples
///
/// ```ignore
/// enable_at(&path(), Path::new("/usr/bin/kiekje"))?;
/// ```
pub fn enable_at(path: &Path, executable: &Path) -> Result<(), AutostartError> {
    write_atomic(path, entry(&exec_for(executable)).as_bytes()).map_err(|source| {
        AutostartError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// Removes the autostart entry at `path`. A missing entry counts as removed.
///
/// # Examples
///
/// ```ignore
/// disable_at(&path())?;
/// ```
pub fn disable_at(path: &Path) -> Result<(), AutostartError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(source) => Err(AutostartError::Remove {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kiekje-autostart-{name}-{}", std::process::id()))
    }

    #[test]
    fn places_the_entry_in_the_autostart_folder() {
        assert_eq!(
            path_in(Path::new("/home/me/.config")),
            Path::new("/home/me/.config/autostart/dev.remcostoeten.Kiekje.desktop")
        );
    }

    #[test]
    fn runs_packaged_binaries_from_path() {
        assert_eq!(exec_for(Path::new("/usr/bin/kiekje")), "kiekje");
        assert_eq!(exec_for(Path::new("/usr/local/bin/kiekje")), "kiekje");
    }

    #[test]
    fn runs_other_binaries_by_their_full_path() {
        assert_eq!(
            exec_for(Path::new("/home/me/.local/bin/kiekje")),
            "/home/me/.local/bin/kiekje"
        );
        assert_eq!(exec_for(Path::new("/usrx/kiekje")), "/usrx/kiekje");
        assert_eq!(
            exec_for(Path::new("/opt/my app/kiekje")),
            "\"/opt/my app/kiekje\""
        );
    }

    #[test]
    fn entry_starts_the_daemon_late_in_the_kde_session() {
        let entry = entry("kiekje");
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("Exec=kiekje daemon\n"));
        assert!(entry.contains("X-KDE-autostart-phase=2\n"));
        assert!(entry.contains("X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2\n"));
    }

    #[test]
    fn manage_script_writes_the_same_entry() {
        let script = include_str!("../../manage.sh");
        let start = "cat >\"$AUTOSTART_FILE\" <<EOF\n";
        let body = script
            .split_once(start)
            .and_then(|(_, rest)| rest.split_once("EOF\n"))
            .map(|(body, _)| body)
            .expect("manage.sh writes the autostart entry with a heredoc");
        let rendered = body.replace("$launch", "kiekje").replace("$APP_ID", APP_ID);
        assert_eq!(rendered, entry("kiekje"));
    }

    #[test]
    fn enables_and_disables_the_entry() {
        let directory = scratch("toggle");
        let path = path_in(&directory);
        assert!(!is_enabled(&path).unwrap());
        enable_at(&path, Path::new("/usr/bin/kiekje")).unwrap();
        assert!(is_enabled(&path).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), entry("kiekje"));
        disable_at(&path).unwrap();
        assert!(!is_enabled(&path).unwrap());
        disable_at(&path).unwrap();
        fs::remove_dir_all(&directory).unwrap();
    }
}
