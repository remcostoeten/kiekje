use gtk::prelude::*;
use gtk::{gio, glib};

use crate::ui::widgets::{group, note, problem, row};

const FIELD_LABEL: usize = 18;
const BACKEND_NAME: usize = 24;
const BACKEND_ROLE: usize = 12;
const HEADLINE: &str = "No problems found.";
const PROBLEMS: &str = "Problems";

#[derive(Debug, PartialEq)]
enum Severity {
    Warning,
    Error,
}

#[derive(Debug, PartialEq)]
enum Entry {
    Field {
        label: String,
        value: String,
    },
    Backend {
        name: String,
        detail: String,
        available: bool,
    },
    Note(String),
    Problem {
        severity: Severity,
        message: String,
        fix: Option<String>,
    },
}

#[derive(Debug, PartialEq)]
struct Section {
    title: String,
    entries: Vec<Entry>,
}

#[derive(Debug, Default, PartialEq)]
struct Checks {
    sections: Vec<Section>,
    all_clear: bool,
}

pub(super) fn append(page: &gtk::Box) {
    let controls = group(page, "Checks");
    let run = gtk::Button::with_label("Run again");
    controls.add(row(
        "Session report",
        Some("What this session supports and what to fix, the same checks as kiekje doctor."),
        Some(&run),
    ));
    let failed = problem();
    controls.footer(&failed);
    let results = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    page.append(&results);

    let rerun = (run.clone(), failed, results);
    run.connect_clicked({
        let rerun = rerun.clone();
        move |_| start(&rerun.0, &rerun.1, &rerun.2)
    });
    start(&rerun.0, &rerun.1, &rerun.2);
}

pub(super) async fn report() -> Result<String, glib::Error> {
    let binary = std::env::current_exe()
        .map_err(|error| glib::Error::new(gio::IOErrorEnum::NotFound, &error.to_string()))?;
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE,
    );
    launcher.setenv("NO_COLOR", "1", true);
    let process = launcher.spawn(&[binary.as_os_str(), "doctor".as_ref()])?;
    let (stdout, _) = process.communicate_utf8_future(None).await?;
    Ok(stdout.map(|text| text.to_string()).unwrap_or_default())
}

fn start(run: &gtk::Button, failed: &gtk::Label, results: &gtk::Box) {
    run.set_sensitive(false);
    failed.set_visible(false);
    let run = run.clone();
    let failed = failed.clone();
    let results = results.clone();
    glib::spawn_future_local(async move {
        match report().await {
            Ok(text) => show(&results, &parse(&text)),
            Err(error) => {
                failed.set_label(&format!("The doctor report failed: {error}"));
                failed.set_visible(true);
            }
        }
        run.set_sensitive(true);
    });
}

fn show(results: &gtk::Box, checks: &Checks) {
    while let Some(child) = results.first_child() {
        results.remove(&child);
    }
    if checks.all_clear {
        results.append(&note(HEADLINE));
    }
    for section in &checks.sections {
        let card = group(results, &section.title);
        for entry in &section.entries {
            match entry {
                Entry::Field { label, value } => {
                    let value = gtk::Label::builder()
                        .label(value)
                        .xalign(1.0)
                        .ellipsize(gtk::pango::EllipsizeMode::Middle)
                        .max_width_chars(48)
                        .tooltip_text(value)
                        .css_classes(["value"])
                        .build();
                    card.add(row(label, None, Some(&value)));
                }
                Entry::Backend {
                    name,
                    detail,
                    available,
                } => {
                    let state = if *available {
                        "Available"
                    } else {
                        "Unavailable"
                    };
                    let state = gtk::Label::builder()
                        .label(state)
                        .css_classes(["value"])
                        .build();
                    card.add(row(name, Some(detail), Some(&state)));
                }
                Entry::Note(text) => card.footer(&note(text)),
                Entry::Problem {
                    severity,
                    message,
                    fix,
                } => {
                    let tag = gtk::Label::builder()
                        .label(match severity {
                            Severity::Warning => "Warning",
                            Severity::Error => "Error",
                        })
                        .css_classes(["value"])
                        .build();
                    card.add(row(message, fix.as_deref(), Some(&tag)));
                }
            }
        }
    }
}

fn parse(text: &str) -> Checks {
    let mut checks = Checks::default();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') {
            if line.trim() == HEADLINE {
                checks.all_clear = true;
            } else {
                checks.sections.push(Section {
                    title: line.trim().to_owned(),
                    entries: Vec::new(),
                });
            }
            continue;
        }
        let Some(section) = checks.sections.last_mut() else {
            continue;
        };
        if section.title == PROBLEMS {
            push_problem(&mut section.entries, line);
        } else {
            section.entries.push(entry(line));
        }
    }
    checks
        .sections
        .sort_by_key(|section| section.title != PROBLEMS);
    checks
}

fn push_problem(entries: &mut Vec<Entry>, line: &str) {
    let trimmed = line.trim();
    let severity = if trimmed.starts_with('✗') {
        Severity::Error
    } else if trimmed.starts_with('!') {
        Severity::Warning
    } else {
        if let Some(Entry::Problem { fix, .. }) = entries.last_mut() {
            *fix = Some(trimmed.to_owned());
        }
        return;
    };
    entries.push(Entry::Problem {
        severity,
        message: trimmed
            .chars()
            .skip(1)
            .collect::<String>()
            .trim()
            .to_owned(),
        fix: None,
    });
}

fn entry(line: &str) -> Entry {
    let body = line.trim_start();
    let available = body.starts_with('●');
    if available || body.starts_with('○') {
        let rest: String = body.chars().skip(2).collect();
        let (name, detail) = split_at_char(&rest, BACKEND_NAME);
        let detail = if available {
            let (role, traits) = split_at_char(detail, BACKEND_ROLE);
            format!("{}, {}", role.trim(), traits.trim())
        } else {
            detail.trim().to_owned()
        };
        return Entry::Backend {
            name: name.trim().to_owned(),
            detail,
            available,
        };
    }
    let content = line.get(2..).unwrap_or_default();
    let (label, value) = split_at_char(content, FIELD_LABEL);
    let is_field = label.ends_with(' ') && !value.starts_with(' ') && !value.is_empty();
    if is_field {
        Entry::Field {
            label: label.trim().to_owned(),
            value: value.trim().to_owned(),
        }
    } else {
        Entry::Note(content.trim().to_owned())
    }
}

fn split_at_char(text: &str, count: usize) -> (&str, &str) {
    match text.char_indices().nth(count) {
        Some((index, _)) => text.split_at(index),
        None => (text, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "kiekje doctor  v0.1.0\n\nSession\n  type              wayland\n  desktop           KDE\n\nMonitors\n  none found\n\nCapture backends\n  ● kwin-screenshot2        used first  can include the pointer, full resolution\n  ○ wlr-screencopy          this compositor does not offer it\n  If the first one fails, the next one down takes over.\n\nProblems\n  ! No clipboard manager\n    install one\n  ✗ No capture backend is available\n\n";

    #[test]
    fn splits_the_report_into_sections_with_problems_first() {
        let checks = parse(REPORT);
        let titles: Vec<_> = checks.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Problems", "Session", "Monitors", "Capture backends"]
        );
        assert!(!checks.all_clear);
    }

    #[test]
    fn reads_fields_notes_and_backends() {
        let checks = parse(REPORT);
        assert_eq!(
            checks.sections[1].entries[0],
            Entry::Field {
                label: "type".into(),
                value: "wayland".into()
            }
        );
        assert_eq!(
            checks.sections[2].entries[0],
            Entry::Note("none found".into())
        );
        assert_eq!(
            checks.sections[3].entries[0],
            Entry::Backend {
                name: "kwin-screenshot2".into(),
                detail: "used first, can include the pointer, full resolution".into(),
                available: true
            }
        );
        assert_eq!(
            checks.sections[3].entries[1],
            Entry::Backend {
                name: "wlr-screencopy".into(),
                detail: "this compositor does not offer it".into(),
                available: false
            }
        );
        assert_eq!(
            checks.sections[3].entries[2],
            Entry::Note("If the first one fails, the next one down takes over.".into())
        );
    }

    #[test]
    fn attaches_the_fix_to_its_problem() {
        let checks = parse(REPORT);
        assert_eq!(
            checks.sections[0].entries,
            [
                Entry::Problem {
                    severity: Severity::Warning,
                    message: "No clipboard manager".into(),
                    fix: Some("install one".into())
                },
                Entry::Problem {
                    severity: Severity::Error,
                    message: "No capture backend is available".into(),
                    fix: None
                }
            ]
        );
    }

    #[test]
    fn marks_a_clean_report_as_all_clear() {
        let checks = parse(
            "kiekje doctor  v0.1.0\n\nSession\n  type              wayland\n\nNo problems found.\n\n",
        );
        assert!(checks.all_clear);
        assert_eq!(checks.sections.len(), 1);
    }
}
