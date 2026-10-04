pub mod template;

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use gtk::glib;

pub use template::{Naming, Stamp, Template};

/// Returns the default capture folder, `$XDG_PICTURES_DIR/Kiekje`.
///
/// # Examples
///
/// ```ignore
/// let directory = default_captures_dir();
/// ```
pub fn default_captures_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| glib::home_dir().join("Pictures"))
        .join("Kiekje")
}

/// Reads the local wall-clock time for naming a capture.
///
/// # Examples
///
/// ```ignore
/// let naming = Naming { mode, stamp: now(), width, height };
/// ```
pub fn now() -> Stamp {
    match glib::DateTime::now_local() {
        Ok(now) => Stamp {
            year: now.year(),
            month: now.month().unsigned_abs(),
            day: now.day_of_month().unsigned_abs(),
            hour: now.hour().unsigned_abs(),
            minute: now.minute().unsigned_abs(),
            second: now.second().unsigned_abs(),
        },
        Err(error) => {
            eprintln!("kiekje: no local time, naming the capture after 1970: {error}");
            Stamp {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            }
        }
    }
}

/// Writes encoded PNG bytes into a new file in `directory`, never overwriting an existing one.
///
/// A taken name gets `-2`, `-3` and so on appended before the extension.
///
/// # Examples
///
/// ```ignore
/// let path = save_png(&png, &directory, "kiekje-region-2026-10-04-11-20-00")?;
/// ```
pub fn save_png(png: &[u8], directory: &Path, base: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let (path, mut file) = create_unique(directory, base, "png")?;

    if let Err(error) = file.write_all(png) {
        // Best-effort cleanup of the partial file; the write error is the one to report.
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

fn create_unique(directory: &Path, base: &str, extension: &str) -> io::Result<(PathBuf, File)> {
    for attempt in 1..=1000 {
        let name = if attempt == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{attempt}.{extension}")
        };
        let path = directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "no free file name after 1000 attempts",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kiekje-storage-{name}-{}", std::process::id()))
    }

    #[test]
    fn creates_a_new_file_when_the_name_exists() {
        let directory = scratch("unique");
        fs::create_dir_all(&directory).unwrap();
        let (first, _) = create_unique(&directory, "same", "png").unwrap();
        let (second, _) = create_unique(&directory, "same", "png").unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("same-2.png"));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn saves_twenty_captures_with_the_same_name() {
        let directory = scratch("burst");
        let paths: Vec<PathBuf> = (0..20)
            .map(|index| save_png(&[index], &directory, "burst").unwrap())
            .collect();
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 20);
        assert_eq!(fs::read(&paths[19]).unwrap(), vec![19]);
        fs::remove_dir_all(&directory).unwrap();
    }
}
