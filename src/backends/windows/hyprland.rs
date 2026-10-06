use std::cmp::Reverse;
use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::{WindowError, WindowGeometrySource};
use crate::capture::LogicalRect;
use crate::capture::mapping::rect_from_drag;

const SOCKET_NAME: &str = ".socket.sock";
const LEGACY_SOCKET_DIR: &str = "/tmp/hypr";
const IO_TIMEOUT: Duration = Duration::from_millis(500);

pub struct Hyprland;

impl WindowGeometrySource for Hyprland {
    fn windows(&self) -> Result<Vec<LogicalRect>, WindowError> {
        let socket = socket_path().ok_or(WindowError::Unavailable)?;
        let clients = request(&socket, "j/clients")?;
        let monitors = request(&socket, "j/monitors")?;
        Ok(parse_windows(&clients, &monitors)?)
    }
}

/// Finds the request socket of the Hyprland instance this session runs in.
///
/// Reads `HYPRLAND_INSTANCE_SIGNATURE` and looks under `$XDG_RUNTIME_DIR/hypr`, then under
/// `/tmp/hypr` where Hyprland before 0.40 put it.
///
/// # Examples
///
/// ```ignore
/// let running = hyprland::socket_path().is_some();
/// ```
pub fn socket_path() -> Option<PathBuf> {
    let signature = env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("hypr"));
    runtime
        .into_iter()
        .chain([PathBuf::from(LEGACY_SOCKET_DIR)])
        .map(|dir| dir.join(&signature).join(SOCKET_NAME))
        .find(|path| path.exists())
}

fn request(socket: &Path, command: &str) -> io::Result<String> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.write_all(command.as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

#[derive(Deserialize)]
struct Workspace {
    id: i64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Fullscreen {
    Flag(bool),
    Level(i64),
}

impl Default for Fullscreen {
    fn default() -> Self {
        Self::Flag(false)
    }
}

impl Fullscreen {
    fn is_on(&self) -> bool {
        match self {
            Self::Flag(on) => *on,
            Self::Level(level) => *level > 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    at: [f64; 2],
    size: [f64; 2],
    workspace: Workspace,
    #[serde(default = "mapped_by_default")]
    mapped: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    floating: bool,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    fullscreen: Fullscreen,
    #[serde(rename = "focusHistoryID", default = "least_recent")]
    focus_history: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Monitor {
    active_workspace: Workspace,
    special_workspace: Option<Workspace>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Tiled,
    Floating,
    Fullscreen,
    Special,
}

impl Client {
    fn layer(&self) -> Layer {
        if self.workspace.id < 0 {
            Layer::Special
        } else if self.fullscreen.is_on() {
            Layer::Fullscreen
        } else if self.floating || self.pinned {
            Layer::Floating
        } else {
            Layer::Tiled
        }
    }

    fn rect(&self) -> LogicalRect {
        let [x, y] = self.at;
        let [width, height] = self.size;
        rect_from_drag((x, y), (x + width, y + height))
    }
}

fn mapped_by_default() -> bool {
    true
}

fn least_recent() -> i64 {
    i64::MAX
}

/// Turns the replies to `j/clients` and `j/monitors` into visible window frames, topmost first.
///
/// Keeps mapped, unhidden windows on a workspace some monitor shows, plus pinned windows.
/// Hyprland reports no stacking order, so special workspaces go above fullscreen windows,
/// those above floating ones and those above tiled ones, and the most recently focused window
/// wins inside each layer.
///
/// # Examples
///
/// ```ignore
/// let windows = parse_windows(&request(&socket, "j/clients")?, &request(&socket, "j/monitors")?)?;
/// ```
pub fn parse_windows(clients: &str, monitors: &str) -> Result<Vec<LogicalRect>, serde_json::Error> {
    let clients: Vec<Client> = serde_json::from_str(clients)?;
    let monitors: Vec<Monitor> = serde_json::from_str(monitors)?;
    let shown: Vec<i64> = monitors
        .iter()
        .filter(|monitor| !monitor.disabled)
        .flat_map(|monitor| {
            [
                Some(monitor.active_workspace.id),
                monitor
                    .special_workspace
                    .as_ref()
                    .map(|workspace| workspace.id)
                    .filter(|id| *id != 0),
            ]
        })
        .flatten()
        .collect();

    let mut visible: Vec<&Client> = clients
        .iter()
        .filter(|client| client.mapped && !client.hidden)
        .filter(|client| client.pinned || shown.contains(&client.workspace.id))
        .filter(|client| client.size[0] > 0.0 && client.size[1] > 0.0)
        .collect();
    visible.sort_by_key(|client| (Reverse(client.layer()), client.focus_history));
    Ok(visible.into_iter().map(Client::rect).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> LogicalRect {
        LogicalRect {
            x,
            y,
            width,
            height,
        }
    }

    const MONITORS: &str = r#"[
        {"id": 0, "name": "DP-1", "x": 0, "y": 0, "width": 3840, "height": 2160, "scale": 2.0,
         "activeWorkspace": {"id": 1, "name": "1"},
         "specialWorkspace": {"id": 0, "name": ""}, "disabled": false},
        {"id": 1, "name": "HDMI-A-1", "x": 1920, "y": 0, "width": 1920, "height": 1080,
         "activeWorkspace": {"id": 2, "name": "2"},
         "specialWorkspace": {"id": -98, "name": "special:magic"}}
    ]"#;

    #[test]
    fn keeps_mapped_windows_on_shown_workspaces() {
        let clients = r#"[
            {"address": "0x1", "mapped": true, "hidden": false, "at": [10, 20], "size": [800, 600],
             "workspace": {"id": 1, "name": "1"}, "floating": false, "pinned": false,
             "fullscreen": 0, "class": "kitty", "title": "fish", "focusHistoryID": 0},
            {"address": "0x2", "mapped": true, "hidden": false, "at": [10, 20], "size": [800, 600],
             "workspace": {"id": 3, "name": "3"}, "floating": false, "focusHistoryID": 1},
            {"address": "0x3", "mapped": false, "hidden": false, "at": [0, 0], "size": [5, 5],
             "workspace": {"id": 1, "name": "1"}, "floating": false, "focusHistoryID": 2},
            {"address": "0x4", "mapped": true, "hidden": true, "at": [0, 0], "size": [5, 5],
             "workspace": {"id": 1, "name": "1"}, "floating": false, "focusHistoryID": 3},
            {"address": "0x5", "mapped": true, "hidden": false, "at": [100, 100], "size": [50, 50],
             "workspace": {"id": 7, "name": "7"}, "floating": true, "pinned": true,
             "focusHistoryID": 4}
        ]"#;
        assert_eq!(
            parse_windows(clients, MONITORS).unwrap(),
            [rect(100, 100, 50, 50), rect(10, 20, 800, 600)]
        );
    }

    #[test]
    fn stacks_special_fullscreen_floating_then_tiled() {
        let clients = r#"[
            {"at": [0, 0], "size": [1920, 1080], "workspace": {"id": 1}, "focusHistoryID": 0},
            {"at": [1, 1], "size": [100, 100], "workspace": {"id": 1}, "floating": true,
             "focusHistoryID": 3},
            {"at": [2, 2], "size": [100, 100], "workspace": {"id": 1}, "floating": true,
             "focusHistoryID": 1},
            {"at": [1920, 0], "size": [1920, 1080], "workspace": {"id": 2}, "fullscreen": true,
             "focusHistoryID": 2},
            {"at": [2000, 50], "size": [600, 400], "workspace": {"id": -98},
             "floating": true, "focusHistoryID": 4}
        ]"#;
        assert_eq!(
            parse_windows(clients, MONITORS).unwrap(),
            [
                rect(2000, 50, 600, 400),
                rect(1920, 0, 1920, 1080),
                rect(2, 2, 100, 100),
                rect(1, 1, 100, 100),
                rect(0, 0, 1920, 1080),
            ]
        );
    }

    #[test]
    fn rounds_fractional_geometry_outward() {
        let clients = r#"[{"at": [10.5, 20.25], "size": [100.0, 50.5], "workspace": {"id": 1}}]"#;
        assert_eq!(
            parse_windows(clients, MONITORS).unwrap(),
            [rect(10, 20, 101, 51)]
        );
    }

    #[test]
    fn ignores_workspaces_of_disabled_monitors() {
        let monitors = r#"[{"activeWorkspace": {"id": 1}, "disabled": true}]"#;
        let clients = r#"[{"at": [0, 0], "size": [10, 10], "workspace": {"id": 1}}]"#;
        assert!(parse_windows(clients, monitors).unwrap().is_empty());
    }

    #[test]
    fn rejects_a_reply_that_is_not_json() {
        assert!(parse_windows("unknown request", MONITORS).is_err());
    }
}
