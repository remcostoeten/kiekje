mod app;
mod backends;
mod capture;
mod clipboard;
mod desktop_file;
mod doctor;
mod help;
mod mascot;
mod output;
mod overlay;
mod shortcuts;
mod style;
mod tray;

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
        Some("daemon") => app::run(Command::Daemon),
        Some("quit") => app::run(Command::Quit),
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

fn parse_capture(arguments: &[String], trigger_us: i64) -> Result<Request, String> {
    let mut request = Request {
        mode: CaptureMode::Region,
        backend: BackendChoice::Auto,
        save: true,
        trigger_us,
    };
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        match argument.as_str() {
            "--no-save" => request.save = false,
            "--backend" => {
                let value = remaining.next().ok_or("--backend needs a value")?;
                request.backend =
                    BackendChoice::parse(value).ok_or(format!("unknown backend `{value}`"))?;
            }
            value => {
                request.mode = CaptureMode::parse(value).ok_or(format!("unknown mode `{value}`"))?
            }
        }
    }
    Ok(request)
}

fn is_help_request(arguments: &[String]) -> bool {
    arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
}
