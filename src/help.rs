use std::io::{IsTerminal, Write};
use std::time::Duration;

const NAME_WIDTH: usize = 12;
const LABEL_WIDTH: usize = 26;

struct Style {
    color: bool,
}

struct Entry {
    name: &'static str,
    argument: &'static str,
    description: &'static str,
    note: &'static str,
}

const CAPTURE: &[Entry] = &[
    Entry {
        name: "capture",
        argument: "region",
        description: "Freeze the screen and select an area",
        note: "(default)",
    },
    Entry {
        name: "capture",
        argument: "fullscreen",
        description: "Freeze the screen and keep all of it",
        note: "",
    },
];

const SESSION: &[Entry] = &[
    Entry {
        name: "daemon",
        argument: "",
        description: "Start the background instance and tray",
        note: "",
    },
    Entry {
        name: "quit",
        argument: "",
        description: "Stop the running instance",
        note: "",
    },
];

const SETUP: &[Entry] = &[
    Entry {
        name: "doctor",
        argument: "",
        description: "Report what this session supports",
        note: "",
    },
    Entry {
        name: "install-desktop-file",
        argument: "",
        description: "Register the launcher entry",
        note: "",
    },
    Entry {
        name: "help",
        argument: "",
        description: "Show this overview",
        note: "(-h, --help)",
    },
];

const FLAGS: &[Entry] = &[
    Entry {
        name: "--backend",
        argument: "<name>",
        description: "Force auto, kwin, wlroots or portal",
        note: "(default: auto)",
    },
    Entry {
        name: "--no-save",
        argument: "",
        description: "Copy to the clipboard without writing a file",
        note: "",
    },
];

const EXAMPLES: &[(&str, &str)] = &[
    ("Select a region, copy it and save it", "capture"),
    (
        "Grab every monitor through KWin, clipboard only",
        "capture fullscreen --backend kwin --no-save",
    ),
];

impl Style {
    fn detect(terminal: bool) -> Self {
        let disabled = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        Self {
            color: terminal && !disabled,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    fn accent(&self, text: &str) -> String {
        self.paint("1;38;2;254;81;1", text)
    }

    fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    fn flag(&self, text: &str) -> String {
        self.paint("1;36", text)
    }

    fn error(&self, text: &str) -> String {
        self.paint("1;31", text)
    }

    fn row(&self, entry: &Entry, paint_name: fn(&Self, &str) -> String) -> String {
        let name_padding = if entry.argument.is_empty() {
            String::new()
        } else {
            pad(entry.name.len(), NAME_WIDTH)
        };
        let label_len = entry.name.len() + name_padding.len() + entry.argument.len();
        let mut line = format!(
            "  {}{name_padding}{}{}{}",
            paint_name(self, entry.name),
            self.dim(entry.argument),
            pad(label_len, LABEL_WIDTH),
            entry.description,
        );
        if !entry.note.is_empty() {
            line.push(' ');
            line.push_str(&self.dim(entry.note));
        }
        line.push('\n');
        line
    }

    fn group(&self, entries: &[Entry], paint_name: fn(&Self, &str) -> String) -> String {
        entries
            .iter()
            .map(|entry| self.row(entry, paint_name))
            .collect()
    }
}

fn pad(len: usize, width: usize) -> String {
    " ".repeat(width.saturating_sub(len))
}

type Rgb = (u8, u8, u8);

const WORDMARK: [&str; 3] = [
    "╦╔═ ╦ ╔═╗ ╦╔═   ╦ ╔═╗",
    "╠╩╗ ║ ║╣  ╠╩╗   ║ ║╣ ",
    "╩ ╩ ╩ ╚═╝ ╩ ╩ ╚═╝ ╚═╝",
];

const ACCENT: Rgb = (254, 81, 1);
const EMBER: Rgb = (110, 50, 20);
const UNLIT: Rgb = (40, 20, 10);
const FRAME_INNER: usize = 32;
const MAX_SPREAD: usize = 4;
const MIN_ARM: usize = 2;
const WORDMARK_INSET: usize = 6;
const ANIMATION_FRAMES: u32 = 54;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    (
        channel(from.0, to.0),
        channel(from.1, to.1),
        channel(from.2, to.2),
    )
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(4)
}

fn rgb_code(color: Rgb, bold: bool) -> String {
    let weight = if bold { "1;" } else { "" };
    format!("{weight}38;2;{};{};{}", color.0, color.1, color.2)
}

fn viewfinder_edge(
    style: &Style,
    left: char,
    right: char,
    spread: usize,
    arm: usize,
    color: Rgb,
) -> String {
    let width = FRAME_INNER + 2 * spread;
    let arm = arm.min(width / 2);
    let segment = format!(
        "{left}{}{}{}{right}",
        "─".repeat(arm),
        " ".repeat(width - 2 * arm),
        "─".repeat(arm),
    );
    style.paint(&rgb_code(color, false), &segment)
}

fn wordmark_row(style: &Style, row: &str, progress: f32) -> String {
    let width = row.chars().count() as f32;
    row.chars()
        .enumerate()
        .map(|(column, character)| {
            if character == ' ' {
                return String::from(" ");
            }
            let local = (progress * 1.4 - column as f32 / width * 0.4).clamp(0.0, 1.0);
            let color = mix(UNLIT, ACCENT, ease_out(local));
            style.paint(&rgb_code(color, true), &character.to_string())
        })
        .collect()
}

fn intro_lines(style: &Style, progress: f32) -> Vec<String> {
    let eased = ease_out(progress);
    let spread = (MAX_SPREAD as f32 * (1.0 - eased)).round() as usize;
    let full_arm = (FRAME_INNER + 2 * spread) / 2;
    let arm = (MIN_ARM as f32 + (full_arm - MIN_ARM) as f32 * (1.0 - eased)).round() as usize;
    let frame_color = mix(EMBER, ACCENT, eased);
    let indent = " ".repeat(2 + MAX_SPREAD - spread);
    let side = style.paint(&rgb_code(frame_color, false), "│");
    let side_row = format!(
        "{indent}{side}{}{side}",
        " ".repeat(FRAME_INNER + 2 * spread)
    );
    let wordmark_indent = " ".repeat(2 + MAX_SPREAD + 1 + WORDMARK_INSET);
    let wordmark_progress = (progress - 0.3) / 0.7;

    let mut lines = vec![
        format!(
            "{indent}{}",
            viewfinder_edge(style, '┌', '┐', spread, arm, frame_color)
        ),
        side_row.clone(),
    ];
    lines.extend(WORDMARK.iter().map(|row| {
        format!(
            "{wordmark_indent}{}",
            wordmark_row(style, row, wordmark_progress)
        )
    }));
    lines.push(side_row);
    lines.push(format!(
        "{indent}{}",
        viewfinder_edge(style, '└', '┘', spread, arm, frame_color)
    ));
    lines.push(String::new());
    lines.push(format!(
        "  {}  {}",
        style.bold("Screenshots for KDE Plasma and Hyprland."),
        style.dim(&format!("v{}", env!("CARGO_PKG_VERSION"))),
    ));
    lines.push(format!(
        "  {}",
        style.dim("The frame freezes when you press the shortcut, then you pick what to keep.")
    ));
    lines.push(String::new());
    lines
}

fn intro(style: &Style) -> String {
    let mut text = String::from("\n");
    for line in intro_lines(style, 1.0) {
        text.push_str(&line);
        text.push('\n');
    }
    text
}

fn animate_intro(style: &Style) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out)?;
    for frame in 0..=ANIMATION_FRAMES {
        let lines = intro_lines(style, frame as f32 / ANIMATION_FRAMES as f32);
        if frame > 0 {
            write!(out, "\x1b[{}A", lines.len())?;
        }
        for line in &lines {
            writeln!(out, "{line}\x1b[K")?;
        }
        out.flush()?;
        if frame < ANIMATION_FRAMES {
            std::thread::sleep(FRAME_INTERVAL);
        }
    }
    Ok(())
}

fn render(style: &Style, with_intro: bool) -> String {
    let binary = env!("CARGO_PKG_NAME");
    let mut text = String::new();

    if with_intro {
        text.push_str(&intro(style));
    }

    text.push_str(&format!(
        "{} {} {} {}\n\n",
        style.bold("Usage:"),
        style.bold(binary),
        style.bold("<command>"),
        style.flag("[...flags]"),
    ));

    text.push_str(&format!("{}\n", style.bold("Commands:")));
    text.push_str(&style.group(CAPTURE, Style::accent));
    text.push('\n');
    text.push_str(&style.group(SESSION, Style::bold));
    text.push('\n');
    text.push_str(&style.group(SETUP, Style::bold));
    text.push('\n');

    text.push_str(&format!("{}\n", style.bold("Capture flags:")));
    text.push_str(&style.group(FLAGS, Style::flag));
    text.push('\n');

    text.push_str(&format!("{}\n", style.bold("Examples:")));
    for (summary, command) in EXAMPLES {
        text.push_str(&format!(
            "  {}\n  {} {binary} {command}\n\n",
            style.dim(summary),
            style.dim("$"),
        ));
    }

    text.push_str(&format!(
        "Run {} to see which capture backends this session offers.\n",
        style.bold(&format!("{binary} doctor")),
    ));

    text
}

/// Prints the command overview to stdout.
///
/// On a color terminal the intro animates for about a second; `NO_COLOR` or a pipe prints it static.
///
/// # Examples
///
/// ```ignore
/// help::print();
/// ```
pub fn print() {
    let style = Style::detect(std::io::stdout().is_terminal());
    if !style.color {
        print!("{}", render(&style, true));
        return;
    }
    if animate_intro(&style).is_err() {
        return;
    }
    print!("{}", render(&style, false));
}

/// Prints an error followed by the command overview to stderr.
///
/// # Examples
///
/// ```ignore
/// help::print_error("missing command");
/// ```
pub fn print_error(message: &str) {
    let style = Style::detect(std::io::stderr().is_terminal());
    eprint!(
        "{} {message}\n\n{}",
        style.error("error:"),
        render(&style, false)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_plain_text_without_escape_codes() {
        let text = render(&Style { color: false }, true);
        assert!(!text.contains('\x1b'));
        assert!(text.contains("Commands:"));
    }

    #[test]
    fn aligns_descriptions_to_one_column() {
        let style = Style { color: false };
        for entry in CAPTURE.iter().chain(SESSION).chain(SETUP).chain(FLAGS) {
            let row = style.row(entry, Style::bold);
            assert_eq!(row.find(entry.description), Some(2 + LABEL_WIDTH));
        }
    }

    #[test]
    fn settles_the_viewfinder_on_corner_marks() {
        let lines = intro_lines(&Style { color: false }, 1.0);
        let indent = " ".repeat(2 + MAX_SPREAD);
        let gap = " ".repeat(FRAME_INNER - 2 * MIN_ARM);
        assert_eq!(lines[0], format!("{indent}┌──{gap}──┐"));
        assert_eq!(lines[6], format!("{indent}└──{gap}──┘"));
    }

    #[test]
    fn keeps_every_intro_frame_the_same_height() {
        let style = Style { color: false };
        let height = intro_lines(&style, 1.0).len();
        assert!((0..=ANIMATION_FRAMES).all(|frame| {
            intro_lines(&style, frame as f32 / ANIMATION_FRAMES as f32).len() == height
        }));
    }
}
