use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

use gtk::glib;
use serde::Deserialize;
use zbus::blocking::{Connection, Proxy, connection, proxy::Builder};
use zbus::proxy::CacheProperties;

use super::{WindowError, WindowGeometrySource};
use crate::capture::LogicalRect;
use crate::capture::mapping::rect_from_drag;

const DESTINATION: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const REPORT_PATH: &str = "/dev/remcostoeten/Kiekje/WindowReport";
const REPORT_INTERFACE: &str = "dev.remcostoeten.Kiekje.WindowReport";
const CALL_TIMEOUT: Duration = Duration::from_secs(1);
const REPORT_TIMEOUT: Duration = Duration::from_millis(600);

static SCRIPT_COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Kwin;

impl WindowGeometrySource for Kwin {
    fn windows(&self) -> Result<Vec<LogicalRect>, WindowError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let connection = connection::Builder::session()?
            .method_timeout(CALL_TIMEOUT)
            .serve_at(REPORT_PATH, ReportSink { sender })?
            .build()?;
        let service = connection
            .unique_name()
            .ok_or(WindowError::Unavailable)?
            .to_string();
        let script = ScriptFile::create(&service)?;
        let scripting = proxy(&connection, SCRIPTING_PATH.to_owned(), SCRIPTING_INTERFACE)?;
        let id: i32 = scripting.call(
            "loadScript",
            &(
                script.path.to_string_lossy().as_ref(),
                script.plugin.as_str(),
            ),
        )?;
        if id < 0 {
            return Err(WindowError::ScriptRejected);
        }
        let report = run_script(&connection, id, &receiver);
        if let Err(error) = scripting.call::<_, _, bool>("unloadScript", &(script.plugin.as_str(),))
        {
            eprintln!("kiekje: could not unload the KWin window script: {error}");
        }
        Ok(parse_report(&report?)?)
    }
}

struct ReportSink {
    sender: SyncSender<String>,
}

#[zbus::interface(name = "dev.remcostoeten.Kiekje.WindowReport")]
impl ReportSink {
    fn windows(&self, report: String) {
        // Only the first report is read; a second run of the script has nobody waiting for it.
        let _ = self.sender.try_send(report);
    }
}

struct ScriptFile {
    path: PathBuf,
    plugin: String,
}

impl ScriptFile {
    fn create(service: &str) -> std::io::Result<Self> {
        let plugin = format!(
            "kiekje-windows-{}-{}",
            std::process::id(),
            SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = glib::user_runtime_dir().join(format!("{plugin}.js"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(script_source(service).as_bytes())?;
        Ok(Self { path, plugin })
    }
}

impl Drop for ScriptFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            eprintln!("kiekje: could not remove {}: {error}", self.path.display());
        }
    }
}

fn proxy<'a>(
    connection: &'a Connection,
    path: String,
    interface: &'static str,
) -> zbus::Result<Proxy<'a>> {
    Builder::new(connection)
        .destination(DESTINATION)?
        .path(path)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
}

fn run_script(
    connection: &Connection,
    id: i32,
    receiver: &Receiver<String>,
) -> Result<String, WindowError> {
    proxy(
        connection,
        format!("{SCRIPTING_PATH}/Script{id}"),
        SCRIPT_INTERFACE,
    )?
    .call::<_, _, ()>("run", &())?;
    receiver
        .recv_timeout(REPORT_TIMEOUT)
        .map_err(|_| WindowError::Timeout)
}

/// Builds the KWin script that reports every window in the stacking order back to `service`.
///
/// # Examples
///
/// ```ignore
/// let source = script_source(":1.42");
/// ```
pub fn script_source(service: &str) -> String {
    format!(
        r#"const desktop = workspace.currentDesktop;
const activity = workspace.currentActivity;
const windows = workspace.stackingOrder.map((window) => {{
  const frame = window.frameGeometry;
  return {{
    x: frame.x,
    y: frame.y,
    width: frame.width,
    height: frame.height,
    normal: Boolean(window.normalWindow || window.dialog),
    minimized: Boolean(window.minimized),
    hidden: Boolean(window.hidden || window.deleted),
    onDesktop: Boolean(window.onAllDesktops
      || (window.desktops || []).some((candidate) => candidate.id === desktop.id)),
    onActivity: !window.activities || window.activities.length === 0
      || window.activities.indexOf(activity) >= 0,
  }};
}});
callDBus("{service}", "{REPORT_PATH}", "{REPORT_INTERFACE}", "Windows", JSON.stringify(windows));
"#
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportedWindow {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    normal: bool,
    minimized: bool,
    hidden: bool,
    on_desktop: bool,
    on_activity: bool,
}

/// Turns the window script's report, ordered bottom to top, into visible window frames,
/// topmost first.
///
/// Keeps normal windows and dialogs that are not minimized or hidden and sit on the current
/// virtual desktop and activity. Panels, the desktop and notifications are dropped.
///
/// # Examples
///
/// ```ignore
/// let windows = parse_report(&report)?;
/// ```
pub fn parse_report(report: &str) -> Result<Vec<LogicalRect>, serde_json::Error> {
    let windows: Vec<ReportedWindow> = serde_json::from_str(report)?;
    Ok(windows
        .iter()
        .rev()
        .filter(|window| window.normal && !window.minimized && !window.hidden)
        .filter(|window| window.on_desktop && window.on_activity)
        .filter(|window| window.width > 0.0 && window.height > 0.0)
        .map(|window| {
            rect_from_drag(
                (window.x, window.y),
                (window.x + window.width, window.y + window.height),
            )
        })
        .collect())
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

    fn window(x: f64, y: f64, normal: bool, minimized: bool, on_desktop: bool) -> String {
        format!(
            r#"{{"x": {x}, "y": {y}, "width": 100, "height": 80, "normal": {normal},
               "minimized": {minimized}, "hidden": false, "onDesktop": {on_desktop},
               "onActivity": true}}"#
        )
    }

    #[test]
    fn reverses_the_stacking_order_to_put_the_topmost_window_first() {
        let report = format!(
            "[{}, {}]",
            window(0.0, 0.0, true, false, true),
            window(50.0, 40.0, true, false, true)
        );
        assert_eq!(
            parse_report(&report).unwrap(),
            [rect(50, 40, 100, 80), rect(0, 0, 100, 80)]
        );
    }

    #[test]
    fn drops_panels_minimized_and_other_desktop_windows() {
        let report = format!(
            "[{}, {}, {}, {}]",
            window(0.0, 0.0, false, false, true),
            window(10.0, 0.0, true, true, true),
            window(20.0, 0.0, true, false, false),
            window(30.0, 0.0, true, false, true)
        );
        assert_eq!(parse_report(&report).unwrap(), [rect(30, 0, 100, 80)]);
    }

    #[test]
    fn drops_windows_on_another_activity_or_hidden() {
        let report = r#"[
            {"x": 0, "y": 0, "width": 10, "height": 10, "normal": true, "minimized": false,
             "hidden": false, "onDesktop": true, "onActivity": false},
            {"x": 0, "y": 0, "width": 10, "height": 10, "normal": true, "minimized": false,
             "hidden": true, "onDesktop": true, "onActivity": true}
        ]"#;
        assert!(parse_report(report).unwrap().is_empty());
    }

    #[test]
    fn rounds_fractional_frames_outward() {
        let report = r#"[{"x": -0.5, "y": 10.25, "width": 100.5, "height": 50, "normal": true,
            "minimized": false, "hidden": false, "onDesktop": true, "onActivity": true}]"#;
        assert_eq!(parse_report(report).unwrap(), [rect(-1, 10, 101, 51)]);
    }

    #[test]
    fn calls_back_the_given_service() {
        let source = script_source(":1.42");
        assert!(source.contains(r#"callDBus(":1.42", "/dev/remcostoeten/Kiekje/WindowReport""#));
        assert!(source.contains("workspace.stackingOrder"));
    }
}
