#[macro_use]
mod log;

mod app;
mod backends;
mod capture;
mod cli;
mod clipboard;
mod config;
mod desktop_file;
mod doctor;
mod editor;
mod fuzzy;
mod help;
mod ids;
mod library;
mod mascot;
mod notify;
mod output;
mod overlay;
mod pin;
mod record;
mod shortcuts;
mod sound;
mod storage;
mod style;
mod tray;
mod ui;
mod update;
mod upload;

use gtk::glib;

use app::Command;
use backends::windows;
use cli::{check_available, is_help_request, parse_capture, parse_pin};
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
            Ok(request) => {
                match check_available(request.mode, || windows::availability(windows::detect())) {
                    Ok(()) => app::run(Command::Capture(request)),
                    Err(message) => {
                        warn!("{message}");
                        glib::ExitCode::FAILURE
                    }
                }
            }
            Err(message) => {
                help::print_error(&message.to_string(), Topic::Capture);
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
                help::print_error(&message.to_string(), Topic::Overview);
                glib::ExitCode::FAILURE
            }
        },
        Some("daemon") => app::run(Command::Daemon),
        Some("quit") => app::run(Command::Quit),
        Some("menu") => app::run(Command::Menu),
        Some("settings") => app::run(Command::Settings),
        Some("gallery") => app::run(Command::Gallery),
        Some("record") => app::run(Command::Record),
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
        error!("could not pin the mmap threshold");
    }
}

#[cfg(not(target_env = "gnu"))]
fn pin_mmap_threshold() {}
