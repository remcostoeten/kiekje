use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use gtk::glib;

use super::{ShortcutAction, hyprland_bind};
use crate::storage;

const MAX_SOURCE_DEPTH: usize = 4;

/// Returns `$XDG_CONFIG_HOME/kiekje/hyprland.conf`, the bind file Kiekje manages.
///
/// # Examples
///
/// ```ignore
/// let line = source_line(&include_path());
/// ```
pub fn include_path() -> PathBuf {
    glib::user_config_dir().join("kiekje").join("hyprland.conf")
}

/// Returns `$XDG_CONFIG_HOME/hypr/hyprland.conf`.
///
/// # Examples
///
/// ```ignore
/// let sourced = is_sourced(&main_config_path(), &include_path(), &glib::home_dir());
/// ```
pub fn main_config_path() -> PathBuf {
    glib::user_config_dir().join("hypr").join("hyprland.conf")
}

/// Renders the managed include with one `bind` per shortcut action.
///
/// # Examples
///
/// ```ignore
/// assert!(include_text("dev.example.App").contains("global, dev.example.App:capture-region"));
/// ```
pub fn include_text(app_id: &str) -> String {
    let mut text = String::from(
        "# Written by Kiekje. Edits are replaced; copy a bind into your own config to change it.\n",
    );
    for action in ShortcutAction::ALL {
        text.push_str(&hyprland_bind(action, app_id));
        text.push('\n');
    }
    text
}

/// Writes the managed include when its content differs and reports whether it changed.
///
/// Touches the disk, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let changed = write_include(&include_path(), APP_ID)?;
/// ```
pub fn write_include(path: &Path, app_id: &str) -> io::Result<bool> {
    let text = include_text(app_id);
    match fs::read_to_string(path) {
        Ok(current) if current == text => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    storage::write_atomic(path, text.as_bytes())?;
    Ok(true)
}

/// The line that makes Hyprland read the managed include, with the home folder as `~`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(source_line(&include_path()), "source = ~/.config/kiekje/hyprland.conf");
/// ```
pub fn source_line(include: &Path) -> String {
    let home = glib::home_dir();
    match include.strip_prefix(&home) {
        Ok(relative) => format!("source = ~/{}", relative.display()),
        Err(_) => format!("source = {}", include.display()),
    }
}

/// Reports whether the Hyprland config at `main`, or a file it sources, sources `include`.
///
/// Follows `source =` lines a few levels deep and expands `~`, but not globs or variables.
///
/// # Examples
///
/// ```ignore
/// let sourced = is_sourced(&main_config_path(), &include_path(), &glib::home_dir());
/// ```
pub fn is_sourced(main: &Path, include: &Path, home: &Path) -> bool {
    let mut pending = vec![(main.to_path_buf(), 0)];
    let mut seen = Vec::new();
    while let Some((path, depth)) = pending.pop() {
        if seen.contains(&path) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for source in sources(&text, home) {
            if source == include {
                return true;
            }
            if depth + 1 < MAX_SOURCE_DEPTH {
                pending.push((source, depth + 1));
            }
        }
        seen.push(path);
    }
    false
}

fn sources(text: &str, home: &Path) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let (key, value) = line.split_once('=')?;
            (key.trim() == "source").then(|| expand(value.trim(), home))
        })
        .collect()
}

fn expand(value: &str, home: &Path) -> PathBuf {
    match value.strip_prefix("~/") {
        Some(relative) => home.join(relative),
        None => PathBuf::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kiekje-hyprland-{name}-{}", std::process::id()))
    }

    #[test]
    fn binds_every_action_in_the_include() {
        let text = include_text("dev.example.App");
        for action in ShortcutAction::ALL {
            assert!(text.contains(&hyprland_bind(action, "dev.example.App")));
        }
        assert!(text.starts_with('#'));
    }

    #[test]
    fn rewrites_the_include_only_when_it_changes() {
        let path = scratch("write").join("hyprland.conf");
        assert!(write_include(&path, "dev.example.App").unwrap());
        assert!(!write_include(&path, "dev.example.App").unwrap());
        assert!(write_include(&path, "dev.example.Other").unwrap());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn finds_a_tilde_source_in_a_nested_file() {
        let home = scratch("nested");
        let hypr = home.join(".config/hypr");
        fs::create_dir_all(&hypr).unwrap();
        fs::write(
            hypr.join("hyprland.conf"),
            "source = ~/.config/hypr/bindings.conf\n",
        )
        .unwrap();
        fs::write(
            hypr.join("bindings.conf"),
            "  source=~/.config/kiekje/hyprland.conf # screenshots\n",
        )
        .unwrap();
        let include = home.join(".config/kiekje/hyprland.conf");
        assert!(is_sourced(&hypr.join("hyprland.conf"), &include, &home));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn ignores_commented_sources_and_cycles() {
        let home = scratch("commented");
        fs::create_dir_all(&home).unwrap();
        let main = home.join("hyprland.conf");
        fs::write(
            &main,
            "# source = ~/kiekje.conf\nsource = ~/hyprland.conf\n",
        )
        .unwrap();
        assert!(!is_sourced(&main, &home.join("kiekje.conf"), &home));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn reports_a_missing_config_as_not_sourced() {
        let home = scratch("missing");
        assert!(!is_sourced(
            &home.join("hyprland.conf"),
            &home.join("kiekje.conf"),
            &home
        ));
    }
}
