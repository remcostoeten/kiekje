use std::ffi::{CString, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use gtk::glib;
use thiserror::Error;

pub const FILES: [&str; 2] = ["Geist-Variable.ttf", "GeistMono-Variable.ttf"];

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigAppFontAddFile(config: *mut c_void, file: *const u8) -> c_int;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FontError {
    #[error("{0} is not installed in any kiekje/fonts folder")]
    Missing(&'static str),
    #[error("fontconfig refused {}", .0.display())]
    Rejected(PathBuf),
}

/// The folders searched for the bundled fonts, most specific first.
///
/// Packages install them to `/usr/share/kiekje/fonts`; a build run from its source tree
/// also finds `resources/fonts`.
///
/// # Examples
///
/// ```ignore
/// let found = fonts::locate(&fonts::search_dirs());
/// ```
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![glib::user_data_dir().join("kiekje").join("fonts")];
    dirs.extend(
        glib::system_data_dirs()
            .into_iter()
            .map(|dir| dir.join("kiekje").join("fonts")),
    );
    dirs.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("fonts"),
    );
    dirs
}

/// Finds each bundled font file in the first folder that has it.
///
/// # Examples
///
/// ```ignore
/// for found in fonts::locate(&fonts::search_dirs()) {
///     println!("{found:?}");
/// }
/// ```
pub fn locate(dirs: &[PathBuf]) -> Vec<Result<PathBuf, FontError>> {
    FILES
        .into_iter()
        .map(|file| {
            dirs.iter()
                .map(|dir| dir.join(file))
                .find(|path| path.is_file())
                .ok_or(FontError::Missing(file))
        })
        .collect()
}

/// Makes the bundled Geist fonts available to this process only.
///
/// Call it once at startup, before the first widget renders text. Missing fonts fall back to the
/// system sans and mono, so each failure is returned for logging, not treated as fatal.
///
/// # Examples
///
/// ```ignore
/// for error in fonts::register() {
///     eprintln!("kiekje: {error}");
/// }
/// ```
pub fn register() -> Vec<FontError> {
    locate(&search_dirs())
        .into_iter()
        .filter_map(|found| found.and_then(|path| add(&path)).err())
        .collect()
}

fn add(path: &Path) -> Result<(), FontError> {
    let file = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| FontError::Rejected(path.to_path_buf()))?;
    // A null config means fontconfig's current config, which Pango reads when it first lays out text.
    let added = unsafe { FcConfigAppFontAddFile(std::ptr::null_mut(), file.as_ptr().cast()) };
    if added == 0 {
        return Err(FontError::Rejected(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_fonts_in_the_source_tree() {
        let dirs = [Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/fonts")];
        assert!(locate(&dirs).iter().all(Result::is_ok));
    }

    #[test]
    fn prefers_the_first_folder_and_reports_missing_files() {
        let scratch = std::env::temp_dir().join(format!("kiekje-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join(FILES[0]), b"").unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/fonts");
        let found = locate(&[scratch.clone(), source.clone()]);
        assert_eq!(found[0], Ok(scratch.join(FILES[0])));
        assert_eq!(found[1], Ok(source.join(FILES[1])));
        let missing = locate(std::slice::from_ref(&scratch));
        assert_eq!(missing[1], Err(FontError::Missing(FILES[1])));
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
