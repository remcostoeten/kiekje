pub mod autostart;
pub mod format;
pub mod last_region;
pub mod template;

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gtk::glib;
use rustix::fs::{CWD, RenameFlags, renameat_with};
use rustix::io::Errno;
use thiserror::Error;

pub use format::{ImageFormat, Quality};
pub use template::{Naming, Stamp, Template};

static TEMPORARY: AtomicU64 = AtomicU64::new(0);

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
/// Shows `path` with the home folder as `~`, the way people write it in config files.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(tilde(&glib::home_dir().join(".config")), "~/.config");
/// ```
pub fn tilde(path: &Path) -> String {
    match path.strip_prefix(glib::home_dir()) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

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

/// Writes encoded image bytes into a new file in `directory`, never overwriting an existing one.
///
/// The file gets the extension of `format`. A taken name gets `-2`, `-3` and so on appended before
/// the extension.
///
/// # Examples
///
/// ```ignore
/// let path = save(&bytes, &directory, "kiekje-region-2026-10-04-11-20-00", ImageFormat::Png)?;
/// ```
pub fn save(
    bytes: &[u8],
    directory: &Path,
    base: &str,
    format: ImageFormat,
) -> io::Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let (path, mut file) = create_unique(directory, base, format.extension())?;

    if let Err(error) = file.write_all(bytes) {
        // Best-effort cleanup of the partial file; the write error is the one to report.
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

/// Writes encoded image bytes to the path a save dialog returned.
///
/// The dialog already confirmed overwriting, so a path whose extension matches `format` is
/// replaced in place. Any other name gets a new file with the right extension next to it instead.
///
/// # Examples
///
/// ```ignore
/// let path = save_as(&bytes, &chosen, ImageFormat::Jpeg)?;
/// ```
pub fn save_as(bytes: &[u8], path: &Path, format: ImageFormat) -> io::Result<PathBuf> {
    if ImageFormat::from_path(path) == Some(format) {
        write_atomic(path, bytes)?;
        return Ok(path.to_path_buf());
    }
    let folder = path.parent().unwrap_or(Path::new("."));
    let base = path.file_stem().map_or_else(
        || "kiekje".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    save(bytes, folder, &base, format)
}

#[derive(Debug, Error)]
pub enum RenameError {
    #[error("type a name")]
    Empty,
    #[error("a name can't contain `/` or be `.` or `..`")]
    Invalid,
    #[error("{} already exists", .0.file_name().unwrap_or_default().display())]
    Taken(PathBuf),
    #[error("could not rename: {0}")]
    Io(#[from] io::Error),
}

/// Renames a file in place to `stem` plus its current extension, never overwriting another file.
///
/// Surrounding whitespace is trimmed. Returns the new path, which is `path` itself when the name
/// did not change.
///
/// # Examples
///
/// ```ignore
/// let renamed = rename(Path::new("/shots/kiekje-1.png"), "invoice bug")?;
/// assert_eq!(renamed, Path::new("/shots/invoice bug.png"));
/// ```
pub fn rename(path: &Path, stem: &str) -> Result<PathBuf, RenameError> {
    let stem = stem.trim();
    if stem.is_empty() {
        return Err(RenameError::Empty);
    }
    if stem.contains(['/', '\0']) || stem == "." || stem == ".." {
        return Err(RenameError::Invalid);
    }
    let mut name = std::ffi::OsString::from(stem);
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    let target = path.with_file_name(name);
    if target == path {
        return Ok(target);
    }
    match renameat_with(CWD, path, CWD, &target, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(target),
        Err(Errno::EXIST) => Err(RenameError::Taken(target)),
        // Some file systems lack RENAME_NOREPLACE; fall back to a checked rename.
        Err(Errno::INVAL | Errno::NOSYS | Errno::OPNOTSUPP) => {
            if target.exists() {
                return Err(RenameError::Taken(target));
            }
            fs::rename(path, &target)?;
            Ok(target)
        }
        Err(errno) => Err(RenameError::Io(errno.into())),
    }
}

/// Replaces the file at `path` with `bytes` through a temporary file and a rename.
///
/// Readers see the old file or the new one, never half of either. Missing parent folders are
/// created.
///
/// # Examples
///
/// ```ignore
/// write_atomic(&config::path(), text.as_bytes())?;
/// ```
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = path.with_file_name(name);
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, path));
    if let Err(error) = written {
        // Best-effort cleanup; the write error is the one to report.
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
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
    fn saves_as_the_chosen_name_and_beside_a_mismatched_one() {
        let folder = std::env::temp_dir().join(format!("kiekje-save-as-{}", std::process::id()));
        // A leftover folder from an earlier run may or may not exist.
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let chosen = folder.join("shot.png");
        fs::write(&chosen, b"old").unwrap();
        assert_eq!(save_as(b"new", &chosen, ImageFormat::Png).unwrap(), chosen);
        assert_eq!(fs::read(&chosen).unwrap(), b"new");
        let photo = folder.join("photo.JPEG");
        fs::write(&photo, b"old").unwrap();
        assert_eq!(save_as(b"jpg", &photo, ImageFormat::Jpeg).unwrap(), photo);
        let other = save_as(b"png", &folder.join("photo.webp"), ImageFormat::Png).unwrap();
        assert_eq!(other, folder.join("photo.png"));
        let mismatched = save_as(b"jpg", &folder.join("photo.png"), ImageFormat::Jpeg).unwrap();
        assert_eq!(mismatched, folder.join("photo.jpg"));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn renames_keeping_the_extension_and_never_overwrites() {
        let directory = scratch("rename");
        fs::create_dir_all(&directory).unwrap();
        let shot = directory.join("kiekje-1.jpg");
        fs::write(&shot, b"shot").unwrap();
        fs::write(directory.join("taken.jpg"), b"other").unwrap();
        let renamed = rename(&shot, "  invoice bug ").unwrap();
        assert_eq!(renamed, directory.join("invoice bug.jpg"));
        assert_eq!(fs::read(&renamed).unwrap(), b"shot");
        assert!(matches!(
            rename(&renamed, "taken"),
            Err(RenameError::Taken(_))
        ));
        assert_eq!(fs::read(directory.join("taken.jpg")).unwrap(), b"other");
        assert!(matches!(rename(&renamed, " "), Err(RenameError::Empty)));
        assert!(matches!(rename(&renamed, "a/b"), Err(RenameError::Invalid)));
        assert!(matches!(rename(&renamed, ".."), Err(RenameError::Invalid)));
        assert_eq!(rename(&renamed, "invoice bug").unwrap(), renamed);
        fs::remove_dir_all(&directory).unwrap();
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
    fn replaces_a_file_atomically_without_leftovers() {
        let directory = scratch("atomic");
        let path = directory.join("nested").join("state.toml");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn writes_the_same_file_from_several_threads_at_once() {
        let directory = scratch("atomic-threads");
        let path = directory.join("config.toml");
        let writers: Vec<_> = (0..8u8)
            .map(|index| {
                let path = path.clone();
                std::thread::spawn(move || write_atomic(&path, &[index; 64]))
            })
            .collect();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        let written = fs::read(&path).unwrap();
        assert_eq!(written.len(), 64);
        assert!(written.iter().all(|byte| *byte == written[0]));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn saves_twenty_captures_with_the_same_name() {
        let directory = scratch("burst");
        let paths: Vec<PathBuf> = (0..20)
            .map(|index| save(&[index], &directory, "burst", ImageFormat::Png).unwrap())
            .collect();
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 20);
        assert_eq!(fs::read(&paths[19]).unwrap(), vec![19]);
        fs::remove_dir_all(&directory).unwrap();
    }
}
