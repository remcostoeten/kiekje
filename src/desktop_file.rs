use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::glib;

use crate::app::APP_ID;

/// Writes the desktop entry that lets KWin grant this executable ScreenShot2 access, then
/// refreshes the KDE service cache.
///
/// # Examples
///
/// ```ignore
/// let path = install()?;
/// ```
pub fn install() -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let directory = glib::user_data_dir().join("applications");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{APP_ID}.desktop"));
    let temporary = path.with_extension("desktop.tmp");
    fs::write(&temporary, entry(&executable))?;
    fs::rename(&temporary, &path)?;

    match Command::new("kbuildsycoca6").output() {
        Ok(output) if !output.status.success() => {
            eprintln!(
                "kiekje: kbuildsycoca6 failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            eprintln!("kiekje: kbuildsycoca6: {error}")
        }
        _ => {}
    }
    Ok(path)
}

pub fn installed_path() -> PathBuf {
    glib::user_data_dir()
        .join("applications")
        .join(format!("{APP_ID}.desktop"))
}

pub fn entry(executable: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Kiekje Next\n\
         Comment=Screenshot prototype\n\
         Exec={} capture region\n\
         Icon=camera-photo\n\
         Terminal=false\n\
         X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2\n",
        quote_exec(&executable.to_string_lossy())
    )
}

/// Quotes a path for a desktop entry `Exec` key per the Desktop Entry specification.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(quote_exec("/opt/my app/kiekje"), "\"/opt/my app/kiekje\"");
/// ```
pub fn quote_exec(path: &str) -> String {
    let field_codes_escaped = path.replace('%', "%%");
    let needs_quotes = field_codes_escaped
        .chars()
        .any(|character| " \t\n\"'\\><~|&;$*?#()`".contains(character));
    if !needs_quotes {
        return field_codes_escaped;
    }
    let mut quoted = String::from("\"");
    for character in field_codes_escaped.chars() {
        if matches!(character, '"' | '`' | '$' | '\\') {
            quoted.push_str("\\\\");
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_plain_paths_unquoted() {
        assert_eq!(quote_exec("/usr/bin/kiekje"), "/usr/bin/kiekje");
    }

    #[test]
    fn quotes_paths_with_spaces() {
        assert_eq!(quote_exec("/opt/my app/kiekje"), "\"/opt/my app/kiekje\"");
    }

    #[test]
    fn escapes_percent_and_reserved_characters() {
        assert_eq!(quote_exec("/opt/100%/kiekje"), "/opt/100%%/kiekje");
        assert_eq!(quote_exec("/opt/$x/kiekje"), "\"/opt/\\\\$x/kiekje\"");
    }

    #[test]
    fn entry_declares_the_restricted_interface() {
        let entry = entry(Path::new("/usr/bin/kiekje-next"));
        assert!(entry.contains("Exec=/usr/bin/kiekje-next capture region\n"));
        assert!(entry.contains("X-KDE-DBUS-Restricted-Interfaces=org.kde.KWin.ScreenShot2\n"));
    }
}
