use std::sync::OnceLock;

use crate::fuzzy;

const TABLE: &str = include_str!("../../resources/emoji/annotations.tsv");

/// Emoji shown before any search, in grid order.
pub const COMMON: [&str; 64] = [
    "👍", "👎", "👏", "🙌", "🙏", "💪", "👀", "🤔", "👉", "👈", "👆", "👇", "✋", "🤷", "🙈", "🤯",
    "😀", "😂", "😍", "😎", "😅", "😢", "😡", "😱", "🥳", "😴", "🤮", "🎉", "🔥", "⭐", "✨", "💥",
    "❤️", "💔", "💯", "💡", "⚠️", "❗", "❓", "✅", "❌", "⛔", "🚫", "🚧", "🐛", "🔧", "🔨", "🚀",
    "📌", "📎", "🔒", "🔓", "🔑", "⏰", "🎯", "🏁", "🏆", "🔍", "💬", "📝", "🔄", "➕", "➖", "➡️",
];

/// Classic chat emoticons, MSN Messenger style, and the emoji they stand for.
///
/// Letters match in either case, so `xd` and `XD` both work.
pub const EMOTICONS: [(&str, &str); 44] = [
    (":)", "🙂"),
    (":-)", "🙂"),
    (":(", "🙁"),
    (":-(", "🙁"),
    (":d", "😃"),
    (":-d", "😃"),
    (":p", "😛"),
    (":-p", "😛"),
    (";)", "😉"),
    (";-)", "😉"),
    (";p", "😜"),
    (":o", "😮"),
    (":-o", "😮"),
    (":s", "😖"),
    (":|", "😐"),
    (":/", "😕"),
    (":$", "😳"),
    (":'(", "😢"),
    (":@", "😡"),
    (":*", "😘"),
    (":x", "🤐"),
    ("xd", "😆"),
    ("b)", "😎"),
    ("8)", "😎"),
    ("<3", "❤️"),
    ("</3", "💔"),
    ("^^", "😊"),
    ("-_-", "😑"),
    ("o_o", "😳"),
    ("(y)", "👍"),
    ("(n)", "👎"),
    ("(h)", "😎"),
    ("(l)", "❤️"),
    ("(u)", "💔"),
    ("(k)", "😘"),
    ("(a)", "😇"),
    ("(6)", "😈"),
    ("(c)", "☕"),
    ("(b)", "🍺"),
    ("(i)", "💡"),
    ("(*)", "⭐"),
    ("(8)", "🎵"),
    ("(f)", "🌹"),
    ("(p)", "📷"),
];

/// One emoji with its English and Dutch name and keywords from Unicode CLDR.
#[derive(Debug)]
pub struct Entry {
    pub glyph: &'static str,
    pub name: &'static str,
    pub dutch_name: &'static str,
    common: bool,
    words: Vec<String>,
    names: [String; 2],
}

impl Entry {
    /// The name shown in a tooltip: the English name, with the Dutch one after it when it differs.
    pub fn label(&self) -> String {
        if self.dutch_name.is_empty() || fuzzy::fold(self.dutch_name) == fuzzy::fold(self.name) {
            self.name.to_owned()
        } else {
            format!("{}, {}", self.name, self.dutch_name)
        }
    }

    fn score(&self, terms: &[String]) -> Option<u32> {
        let mut total = 0;
        for term in terms {
            let best = self
                .names
                .iter()
                .map(|name| score_name(name, term))
                .chain(self.words.iter().map(|word| score_word(word, term)))
                .max()
                .unwrap_or(0);
            if best == 0 {
                return None;
            }
            total += best;
        }
        Some(total + u32::from(self.common))
    }
}

fn score_name(name: &str, term: &str) -> u32 {
    let mut words = name.split(|c: char| !c.is_alphanumeric());
    if name == term {
        40
    } else if words.any(|word| word == term) {
        30
    } else if name.starts_with(term) {
        20
    } else if name.contains(term) {
        10
    } else if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| fuzzy::close(term, word))
    {
        5
    } else {
        0
    }
}

fn score_word(word: &str, term: &str) -> u32 {
    if word == term {
        30
    } else if word.starts_with(term) {
        20
    } else if word.contains(term) {
        10
    } else if fuzzy::close(term, word) {
        5
    } else {
        0
    }
}

/// Every emoji in the bundled table, in Unicode order.
///
/// # Examples
///
/// ```ignore
/// assert!(emoji_index::all().iter().any(|entry| entry.glyph == "🔥"));
/// ```
pub fn all() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| TABLE.lines().filter_map(parse).collect())
}

fn parse(line: &'static str) -> Option<Entry> {
    let mut fields = line.split('\t');
    let glyph = fields.next()?;
    let _group = fields.next()?;
    let name = fields.next()?;
    let keywords = fields.next()?;
    let dutch_name = fields.next()?;
    let dutch_keywords = fields.next()?;
    let words = keywords
        .split('|')
        .chain(dutch_keywords.split('|'))
        .filter(|word| !word.is_empty())
        .map(fuzzy::fold)
        .collect();
    Some(Entry {
        glyph,
        name,
        dutch_name,
        common: COMMON.contains(&glyph),
        words,
        names: [fuzzy::fold(name), fuzzy::fold(dutch_name)],
    })
}

/// Looks up one emoji by its glyph.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(emoji_index::find("🔥").map(|entry| entry.name), Some("fire"));
/// ```
pub fn find(glyph: &str) -> Option<&'static Entry> {
    all().iter().find(|entry| entry.glyph == glyph)
}

/// The emoji behind a chat emoticon such as `:p`, `xD` or `(y)`.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(emoji_index::emoticon(":P").map(|entry| entry.glyph), Some("😛"));
/// ```
pub fn emoticon(text: &str) -> Option<&'static Entry> {
    let text = text.trim().to_lowercase();
    EMOTICONS
        .iter()
        .find(|(emoticon, _)| *emoticon == text)
        .and_then(|(_, glyph)| find(glyph))
}

/// Returns up to `limit` emoji for `query`, best match first.
///
/// An empty query returns [`COMMON`]. A chat emoticon such as `:p` or `(y)` returns its emoji.
/// Colons around the query are ignored, so `:fire:` finds the fire. Otherwise every word of the query has to match an English
/// or Dutch name or keyword: exactly, as a prefix, as a substring, or within a small edit
/// distance. Exact names rank highest, then the common emoji and shorter names win ties.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(emoji_index::search("vuur", 1)[0].glyph, "🔥");
/// ```
pub fn search(query: &str, limit: usize) -> Vec<&'static Entry> {
    let query = query.trim();
    if let Some(entry) = emoticon(query) {
        return vec![entry];
    }
    let terms: Vec<String> = fuzzy::fold(query.trim_matches(':'))
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    if terms.is_empty() {
        return COMMON
            .iter()
            .filter_map(|glyph| find(glyph))
            .take(limit)
            .collect();
    }
    let mut scored: Vec<(u32, usize, &Entry)> = all()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| entry.score(&terms).map(|score| (score, index, entry)))
        .collect();
    scored
        .sort_by_key(|(score, index, entry)| (std::cmp::Reverse(*score), entry.name.len(), *index));
    scored
        .into_iter()
        .map(|(_, _, entry)| entry)
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first(query: &str) -> Option<&'static str> {
        search(query, 1).first().map(|entry| entry.glyph)
    }

    #[test]
    fn loads_every_common_emoji_from_the_table() {
        for glyph in COMMON {
            assert!(find(glyph).is_some(), "{glyph} is missing from the table");
        }
        assert!(all().len() > 1800);
    }

    #[test]
    fn shows_the_common_emoji_for_an_empty_search() {
        let glyphs: Vec<&str> = search("  ", 64).iter().map(|entry| entry.glyph).collect();
        assert_eq!(glyphs, COMMON);
    }

    #[test]
    fn ranks_an_exact_english_name_first() {
        assert_eq!(first("fire"), Some("🔥"));
        assert_eq!(first("thumbs down"), Some("👎"));
        assert_eq!(first("bug"), Some("🐛"));
    }

    #[test]
    fn finds_dutch_names_and_keywords() {
        assert_eq!(first("vuur"), Some("🔥"));
        assert_eq!(first("duim omlaag"), Some("👎"));
        assert_eq!(first("vlag nederland"), Some("🇳🇱"));
    }

    #[test]
    fn tolerates_a_typo() {
        assert_eq!(first("insekt"), Some("🐛"));
        assert_eq!(first("raket"), Some("🚀"));
        assert_eq!(first("rockett"), Some("🚀"));
    }

    #[test]
    fn knows_chat_emoticons_and_ignores_colons() {
        assert_eq!(first(":p"), Some("😛"));
        assert_eq!(first(":P"), Some("😛"));
        assert_eq!(first("xD"), Some("😆"));
        assert_eq!(first(":)"), Some("🙂"));
        assert_eq!(first("(y)"), Some("👍"));
        assert_eq!(first("<3"), Some("❤️"));
        assert_eq!(first(":fire:"), Some("🔥"));
        assert_eq!(first(":"), Some("👍"));
        for (_, glyph) in EMOTICONS {
            assert!(find(glyph).is_some(), "{glyph} is missing from the table");
        }
    }

    #[test]
    fn needs_every_word_to_match() {
        assert_eq!(first("nothing like this"), None);
        assert!(search("fire", 500).iter().all(|entry| entry.glyph != "👎"));
    }

    #[test]
    fn labels_in_both_languages_when_they_differ() {
        assert_eq!(find("🔥").map(Entry::label), Some("fire, vuur".to_owned()));
        assert_eq!(find("🐛").map(Entry::label), Some("bug, insect".to_owned()));
    }
}
