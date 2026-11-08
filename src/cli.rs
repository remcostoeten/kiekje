use std::io;
use std::path::{Path, PathBuf};
use std::slice::Iter;
use std::time::Duration;

use thiserror::Error;

use crate::app::Request;
use crate::capture::mapping::PixelRect;
use crate::capture::{Availability, BackendChoice, CaptureMode};
use crate::storage::ImageFormat;

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
    #[error("text capture is unavailable: {0}")]
    TextUnavailable(&'static str),
    #[error("pin takes the path of one image")]
    PinArguments,
    #[error("could not resolve `{path}`: {source}")]
    Resolve { path: String, source: io::Error },
    #[error("`{0}` is not a file")]
    NotAFile(String),
    #[error("{0} only works with --output")]
    NeedsOutput(&'static str),
    #[error("{0} does not work with --output")]
    NotWithOutput(&'static str),
    #[error("capture {0} does not work with --output; use fullscreen or window")]
    ModeWithOutput(&'static str),
    #[error("capture window --output needs --id; `kiekje windows` lists the ids")]
    MissingWindowId,
    #[error("--id only works with capture window")]
    IdNeedsWindow,
    #[error("--crop takes x,y,width,height in pixels of the capture, not `{0}`")]
    BadCrop(String),
    #[error("--output must end in .png, .jpg or .jpeg, not `{0}`")]
    OutputFormat(String),
}

pub enum CaptureCommand {
    Interactive(Request),
    Headless(Headless),
}

pub struct Headless {
    pub target: HeadlessTarget,
    pub output: PathBuf,
    pub format: ImageFormat,
    pub crop: Option<PixelRect>,
    pub backend: BackendChoice,
    pub delay: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadlessTarget {
    Screen,
    Window(String),
}

/// Reads the arguments after `kiekje capture` into an interactive request, or a headless one
/// when `--output` is given.
///
/// # Examples
///
/// ```ignore
/// let command = parse_capture(&["region".into(), "--delay".into(), "3".into()], 0)?;
/// ```
pub fn parse_capture(arguments: &[String], trigger_us: i64) -> Result<CaptureCommand, CliError> {
    let mut request = Request {
        mode: CaptureMode::Region,
        backend: BackendChoice::Auto,
        save: true,
        edit: false,
        gif: false,
        delay: Duration::ZERO,
        trigger_us,
    };
    let mut output = None;
    let mut id = None;
    let mut crop = None;
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--no-save" => request.save = false,
            "--edit" => request.edit = true,
            "--backend" => {
                let value = value_of(&mut remaining, "--backend")?;
                request.backend = BackendChoice::parse(value)
                    .ok_or_else(|| CliError::UnknownBackend(value.clone()))?;
            }
            "--delay" => request.delay = parse_delay(value_of(&mut remaining, "--delay")?)?,
            "--output" => output = Some(value_of(&mut remaining, "--output")?),
            "--id" => id = Some(value_of(&mut remaining, "--id")?.clone()),
            "--crop" => crop = Some(parse_crop(value_of(&mut remaining, "--crop")?)?),
            value => {
                request.mode = CaptureMode::parse(value)
                    .ok_or_else(|| CliError::UnknownMode(value.to_owned()))?
            }
        }
    }
    match output {
        Some(output) => headless(request, output, id, crop).map(CaptureCommand::Headless),
        None if id.is_some() => Err(CliError::NeedsOutput("--id")),
        None if crop.is_some() => Err(CliError::NeedsOutput("--crop")),
        None => Ok(CaptureCommand::Interactive(request)),
    }
}

fn value_of<'a>(
    remaining: &mut Iter<'a, String>,
    flag: &'static str,
) -> Result<&'a String, CliError> {
    remaining.next().ok_or(CliError::MissingValue(flag))
}

fn headless(
    request: Request,
    output: &str,
    id: Option<String>,
    crop: Option<PixelRect>,
) -> Result<Headless, CliError> {
    if request.edit {
        return Err(CliError::NotWithOutput("--edit"));
    }
    if !request.save {
        return Err(CliError::NotWithOutput("--no-save"));
    }
    let target = match (request.mode, id) {
        (CaptureMode::Fullscreen, None) => HeadlessTarget::Screen,
        (CaptureMode::Window, Some(id)) => HeadlessTarget::Window(id),
        (CaptureMode::Window, None) => return Err(CliError::MissingWindowId),
        (CaptureMode::Fullscreen, Some(_)) => return Err(CliError::IdNeedsWindow),
        (mode, _) => return Err(CliError::ModeWithOutput(mode.as_str())),
    };
    let format = ImageFormat::from_path(Path::new(output))
        .ok_or_else(|| CliError::OutputFormat(output.to_owned()))?;
    let output = std::path::absolute(output).map_err(|source| CliError::Resolve {
        path: output.to_owned(),
        source,
    })?;
    Ok(Headless {
        target,
        output,
        format,
        crop,
        backend: request.backend,
        delay: request.delay,
    })
}

fn parse_crop(value: &str) -> Result<PixelRect, CliError> {
    let numbers: Option<Vec<u32>> = value
        .split(',')
        .map(|part| part.trim().parse().ok())
        .collect();
    match numbers.as_deref() {
        Some(&[x, y, width, height]) if width > 0 && height > 0 => Ok(PixelRect {
            x,
            y,
            width,
            height,
        }),
        _ => Err(CliError::BadCrop(value.to_owned())),
    }
}

/// Refuses a window capture where the session has no window source, and a text capture where
/// Tesseract does not work.
///
/// Only the probe for the requested mode runs.
///
/// # Examples
///
/// ```ignore
/// check_available(CaptureMode::Window, || windows::availability(windows::detect()), ocr::availability)?;
/// ```
pub fn check_available(
    mode: CaptureMode,
    window: impl FnOnce() -> Availability,
    text: impl FnOnce() -> Availability,
) -> Result<(), CliError> {
    match mode {
        CaptureMode::Window => match window() {
            Availability::Available => Ok(()),
            Availability::Unavailable(reason) => Err(CliError::WindowUnavailable(reason)),
        },
        CaptureMode::Text => match text() {
            Availability::Available => Ok(()),
            Availability::Unavailable(reason) => Err(CliError::TextUnavailable(reason)),
        },
        _ => Ok(()),
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

    fn parse_command(arguments: &[&str]) -> Result<CaptureCommand, CliError> {
        let arguments: Vec<String> = arguments
            .iter()
            .map(|argument| (*argument).into())
            .collect();
        parse_capture(&arguments, 0)
    }

    fn parse(arguments: &[&str]) -> Result<Request, CliError> {
        match parse_command(arguments)? {
            CaptureCommand::Interactive(request) => Ok(request),
            CaptureCommand::Headless(_) => panic!("{arguments:?} parsed as a headless capture"),
        }
    }

    fn parse_headless(arguments: &[&str]) -> Result<Headless, CliError> {
        match parse_command(arguments)? {
            CaptureCommand::Headless(request) => Ok(request),
            CaptureCommand::Interactive(_) => panic!("{arguments:?} parsed as interactive"),
        }
    }

    #[test]
    fn captures_a_window_by_id_into_a_file_with_output() {
        let request =
            parse_headless(&["window", "--id", "{b4ac}", "--output", "shot.png"]).unwrap();
        assert_eq!(request.target, HeadlessTarget::Window("{b4ac}".into()));
        assert_eq!(request.format, ImageFormat::Png);
        assert!(request.output.is_absolute());
        assert!(request.output.ends_with("shot.png"));
        assert_eq!(request.crop, None);
    }

    #[test]
    fn captures_every_monitor_with_a_crop_and_reads_the_format_from_the_extension() {
        let request = parse_headless(&[
            "fullscreen",
            "--output",
            "/tmp/detail.JPG",
            "--crop",
            "10, 20,300,200",
            "--delay",
            "2",
        ])
        .unwrap();
        assert_eq!(request.target, HeadlessTarget::Screen);
        assert_eq!(request.format, ImageFormat::Jpeg);
        assert_eq!(
            request.crop,
            Some(PixelRect {
                x: 10,
                y: 20,
                width: 300,
                height: 200
            })
        );
        assert_eq!(request.delay, Duration::from_secs(2));
    }

    #[test]
    fn needs_an_id_for_a_window_and_refuses_it_elsewhere() {
        assert!(matches!(
            parse_command(&["window", "--output", "a.png"]),
            Err(CliError::MissingWindowId)
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--id", "{a}", "--output", "a.png"]),
            Err(CliError::IdNeedsWindow)
        ));
    }

    #[test]
    fn refuses_interactive_modes_and_flags_with_output() {
        assert!(matches!(
            parse_command(&["--output", "a.png"]),
            Err(CliError::ModeWithOutput("region"))
        ));
        assert!(matches!(
            parse_command(&["repeat", "--output", "a.png"]),
            Err(CliError::ModeWithOutput("repeat"))
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--edit", "--output", "a.png"]),
            Err(CliError::NotWithOutput("--edit"))
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--no-save", "--output", "a.png"]),
            Err(CliError::NotWithOutput("--no-save"))
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--output", "a.webp"]),
            Err(CliError::OutputFormat(path)) if path == "a.webp"
        ));
    }

    #[test]
    fn refuses_id_and_crop_without_output() {
        assert!(matches!(
            parse_command(&["window", "--id", "{a}"]),
            Err(CliError::NeedsOutput("--id"))
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--crop", "0,0,1,1"]),
            Err(CliError::NeedsOutput("--crop"))
        ));
        assert!(matches!(
            parse_command(&["fullscreen", "--output"]),
            Err(CliError::MissingValue("--output"))
        ));
    }

    #[test]
    fn rejects_a_crop_that_is_not_four_whole_numbers_with_a_size() {
        for value in [
            "1,2,3",
            "1,2,3,4,5",
            "0,0,0,10",
            "0,0,10,0",
            "-1,0,5,5",
            "a,b,c,d",
        ] {
            assert!(
                matches!(parse_crop(value), Err(CliError::BadCrop(_))),
                "{value}"
            );
        }
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
        let unprobed = || panic!("probed text support for a window capture");
        assert!(matches!(
            check_available(CaptureMode::Window, unavailable, unprobed),
            Err(CliError::WindowUnavailable("no window source"))
        ));
        assert!(check_available(CaptureMode::Window, || Availability::Available, unprobed).is_ok());
    }

    #[test]
    fn refuses_text_capture_without_tesseract() {
        let missing = || Availability::Unavailable("Tesseract is not installed");
        let unprobed = || panic!("probed window support for a text capture");
        assert!(matches!(
            check_available(CaptureMode::Text, unprobed, missing),
            Err(CliError::TextUnavailable("Tesseract is not installed"))
        ));
        assert_eq!(parse(&["text"]).unwrap().mode, CaptureMode::Text);
        assert!(matches!(
            parse_capture(&["text".into(), "--output".into(), "a.png".into()], 0),
            Err(CliError::ModeWithOutput("text"))
        ));
    }

    #[test]
    fn checks_window_support_only_for_window_capture() {
        for mode in [
            CaptureMode::Region,
            CaptureMode::Fullscreen,
            CaptureMode::Repeat,
        ] {
            assert!(
                check_available(
                    mode,
                    || panic!("probed windows for {mode:?}"),
                    || panic!("probed text for {mode:?}")
                )
                .is_ok()
            );
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
