mod migrate;

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use gtk::glib;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml::Table;

use crate::editor::{Color, DrawAction, DrawKey, DrawingKeys, KeyConflict};
use crate::shortcuts::{Conflict, ShortcutAction, Shortcuts, Slot};
use crate::storage::{self, ImageFormat, Quality, Template};
pub use migrate::{MigrateError, VERSION};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: i64,
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub saving: Saving,
    #[serde(default)]
    pub capture: Capture,
    #[serde(default)]
    pub overlay: Overlay,
    #[serde(default)]
    pub notifications: Notifications,
    #[serde(default)]
    pub drawing: Drawing,
    #[serde(default)]
    pub shortcuts: Shortcuts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub check_for_updates: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Drawing {
    pub color: Color,
    pub size: u32,
    pub opacity: u32,
    pub keys: DrawingKeys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Saving {
    pub enabled: bool,
    pub keep_unsaved_days: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Capture {
    pub after_selection: AfterSelection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AfterSelection {
    #[default]
    Finish,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overlay {
    pub magnifier: bool,
    pub magnifier_pixels: u32,
    pub slow_pointer: SlowPointer,
    pub window_snap: bool,
    pub hints: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SlowPointer {
    Quarter,
    #[default]
    Eighth,
    Sixteenth,
}

impl SlowPointer {
    pub const ALL: [Self; 3] = [Self::Quarter, Self::Eighth, Self::Sixteenth];

    pub fn factor(self) -> f64 {
        match self {
            Self::Quarter => 0.25,
            Self::Eighth => 0.125,
            Self::Sixteenth => 0.0625,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quarter => "1/4 speed",
            Self::Eighth => "1/8 speed",
            Self::Sixteenth => "1/16 speed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub enabled: bool,
    pub sound: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Storage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<Folder>,
    pub filename: Template,
    pub format: ImageFormat,
    pub quality: Quality,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Folder {
    source: String,
    path: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{} is not valid: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("{}: {source}", path.display())]
    Migrate { path: PathBuf, source: MigrateError },
    #[error("could not write {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("{}: {source}", path.display())]
    Shortcuts { path: PathBuf, source: Conflict },
    #[error("{}: {source}", path.display())]
    Keys { path: PathBuf, source: KeyConflict },
    #[error("could not encode the config: {0}")]
    Encode(#[from] toml::ser::Error),
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("folder `{0}` must be an absolute path or start with `~/`")]
pub struct FolderError(String);

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            general: General::default(),
            storage: Storage::default(),
            saving: Saving::default(),
            capture: Capture::default(),
            overlay: Overlay::default(),
            notifications: Notifications::default(),
            drawing: Drawing::default(),
            shortcuts: Shortcuts::default(),
        }
    }
}

impl Default for Saving {
    fn default() -> Self {
        Self {
            enabled: true,
            keep_unsaved_days: 7,
        }
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            magnifier: true,
            magnifier_pixels: 15,
            slow_pointer: SlowPointer::default(),
            window_snap: true,
            hints: true,
        }
    }
}

impl Default for Drawing {
    fn default() -> Self {
        Self {
            color: Color::default(),
            size: 3,
            opacity: 100,
            keys: DrawingKeys::default(),
        }
    }
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: true,
        }
    }
}

impl Storage {
    /// Returns the folder captures are saved in.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let path = storage::save(&bytes, &config.storage.captures_dir(), &base, format)?;
    /// ```
    pub fn captures_dir(&self) -> PathBuf {
        self.folder
            .as_ref()
            .map_or_else(storage::default_captures_dir, |folder| folder.path.clone())
    }
}

impl Folder {
    fn parse(source: String, home: &Path) -> Result<Self, FolderError> {
        let path = match source.strip_prefix("~/") {
            Some(relative) => home.join(relative),
            None if source == "~" => home.to_path_buf(),
            None => PathBuf::from(&source),
        };
        if !path.is_absolute() {
            return Err(FolderError(source));
        }
        Ok(Self { source, path })
    }
}

impl Folder {
    /// Wraps a chosen absolute folder, written with `~/` when it sits under the home folder.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let folder = Folder::from_path(glib::home_dir().join("Shots"))?;
    /// assert_eq!(String::from(folder), "~/Shots");
    /// ```
    pub fn from_path(path: PathBuf) -> Result<Self, FolderError> {
        Self::chosen(path, &glib::home_dir())
    }

    fn chosen(path: PathBuf, home: &Path) -> Result<Self, FolderError> {
        if !path.is_absolute() {
            return Err(FolderError(path.display().to_string()));
        }
        let source = match path.strip_prefix(home) {
            Ok(relative) if relative.as_os_str().is_empty() => "~".to_owned(),
            Ok(relative) => format!("~/{}", relative.display()),
            Err(_) => path.display().to_string(),
        };
        Ok(Self { source, path })
    }
}

impl TryFrom<String> for Folder {
    type Error = FolderError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        Self::parse(source, &glib::home_dir())
    }
}

impl From<Folder> for String {
    fn from(folder: Folder) -> Self {
        folder.source
    }
}

/// Returns `$XDG_CONFIG_HOME/kiekje/config.toml`.
///
/// # Examples
///
/// ```ignore
/// let config = config::load_from(&config::path())?;
/// ```
pub fn path() -> PathBuf {
    glib::user_config_dir().join("kiekje").join("config.toml")
}

/// Reads and migrates the config at `path`, or returns `None` when the file does not exist.
///
/// # Examples
///
/// ```ignore
/// let config = load_from(&path())?.unwrap_or_default();
/// ```
pub fn load_from(path: &Path) -> Result<Option<Config>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    parse(&text, path).map(Some)
}

/// Loads the user's config, falling back to defaults and logging why when it can't be used.
///
/// Reads from disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let directory = config::load().storage.captures_dir();
/// ```
pub fn load() -> Config {
    match load_from(&path()) {
        Ok(config) => config.unwrap_or_default(),
        Err(error) => {
            eprintln!("kiekje: using the default config: {error}");
            Config::default()
        }
    }
}

/// Writes `config` to `path` through a temporary file and a rename, so readers never see half a file.
///
/// # Examples
///
/// ```ignore
/// save_to(&path(), &config)?;
/// ```
pub fn save_to(path: &Path, config: &Config) -> Result<(), ConfigError> {
    let _writing = writing();
    write(path, config)
}

static WRITES: Mutex<()> = Mutex::new(());

fn writing() -> MutexGuard<'static, ()> {
    // The guarded value is (), so a panic in another writer leaves nothing inconsistent.
    WRITES.lock().unwrap_or_else(PoisonError::into_inner)
}

fn write(path: &Path, config: &Config) -> Result<(), ConfigError> {
    let text = toml::to_string_pretty(config)?;
    storage::write_atomic(path, text.as_bytes()).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes the default config when none exists yet, so there is a file to edit.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// gio::spawn_blocking(|| config::ensure_file(&config::path()));
/// ```
pub fn ensure_file(path: &Path) -> Result<bool, ConfigError> {
    let _writing = writing();
    if path.try_exists().map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })? {
        return Ok(false);
    }
    write(path, &Config::default())?;
    Ok(true)
}

/// Sets the key for one drawing action in the config at `path` and returns the keys as saved.
///
/// A key another drawing action already uses, or a config that can't be read, leaves the file
/// untouched. Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let keys = config::set_drawing_key(&config::path(), DrawAction::Smaller, DrawKey::Key('-'))?;
/// ```
pub fn set_drawing_key(
    path: &Path,
    action: DrawAction,
    key: DrawKey,
) -> Result<DrawingKeys, ConfigError> {
    let mut config = load_from(path)?.unwrap_or_default();
    config.drawing.keys.set(action, key);
    if let Some(source) = config.drawing.keys.conflict() {
        return Err(ConfigError::Keys {
            path: path.to_path_buf(),
            source,
        });
    }
    save_to(path, &config)?;
    Ok(config.drawing.keys)
}

/// Sets one shortcut in the config at `path` and returns the shortcuts as saved.
///
/// The rest of the file is kept. A trigger another action already uses, or a config that
/// can't be read, leaves the file untouched. Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let shortcuts = config::set_shortcut(&config::path(), ShortcutAction::Region, Slot::Off)?;
/// ```
pub fn set_shortcut(
    path: &Path,
    action: ShortcutAction,
    slot: Slot,
) -> Result<Shortcuts, ConfigError> {
    let _writing = writing();
    let mut config = load_from(path)?.unwrap_or_default();
    config.shortcuts.set(action, slot);
    if let Some(source) = config.shortcuts.conflict() {
        return Err(ConfigError::Shortcuts {
            path: path.to_path_buf(),
            source,
        });
    }
    write(path, &config)?;
    Ok(config.shortcuts)
}

/// Applies `change` to the config at `path` and saves it, keeping everything else in the file.
///
/// A config that can't be read is left untouched. Touches the disk, so call it off the GTK
/// main thread.
///
/// # Examples
///
/// ```ignore
/// config::update(&config::path(), |config| config.notifications.sound = false)?;
/// ```
pub fn update(path: &Path, change: impl FnOnce(&mut Config)) -> Result<Config, ConfigError> {
    let _writing = writing();
    let mut config = load_from(path)?.unwrap_or_default();
    change(&mut config);
    write(path, &config)?;
    Ok(config)
}

fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
    let parse_error = |source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    };
    let table: Table = toml::from_str(text).map_err(parse_error)?;
    let table = migrate::migrate(table).map_err(|source| ConfigError::Migrate {
        path: path.to_path_buf(),
        source,
    })?;
    let config: Config = table.try_into().map_err(parse_error)?;
    if let Some(source) = config.shortcuts.conflict() {
        return Err(ConfigError::Shortcuts {
            path: path.to_path_buf(),
            source,
        });
    }
    if let Some(source) = config.drawing.keys.conflict() {
        return Err(ConfigError::Keys {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcuts::Trigger;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("kiekje-config-{name}-{}", std::process::id()))
            .join("config.toml")
    }

    #[test]
    fn writes_a_chosen_folder_under_home_with_a_tilde() {
        let home = Path::new("/home/ada");
        let folder = Folder::chosen(home.join("Shots/2026"), home).unwrap();
        assert_eq!(String::from(folder.clone()), "~/Shots/2026");
        assert_eq!(folder.path, PathBuf::from("/home/ada/Shots/2026"));
        let outside = Folder::chosen(PathBuf::from("/srv/shots"), home).unwrap();
        assert_eq!(String::from(outside), "/srv/shots");
        assert!(Folder::chosen(PathBuf::from("shots"), home).is_err());
    }

    #[test]
    fn updates_one_setting_and_keeps_the_rest() {
        let path = scratch("update");
        let _ = fs::remove_dir_all(path.parent().unwrap());
        set_shortcut(&path, ShortcutAction::Region, Slot::Off).unwrap();
        let config = update(&path, |config| config.notifications.sound = false).unwrap();
        assert!(!config.notifications.sound);
        let saved = load_from(&path).unwrap().unwrap();
        assert!(!saved.notifications.sound);
        assert_eq!(saved.shortcuts.slot(ShortcutAction::Region), &Slot::Off);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn keeps_every_change_from_concurrent_updates() {
        let path = scratch("concurrent");
        let _ = fs::remove_dir_all(path.parent().unwrap());
        ensure_file(&path).unwrap();
        let sound = {
            let path = path.clone();
            std::thread::spawn(move || {
                for _ in 0..50 {
                    update(&path, |config| config.notifications.sound = false).unwrap();
                }
            })
        };
        for _ in 0..50 {
            set_shortcut(&path, ShortcutAction::Region, Slot::Off).unwrap();
        }
        sound.join().unwrap();
        let saved = load_from(&path).unwrap().unwrap();
        assert!(!saved.notifications.sound);
        assert_eq!(saved.shortcuts.slot(ShortcutAction::Region), &Slot::Off);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn leaves_an_unreadable_config_alone_on_update() {
        let path = scratch("update-broken");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "version = ").unwrap();
        assert!(update(&path, |config| config.notifications.enabled = false).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "version = ");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn reads_a_missing_file_as_none() {
        assert!(load_from(&scratch("missing")).unwrap().is_none());
    }

    #[test]
    fn fills_missing_sections_with_defaults() {
        let config = parse("version = 1", Path::new("config.toml")).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn reads_a_custom_folder_and_filename() {
        let config = parse(
            "[storage]\nfolder = \"/srv/shots\"\nfilename = \"shot-{date}\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(config.storage.captures_dir(), PathBuf::from("/srv/shots"));
        assert_eq!(config.storage.filename.to_string(), "shot-{date}");
    }

    #[test]
    fn reads_a_jpeg_format_with_quality() {
        let config = parse(
            "[storage]\nformat = \"jpg\"\nquality = 75",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(config.storage.format, ImageFormat::Jpeg);
        assert_eq!(config.storage.quality.get(), 75);
    }

    #[test]
    fn rejects_a_quality_above_one_hundred() {
        let parsed = parse("[storage]\nquality = 101", Path::new("config.toml"));
        assert!(matches!(parsed, Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn expands_a_tilde_folder_against_home() {
        let folder = Folder::parse("~/Shots".into(), Path::new("/home/test")).unwrap();
        assert_eq!(folder.path, PathBuf::from("/home/test/Shots"));
        assert_eq!(String::from(folder), "~/Shots");
    }

    #[test]
    fn reads_notification_switches_and_defaults_the_rest() {
        let config = parse("[notifications]\nsound = false", Path::new("config.toml")).unwrap();
        assert_eq!(
            config.notifications,
            Notifications {
                enabled: true,
                sound: false
            }
        );
    }

    #[test]
    fn saves_to_disk_and_keeps_unsaved_captures_a_week_by_default() {
        let older = parse(
            "version = 1\n[storage]\nfilename = \"a\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(
            older.saving,
            Saving {
                enabled: true,
                keep_unsaved_days: 7
            }
        );
        assert_eq!(Config::default().saving, older.saving);
    }

    #[test]
    fn reads_saving_switched_off_and_defaults_the_keep_days() {
        let config = parse("[saving]\nenabled = false", Path::new("config.toml")).unwrap();
        assert!(!config.saving.enabled);
        assert_eq!(config.saving.keep_unsaved_days, 7);
        let custom = parse("[saving]\nkeep_unsaved_days = 2", Path::new("config.toml")).unwrap();
        assert!(custom.saving.enabled);
        assert_eq!(custom.saving.keep_unsaved_days, 2);
    }

    #[test]
    fn rejects_negative_keep_days_and_unknown_saving_keys() {
        assert!(parse("[saving]\nkeep_unsaved_days = -1", Path::new("config.toml")).is_err());
        assert!(parse("[saving]\nkeep = 3", Path::new("config.toml")).is_err());
    }

    #[test]
    fn reads_inline_editing_after_selection() {
        let config = parse(
            "[capture]\nafter_selection = \"edit\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(config.capture.after_selection, AfterSelection::Edit);
        assert_eq!(
            Config::default().capture.after_selection,
            AfterSelection::Finish
        );
    }

    #[test]
    fn reads_the_drawing_style_and_defaults_the_rest() {
        let config = parse(
            "[drawing]\ncolor = \"blue\"\nsize = 6",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(
            config.drawing,
            Drawing {
                color: Color::Blue,
                size: 6,
                opacity: 100,
                keys: DrawingKeys::default()
            }
        );
        let orange = parse("[drawing]\ncolor = \"orange\"", Path::new("config.toml")).unwrap();
        assert_eq!(orange.drawing.color, Color::Accent);
        assert_eq!(Config::default().drawing.size, 3);
    }

    #[test]
    fn reads_drawing_keys_and_an_empty_one_as_off() {
        let config = parse(
            "[drawing.keys]\nbox = \"b\"\nthinner = \"-\"\nstep = \"\"",
            Path::new("config.toml"),
        )
        .unwrap();
        let keys = config.drawing.keys;
        assert_eq!(keys.rectangle, DrawKey::Key('b'));
        assert_eq!(keys.thinner, DrawKey::Key('-'));
        assert_eq!(keys.step, DrawKey::Off);
        assert_eq!(keys.pen, DrawKey::Key('p'));
    }

    #[test]
    fn rejects_drawing_keys_that_clash_or_the_overlay_uses() {
        let clash = parse("[drawing.keys]\nbox = \"p\"", Path::new("config.toml"));
        assert!(matches!(clash, Err(ConfigError::Keys { .. })));
        assert!(parse("[drawing.keys]\npen = \"f\"", Path::new("config.toml")).is_err());
    }

    #[test]
    fn sets_one_drawing_key_and_refuses_a_taken_one() {
        let path = scratch("drawing-keys");
        let _ = fs::remove_dir_all(path.parent().unwrap());
        let keys = set_drawing_key(&path, DrawAction::Smaller, DrawKey::Key('-')).unwrap();
        assert_eq!(keys.thinner, DrawKey::Key('-'));
        let taken = set_drawing_key(&path, DrawAction::Larger, DrawKey::Key('-'));
        assert!(matches!(taken, Err(ConfigError::Keys { .. })));
        let saved = load_from(&path).unwrap().unwrap();
        assert_eq!(saved.drawing.keys.thinner, DrawKey::Key('-'));
        assert_eq!(saved.drawing.keys.thicker, DrawKey::Key(']'));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_an_unknown_drawing_colour() {
        assert!(parse("[drawing]\ncolor = \"purple\"", Path::new("config.toml")).is_err());
    }

    #[test]
    fn reads_the_magnifier_switch_and_defaults_it_on() {
        let config = parse("[overlay]\nmagnifier = false", Path::new("config.toml")).unwrap();
        assert!(!config.overlay.magnifier);
        assert!(Config::default().overlay.magnifier);
        let older = parse("version = 1\n[capture]", Path::new("config.toml")).unwrap();
        assert!(older.overlay.magnifier);
    }

    #[test]
    fn reads_the_loupe_zoom_and_slow_pointer_with_defaults() {
        let config = parse(
            "[overlay]\nmagnifier_pixels = 21\nslow_pointer = \"sixteenth\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(config.overlay.magnifier_pixels, 21);
        assert_eq!(config.overlay.slow_pointer, SlowPointer::Sixteenth);
        let older = parse("[overlay]\nmagnifier = true", Path::new("config.toml")).unwrap();
        assert_eq!(older.overlay.magnifier_pixels, 15);
        assert_eq!(older.overlay.slow_pointer, SlowPointer::Eighth);
    }

    #[test]
    fn reads_the_window_snap_switch_and_defaults_it_on() {
        let config = parse("[overlay]\nwindow_snap = false", Path::new("config.toml")).unwrap();
        assert!(!config.overlay.window_snap);
        assert!(config.overlay.magnifier);
        assert!(Config::default().overlay.window_snap);
    }

    #[test]
    fn leaves_the_update_check_off_unless_turned_on() {
        assert!(!Config::default().general.check_for_updates);
        let older = parse(
            "version = 1\n[notifications]\nsound = false",
            Path::new("config.toml"),
        )
        .unwrap();
        assert!(!older.general.check_for_updates);
        let on = parse(
            "[general]\ncheck_for_updates = true",
            Path::new("config.toml"),
        )
        .unwrap();
        assert!(on.general.check_for_updates);
        assert!(parse("[general]\nupdates = true", Path::new("config.toml")).is_err());
    }

    #[test]
    fn reads_the_hints_switch_and_defaults_it_on() {
        let config = parse("[overlay]\nhints = false", Path::new("config.toml")).unwrap();
        assert!(!config.overlay.hints);
        assert!(Config::default().overlay.hints);
    }

    #[test]
    fn rejects_an_unknown_overlay_key() {
        let error = parse("[overlay]\nloupe = true", Path::new("config.toml")).unwrap_err();
        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn rejects_an_unknown_after_selection() {
        assert!(
            parse(
                "[capture]\nafter_selection = \"maybe\"",
                Path::new("config.toml")
            )
            .is_err()
        );
    }

    #[test]
    fn reads_shortcuts_and_an_empty_one_as_off() {
        let config = parse(
            "[shortcuts]\ncapture-region = \"super+shift+S\"\ncapture-fullscreen = \"\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(
            config.shortcuts.capture_region,
            Slot::Key(Trigger::parse("SHIFT+LOGO+s").unwrap())
        );
        assert_eq!(config.shortcuts.capture_fullscreen, Slot::Off);
    }

    #[test]
    fn reads_a_config_from_before_window_capture_with_the_window_shortcut_off() {
        let config = parse(
            "version = 1\n[shortcuts]\ncapture-region = \"Print\"\ncapture-fullscreen = \"SHIFT+Print\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(config.shortcuts.capture_window, Slot::Off);
        assert_eq!(config.shortcuts, Shortcuts::default());
    }

    #[test]
    fn reads_a_window_shortcut() {
        let config = parse(
            "[shortcuts]\ncapture-window = \"alt+Print\"",
            Path::new("config.toml"),
        )
        .unwrap();
        assert_eq!(
            config.shortcuts.capture_window,
            Slot::Key(Trigger::parse("ALT+Print").unwrap())
        );
    }

    #[test]
    fn rejects_a_window_shortcut_another_action_uses() {
        let error = parse(
            "[shortcuts]\ncapture-window = \"Print\"",
            Path::new("config.toml"),
        )
        .unwrap_err();
        assert!(matches!(error, ConfigError::Shortcuts { .. }));
    }

    #[test]
    fn leaves_an_unset_window_shortcut_out_of_the_saved_file() {
        let path = scratch("window-off");
        save_to(&path, &Config::default()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("capture-window"), "{text}");
        let mut config = Config::default();
        config.shortcuts.capture_window = Slot::Key(Trigger::parse("ALT+Print").unwrap());
        save_to(&path, &config).unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("capture-window = \"ALT+Print\"")
        );
        assert_eq!(load_from(&path).unwrap(), Some(config));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_a_shortcut_that_breaks_the_rules() {
        let error = parse(
            "[shortcuts]\ncapture-region = \"s\"",
            Path::new("config.toml"),
        )
        .unwrap_err();
        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn rejects_one_trigger_on_two_actions() {
        let error = parse(
            "[shortcuts]\ncapture-region = \"Print\"\ncapture-fullscreen = \"Print\"",
            Path::new("config.toml"),
        )
        .unwrap_err();
        assert!(matches!(error, ConfigError::Shortcuts { .. }));
    }

    #[test]
    fn rejects_a_relative_folder() {
        assert_eq!(
            Folder::parse("Shots".into(), Path::new("/home/test")),
            Err(FolderError("Shots".into()))
        );
    }

    #[test]
    fn rejects_an_invalid_filename_template() {
        let error = parse(
            "[storage]\nfilename = \"{weekday}\"",
            Path::new("config.toml"),
        )
        .unwrap_err();
        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = parse("[storage]\nfolders = \"/srv\"", Path::new("config.toml")).unwrap_err();
        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn refuses_a_config_from_a_newer_build() {
        let error = parse("version = 9", Path::new("config.toml")).unwrap_err();
        assert!(matches!(
            error,
            ConfigError::Migrate {
                source: MigrateError::Newer(9),
                ..
            }
        ));
    }

    #[test]
    fn round_trips_through_an_atomic_save() {
        let path = scratch("round-trip");
        let mut config = Config::default();
        config.storage.folder = Some(Folder::parse("/srv/shots".into(), Path::new("/")).unwrap());
        config.storage.filename = Template::parse("{date} {time}").unwrap();
        config.storage.format = ImageFormat::Jpeg;
        config.storage.quality = Quality::try_from(60).unwrap();
        config.shortcuts.capture_fullscreen = Slot::Off;
        config.saving = Saving {
            enabled: false,
            keep_unsaved_days: 3,
        };
        save_to(&path, &config).unwrap();
        save_to(&path, &config).unwrap();
        assert_eq!(load_from(&path).unwrap(), Some(config));
        let leftovers = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn sets_one_shortcut_and_keeps_the_rest() {
        let path = scratch("set-shortcut");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[storage]\nfilename = \"mine\"").unwrap();
        let trigger = Trigger::parse("CTRL+ALT+s").unwrap();
        let shortcuts =
            set_shortcut(&path, ShortcutAction::Region, Slot::Key(trigger.clone())).unwrap();
        assert_eq!(shortcuts.capture_region, Slot::Key(trigger));
        let saved = load_from(&path).unwrap().unwrap();
        assert_eq!(saved.shortcuts, shortcuts);
        assert_eq!(saved.storage.filename.to_string(), "mine");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn refuses_a_shortcut_another_action_uses() {
        let path = scratch("set-conflict");
        let taken = Shortcuts::default().capture_fullscreen;
        let error = set_shortcut(&path, ShortcutAction::Region, taken).unwrap_err();
        assert!(matches!(error, ConfigError::Shortcuts { .. }));
        assert!(!path.exists());
    }

    #[test]
    fn writes_defaults_only_when_no_file_exists() {
        let path = scratch("ensure");
        assert!(ensure_file(&path).unwrap());
        fs::write(&path, "[storage]\nfilename = \"mine\"").unwrap();
        assert!(!ensure_file(&path).unwrap());
        assert_eq!(
            load_from(&path)
                .unwrap()
                .unwrap()
                .storage
                .filename
                .to_string(),
            "mine"
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
