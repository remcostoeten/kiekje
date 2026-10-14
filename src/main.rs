mod app;
mod backends;
mod capture;
mod clipboard;
mod config;
mod desktop_file;
mod doctor;
mod editor;
mod help;
mod library;
mod mascot;
mod notify;
mod output;
mod overlay;
mod pin;
mod shortcuts;
mod sound;
mod storage;
mod style;
mod tray;
mod ui;
mod update;

use std::path::PathBuf;
use std::time::Duration;

use gtk::glib;

use app::{Command, Request};
use capture::{BackendChoice, CaptureMode};
use help::Topic;

fn main() -> glib::ExitCode {
    let trigger_us = glib::monotonic_time();
    pin_mmap_threshold();
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    match arguments.first().map(String::as_str) {
        Some("capture") if is_help_request(&arguments[1..]) => {
            help::print(Topic::Capture);
            glib::ExitCode::SUCCESS
        }
        Some("capture") => match parse_capture(&arguments[1..], trigger_us) {
            Ok(request) => app::run(Command::Capture(request)),
            Err(message) => {
                help::print_error(&message, Topic::Capture);
                glib::ExitCode::FAILURE
            }
        },
        Some("pin") if is_help_request(&arguments[1..]) => {
            help::print(Topic::Overview);
            glib::ExitCode::SUCCESS
        }
        Some("pin") => match parse_pin(&arguments[1..]) {
            Ok(path) => app::run(Command::Pin(path)),
            Err(message) => {
                help::print_error(&message, Topic::Overview);
                glib::ExitCode::FAILURE
            }
        },
        Some("daemon") => app::run(Command::Daemon),
        Some("quit") => app::run(Command::Quit),
        Some("menu") => app::run(Command::Menu),
        Some("settings") => app::run(Command::Settings),
        Some("gallery") => app::run(Command::Gallery),
        Some("doctor") => {
            doctor::run();
            glib::ExitCode::SUCCESS
        }
        Some("install-desktop-file") => match desktop_file::install() {
            Ok(path) => {
                println!("kiekje: wrote {}", path.display());
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("kiekje: could not write the desktop file: {error}");
                glib::ExitCode::FAILURE
            }
        },
        Some("help" | "-h" | "--help") | None => {
            help::print(Topic::Overview);
            glib::ExitCode::SUCCESS
        }
        Some("version" | "-V" | "--version") => {
            help::print_version();
            glib::ExitCode::SUCCESS
        }
        Some(other) => {
            help::print_unknown_command(other);
            glib::ExitCode::FAILURE
        }
    }
}

// glibc raises the mmap threshold after a large free, so later frame buffers land in the heap and stay resident.
#[cfg(target_env = "gnu")]
fn pin_mmap_threshold() {
    const THRESHOLD: libc::c_int = 128 * 1024;
    if unsafe { libc::mallopt(libc::M_MMAP_THRESHOLD, THRESHOLD) } == 0 {
        eprintln!("kiekje: could not pin the mmap threshold");
    }
}

#[cfg(not(target_env = "gnu"))]
fn pin_mmap_threshold() {}

const MAX_DELAY_SECONDS: u64 = 60;

fn parse_capture(arguments: &[String], trigger_us: i64) -> Result<Request, String> {
    let mut request = Request {
        mode: CaptureMode::Region,
        backend: BackendChoice::Auto,
        save: true,
        edit: false,
        delay: Duration::ZERO,
        trigger_us,
    };
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--no-save" => request.save = false,
            "--edit" => request.edit = true,
            "--backend" => {
                let value = remaining.next().ok_or("--backend needs a value")?;
                request.backend =
                    BackendChoice::parse(value).ok_or(format!("unknown backend `{value}`"))?;
            }
            "--delay" => {
                let value = remaining.next().ok_or("--delay needs a value")?;
                request.delay = parse_delay(value)?;
            }
            value => {
                request.mode = CaptureMode::parse(value).ok_or(format!("unknown mode `{value}`"))?
            }
        }
    }
    Ok(request)
}

fn parse_delay(value: &str) -> Result<Duration, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds <= MAX_DELAY_SECONDS)
        .map(Duration::from_secs)
        .ok_or(format!(
            "--delay takes whole seconds from 0 to {MAX_DELAY_SECONDS}, not `{value}`"
        ))
}

fn parse_pin(arguments: &[String]) -> Result<PathBuf, String> {
    let [path] = arguments else {
        return Err("pin takes the path of one image".into());
    };
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("could not resolve `{path}`: {error}"))?;
    if !absolute.is_file() {
        return Err(format!("`{path}` is not a file"));
    }
    Ok(absolute)
}

fn is_help_request(arguments: &[String]) -> bool {
    arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Request, String> {
        let arguments: Vec<String> = arguments
            .iter()
            .map(|argument| (*argument).into())
            .collect();
        parse_capture(&arguments, 0)
    }

    #[test]
    fn captures_a_region_now_by_default() {
        let request = parse(&[]).unwrap();
        assert_eq!(request.mode, CaptureMode::Region);
        assert_eq!(request.delay, Duration::ZERO);
        assert!(request.save);
    }

    #[test]
    fn parses_a_delayed_repeat_of_the_last_region() {
        let request = parse(&["repeat", "--delay", "5", "--no-save"]).unwrap();
        assert_eq!(request.mode, CaptureMode::Repeat);
        assert_eq!(request.delay, Duration::from_secs(5));
        assert!(!request.save);
    }

    #[test]
    fn asks_for_the_editor_with_edit() {
        assert!(parse(&["--edit"]).unwrap().edit);
        assert!(!parse(&[]).unwrap().edit);
    }

    #[test]
    fn pins_one_existing_file_by_its_absolute_path() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = parse_pin(&[format!("{manifest}/Cargo.toml")]).unwrap();
        assert!(path.is_absolute());
        assert!(parse_pin(&[]).is_err());
        assert!(parse_pin(&[format!("{manifest}/missing.png")]).is_err());
        assert!(parse_pin(&["a.png".into(), "b.png".into()]).is_err());
    }

    #[test]
    fn rejects_a_delay_that_is_not_whole_seconds_in_range() {
        for value in ["-1", "1.5", "61", "soon"] {
            assert!(parse(&["--delay", value]).is_err(), "{value}");
        }
        assert_eq!(
            parse(&["--delay"]).err().as_deref(),
            Some("--delay needs a value")
        );
    }
}
