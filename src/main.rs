mod app;
mod capture;
mod clipboard;
mod diagnostics;
mod editor;
mod image;
mod platform;
mod settings;
mod storage;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use settings::config::{CaptureMode, Settings};
use std::io::{self, Write};

#[derive(Debug, Clone, ValueEnum)]
enum CliMode {
    Region,
    Fullscreen,
    Window,
}

#[derive(Debug, Parser)]
#[command(name = "capture-app", version, about = "Wayland-first screenshot utility")]
struct Cli {
    #[arg(value_enum)]
    mode: Option<CliMode>,
    #[arg(short, long, help = "Open interactive terminal menu")]
    interactive: bool,
    #[arg(long, help = "Check system dependencies and print readiness report")]
    doctor: bool,
}

fn main() {
    if let Err(err) = run() {
        render_error(&err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut settings = Settings::load_or_default()?;

    if cli.doctor {
        println!("{}", diagnostics::doctor_report());
        return Ok(());
    }

    if cli.interactive {
        run_interactive_menu(&mut settings)?;
        settings.save()?;
        return Ok(());
    }

    let mode = match cli.mode {
        Some(CliMode::Region) => CaptureMode::Region,
        Some(CliMode::Fullscreen) => CaptureMode::Fullscreen,
        Some(CliMode::Window) => CaptureMode::Window,
        None => settings.default_capture_mode,
    };

    run_capture(mode, &settings)?;

    settings.save()?;
    Ok(())
}

fn run_capture(mode: CaptureMode, settings: &Settings) -> Result<()> {
    diagnostics::check_capture_requirements(mode, settings)?;

    if settings.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(settings.delay_ms));
    }

    let capture = capture::capture(mode)?;
    image::processing::validate_png(&capture.png_data)?;

    if settings.copy_to_clipboard {
        clipboard::copy_png(&capture.png_data)?;
    }

    if settings.auto_save {
        let path = storage::save::save_capture(&capture.png_data, settings, mode)?;
        eprintln!("Saved: {}", path.display());
    }

    if settings.open_editor {
        app::run_editor(capture, settings.clone())?;
    }

    Ok(())
}

fn run_interactive_menu(settings: &mut Settings) -> Result<()> {
    loop {
        println!();
        println!("capture-app interactive");
        println!("1) Capture region");
        println!("2) Capture fullscreen");
        println!("3) Capture window (active Hyprland window)");
        println!("4) Toggle clipboard copy    [{}]", on_off(settings.copy_to_clipboard));
        println!("5) Toggle open editor       [{}]", on_off(settings.open_editor));
        println!("6) Toggle auto-save         [{}]", on_off(settings.auto_save));
        println!("7) Set delay (ms)           [{}]", settings.delay_ms);
        println!(
            "8) Set default mode         [{}]",
            capture_mode_label(settings.default_capture_mode)
        );
        println!("9) Show current settings");
        println!("0) Save and exit");
        print!("Select option: ");
        io::stdout().flush()?;

        let choice = read_line_trimmed()?;
        match choice.as_str() {
            "1" | "r" | "region" => run_capture(CaptureMode::Region, settings)?,
            "2" | "f" | "fullscreen" => run_capture(CaptureMode::Fullscreen, settings)?,
            "3" | "w" | "window" => run_capture(CaptureMode::Window, settings)?,
            "4" => {
                settings.copy_to_clipboard = !settings.copy_to_clipboard;
                println!("copy_to_clipboard = {}", settings.copy_to_clipboard);
            }
            "5" => {
                settings.open_editor = !settings.open_editor;
                println!("open_editor = {}", settings.open_editor);
            }
            "6" => {
                settings.auto_save = !settings.auto_save;
                println!("auto_save = {}", settings.auto_save);
            }
            "7" => {
                print!("New delay in ms: ");
                io::stdout().flush()?;
                let input = read_line_trimmed()?;
                match input.parse::<u64>() {
                    Ok(delay) => {
                        settings.delay_ms = delay;
                        println!("delay_ms = {}", settings.delay_ms);
                    }
                    Err(_) => println!("Invalid number: {}", input),
                }
            }
            "8" => {
                println!("Choose default mode: 1=region, 2=fullscreen, 3=window");
                print!("Mode: ");
                io::stdout().flush()?;
                let mode = read_line_trimmed()?;
                settings.default_capture_mode = match mode.as_str() {
                    "1" | "region" => CaptureMode::Region,
                    "2" | "fullscreen" => CaptureMode::Fullscreen,
                    "3" | "window" => CaptureMode::Window,
                    _ => {
                        println!("Invalid mode: {}", mode);
                        settings.default_capture_mode
                    }
                };
                println!(
                    "default_capture_mode = {}",
                    capture_mode_label(settings.default_capture_mode)
                );
            }
            "9" => print_settings(settings),
            "0" | "q" | "quit" | "exit" => {
                println!("Saving config and exiting.");
                return Ok(());
            }
            _ => println!("Unknown option: {}", choice),
        }
    }
}

fn read_line_trimmed() -> Result<String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn on_off(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

fn capture_mode_label(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Region => "region",
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::Window => "window",
    }
}

fn print_settings(settings: &Settings) {
    println!("delay_ms: {}", settings.delay_ms);
    println!("default_save_location: {}", settings.default_save_location.display());
    println!("copy_to_clipboard: {}", settings.copy_to_clipboard);
    println!("open_editor: {}", settings.open_editor);
    println!(
        "default_capture_mode: {}",
        capture_mode_label(settings.default_capture_mode)
    );
    println!("auto_save: {}", settings.auto_save);
    println!("filename_template: {}", settings.filename_template);
}

fn render_error(err: &anyhow::Error) {
    eprintln!("Screeny Error");
    eprintln!("============");

    if let Some(missing) = err.downcast_ref::<diagnostics::MissingDependenciesError>() {
        eprintln!("Code: SCREENY-E001");
        eprintln!("Missing required dependencies:");
        for item in &missing.items {
            eprintln!("  - {} ({})", item.tool, item.required_for);
            if let Some(cmd) = &item.install_command {
                eprintln!("    Install: {}", cmd);
            }
            if let Some(workaround) = &item.workaround {
                eprintln!("    Option:  {}", workaround);
            }
        }
        eprintln!();
        eprintln!("Alternative: run `capture-app --doctor` for full environment diagnostics.");
        return;
    }

    eprintln!("Code: SCREENY-E999");
    eprintln!("{err:#}");
}
