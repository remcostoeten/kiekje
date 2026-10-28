use std::io;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::app::Request;
use crate::capture::{Availability, BackendChoice, CaptureMode};

pub const MAX_DELAY_SECONDS: u64 = 60;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0} needs a value")]
    MissingValue(&'static str),
    #[error("unknown backend `{0}`")]
    UnknownBackend(String),
    #[error("unknown mode `{0}`")]
    UnknownMode(String),
    #[error("--delay takes whole seconds from 0 to {MAX_DELAY_SECONDS}, not `{0}`")]
    BadDelay(String),
    #[error("window capture is unavailable: {0}")]
    WindowUnavailable(&'static str),
    #[error("pin takes the path of one image")]
    PinArguments,
    #[error("could not resolve `{path}`: {source}")]
    Resolve { path: String, source: io::Error },
    #[error("`{0}` is not a file")]
    NotAFile(String),
}

/// Reads the arguments after `kiekje capture` into a request.
///
/// # Examples
///
/// ```ignore
/// let request = parse_capture(&["region".into(), "--delay".into(), "3".into()], 0)?;
/// ```
pub fn parse_capture(arguments: &[String], trigger_us: i64) -> Result<Request, CliError> {
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
                let value = remaining
                    .next()
                    .ok_or(CliError::MissingValue("--backend"))?;
                request.backend = BackendChoice::parse(value)
                    .ok_or_else(|| CliError::UnknownBackend(value.clone()))?;
            }
            "--delay" => {
                let value = remaining.next().ok_or(CliError::MissingValue("--delay"))?;
                request.delay = parse_delay(value)?;
            }
            value => {
                request.mode = CaptureMode::parse(value)
                    .ok_or_else(|| CliError::UnknownMode(value.to_owned()))?
            }
        }
    }
    Ok(request)
}

/// Refuses a window capture where the session has no window source.
///
/// # Examples
///
/// ```ignore
/// check_available(CaptureMode::Window, || windows::availability(windows::detect()))?;
/// ```
pub fn check_available(
    mode: CaptureMode,
    window: impl FnOnce() -> Availability,
) -> Result<(), CliError> {
    if mode != CaptureMode::Window {
        return Ok(());
    }
    match window() {
        Availability::Available => Ok(()),
        Availability::Unavailable(reason) => Err(CliError::WindowUnavailable(reason)),
    }
}

fn parse_delay(value: &str) -> Result<Duration, CliError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|seconds| *seconds <= MAX_DELAY_SECONDS)
        .map(Duration::from_secs)
        .ok_or_else(|| CliError::BadDelay(value.to_owned()))
}

/// Resolves the one image path given to `kiekje pin`.
///
/// # Examples
///
/// ```ignore
/// let path = parse_pin(&["shot.png".into()])?;
/// ```
pub fn parse_pin(arguments: &[String]) -> Result<PathBuf, CliError> {
    let [path] = arguments else {
        return Err(CliError::PinArguments);
    };
    let absolute = std::path::absolute(path).map_err(|source| CliError::Resolve {
        path: path.clone(),
        source,
    })?;
    if !absolute.is_file() {
        return Err(CliError::NotAFile(path.clone()));
    }
    Ok(absolute)
}

pub fn is_help_request(arguments: &[String]) -> bool {
    arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Request, CliError> {
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
    fn parses_window_capture_with_its_flags() {
        let request = parse(&["window", "--edit", "--no-save"]).unwrap();
        assert_eq!(request.mode, CaptureMode::Window);
        assert!(request.edit);
        assert!(!request.save);
        assert_eq!(
            parse(&["--delay", "2", "window"]).unwrap().mode,
            CaptureMode::Window
        );
        assert!(matches!(
            parse(&["windows"]),
            Err(CliError::UnknownMode(mode)) if mode == "windows"
        ));
    }

    #[test]
    fn refuses_window_capture_where_the_session_reports_no_windows() {
        let unavailable = || Availability::Unavailable("no window source");
        assert!(matches!(
            check_available(CaptureMode::Window, unavailable),
            Err(CliError::WindowUnavailable("no window source"))
        ));
        assert!(check_available(CaptureMode::Window, || Availability::Available).is_ok());
    }

    #[test]
    fn checks_window_support_only_for_window_capture() {
        for mode in [
            CaptureMode::Region,
            CaptureMode::Fullscreen,
            CaptureMode::Repeat,
        ] {
            assert!(check_available(mode, || panic!("probed for {mode:?}")).is_ok());
        }
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
        assert!(matches!(
            parse(&["--delay"]),
            Err(CliError::MissingValue("--delay"))
        ));
    }
}
