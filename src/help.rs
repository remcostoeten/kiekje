use std::io::Write;
use std::time::Duration;

use crate::mascot::{self, Pose};
use crate::style::{Rgb, Style, mix, rgb_code};

const LABEL_WIDTH: usize = 26;
const BINARY: &str = env!("CARGO_PKG_NAME");
const COMMANDS: &[&str] = &[
    "capture",
    "daemon",
    "quit",
    "doctor",
    "install-desktop-file",
    "help",
    "version",
];
const MAX_SUGGESTION_DISTANCE: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Overview,
    Capture,
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
        description: "Start the background instance",
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
    Entry {
        name: "version",
        argument: "",
        description: "Print the version",
        note: "(-V, --version)",
    },
];

const MODES: &[Entry] = &[
    Entry {
        name: "region",
        argument: "",
        description: "Freeze the screen and select an area",
        note: "(default)",
    },
    Entry {
        name: "fullscreen",
        argument: "",
        description: "Freeze the screen and keep all of it",
        note: "",
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

fn row(style: &Style, entry: &Entry, paint_name: fn(&Style, &str) -> String) -> String {
    let (plain, painted) = if entry.argument.is_empty() {
        (entry.name.to_owned(), paint_name(style, entry.name))
    } else {
        (
            format!("{} {}", entry.name, entry.argument),
            format!(
                "{} {}",
                paint_name(style, entry.name),
                style.dim(entry.argument)
            ),
        )
    };
    let padding = " ".repeat(LABEL_WIDTH.saturating_sub(plain.chars().count()));
    let mut line = format!("  {painted}{padding}{}", entry.description);
    if !entry.note.is_empty() {
        line.push(' ');
        line.push_str(&style.dim(entry.note));
    }
    line.push('\n');
    line
}

fn group(style: &Style, entries: &[Entry], paint_name: fn(&Style, &str) -> String) -> String {
    entries
        .iter()
        .map(|entry| row(style, entry, paint_name))
        .collect()
}

const WORDMARK: [&str; 3] = [
    "╦╔═ ╦ ╔═╗ ╦╔═   ╦ ╔═╗",
    "╠╩╗ ║ ║╣  ╠╩╗   ║ ║╣ ",
    "╩ ╩ ╩ ╚═╝ ╩ ╩ ╚═╝ ╚═╝",
];

const ACCENT: Rgb = (254, 81, 1);
const UNLIT: Rgb = (40, 20, 10);
const INDENT: usize = 2;
const MASCOT_GAP: usize = 4;
const CAPTION_ROW: usize = 4;
const GLOW_END: f32 = 0.5;
const HOP_START: f32 = 0.45;
const HOP_END: f32 = 0.68;
const HOP_HEIGHT: usize = 2;
const BLINK_START: f32 = 0.8;
const BLINK_END: f32 = 0.95;
const ANIMATION_FRAMES: u32 = 72;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(4)
}

fn phase(progress: f32, start: f32, end: f32) -> f32 {
    ((progress - start) / (end - start)).clamp(0.0, 1.0)
}

fn mascot_pose(progress: f32) -> Pose {
    let hop = phase(progress, HOP_START, HOP_END);
    let height = HOP_HEIGHT.min(mascot::headroom()) as f32;
    let blink = phase(progress, BLINK_START, BLINK_END);
    Pose {
        lift: (height * (hop * std::f32::consts::PI).sin()).round() as usize,
        glow: progress / GLOW_END,
        lid: 1.0 - (blink * 2.0 - 1.0).abs(),
        unlit: UNLIT,
    }
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

fn caption(style: &Style, progress: f32) -> Vec<String> {
    let wordmark_progress = (progress - 0.25) / 0.75;
    let mut lines: Vec<String> = WORDMARK
        .iter()
        .map(|row| wordmark_row(style, row, wordmark_progress))
        .collect();
    lines.push(String::new());
    lines.push(style.bold("Screenshots for KDE Plasma and Hyprland."));
    lines.push(style.dim(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    lines
}

fn intro_lines(style: &Style, progress: f32) -> Vec<String> {
    let indent = " ".repeat(INDENT);
    let caption = caption(style, progress);
    let mut lines: Vec<String> = if style.color {
        let gap = " ".repeat(MASCOT_GAP);
        mascot::rows(style, mascot_pose(progress))
            .into_iter()
            .enumerate()
            .map(|(index, body)| {
                let text = index
                    .checked_sub(CAPTION_ROW)
                    .and_then(|offset| caption.get(offset))
                    .map_or("", String::as_str);
                format!("{indent}{body}{gap}{text}")
            })
            .collect()
    } else {
        caption
            .iter()
            .map(|line| {
                if line.is_empty() {
                    String::new()
                } else {
                    format!("{indent}{line}")
                }
            })
            .collect()
    };
    lines.push(String::new());
    lines.push(format!(
        "{indent}{}",
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

fn usage(style: &Style, topic: Topic) -> String {
    let (command, arguments) = match topic {
        Topic::Overview => ("<command>", "[...flags]"),
        Topic::Capture => ("capture", "[region|fullscreen] [...flags]"),
    };
    format!(
        "{} {} {} {}\n",
        style.bold("Usage:"),
        style.bold(BINARY),
        style.bold(command),
        style.flag(arguments),
    )
}

fn examples(style: &Style) -> String {
    let mut text = format!("{}\n", style.bold("Examples:"));
    for (summary, command) in EXAMPLES {
        text.push_str(&format!(
            "  {}\n  {} {BINARY} {command}\n\n",
            style.dim(summary),
            style.dim("$"),
        ));
    }
    text
}

fn render(style: &Style, topic: Topic, with_intro: bool) -> String {
    let mut text = String::new();
    if with_intro {
        text.push_str(&intro(style));
    }
    text.push_str(&usage(style, topic));
    text.push('\n');

    match topic {
        Topic::Overview => {
            text.push_str(&format!("{}\n", style.bold("Commands:")));
            text.push_str(&group(style, CAPTURE, Style::accent));
            text.push('\n');
            text.push_str(&group(style, SESSION, Style::bold));
            text.push('\n');
            text.push_str(&group(style, SETUP, Style::bold));
            text.push('\n');
            text.push_str(&format!("{}\n", style.bold("Capture flags:")));
        }
        Topic::Capture => {
            text.push_str(&format!("{}\n", style.bold("Modes:")));
            text.push_str(&group(style, MODES, Style::accent));
            text.push('\n');
            text.push_str(&format!("{}\n", style.bold("Flags:")));
        }
    }
    text.push_str(&group(style, FLAGS, Style::flag));
    text.push('\n');
    text.push_str(&examples(style));
    text.push_str(&format!(
        "Run {} to see which capture backends this session offers.\n",
        style.bold(&format!("{BINARY} doctor")),
    ));
    text
}

fn render_error(style: &Style, message: &str, topic: Topic, hint: Option<&str>) -> String {
    let more = match topic {
        Topic::Overview => format!("{BINARY} help"),
        Topic::Capture => format!("{BINARY} capture --help"),
    };
    let mut text = format!("{} {message}\n", style.error("error:"));
    if let Some(hint) = hint {
        text.push_str(&format!("       {hint}\n"));
    }
    text.push('\n');
    text.push_str(&usage(style, topic));
    text.push_str(&format!("Run {} for details.\n", style.bold(&more)));
    text
}

fn distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current.push(
                substitution
                    .min(previous[column + 1] + 1)
                    .min(current[column] + 1),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

fn suggest(input: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .map(|command| (distance(input, command), *command))
        .filter(|(distance, _)| *distance <= MAX_SUGGESTION_DISTANCE)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, command)| command)
}

/// Prints the overview or the help for one command to stdout.
///
/// The overview animates its intro for about a second on a color terminal; `NO_COLOR` or a
/// pipe prints it static.
///
/// # Examples
///
/// ```ignore
/// help::print(help::Topic::Capture);
/// ```
pub fn print(topic: Topic) {
    let style = Style::stdout();
    if topic != Topic::Overview || !style.color {
        print!("{}", render(&style, topic, topic == Topic::Overview));
        return;
    }
    if animate_intro(&style).is_err() {
        return;
    }
    print!("{}", render(&style, topic, false));
}

/// Prints the binary name and version to stdout.
///
/// # Examples
///
/// ```ignore
/// help::print_version();
/// ```
pub fn print_version() {
    println!("{BINARY} {}", env!("CARGO_PKG_VERSION"));
}

/// Prints an error with the usage line for `topic` to stderr.
///
/// # Examples
///
/// ```ignore
/// help::print_error("unknown mode `box`", help::Topic::Capture);
/// ```
pub fn print_error(message: &str, topic: Topic) {
    eprint!("{}", render_error(&Style::stderr(), message, topic, None));
}

/// Prints an unknown-command error, suggesting the closest command when one is near.
///
/// # Examples
///
/// ```ignore
/// help::print_unknown_command("doctr");
/// ```
pub fn print_unknown_command(input: &str) {
    let hint = suggest(input).map(|command| format!("did you mean `{command}`?"));
    eprint!(
        "{}",
        render_error(
            &Style::stderr(),
            &format!("unknown command `{input}`"),
            Topic::Overview,
            hint.as_deref(),
        )
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Style = Style { color: false };

    #[test]
    fn renders_plain_text_without_escape_codes() {
        let text = render(&PLAIN, Topic::Overview, true);
        assert!(!text.contains('\x1b'));
        assert!(text.contains("Commands:"));
    }

    #[test]
    fn aligns_descriptions_to_one_column() {
        for entry in CAPTURE
            .iter()
            .chain(SESSION)
            .chain(SETUP)
            .chain(MODES)
            .chain(FLAGS)
        {
            let line = row(&PLAIN, entry, Style::bold);
            assert_eq!(line.find(entry.description), Some(2 + LABEL_WIDTH));
        }
    }

    #[test]
    fn keeps_capture_help_to_modes_and_flags() {
        let text = render(&PLAIN, Topic::Capture, false);
        assert!(text.contains("Modes:"));
        assert!(text.contains("--backend"));
        assert!(!text.contains("install-desktop-file"));
    }

    #[test]
    fn suggests_the_nearest_command_for_a_typo() {
        assert_eq!(suggest("doctr"), Some("doctor"));
        assert_eq!(suggest("captrue"), Some("capture"));
        assert_eq!(suggest("screenshot"), None);
    }

    #[test]
    fn prints_a_short_error_instead_of_the_full_overview() {
        let text = render_error(&PLAIN, "unknown command `x`", Topic::Overview, None);
        assert!(text.starts_with("error: unknown command `x`\n"));
        assert!(!text.contains("Commands:"));
    }

    #[test]
    fn fits_the_caption_beside_the_mascot() {
        assert!(CAPTION_ROW + caption(&PLAIN, 1.0).len() <= mascot::ROWS);
    }

    #[test]
    fn rests_the_mascot_with_open_eyes() {
        let pose = mascot_pose(1.0);
        assert_eq!((pose.lift, pose.lid), (0, 0.0));
        assert!(pose.glow >= 1.0);
    }

    #[test]
    fn hops_without_clipping_the_mascot() {
        let highest = (0..=ANIMATION_FRAMES)
            .map(|frame| mascot_pose(frame as f32 / ANIMATION_FRAMES as f32).lift)
            .max();
        assert_eq!(highest, Some(HOP_HEIGHT));
        assert!(HOP_HEIGHT <= mascot::headroom());
    }

    #[test]
    fn keeps_every_intro_frame_the_same_height() {
        let height = intro_lines(&PLAIN, 1.0).len();
        assert!((0..=ANIMATION_FRAMES).all(|frame| {
            intro_lines(&PLAIN, frame as f32 / ANIMATION_FRAMES as f32).len() == height
        }));
    }
}
