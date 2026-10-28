use std::path::{Path, PathBuf};

use gtk::glib;
use rusqlite::{Connection, TransactionBehavior, params};
use thiserror::Error;

use crate::capture::CaptureMode;
use crate::storage;

pub mod query;
pub mod thumbs;

const VIDEO_EXTENSIONS: [&str; 2] = ["webm", "mp4"];
const GIF_EXTENSION: &str = "gif";
const COLOUR_LIMIT: i64 = 200;
const EMOJI_LIMIT: i64 = 32;

const SCHEMA: &[&str] = &[
    "CREATE TABLE captures (
        id INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE,
        mode TEXT NOT NULL,
        width INTEGER NOT NULL,
        height INTEGER NOT NULL,
        bytes INTEGER NOT NULL,
        created_us INTEGER NOT NULL,
        missing INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX captures_created ON captures (created_us DESC);",
    "CREATE TABLE colours (
        id INTEGER PRIMARY KEY,
        rgb INTEGER NOT NULL UNIQUE,
        picked_us INTEGER NOT NULL
    );
    CREATE INDEX colours_picked ON colours (picked_us DESC);",
    "ALTER TABLE captures ADD COLUMN unsaved INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE captures ADD COLUMN annotated INTEGER NOT NULL DEFAULT 0;",
    "CREATE TABLE recent_emoji (
        glyph TEXT PRIMARY KEY,
        used_us INTEGER NOT NULL
    );",
];

const DAY_US: i64 = 24 * 60 * 60 * 1_000_000;

const COLUMNS: &str =
    "id, path, mode, width, height, bytes, created_us, missing, unsaved, annotated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub id: i64,
    pub path: PathBuf,
    pub mode: CaptureMode,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub created_us: i64,
    pub missing: bool,
    pub unsaved: bool,
    pub annotated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCapture {
    pub path: PathBuf,
    pub mode: CaptureMode,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub created_us: i64,
    pub unsaved: bool,
    pub annotated: bool,
}

impl Capture {
    /// Reports whether this entry is a recording rather than an image.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if capture.is_video() { return; }
    /// ```
    pub fn is_video(&self) -> bool {
        is_video(&self.path)
    }

    /// Reports whether this entry is a GIF exported from a recording.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let badge = if capture.is_gif() { "GIF" } else { "" };
    /// ```
    pub fn is_gif(&self) -> bool {
        is_gif(&self.path)
    }

    /// Reports whether this entry moves: a recording or a GIF, as opposed to a still image.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let editable = !capture.is_animated();
    /// ```
    pub fn is_animated(&self) -> bool {
        self.is_video() || self.is_gif()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    pub id: i64,
    pub rgb: [u8; 3],
    pub picked_us: i64,
}

impl Colour {
    /// Formats the colour as uppercase hex with a leading `#`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(colour.hex(), "#FE5101");
    /// ```
    pub fn hex(&self) -> String {
        let [red, green, blue] = self.rgb;
        format!("#{red:02X}{green:02X}{blue:02X}")
    }

    /// Formats the colour as a CSS `rgb()` value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(colour.css_rgb(), "rgb(254, 81, 1)");
    /// ```
    pub fn css_rgb(&self) -> String {
        let [red, green, blue] = self.rgb;
        format!("rgb({red}, {green}, {blue})")
    }
}

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("could not create {}: {source}", path.display())]
    Folder {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("library database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("the library was written by a newer build (schema {0}, this build reads up to {max})", max = SCHEMA.len())]
    Newer(i64),
    #[error("{0} is not valid UTF-8, so it can't be stored")]
    Path(PathBuf),
    #[error("{} is already in the gallery", .0.display())]
    Taken(PathBuf),
    #[error("{} does not exist", .0.display())]
    Absent(PathBuf),
    #[error("could not move {}: {source}", path.display())]
    Move {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Reports whether `path` is a recording, judged by its extension.
///
/// # Examples
///
/// ```ignore
/// assert!(library::is_video(Path::new("clip.webm")));
/// ```
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|video| extension.eq_ignore_ascii_case(video))
        })
}

/// Reports whether `path` is a GIF, judged by its extension.
///
/// # Examples
///
/// ```ignore
/// assert!(library::is_gif(Path::new("clip.gif")));
/// ```
pub fn is_gif(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(GIF_EXTENSION))
}

/// Reports whether an unsaved capture made at `created_us` is past its `keep_days` at `now_us`.
///
/// A capture exactly `keep_days` old is kept; with `keep_days` at 0 every unsaved capture is due.
///
/// # Examples
///
/// ```ignore
/// let due = expired(capture.created_us, glib::real_time(), config.saving.keep_unsaved_days);
/// ```
pub fn expired(created_us: i64, now_us: i64, keep_days: u32) -> bool {
    let keep_us = i64::from(keep_days).saturating_mul(DAY_US);
    keep_days == 0 || now_us.saturating_sub(created_us) > keep_us
}

pub struct Library {
    connection: Connection,
}

/// Returns `$XDG_DATA_HOME/kiekje/library.db`.
///
/// # Examples
///
/// ```ignore
/// let library = Library::open(&library::path())?;
/// ```
pub fn path() -> PathBuf {
    glib::user_data_dir().join("kiekje").join("library.db")
}

fn use_wal(connection: &Connection) -> Result<(), LibraryError> {
    let mode: String = connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if mode.eq_ignore_ascii_case("wal") {
        return Ok(());
    }
    // Switching to WAL skips SQLite's busy handler, so a library opened twice at once retries here.
    for _ in 0..20 {
        match connection.pragma_update(None, "journal_mode", "WAL") {
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            done => return Ok(done?),
        }
    }
    Ok(connection.pragma_update(None, "journal_mode", "WAL")?)
}

impl Library {
    /// Opens the library at `path`, creating it and bringing its schema up to date.
    ///
    /// Touches the disk, so call it off the GTK main thread.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let library = Library::open(&library::path())?;
    /// ```
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| LibraryError::Folder {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        use_wal(&connection)?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let mut library = Self { connection };
        library.migrate()?;
        Ok(library)
    }

    fn migrate(&mut self) -> Result<(), LibraryError> {
        let latest = i64::try_from(SCHEMA.len()).unwrap_or(i64::MAX);
        if Self::version(&self.connection)? == latest {
            return Ok(());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = Self::version(&transaction)?;
        if version > latest {
            return Err(LibraryError::Newer(version));
        }
        for (index, step) in SCHEMA
            .iter()
            .enumerate()
            .skip(usize::try_from(version).unwrap_or(0))
        {
            transaction.execute_batch(step)?;
            transaction.pragma_update(None, "user_version", index + 1)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn version(connection: &Connection) -> Result<i64, LibraryError> {
        Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    /// Records a saved capture and returns its id; saving the same path again updates it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let id = library.insert(&NewCapture { path, mode, width, height, bytes, created_us })?;
    /// ```
    pub fn insert(&self, capture: &NewCapture) -> Result<i64, LibraryError> {
        let path = text(&capture.path)?;
        let id = self.connection.query_row(
            "INSERT INTO captures
                (path, mode, width, height, bytes, created_us, missing, unsaved, annotated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
             ON CONFLICT (path) DO UPDATE SET
                mode = excluded.mode, width = excluded.width, height = excluded.height,
                bytes = excluded.bytes, created_us = excluded.created_us, missing = 0,
                unsaved = excluded.unsaved, annotated = excluded.annotated
             RETURNING id",
            params![
                path,
                capture.mode.as_str(),
                capture.width,
                capture.height,
                i64::try_from(capture.bytes).unwrap_or(i64::MAX),
                capture.created_us,
                capture.unsaved,
                capture.annotated,
            ],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Returns up to `limit` captures that are still on disk, newest first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let shown = library.recent(4)?;
    /// ```
    pub fn recent(&self, limit: usize) -> Result<Vec<Capture>, LibraryError> {
        self.select(
            &format!(
                "SELECT {COLUMNS} FROM captures
                 WHERE missing = 0 ORDER BY created_us DESC, id DESC LIMIT ?1"
            ),
            params![i64::try_from(limit).unwrap_or(i64::MAX)],
        )
    }

    /// Returns every capture, including the ones marked missing, newest first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let missing = library.all()?.into_iter().filter(|capture| capture.missing).count();
    /// ```
    pub fn all(&self) -> Result<Vec<Capture>, LibraryError> {
        self.select(
            &format!("SELECT {COLUMNS} FROM captures ORDER BY created_us DESC, id DESC"),
            [],
        )
    }

    /// Moves the unsaved capture `id` into `folder` and records it as saved there.
    ///
    /// The file keeps its name unless one in `folder` is taken, then it gets `-2`, `-3` and so on.
    /// Touches the disk, so call it off the GTK main thread.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let kept = library.keep(capture.id, &config.storage.captures_dir())?;
    /// ```
    pub fn keep(&self, id: i64, folder: &Path) -> Result<PathBuf, LibraryError> {
        let path: String = self.connection.query_row(
            "SELECT path FROM captures WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let path = PathBuf::from(path);
        let kept = storage::move_into(&path, folder).map_err(|source| LibraryError::Move {
            path: path.clone(),
            source,
        })?;
        let updated = text(&kept).and_then(|target| {
            Ok(self.connection.execute(
                "UPDATE captures SET path = ?1, unsaved = 0, missing = 0 WHERE id = ?2",
                params![target, id],
            )?)
        });
        if let Err(error) = updated {
            // Best-effort move back so the record still points at its file; the update error is the one to report.
            let _ = std::fs::rename(&kept, &path);
            return Err(error);
        }
        Ok(kept)
    }

    /// Deletes unsaved captures older than `keep_days`, file and record, and returns how many went.
    ///
    /// A file that is already gone still has its record removed. Touches the disk, so call it off
    /// the GTK main thread.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pruned = library.prune_unsaved(glib::real_time(), config.saving.keep_unsaved_days)?;
    /// ```
    pub fn prune_unsaved(&self, now_us: i64, keep_days: u32) -> Result<usize, LibraryError> {
        let unsaved: Vec<(i64, String, i64)> = {
            let mut statement = self
                .connection
                .prepare("SELECT id, path, created_us FROM captures WHERE unsaved = 1")?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<Result<_, _>>()?
        };
        let mut pruned = 0;
        for (id, path, created_us) in unsaved {
            if !expired(created_us, now_us, keep_days) {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    warn!("kept the unsaved capture {path}: {error}");
                    continue;
                }
            }
            pruned += self.connection.execute(
                "DELETE FROM captures WHERE id = ?1 AND path = ?2 AND unsaved = 1",
                params![id, path],
            )?;
        }
        Ok(pruned)
    }

    fn select(
        &self,
        sql: &str,
        parameters: impl rusqlite::Params,
    ) -> Result<Vec<Capture>, LibraryError> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(parameters, row)?;
        let mut captures = Vec::new();
        for capture in rows {
            if let Some(capture) = capture? {
                captures.push(capture);
            }
        }
        Ok(captures)
    }

    /// Points the record `id` at `path`, after its file was moved or renamed, and clears its
    /// missing mark.
    ///
    /// Fails with `Absent` when `path` is not a file and `Taken` when another record already uses
    /// it. Size and byte count are taken from the record, not re-read from the new file.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// library.relink(capture.id, Path::new("/home/me/Pictures/moved.png"))?;
    /// ```
    pub fn relink(&self, id: i64, path: &Path) -> Result<(), LibraryError> {
        if !path.is_file() {
            return Err(LibraryError::Absent(path.to_path_buf()));
        }
        let updated = self.connection.execute(
            "UPDATE captures SET path = ?1, missing = 0 WHERE id = ?2",
            params![text(path)?, id],
        );
        match updated {
            Err(rusqlite::Error::SqliteFailure(failure, _))
                if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(LibraryError::Taken(path.to_path_buf()))
            }
            Err(error) => Err(error.into()),
            Ok(_) => Ok(()),
        }
    }

    /// Removes every record marked missing and returns how many there were.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let forgotten = library.remove_missing()?;
    /// ```
    pub fn remove_missing(&self) -> Result<usize, LibraryError> {
        Ok(self
            .connection
            .execute("DELETE FROM captures WHERE missing = 1", [])?)
    }

    /// Reports whether the capture at `path` is recorded as annotated; an unknown path is not.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let annotated = library.is_annotated(&original)?;
    /// ```
    pub fn is_annotated(&self, path: &Path) -> Result<bool, LibraryError> {
        let annotated = self.connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM captures WHERE path = ?1 AND annotated = 1)",
            params![text(path)?],
            |row| row.get(0),
        )?;
        Ok(annotated)
    }

    /// Removes every record and returns how many there were. Files on disk are not touched.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let forgotten = library.clear()?;
    /// ```
    pub fn clear(&self) -> Result<usize, LibraryError> {
        Ok(self.connection.execute("DELETE FROM captures", [])?)
    }

    /// Returns the paths of every capture that is still on disk.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// thumbs::prune(&thumbs::dir(), &library.present()?)?;
    /// ```
    pub fn present(&self) -> Result<Vec<PathBuf>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM captures WHERE missing = 0")?;
        let paths = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .map(|path| path.map(PathBuf::from))
            .collect::<Result<_, _>>()?;
        Ok(paths)
    }

    /// Removes the record for `path` and reports whether there was one. The file is not touched.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// library.remove(&capture.path)?;
    /// ```
    pub fn remove(&self, path: &Path) -> Result<bool, LibraryError> {
        let removed = self
            .connection
            .execute("DELETE FROM captures WHERE path = ?1", params![text(path)?])?;
        Ok(removed > 0)
    }

    /// Marks every capture whose file is gone as missing, and clears the mark when it is back.
    ///
    /// Returns how many records changed state. Stats every file, so call it off the GTK main thread.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let changed = library.check_missing()?;
    /// ```
    pub fn check_missing(&mut self) -> Result<usize, LibraryError> {
        let entries: Vec<(i64, String, bool)> = {
            let mut statement = self
                .connection
                .prepare("SELECT id, path, missing FROM captures")?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<Result<_, _>>()?
        };
        let flips: Vec<(i64, String, bool)> = entries
            .into_iter()
            .filter_map(|(id, path, missing)| {
                let gone = !Path::new(&path).is_file();
                (gone != missing).then_some((id, path, gone))
            })
            .collect();
        if flips.is_empty() {
            return Ok(0);
        }
        let transaction = self.connection.transaction()?;
        for (id, path, gone) in &flips {
            transaction.execute(
                "UPDATE captures SET missing = ?1 WHERE id = ?2 AND path = ?3",
                params![gone, id, path],
            )?;
        }
        transaction.commit()?;
        Ok(flips.len())
    }

    /// Records a colour picked from the screen and returns its id.
    ///
    /// Picking a colour that is already listed moves it to the top. Only the newest
    /// colours are kept.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// library.pick_colour([254, 81, 1], glib::real_time())?;
    /// ```
    pub fn pick_colour(&self, rgb: [u8; 3], picked_us: i64) -> Result<i64, LibraryError> {
        let id = self.connection.query_row(
            "INSERT INTO colours (rgb, picked_us) VALUES (?1, ?2)
             ON CONFLICT (rgb) DO UPDATE SET picked_us = excluded.picked_us
             RETURNING id",
            params![pack(rgb), picked_us],
            |row| row.get(0),
        )?;
        self.connection.execute(
            "DELETE FROM colours WHERE id NOT IN
             (SELECT id FROM colours ORDER BY picked_us DESC, id DESC LIMIT ?1)",
            params![COLOUR_LIMIT],
        )?;
        Ok(id)
    }

    /// Returns every picked colour, newest first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let latest = library.colours()?.first().map(Colour::hex);
    /// ```
    pub fn colours(&self) -> Result<Vec<Colour>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, rgb, picked_us FROM colours ORDER BY picked_us DESC, id DESC")?;
        let colours = statement
            .query_map([], |row| {
                Ok(Colour {
                    id: row.get(0)?,
                    rgb: unpack(row.get(1)?),
                    picked_us: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(colours)
    }

    /// Removes the colour `id` and reports whether there was one.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// library.remove_colour(colour.id)?;
    /// ```
    pub fn remove_colour(&self, id: i64) -> Result<bool, LibraryError> {
        let removed = self
            .connection
            .execute("DELETE FROM colours WHERE id = ?1", params![id])?;
        Ok(removed > 0)
    }

    /// Removes every picked colour and returns how many there were.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let forgotten = library.clear_colours()?;
    /// ```
    pub fn clear_colours(&self) -> Result<usize, LibraryError> {
        Ok(self.connection.execute("DELETE FROM colours", [])?)
    }

    /// Records that `glyph` was stamped at `used_us`; using it again moves it to the front.
    ///
    /// Only the newest emoji are kept.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// library.use_emoji("🔥", glib::real_time())?;
    /// ```
    pub fn use_emoji(&self, glyph: &str, used_us: i64) -> Result<(), LibraryError> {
        self.connection.execute(
            "INSERT INTO recent_emoji (glyph, used_us) VALUES (?1, ?2)
             ON CONFLICT (glyph) DO UPDATE SET used_us = excluded.used_us",
            params![glyph, used_us],
        )?;
        self.connection.execute(
            "DELETE FROM recent_emoji WHERE glyph NOT IN
             (SELECT glyph FROM recent_emoji ORDER BY used_us DESC, rowid DESC LIMIT ?1)",
            params![EMOJI_LIMIT],
        )?;
        Ok(())
    }

    /// Returns up to `limit` recently stamped emoji, newest first.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let row = library.recent_emoji(8)?;
    /// ```
    pub fn recent_emoji(&self, limit: usize) -> Result<Vec<String>, LibraryError> {
        let mut statement = self
            .connection
            .prepare("SELECT glyph FROM recent_emoji ORDER BY used_us DESC, rowid DESC LIMIT ?1")?;
        let glyphs = statement
            .query_map(params![i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?;
        Ok(glyphs)
    }
}

fn pack([red, green, blue]: [u8; 3]) -> i64 {
    i64::from(red) << 16 | i64::from(green) << 8 | i64::from(blue)
}

fn unpack(rgb: i64) -> [u8; 3] {
    let [_, _, _, _, _, red, green, blue] = rgb.to_be_bytes();
    [red, green, blue]
}

fn text(path: &Path) -> Result<&str, LibraryError> {
    path.to_str()
        .ok_or_else(|| LibraryError::Path(path.to_path_buf()))
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<Capture>> {
    let mode: String = row.get(2)?;
    let path: String = row.get(1)?;
    let bytes: i64 = row.get(5)?;
    let Some(mode) = CaptureMode::parse(&mode) else {
        warn!("skipped library entry {path} with unknown mode {mode}");
        return Ok(None);
    };
    Ok(Some(Capture {
        id: row.get(0)?,
        path: PathBuf::from(path),
        mode,
        width: row.get(3)?,
        height: row.get(4)?,
        bytes: u64::try_from(bytes).unwrap_or(0),
        created_us: row.get(6)?,
        missing: row.get(7)?,
        unsaved: row.get(8)?,
        annotated: row.get(9)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("kiekje-library-{name}-{}", std::process::id()));
        // A leftover folder from an earlier run may or may not exist.
        let _ = std::fs::remove_dir_all(&folder);
        folder
    }

    fn capture(path: &Path, created_us: i64) -> NewCapture {
        NewCapture {
            path: path.to_path_buf(),
            mode: CaptureMode::Region,
            width: 640,
            height: 400,
            bytes: 1234,
            created_us,
            unsaved: false,
            annotated: false,
        }
    }

    #[test]
    fn recognises_recordings_by_extension() {
        assert!(is_video(Path::new("/v/clip.webm")));
        assert!(is_video(Path::new("/v/CLIP.MP4")));
        assert!(!is_video(Path::new("/p/shot.png")));
        assert!(!is_video(Path::new("/p/webm")));
        assert!(is_gif(Path::new("/v/clip.GIF")));
        assert!(!is_gif(Path::new("/v/clip.webm")));
    }

    #[test]
    fn creates_the_schema_once_and_reopens() {
        let folder = scratch("open");
        let path = folder.join("library.db");
        let library = Library::open(&path).unwrap();
        library.insert(&capture(&folder.join("a.png"), 1)).unwrap();
        drop(library);
        let reopened = Library::open(&path).unwrap();
        assert_eq!(reopened.recent(5).unwrap()[0].path, folder.join("a.png"));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn clears_every_record_and_keeps_the_files() {
        let folder = scratch("clear");
        let library = Library::open(&folder.join("library.db")).unwrap();
        for name in ["a.png", "b.png"] {
            std::fs::write(folder.join(name), b"png").unwrap();
            library.insert(&capture(&folder.join(name), 1)).unwrap();
        }
        assert_eq!(library.clear().unwrap(), 2);
        assert!(library.all().unwrap().is_empty());
        assert!(folder.join("a.png").is_file());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn migrates_once_when_opened_from_several_threads() {
        let folder = scratch("concurrent");
        let path = folder.join("library.db");
        let opens: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || Library::open(&path).map(drop))
            })
            .collect();
        for open in opens {
            assert!(open.join().unwrap().is_ok());
        }
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn refuses_a_library_from_a_newer_build() {
        let folder = scratch("newer");
        let path = folder.join("library.db");
        drop(Library::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        assert!(matches!(Library::open(&path), Err(LibraryError::Newer(99))));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn lists_recent_captures_newest_first_and_skips_missing_ones() {
        let folder = scratch("recent");
        let mut library = Library::open(&folder.join("library.db")).unwrap();
        for (name, created) in [("old.png", 1), ("new.png", 3), ("mid.png", 2)] {
            std::fs::write(folder.join(name), b"png").unwrap();
            library
                .insert(&capture(&folder.join(name), created))
                .unwrap();
        }
        let names = |library: &Library| -> Vec<String> {
            library
                .recent(10)
                .unwrap()
                .into_iter()
                .map(|capture| {
                    capture
                        .path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        };
        assert_eq!(names(&library), ["new.png", "mid.png", "old.png"]);

        std::fs::remove_file(folder.join("mid.png")).unwrap();
        assert_eq!(library.check_missing().unwrap(), 1);
        assert_eq!(names(&library), ["new.png", "old.png"]);

        std::fs::write(folder.join("mid.png"), b"png").unwrap();
        assert_eq!(library.check_missing().unwrap(), 1);
        assert_eq!(names(&library), ["new.png", "mid.png", "old.png"]);
        assert_eq!(library.present().unwrap().len(), 3);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn checks_for_missing_files_without_writing_when_nothing_changed() {
        let folder = scratch("quiet-check");
        let mut library = Library::open(&folder.join("library.db")).unwrap();
        std::fs::write(folder.join("a.png"), b"png").unwrap();
        library.insert(&capture(&folder.join("a.png"), 1)).unwrap();
        let other = Library::open(&folder.join("library.db")).unwrap();
        other.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let started = std::time::Instant::now();
        assert_eq!(library.check_missing().unwrap(), 0);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        other.connection.execute_batch("ROLLBACK").unwrap();
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn updates_a_capture_saved_to_the_same_path() {
        let folder = scratch("upsert");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let path = folder.join("shot.png");
        let first = library.insert(&capture(&path, 1)).unwrap();
        let mut again = capture(&path, 5);
        again.width = 10;
        assert_eq!(library.insert(&again).unwrap(), first);
        let recent = library.recent(5).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].width, 10);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn relinks_a_moved_capture_and_refuses_taken_or_absent_paths() {
        let folder = scratch("relink");
        let mut library = Library::open(&folder.join("library.db")).unwrap();
        let (old, moved, other) = (
            folder.join("old.png"),
            folder.join("moved.png"),
            folder.join("other.png"),
        );
        std::fs::write(&old, b"png").unwrap();
        std::fs::write(&other, b"png").unwrap();
        let id = library.insert(&capture(&old, 1)).unwrap();
        library.insert(&capture(&other, 2)).unwrap();
        std::fs::rename(&old, &moved).unwrap();
        library.check_missing().unwrap();
        assert!(library.all().unwrap().iter().any(|capture| capture.missing));

        assert!(matches!(
            library.relink(id, &folder.join("nowhere.png")),
            Err(LibraryError::Absent(_))
        ));
        assert!(matches!(
            library.relink(id, &other),
            Err(LibraryError::Taken(_))
        ));
        library.relink(id, &moved).unwrap();
        let all = library.all().unwrap();
        assert!(all.iter().all(|capture| !capture.missing));
        assert!(all.iter().any(|capture| capture.path == moved));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn forgets_only_missing_captures() {
        let folder = scratch("forget-missing");
        let mut library = Library::open(&folder.join("library.db")).unwrap();
        let (kept, gone) = (folder.join("kept.png"), folder.join("gone.png"));
        std::fs::write(&kept, b"png").unwrap();
        library.insert(&capture(&kept, 1)).unwrap();
        library.insert(&capture(&gone, 2)).unwrap();
        library.check_missing().unwrap();
        assert_eq!(library.remove_missing().unwrap(), 1);
        let all = library.all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].path, kept);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn lists_picked_colours_newest_first_and_moves_a_repeat_to_the_top() {
        let folder = scratch("colours");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let orange = library.pick_colour([254, 81, 1], 1).unwrap();
        library.pick_colour([0, 0, 0], 2).unwrap();
        assert_eq!(library.pick_colour([254, 81, 1], 3).unwrap(), orange);
        let colours = library.colours().unwrap();
        let hexes: Vec<String> = colours.iter().map(Colour::hex).collect();
        assert_eq!(hexes, ["#FE5101", "#000000"]);
        assert_eq!(colours[0].css_rgb(), "rgb(254, 81, 1)");
        assert!(library.remove_colour(orange).unwrap());
        assert!(!library.remove_colour(orange).unwrap());
        assert_eq!(library.clear_colours().unwrap(), 1);
        assert!(library.colours().unwrap().is_empty());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn lists_recent_emoji_newest_first_and_moves_a_repeat_to_the_front() {
        let folder = scratch("recent-emoji");
        let library = Library::open(&folder.join("library.db")).unwrap();
        library.use_emoji("🔥", 1).unwrap();
        library.use_emoji("👍", 2).unwrap();
        library.use_emoji("🐛", 3).unwrap();
        assert_eq!(library.recent_emoji(8).unwrap(), ["🐛", "👍", "🔥"]);
        library.use_emoji("🔥", 4).unwrap();
        assert_eq!(library.recent_emoji(2).unwrap(), ["🔥", "🐛"]);
        for step in 0..40 {
            library.use_emoji(&format!("{step}"), 10 + step).unwrap();
        }
        assert_eq!(library.recent_emoji(100).unwrap().len() as i64, EMOJI_LIMIT);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn keeps_only_the_newest_picked_colours() {
        let folder = scratch("colour-limit");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let limit = u8::try_from(COLOUR_LIMIT).unwrap();
        for index in 0..=limit {
            library
                .pick_colour([index, 0, 0], i64::from(index))
                .unwrap();
        }
        let colours = library.colours().unwrap();
        assert_eq!(colours.len(), usize::from(limit));
        assert_eq!(colours[0].rgb, [limit, 0, 0]);
        assert!(colours.iter().all(|colour| colour.rgb != [0, 0, 0]));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn adds_the_unsaved_and_annotated_columns_to_an_older_library() {
        let folder = scratch("migrate-flags");
        let path = folder.join("library.db");
        std::fs::create_dir_all(&folder).unwrap();
        let old = Connection::open(&path).unwrap();
        for step in &SCHEMA[..2] {
            old.execute_batch(step).unwrap();
        }
        old.pragma_update(None, "user_version", 2).unwrap();
        old.execute(
            "INSERT INTO captures (path, mode, width, height, bytes, created_us)
             VALUES ('/shots/old.png', 'region', 1, 1, 1, 1)",
            [],
        )
        .unwrap();
        drop(old);
        let library = Library::open(&path).unwrap();
        let all = library.all().unwrap();
        assert_eq!(all.len(), 1);
        assert!(!all[0].unsaved);
        assert!(!all[0].annotated);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn records_the_unsaved_and_annotated_flags() {
        let folder = scratch("flags");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let mut new = capture(&folder.join("a.png"), 1);
        new.unsaved = true;
        new.annotated = true;
        library.insert(&new).unwrap();
        let stored = &library.all().unwrap()[0];
        assert!(stored.unsaved && stored.annotated);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn reports_whether_a_path_is_annotated() {
        let folder = scratch("is-annotated");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let mut drawn = capture(&folder.join("drawn.png"), 1);
        drawn.annotated = true;
        library.insert(&drawn).unwrap();
        library
            .insert(&capture(&folder.join("plain.png"), 2))
            .unwrap();
        assert!(library.is_annotated(&folder.join("drawn.png")).unwrap());
        assert!(!library.is_annotated(&folder.join("plain.png")).unwrap());
        assert!(!library.is_annotated(&folder.join("unknown.png")).unwrap());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn keeps_an_unsaved_capture_by_moving_it_into_the_folder() {
        let folder = scratch("keep");
        let (cache, shots) = (folder.join("unsaved"), folder.join("shots"));
        std::fs::create_dir_all(&cache).unwrap();
        let library = Library::open(&folder.join("library.db")).unwrap();
        let source = cache.join("shot.png");
        std::fs::write(&source, b"png").unwrap();
        let mut new = capture(&source, 1);
        new.unsaved = true;
        let id = library.insert(&new).unwrap();
        let kept = library.keep(id, &shots).unwrap();
        assert_eq!(kept, shots.join("shot.png"));
        assert!(!source.exists());
        let stored = &library.all().unwrap()[0];
        assert_eq!(stored.path, kept);
        assert!(!stored.unsaved);
        std::fs::write(&source, b"again").unwrap();
        let mut again = capture(&source, 2);
        again.unsaved = true;
        let second = library.insert(&again).unwrap();
        assert_eq!(
            library.keep(second, &shots).unwrap(),
            shots.join("shot-2.png")
        );
        assert_eq!(std::fs::read(&kept).unwrap(), b"png");
        std::fs::remove_file(shots.join("shot-2.png")).unwrap();
        assert!(matches!(
            library.keep(second, &folder.join("elsewhere")),
            Err(LibraryError::Move { .. })
        ));
        assert!(!folder.join("elsewhere").join("shot-2.png").exists());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn expires_unsaved_captures_only_after_the_kept_days() {
        let now = 30 * DAY_US;
        assert!(!expired(now - DAY_US, now, 7));
        assert!(!expired(now - 7 * DAY_US, now, 7));
        assert!(expired(now - 7 * DAY_US - 1, now, 7));
        assert!(expired(now, now, 0));
        assert!(!expired(now + DAY_US, now, 1));
        assert!(!expired(i64::MIN, i64::MAX, u32::MAX));
    }

    #[test]
    fn prunes_old_unsaved_captures_and_leaves_the_rest() {
        let folder = scratch("prune");
        std::fs::create_dir_all(&folder).unwrap();
        let library = Library::open(&folder.join("library.db")).unwrap();
        let now = 30 * DAY_US;
        let entries = [
            ("old-unsaved.png", now - 8 * DAY_US, true),
            ("new-unsaved.png", now - DAY_US, true),
            ("old-saved.png", now - 20 * DAY_US, false),
            ("gone-unsaved.png", now - 9 * DAY_US, true),
        ];
        for (name, created, unsaved) in entries {
            let path = folder.join(name);
            if name != "gone-unsaved.png" {
                std::fs::write(&path, b"png").unwrap();
            }
            let mut new = capture(&path, created);
            new.unsaved = unsaved;
            library.insert(&new).unwrap();
        }
        assert_eq!(library.prune_unsaved(now, 7).unwrap(), 2);
        assert!(!folder.join("old-unsaved.png").exists());
        assert!(folder.join("new-unsaved.png").is_file());
        assert!(folder.join("old-saved.png").is_file());
        let mut left: Vec<String> = library
            .all()
            .unwrap()
            .into_iter()
            .map(|capture| {
                capture
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        left.sort();
        assert_eq!(left, ["new-unsaved.png", "old-saved.png"]);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn removes_a_record_without_touching_the_file() {
        let folder = scratch("remove");
        let library = Library::open(&folder.join("library.db")).unwrap();
        let path = folder.join("shot.png");
        std::fs::write(&path, b"png").unwrap();
        library.insert(&capture(&path, 1)).unwrap();
        assert!(library.remove(&path).unwrap());
        assert!(!library.remove(&path).unwrap());
        assert!(library.recent(5).unwrap().is_empty());
        assert!(path.is_file());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
