//! Text search with position — find text patterns and return matches with bounding boxes.

use regex::Regex;

use crate::geometry::BBox;

/// Options controlling text search behavior.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SearchOptions {
    /// Whether to interpret the pattern as a regex (default: `true`).
    /// When `false`, the pattern is treated as a literal string.
    pub regex: bool,
    /// Whether the search is case-sensitive (default: `true`).
    pub case_sensitive: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            regex: true,
            case_sensitive: true,
        }
    }
}

/// A single text search match with its bounding box and position information.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SearchMatch {
    /// The matched text.
    pub text: String,
    /// Union bounding box of all constituent characters.
    pub bbox: BBox,
    /// Page number (0-indexed).
    pub page_number: usize,
    /// Indices into the page's char array for the matched characters.
    pub char_indices: Vec<usize>,
}

/// Search for a pattern in a sequence of characters and return matches with bounding boxes.
///
/// The algorithm:
/// 1. Concatenate all char texts into a single string, tracking byte-offset → char-index mapping.
/// 2. Run the pattern (regex or literal) against the concatenated string.
/// 3. For each match, determine which chars contribute and compute the union bbox.
///
/// # Arguments
///
/// * `chars` - The characters to search within (from a page).
/// * `pattern` - The search pattern (regex or literal string).
/// * `options` - Search options (regex mode, case sensitivity).
/// * `page_number` - The page number for the returned matches.
///
/// # Returns
///
/// A vector of [`SearchMatch`] with bounding boxes computed from constituent chars.
/// Returns an empty vector if the pattern is invalid or no matches are found.
pub fn search_chars(
    chars: &[crate::text::Char],
    pattern: &str,
    options: &SearchOptions,
    page_number: usize,
) -> Vec<SearchMatch> {
    if chars.is_empty() || pattern.is_empty() {
        return Vec::new();
    }

    // Build the concatenated text and mapping from byte offset to char index.
    // Each char's text maps to a range of byte offsets in the concatenated string.
    let mut full_text = String::new();
    // byte_to_char_idx[byte_offset] = index into chars array
    let mut byte_to_char_idx: Vec<usize> = Vec::new();

    for (i, ch) in chars.iter().enumerate() {
        let start = full_text.len();
        full_text.push_str(&ch.text);
        let end = full_text.len();
        for _ in start..end {
            byte_to_char_idx.push(i);
        }
    }

    // Build the regex pattern
    let regex_pattern = if options.regex {
        if options.case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){pattern}")
        }
    } else {
        let escaped = regex::escape(pattern);
        if options.case_sensitive {
            escaped
        } else {
            format!("(?i){escaped}")
        }
    };

    let re = match Regex::new(&regex_pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    for m in re.find_iter(&full_text) {
        let match_start = m.start();
        let match_end = m.end();

        if match_start >= byte_to_char_idx.len() || match_end == 0 {
            continue;
        }

        // Collect unique char indices for this match
        let mut char_indices: Vec<usize> = Vec::new();
        for byte_offset in match_start..match_end {
            if byte_offset < byte_to_char_idx.len() {
                let idx = byte_to_char_idx[byte_offset];
                if char_indices.last() != Some(&idx) {
                    char_indices.push(idx);
                }
            }
        }

        if char_indices.is_empty() {
            continue;
        }

        // Compute the union bbox of matched chars
        let mut bbox = chars[char_indices[0]].bbox;
        for &idx in &char_indices[1..] {
            bbox = bbox.union(&chars[idx].bbox);
        }

        results.push(SearchMatch {
            text: m.as_str().to_string(),
            bbox,
            page_number,
            char_indices,
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{Char, TextDirection};

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn search_options_defaults() {
        let opts = SearchOptions::default();
        assert!(opts.regex);
        assert!(opts.case_sensitive);
    }

    #[test]
    fn simple_string_search() {
        // "Hello World" — search for "World"
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("e", 18.0, 100.0, 26.0, 112.0),
            make_char("l", 26.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 34.0, 112.0),
            make_char("o", 34.0, 100.0, 42.0, 112.0),
            make_char(" ", 42.0, 100.0, 46.0, 112.0),
            make_char("W", 46.0, 100.0, 56.0, 112.0),
            make_char("o", 56.0, 100.0, 64.0, 112.0),
            make_char("r", 64.0, 100.0, 70.0, 112.0),
            make_char("l", 70.0, 100.0, 74.0, 112.0),
            make_char("d", 74.0, 100.0, 82.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: false,
            ..Default::default()
        };
        let matches = search_chars(&chars, "World", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "World");
        assert_eq!(matches[0].page_number, 0);
        assert_eq!(matches[0].char_indices, vec![6, 7, 8, 9, 10]);
        // Union bbox: x0=46, top=100, x1=82, bottom=112
        assert_eq!(matches[0].bbox, BBox::new(46.0, 100.0, 82.0, 112.0));
    }

    #[test]
    fn regex_search() {
        // "Hello World" — search for regex "H.llo"
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("e", 18.0, 100.0, 26.0, 112.0),
            make_char("l", 26.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 34.0, 112.0),
            make_char("o", 34.0, 100.0, 42.0, 112.0),
            make_char(" ", 42.0, 100.0, 46.0, 112.0),
            make_char("W", 46.0, 100.0, 56.0, 112.0),
        ];
        let opts = SearchOptions::default(); // regex=true
        let matches = search_chars(&chars, "H.llo", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "Hello");
        assert_eq!(matches[0].char_indices, vec![0, 1, 2, 3, 4]);
        assert_eq!(matches[0].bbox, BBox::new(10.0, 100.0, 42.0, 112.0));
    }

    #[test]
    fn case_insensitive_search() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("e", 18.0, 100.0, 26.0, 112.0),
            make_char("l", 26.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 34.0, 112.0),
            make_char("o", 34.0, 100.0, 42.0, 112.0),
        ];
        // Search for "hello" (lowercase) with case_insensitive
        let opts = SearchOptions {
            regex: false,
            case_sensitive: false,
        };
        let matches = search_chars(&chars, "hello", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "Hello");
    }

    #[test]
    fn case_sensitive_no_match() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("i", 18.0, 100.0, 26.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: false,
            case_sensitive: true,
        };
        let matches = search_chars(&chars, "hi", &opts, 0);

        assert!(matches.is_empty());
    }

    #[test]
    fn multi_word_match_bbox() {
        // "Hello World" — search for "lo Wo" spanning multiple words
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("e", 18.0, 100.0, 26.0, 112.0),
            make_char("l", 26.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 34.0, 112.0),
            make_char("o", 34.0, 100.0, 42.0, 112.0),
            make_char(" ", 42.0, 100.0, 46.0, 112.0),
            make_char("W", 46.0, 100.0, 56.0, 112.0),
            make_char("o", 56.0, 100.0, 64.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: false,
            ..Default::default()
        };
        let matches = search_chars(&chars, "lo Wo", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "lo Wo");
        // chars[3] (l: 30-34), [4] (o: 34-42), [5] (space: 42-46), [6] (W: 46-56), [7] (o: 56-64)
        assert_eq!(matches[0].char_indices, vec![3, 4, 5, 6, 7]);
        assert_eq!(matches[0].bbox, BBox::new(30.0, 100.0, 64.0, 112.0));
    }

    #[test]
    fn no_matches_returns_empty() {
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
        ];
        let opts = SearchOptions::default();
        let matches = search_chars(&chars, "XYZ", &opts, 0);

        assert!(matches.is_empty());
    }

    #[test]
    fn empty_chars_returns_empty() {
        let matches = search_chars(&[], "test", &SearchOptions::default(), 0);
        assert!(matches.is_empty());
    }

    #[test]
    fn empty_pattern_returns_empty() {
        let chars = vec![make_char("A", 10.0, 100.0, 20.0, 112.0)];
        let matches = search_chars(&chars, "", &SearchOptions::default(), 0);
        assert!(matches.is_empty());
    }

    #[test]
    fn multiple_matches() {
        // "abab" — search for "ab" should return 2 matches
        let chars = vec![
            make_char("a", 10.0, 100.0, 18.0, 112.0),
            make_char("b", 18.0, 100.0, 26.0, 112.0),
            make_char("a", 26.0, 100.0, 34.0, 112.0),
            make_char("b", 34.0, 100.0, 42.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: false,
            ..Default::default()
        };
        let matches = search_chars(&chars, "ab", &opts, 0);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].text, "ab");
        assert_eq!(matches[0].char_indices, vec![0, 1]);
        assert_eq!(matches[1].text, "ab");
        assert_eq!(matches[1].char_indices, vec![2, 3]);
    }

    #[test]
    fn multiline_match_bbox() {
        // Chars on different lines — match spanning them should have union bbox
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("C", 10.0, 120.0, 20.0, 132.0),
        ];
        let opts = SearchOptions {
            regex: false,
            ..Default::default()
        };
        let matches = search_chars(&chars, "ABC", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "ABC");
        // Union: x0=10, top=100, x1=30, bottom=132
        assert_eq!(matches[0].bbox, BBox::new(10.0, 100.0, 30.0, 132.0));
    }

    #[test]
    fn invalid_regex_returns_empty() {
        let chars = vec![make_char("A", 10.0, 100.0, 20.0, 112.0)];
        let opts = SearchOptions {
            regex: true,
            ..Default::default()
        };
        let matches = search_chars(&chars, "[invalid", &opts, 0);
        assert!(matches.is_empty());
    }

    #[test]
    fn regex_case_insensitive() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("i", 18.0, 100.0, 26.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: true,
            case_sensitive: false,
        };
        let matches = search_chars(&chars, "h.", &opts, 0);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].text, "Hi");
    }

    #[test]
    fn page_number_in_result() {
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
        ];
        let opts = SearchOptions {
            regex: false,
            ..Default::default()
        };
        let matches = search_chars(&chars, "AB", &opts, 5);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].page_number, 5);
    }
}
