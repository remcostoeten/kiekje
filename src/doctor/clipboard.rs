use std::fs;
use std::path::Path;

use crate::backends::detect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardManager {
    Klipper,
    GPaste,
    CopyQ,
    Cliphist,
    Clipman,
    WlClipPersist,
    Clipse,
    Clipcat,
    Diodon,
    Parcellite,
    Greenclip,
    XfceClipman,
    WlPasteWatch,
}

const BUS_NAMES: [(&str, ClipboardManager); 2] = [
    ("org.kde.klipper", ClipboardManager::Klipper),
    ("org.gnome.GPaste", ClipboardManager::GPaste),
];

const PROGRAMS: [(&str, ClipboardManager); 13] = [
    ("klipper", ClipboardManager::Klipper),
    ("gpaste-daemon", ClipboardManager::GPaste),
    ("copyq", ClipboardManager::CopyQ),
    ("cliphist", ClipboardManager::Cliphist),
    ("clipman", ClipboardManager::Clipman),
    ("wl-clip-persist", ClipboardManager::WlClipPersist),
    ("clipse", ClipboardManager::Clipse),
    ("clipcatd", ClipboardManager::Clipcat),
    ("diodon", ClipboardManager::Diodon),
    ("parcellite", ClipboardManager::Parcellite),
    ("greenclip", ClipboardManager::Greenclip),
    ("xfce4-clipman", ClipboardManager::XfceClipman),
    ("gpaste", ClipboardManager::GPaste),
];

impl ClipboardManager {
    /// The name people know the clipboard manager by.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert_eq!(ClipboardManager::Klipper.name(), "Klipper");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Self::Klipper => "Klipper",
            Self::GPaste => "GPaste",
            Self::CopyQ => "CopyQ",
            Self::Cliphist => "cliphist",
            Self::Clipman => "clipman",
            Self::WlClipPersist => "wl-clip-persist",
            Self::Clipse => "clipse",
            Self::Clipcat => "clipcat",
            Self::Diodon => "Diodon",
            Self::Parcellite => "Parcellite",
            Self::Greenclip => "greenclip",
            Self::XfceClipman => "Xfce Clipman",
            Self::WlPasteWatch => "wl-paste --watch",
        }
    }
}

/// Looks for a running clipboard manager on the session bus and in `/proc`.
///
/// Nothing is executed: bus names are asked from the bus daemon and command lines are read from
/// `/proc`. Blocks on D-Bus and the file system, so call it off the GTK main thread.
///
/// # Examples
///
/// ```ignore
/// let manager = gio::spawn_blocking(clipboard::detect).await?;
/// ```
pub fn detect() -> Option<ClipboardManager> {
    let owned: Vec<&str> = match detect::session_bus() {
        Ok(connection) => BUS_NAMES
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| detect::name_has_owner(&connection, name))
            .collect(),
        Err(error) => {
            eprintln!("kiekje: no session bus to look for a clipboard manager: {error}");
            Vec::new()
        }
    };
    identify(&owned, &command_lines(Path::new("/proc")))
}

/// Picks the clipboard manager from the bus names that have an owner and the running command lines.
///
/// # Examples
///
/// ```ignore
/// let found = identify(&["org.kde.klipper"], &[]);
/// assert_eq!(found, Some(ClipboardManager::Klipper));
/// ```
pub fn identify(owned_names: &[&str], command_lines: &[Vec<String>]) -> Option<ClipboardManager> {
    let on_bus = BUS_NAMES
        .iter()
        .find(|(name, _)| owned_names.contains(name))
        .map(|(_, manager)| *manager);
    on_bus.or_else(|| command_lines.iter().find_map(|args| from_command(args)))
}

fn from_command(args: &[String]) -> Option<ClipboardManager> {
    let program = program(args.first()?);
    if let Some(manager) = known(program) {
        return Some(manager);
    }
    let watches = program == "wl-paste"
        && args
            .iter()
            .skip(1)
            .any(|arg| arg == "--watch" || arg == "-w");
    if !watches {
        return None;
    }
    let found = args.iter().skip(1).find_map(|arg| known(program_of(arg)));
    Some(found.unwrap_or(ClipboardManager::WlPasteWatch))
}

fn known(program: &str) -> Option<ClipboardManager> {
    PROGRAMS
        .iter()
        .find(|(name, _)| *name == program)
        .map(|(_, manager)| *manager)
}

fn program(arg: &str) -> &str {
    program_of(arg.split_whitespace().next().unwrap_or(arg))
}

fn program_of(arg: &str) -> &str {
    arg.rsplit('/').next().unwrap_or(arg)
}

fn command_lines(proc: &Path) -> Vec<Vec<String>> {
    let Ok(entries) = fs::read_dir(proc) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
        })
        .filter_map(|entry| fs::read(entry.path().join("cmdline")).ok())
        .map(|bytes| {
            bytes
                .split(|byte| *byte == 0)
                .filter(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect::<Vec<_>>()
        })
        .filter(|args| !args.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn finds_klipper_on_the_bus() {
        assert_eq!(
            identify(&["org.kde.klipper"], &[]),
            Some(ClipboardManager::Klipper)
        );
    }

    #[test]
    fn finds_cliphist_behind_wl_paste_watch() {
        let lines = [
            line("/usr/bin/waybar"),
            line("wl-paste --type image --watch cliphist store"),
        ];
        assert_eq!(identify(&[], &lines), Some(ClipboardManager::Cliphist));
    }

    #[test]
    fn names_an_unknown_wl_paste_watcher_generically() {
        let lines = [line("wl-paste -w /home/ada/bin/keep-clip")];
        assert_eq!(identify(&[], &lines), Some(ClipboardManager::WlPasteWatch));
    }

    #[test]
    fn finds_a_manager_by_its_program_path() {
        let lines = [line("/usr/bin/copyq --start-server")];
        assert_eq!(identify(&[], &lines), Some(ClipboardManager::CopyQ));
        let script = [line("/usr/bin/wl-clip-persist --clipboard regular")];
        assert_eq!(
            identify(&[], &script),
            Some(ClipboardManager::WlClipPersist)
        );
    }

    #[test]
    fn ignores_programs_that_only_mention_a_manager() {
        let lines = [
            line("nvim /home/ada/.config/copyq/copyq.conf"),
            line("wl-paste --no-newline"),
            line("wl-paste cliphist"),
        ];
        assert_eq!(identify(&[], &lines), None);
    }

    #[test]
    fn finds_nothing_in_a_bare_session() {
        assert_eq!(identify(&["org.kde.KWin"], &[line("kwin_wayland")]), None);
    }

    #[test]
    fn reads_command_lines_from_a_proc_tree() {
        let root = std::env::temp_dir().join(format!("kiekje-proc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("42")).unwrap();
        fs::create_dir_all(root.join("43")).unwrap();
        fs::create_dir_all(root.join("self")).unwrap();
        fs::write(
            root.join("42/cmdline"),
            b"wl-paste\0--watch\0cliphist\0store\0",
        )
        .unwrap();
        fs::write(root.join("43/cmdline"), b"").unwrap();
        fs::write(root.join("self/cmdline"), b"copyq\0").unwrap();
        let lines = command_lines(&root);
        assert_eq!(lines, vec![line("wl-paste --watch cliphist store")]);
        fs::remove_dir_all(root).unwrap();
    }
}
