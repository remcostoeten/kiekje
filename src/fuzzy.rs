/// A search text folded once, to test against many haystacks without refolding it.
///
/// # Examples
///
/// ```ignore
/// let needle = fuzzy::Needle::new("invoce");
/// let hits = names.iter().filter(|name| needle.matches(name)).count();
/// ```
pub struct Needle {
    terms: Vec<Term>,
}

struct Term {
    text: String,
    chars: Vec<char>,
    allowed: usize,
}

#[derive(Default)]
struct Scratch {
    haystack: String,
    word: Vec<char>,
    previous: Vec<usize>,
    current: Vec<usize>,
}

thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

impl Needle {
    /// Folds `text` and splits it into its whitespace-separated terms.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let needle = fuzzy::Needle::new("factuur bug");
    /// ```
    pub fn new(text: &str) -> Self {
        let terms = fold(text)
            .split_whitespace()
            .map(|term| {
                let chars: Vec<char> = term.chars().collect();
                Term {
                    text: term.to_owned(),
                    allowed: allowed_edits(chars.len()),
                    chars,
                }
            })
            .collect();
        Self { terms }
    }

    /// Says whether every term matches `haystack`.
    ///
    /// Both sides are lowercased and common accented Latin letters are folded, so "cafe" finds
    /// "Café". Every whitespace-separated term must match: either as a substring of the haystack,
    /// or within a small edit distance of one of the haystack's words. The allowed distance grows
    /// with the term length, so short terms only match exactly.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(fuzzy::Needle::new("invoce").matches("Invoice.png"));
    /// ```
    pub fn matches(&self, haystack: &str) -> bool {
        SCRATCH.with_borrow_mut(|scratch| {
            let Scratch {
                haystack: folded,
                word,
                previous,
                current,
            } = scratch;
            folded.clear();
            fold_into(haystack, folded);
            self.terms.iter().all(|term| {
                folded.contains(term.text.as_str())
                    || (term.allowed > 0
                        && folded
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|candidate| !candidate.is_empty())
                            .any(|candidate| {
                                word.clear();
                                word.extend(candidate.chars());
                                word.len().abs_diff(term.chars.len()) <= term.allowed
                                    && distance(&term.chars, word, previous, current)
                                        <= term.allowed
                            }))
            })
        })
    }
}

fn allowed_edits(length: usize) -> usize {
    match length {
        0..=3 => 0,
        4..=6 => 1,
        _ => 2,
    }
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
    let term: Vec<char> = term.chars().collect();
    let word: Vec<char> = word.chars().collect();
    let allowed = allowed_edits(term.len());
    allowed > 0
        && term.len().abs_diff(word.len()) <= allowed
        && distance(&term, &word, &mut Vec::new(), &mut Vec::new()) <= allowed
}

#[cfg(test)]
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    distance(&a, &b, &mut Vec::new(), &mut Vec::new())
}

fn distance(a: &[char], b: &[char], previous: &mut Vec<usize>, current: &mut Vec<usize>) -> usize {
    previous.clear();
    previous.extend(0..=b.len());
    current.clear();
    current.resize(b.len() + 1, 0);
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(previous, current);
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
    let mut folded = String::with_capacity(text.len());
    fold_into(text, &mut folded);
    folded
}

fn fold_into(text: &str, folded: &mut String) {
    for lower in text.chars().flat_map(char::to_lowercase) {
        folded.push(match lower {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ß' => 's',
            other => other,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(needle: &str, haystack: &str) -> bool {
        Needle::new(needle).matches(haystack)
    }

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
    fn reuses_one_needle_across_haystacks() {
        let needle = Needle::new("  Factuur  BUG ");
        assert!(needle.matches("bug-fatcuur-2026.png"));
        assert!(!needle.matches("desk.png"));
        assert!(needle.matches("BUG factuur.png"));
        assert!(Needle::new("   ").matches("anything"));
    }

    #[test]
    fn close_agrees_with_the_edit_budget() {
        assert!(close("invoce", "invoice"));
        assert!(!close("bag", "bug"));
        assert!(close("screenshoot", "screenshot"));
        assert!(!close("invoce", "invoicexyz"));
        assert!(!close("abcdefgh", "abxxxxgh"));
    }

    #[test]
    fn computes_edit_distance() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }
}
