/// Says whether `needle` loosely matches `haystack`.
///
/// Both sides are lowercased and common accented Latin letters are folded, so "cafe" finds
/// "Café". Every whitespace-separated word of the needle must match: either as a substring of
/// the haystack, or within a small edit distance of one of the haystack's words. The allowed
/// distance grows with the word length, so short words only match exactly.
///
/// # Examples
///
/// ```ignore
/// assert!(fuzzy::matches("invoce", "Invoice.png"));
/// assert!(fuzzy::matches("factuur bug", "bug-factuur-2026.png"));
/// assert!(!fuzzy::matches("inv", "desk.png"));
/// ```
pub fn matches(needle: &str, haystack: &str) -> bool {
    let haystack = fold(haystack);
    let words: Vec<&str> = haystack
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    fold(needle)
        .split_whitespace()
        .all(|term| haystack.contains(term) || words.iter().any(|word| close(term, word)))
}

/// Says whether `term` is within a small edit distance of `word`, both already folded.
///
/// Terms of three characters or fewer never match this way; four to six allow one edit, longer
/// ones two.
///
/// # Examples
///
/// ```ignore
/// assert!(fuzzy::close("invoce", "invoice"));
/// assert!(!fuzzy::close("bag", "bug"));
/// ```
pub fn close(term: &str, word: &str) -> bool {
    let allowed = match term.chars().count() {
        0..=3 => return false,
        4..=6 => 1,
        _ => 2,
    };
    if term.chars().count().abs_diff(word.chars().count()) > allowed {
        return false;
    }
    edit_distance(term, word) <= allowed
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// Lowercases `text` and folds common accented Latin letters to their base letter.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(fuzzy::fold("Café"), "cafe");
/// ```
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ß' => 's',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_substrings_ignoring_case() {
        assert!(matches("VOICE", "Invoice.png"));
        assert!(matches("2026-10", "kiekje-2026-10-23.png"));
    }

    #[test]
    fn tolerates_a_typo_in_longer_words() {
        assert!(matches("invoce", "Invoice.png"));
        assert!(matches("factuur", "bug-fatcuur-2026.png"));
        assert!(matches("screenshoot", "screenshot-desk.png"));
    }

    #[test]
    fn keeps_short_words_exact() {
        assert!(!matches("bag", "bug-invoice.png"));
        assert!(matches("bug", "bug-invoice.png"));
    }

    #[test]
    fn needs_every_word_to_match() {
        assert!(matches("bug invoice", "bug-invoice.png"));
        assert!(!matches("bug desk", "bug-invoice.png"));
    }

    #[test]
    fn folds_accents_both_ways() {
        assert!(matches("cafe", "Café-menu.png"));
        assert!(matches("café", "cafe-menu.png"));
    }

    #[test]
    fn computes_edit_distance() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }
}
