use std::cmp::Reverse;

use crate::capture::CaptureMode;
use crate::library::Capture;

const DAY_US: i64 = 24 * 60 * 60 * 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    Largest,
    Name,
}

impl Sort {
    pub const ALL: [Self; 4] = [Self::Newest, Self::Oldest, Self::Largest, Self::Name];

    pub fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest first",
            Self::Oldest => "Oldest first",
            Self::Largest => "Largest first",
            Self::Name => "By name",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Any,
    Region,
    Fullscreen,
    Window,
}

impl Kind {
    pub const ALL: [Self; 4] = [Self::Any, Self::Region, Self::Fullscreen, Self::Window];

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "All",
            Self::Region => "Region",
            Self::Fullscreen => "Screen",
            Self::Window => "Window",
        }
    }

    fn matches(self, mode: CaptureMode) -> bool {
        match self {
            Self::Any => true,
            Self::Region => matches!(mode, CaptureMode::Region | CaptureMode::Repeat),
            Self::Fullscreen => mode == CaptureMode::Fullscreen,
            Self::Window => mode == CaptureMode::Window,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Period {
    #[default]
    Any,
    Day,
    Week,
    Month,
}

impl Period {
    pub const ALL: [Self; 4] = [Self::Any, Self::Day, Self::Week, Self::Month];

    pub fn label(self) -> &'static str {
        match self {
            Self::Any => "Any time",
            Self::Day => "24 h",
            Self::Week => "7 days",
            Self::Month => "30 days",
        }
    }

    fn since(self, now_us: i64) -> Option<i64> {
        let days = match self {
            Self::Any => return None,
            Self::Day => 1,
            Self::Week => 7,
            Self::Month => 30,
        };
        Some(now_us.saturating_sub(days * DAY_US))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub kind: Kind,
    pub period: Period,
    pub sort: Sort,
    pub missing: bool,
}

/// Returns the captures that match `query`, in its sort order.
///
/// Text matches any part of the file name, ignoring case. A repeated region counts as a region.
/// Periods are rolling windows ending at `now_us`. Only missing captures show when
/// `query.missing` is set, and only present ones otherwise.
///
/// # Examples
///
/// ```ignore
/// let shown = query::apply(&captures, &Query { text: "invoice".into(), ..Query::default() }, now);
/// ```
pub fn apply<'a>(captures: &'a [Capture], query: &Query, now_us: i64) -> Vec<&'a Capture> {
    let needle = query.text.trim().to_lowercase();
    let since = query.period.since(now_us);
    let mut shown: Vec<&Capture> = captures
        .iter()
        .filter(|capture| capture.missing == query.missing)
        .filter(|capture| query.kind.matches(capture.mode))
        .filter(|capture| since.is_none_or(|since| capture.created_us >= since))
        .filter(|capture| needle.is_empty() || name(capture).contains(&needle))
        .collect();
    match query.sort {
        Sort::Newest => shown.sort_by_key(|capture| Reverse((capture.created_us, capture.id))),
        Sort::Oldest => shown.sort_by_key(|capture| (capture.created_us, capture.id)),
        Sort::Largest => {
            shown.sort_by_key(|capture| Reverse((capture.bytes, capture.created_us)));
        }
        Sort::Name => shown.sort_by_cached_key(|capture| (name(capture), capture.id)),
    }
    shown
}

fn name(capture: &Capture) -> String {
    capture
        .path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn capture(id: i64, name: &str, mode: CaptureMode, bytes: u64, created_us: i64) -> Capture {
        Capture {
            id,
            path: PathBuf::from("/shots").join(name),
            mode,
            width: 10,
            height: 10,
            bytes,
            created_us,
            missing: false,
        }
    }

    fn sample() -> Vec<Capture> {
        vec![
            capture(1, "Invoice.png", CaptureMode::Region, 300, 10 * DAY_US),
            capture(2, "desk.png", CaptureMode::Fullscreen, 900, 29 * DAY_US),
            capture(3, "again.png", CaptureMode::Repeat, 100, 33 * DAY_US),
            capture(
                4,
                "bug-invoice.png",
                CaptureMode::Fullscreen,
                500,
                35 * DAY_US,
            ),
        ]
    }

    fn ids(shown: &[&Capture]) -> Vec<i64> {
        shown.iter().map(|capture| capture.id).collect()
    }

    #[test]
    fn sorts_newest_first_by_default() {
        let captures = sample();
        assert_eq!(ids(&apply(&captures, &Query::default(), 0)), [4, 3, 2, 1]);
    }

    #[test]
    fn sorts_by_age_size_and_name() {
        let captures = sample();
        let sorted = |sort| {
            let query = Query {
                sort,
                ..Query::default()
            };
            ids(&apply(&captures, &query, 0))
        };
        assert_eq!(sorted(Sort::Oldest), [1, 2, 3, 4]);
        assert_eq!(sorted(Sort::Largest), [2, 4, 1, 3]);
        assert_eq!(sorted(Sort::Name), [3, 4, 2, 1]);
    }

    #[test]
    fn matches_name_text_ignoring_case() {
        let captures = sample();
        let query = Query {
            text: "  INVOICE ".into(),
            ..Query::default()
        };
        assert_eq!(ids(&apply(&captures, &query, 0)), [4, 1]);
    }

    #[test]
    fn counts_a_repeated_region_as_a_region() {
        let captures = sample();
        let kind = |kind| {
            let query = Query {
                kind,
                ..Query::default()
            };
            ids(&apply(&captures, &query, 0))
        };
        assert_eq!(kind(Kind::Region), [3, 1]);
        assert_eq!(kind(Kind::Fullscreen), [4, 2]);
        assert_eq!(kind(Kind::Window), [0; 0]);
    }

    #[test]
    fn filters_window_captures_on_their_own() {
        let mut captures = sample();
        captures.push(capture(
            5,
            "editor.png",
            CaptureMode::Window,
            200,
            36 * DAY_US,
        ));
        let query = Query {
            kind: Kind::Window,
            ..Query::default()
        };
        assert_eq!(ids(&apply(&captures, &query, 0)), [5]);
    }

    #[test]
    fn keeps_captures_inside_a_rolling_period() {
        let captures = sample();
        let period = |period| {
            let query = Query {
                period,
                ..Query::default()
            };
            ids(&apply(&captures, &query, 35 * DAY_US))
        };
        assert_eq!(period(Period::Day), [4]);
        assert_eq!(period(Period::Week), [4, 3, 2]);
        assert_eq!(period(Period::Month), [4, 3, 2, 1]);
        assert_eq!(period(Period::Any), [4, 3, 2, 1]);
    }

    #[test]
    fn shows_missing_captures_only_when_asked() {
        let mut captures = sample();
        captures[1].missing = true;
        assert_eq!(ids(&apply(&captures, &Query::default(), 0)), [4, 3, 1]);
        let query = Query {
            missing: true,
            ..Query::default()
        };
        assert_eq!(ids(&apply(&captures, &query, 0)), [2]);
    }

    #[test]
    fn combines_every_filter() {
        let captures = sample();
        let query = Query {
            text: "invoice".into(),
            kind: Kind::Fullscreen,
            period: Period::Week,
            sort: Sort::Oldest,
            missing: false,
        };
        assert_eq!(ids(&apply(&captures, &query, 35 * DAY_US)), [4]);
    }
}
