use crate::geometry::BBox;
use crate::text::{Char, TextDirection, is_cjk_text};

/// Options for word extraction, matching pdfplumber defaults.
#[derive(Debug, Clone)]
pub struct WordOptions {
    /// Maximum horizontal distance between characters to group into a word.
    pub x_tolerance: f64,
    /// Maximum vertical distance between characters to group into a word.
    pub y_tolerance: f64,
    /// If true, include blank/space characters in words instead of splitting on them.
    pub keep_blank_chars: bool,
    /// If true, use the text flow order from the PDF content stream instead of spatial ordering.
    pub use_text_flow: bool,
    /// Text direction for grouping characters.
    pub text_direction: TextDirection,
    /// If true, expand common Latin ligatures (U+FB00–U+FB06) to their multi-character equivalents.
    pub expand_ligatures: bool,
}

impl Default for WordOptions {
    fn default() -> Self {
        Self {
            x_tolerance: 3.0,
            y_tolerance: 3.0,
            keep_blank_chars: false,
            use_text_flow: false,
            text_direction: TextDirection::default(),
            expand_ligatures: true,
        }
    }
}

/// A word extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Word {
    /// The text content of this word.
    pub text: String,
    /// Bounding box encompassing all constituent characters.
    pub bbox: BBox,
    /// Distance from the top of the first page (minimum doctop of constituent chars).
    pub doctop: f64,
    /// Text direction for this word.
    pub direction: TextDirection,
    /// The characters that make up this word.
    pub chars: Vec<Char>,
}

/// Extracts words from a sequence of characters based on spatial proximity.
pub struct WordExtractor;

impl WordExtractor {
    /// Extract words from the given characters using the specified options.
    ///
    /// Characters are grouped into words based on spatial proximity:
    /// - Characters within `x_tolerance` horizontally and `y_tolerance` vertically
    ///   are grouped together.
    /// - For CJK characters, character width (or height for vertical text) is used
    ///   as the tolerance instead of the fixed `x_tolerance`/`y_tolerance`.
    /// - By default, whitespace characters split words. Set `keep_blank_chars`
    ///   to include them.
    /// - By default, characters are sorted spatially. Set `use_text_flow` to
    ///   preserve PDF content stream order.
    /// - `text_direction` controls sorting and gap logic for vertical text.
    pub fn extract(chars: &[Char], options: &WordOptions) -> Vec<Word> {
        if chars.is_empty() {
            return Vec::new();
        }

        let mut sorted_chars: Vec<&Char> = chars.iter().collect();
        if !options.use_text_flow {
            match options.text_direction {
                TextDirection::Ttb => {
                    // Vertical: columns right-to-left, top-to-bottom within column
                    sorted_chars.sort_by(|a, b| {
                        b.bbox
                            .x0
                            .partial_cmp(&a.bbox.x0)
                            .unwrap()
                            .then(a.bbox.top.partial_cmp(&b.bbox.top).unwrap())
                    });
                }
                TextDirection::Btt => {
                    // Vertical bottom-to-top: columns right-to-left, bottom-to-top
                    sorted_chars.sort_by(|a, b| {
                        b.bbox
                            .x0
                            .partial_cmp(&a.bbox.x0)
                            .unwrap()
                            .then(b.bbox.bottom.partial_cmp(&a.bbox.bottom).unwrap())
                    });
                }
                TextDirection::Rtl => {
                    // Horizontal right-to-left: top-to-bottom, right-to-left within row
                    sorted_chars.sort_by(|a, b| {
                        a.bbox
                            .top
                            .partial_cmp(&b.bbox.top)
                            .unwrap()
                            .then(b.bbox.x0.partial_cmp(&a.bbox.x0).unwrap())
                    });
                }
                _ => {
                    // Horizontal left-to-right: top-to-bottom, left-to-right
                    sorted_chars.sort_by(|a, b| {
                        a.bbox
                            .top
                            .partial_cmp(&b.bbox.top)
                            .unwrap()
                            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
                    });
                }
            }
        }

        let is_vertical = matches!(
            options.text_direction,
            TextDirection::Ttb | TextDirection::Btt
        );

        let mut words = Vec::new();
        let mut current_chars: Vec<Char> = Vec::new();

        for &ch in &sorted_chars {
            let is_blank = ch.text.chars().all(|c| c.is_whitespace());

            // If this is a blank and we're not keeping blanks, finish current word
            if is_blank && !options.keep_blank_chars {
                if !current_chars.is_empty() {
                    words.push(Self::make_word(&current_chars, options.expand_ligatures));
                    current_chars.clear();
                }
                continue;
            }

            if current_chars.is_empty() {
                current_chars.push(ch.clone());
                continue;
            }

            let last = current_chars.last().unwrap();

            let should_split = if is_vertical {
                Self::should_split_vertical(last, ch, options)
            } else {
                Self::should_split_horizontal(last, ch, options)
            };

            if should_split {
                words.push(Self::make_word(&current_chars, options.expand_ligatures));
                current_chars.clear();
            }

            current_chars.push(ch.clone());
        }

        if !current_chars.is_empty() {
            words.push(Self::make_word(&current_chars, options.expand_ligatures));
        }

        words
    }

    /// Determine the effective x-tolerance between two characters.
    ///
    /// For CJK characters, uses the previous character's width as tolerance,
    /// which accounts for the wider spacing of full-width characters.
    fn effective_x_tolerance(last: &Char, current: &Char, base: f64) -> f64 {
        if is_cjk_text(&last.text) || is_cjk_text(&current.text) {
            last.bbox.width().max(base)
        } else {
            base
        }
    }

    /// Determine the effective y-tolerance between two characters (for vertical text).
    fn effective_y_tolerance(last: &Char, current: &Char, base: f64) -> f64 {
        if is_cjk_text(&last.text) || is_cjk_text(&current.text) {
            last.bbox.height().max(base)
        } else {
            base
        }
    }

    /// Check if two horizontally-adjacent chars should be split into separate words.
    ///
    /// Uses direction-agnostic gap: the geometric distance between x-intervals.
    /// Returns 0 for overlapping/touching chars and positive for separated chars.
    /// This matches Python pdfplumber behavior where overlapping chars (e.g.,
    /// duplicate chars for bold rendering) are always grouped together.
    fn should_split_horizontal(last: &Char, current: &Char, options: &WordOptions) -> bool {
        let x_gap =
            (last.bbox.x0.max(current.bbox.x0) - last.bbox.x1.min(current.bbox.x1)).max(0.0);
        let y_diff = (current.bbox.top - last.bbox.top).abs();
        let x_tol = Self::effective_x_tolerance(last, current, options.x_tolerance);
        x_gap > x_tol || y_diff > options.y_tolerance
    }

    /// Check if two vertically-adjacent chars should be split into separate words.
    ///
    /// Uses direction-agnostic gap: the geometric distance between y-intervals.
    /// Handles both TTB and BTT text correctly.
    fn should_split_vertical(last: &Char, current: &Char, options: &WordOptions) -> bool {
        let y_gap = (last.bbox.top.max(current.bbox.top)
            - last.bbox.bottom.min(current.bbox.bottom))
        .max(0.0);
        let x_diff = (current.bbox.x0 - last.bbox.x0).abs();
        let y_tol = Self::effective_y_tolerance(last, current, options.y_tolerance);
        y_gap > y_tol || x_diff > options.x_tolerance
    }

    fn make_word(chars: &[Char], expand_ligatures: bool) -> Word {
        let raw_text: String = chars.iter().map(|c| c.text.as_str()).collect();
        let text = if expand_ligatures {
            expand_ligatures_in_text(&raw_text)
        } else {
            raw_text
        };
        let bbox = chars
            .iter()
            .map(|c| c.bbox)
            .reduce(|a, b| a.union(&b))
            .expect("make_word called with non-empty chars");
        let doctop = chars.iter().map(|c| c.doctop).fold(f64::INFINITY, f64::min);
        let direction = chars[0].direction;
        Word {
            text,
            bbox,
            doctop,
            direction,
            chars: chars.to_vec(),
        }
    }
}

/// Expand common Latin ligatures (U+FB00–U+FB06) to their multi-character equivalents.
fn expand_ligatures_in_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{FB00}' => result.push_str("ff"),
            '\u{FB01}' => result.push_str("fi"),
            '\u{FB02}' => result.push_str("fl"),
            '\u{FB03}' => result.push_str("ffi"),
            '\u{FB04}' => result.push_str("ffl"),
            '\u{FB05}' => result.push_str("\u{017F}t"), // long s + t
            '\u{FB06}' => result.push_str("st"),
            _ => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_word_has_doctop_and_direction() {
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].doctop, 100.0);
        assert_eq!(words[0].direction, TextDirection::Ltr);
    }

    #[test]
    fn test_word_doctop_uses_min_char_doctop() {
        // Characters with different doctop values - word should use minimum
        let mut chars = vec![
            make_char("X", 10.0, 100.0, 20.0, 112.0),
            make_char("Y", 20.0, 100.0, 30.0, 112.0),
        ];
        chars[0].doctop = 900.0;
        chars[1].doctop = 892.0;
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words[0].doctop, 892.0);
    }

    #[test]
    fn test_default_options() {
        let opts = WordOptions::default();
        assert_eq!(opts.x_tolerance, 3.0);
        assert_eq!(opts.y_tolerance, 3.0);
        assert!(!opts.keep_blank_chars);
        assert!(!opts.use_text_flow);
    }

    #[test]
    fn test_empty_chars() {
        let words = WordExtractor::extract(&[], &WordOptions::default());
        assert!(words.is_empty());
    }

    #[test]
    fn test_single_char() {
        let chars = vec![make_char("A", 10.0, 100.0, 20.0, 112.0)];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "A");
        assert_eq!(words[0].chars.len(), 1);
    }

    #[test]
    fn test_simple_horizontal_text() {
        // "Hello" — 5 consecutive touching chars on one line
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("e", 20.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 35.0, 112.0),
            make_char("l", 35.0, 100.0, 40.0, 112.0),
            make_char("o", 40.0, 100.0, 50.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[0].bbox, BBox::new(10.0, 100.0, 50.0, 112.0));
        assert_eq!(words[0].chars.len(), 5);
    }

    #[test]
    fn test_multi_line_text() {
        // "Hi" on line 1 (top=100), "Lo" on line 2 (top=120)
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("i", 20.0, 100.0, 30.0, 112.0),
            make_char("L", 10.0, 120.0, 20.0, 132.0),
            make_char("o", 20.0, 120.0, 30.0, 132.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[1].text, "Lo");
    }

    #[test]
    fn test_text_with_large_gap() {
        // "AB" then gap of 20 then "CD" — should be separate words
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("C", 50.0, 100.0, 60.0, 112.0), // gap = 50-30 = 20 > 3
            make_char("D", 60.0, 100.0, 70.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "AB");
        assert_eq!(words[1].text, "CD");
    }

    #[test]
    fn test_text_with_small_gap_within_tolerance() {
        // Gap of 2 which is within default tolerance of 3
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 22.0, 100.0, 32.0, 112.0), // gap = 22-20 = 2 <= 3
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "AB");
    }

    #[test]
    fn test_split_on_space_char() {
        // "A B" with an explicit space character
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char(" ", 20.0, 100.0, 25.0, 112.0),
            make_char("B", 25.0, 100.0, 35.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "A");
        assert_eq!(words[1].text, "B");
    }

    #[test]
    fn test_keep_blank_chars_true() {
        // "A B" with space — keep_blank_chars groups them as one word
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char(" ", 20.0, 100.0, 25.0, 112.0),
            make_char("B", 25.0, 100.0, 35.0, 112.0),
        ];
        let opts = WordOptions {
            keep_blank_chars: true,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "A B");
    }

    #[test]
    fn test_configurable_x_tolerance() {
        // Gap of 10 between A and B
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 30.0, 100.0, 40.0, 112.0), // gap = 10
        ];

        // Default tolerance (3) — two words
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);

        // Larger tolerance (15) — one word
        let opts = WordOptions {
            x_tolerance: 15.0,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "AB");
    }

    #[test]
    fn test_configurable_y_tolerance() {
        // Chars on slightly different vertical positions (y_diff = 5)
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 105.0, 30.0, 117.0), // y_diff = 5
        ];

        // Default y_tolerance (3) — two words
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);

        // Larger y_tolerance (10) — one word
        let opts = WordOptions {
            y_tolerance: 10.0,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "AB");
    }

    #[test]
    fn test_word_bbox_is_union_of_char_bboxes() {
        // Characters with varying heights; tops increase left-to-right
        // so spatial sort preserves left-to-right order.
        let chars = vec![
            make_char("A", 10.0, 97.0, 20.0, 112.0),
            make_char("b", 20.0, 98.0, 28.0, 110.0),
            make_char("C", 28.0, 99.0, 38.0, 113.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].bbox, BBox::new(10.0, 97.0, 38.0, 113.0));
    }

    #[test]
    fn test_unsorted_chars_are_sorted_spatially() {
        // Chars given in reverse spatial order
        let chars = vec![
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("A", 10.0, 100.0, 20.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "AB");
    }

    #[test]
    fn test_use_text_flow_preserves_order() {
        // Chars in PDF content stream order (reverse of spatial).
        // Adjacent/touching chars are grouped even in text flow mode.
        let chars = vec![
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("A", 10.0, 100.0, 20.0, 112.0),
        ];

        // Without text_flow: sorted left-to-right → [A, B] → gap=0 → "AB"
        let normal = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(normal.len(), 1);
        assert_eq!(normal[0].text, "AB");

        // With text_flow: stream order [B, A] — these are spatially adjacent
        // (B.x0=20 touches A.x1=20), so they group as "BA".
        let opts = WordOptions {
            use_text_flow: true,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "BA");
    }

    #[test]
    fn test_use_text_flow_splits_non_adjacent() {
        // Chars far apart in text flow mode should still split.
        let chars = vec![
            make_char("B", 100.0, 100.0, 110.0, 112.0),
            make_char("A", 10.0, 100.0, 20.0, 112.0),
        ];

        let opts = WordOptions {
            use_text_flow: true,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "B");
        assert_eq!(words[1].text, "A");
    }

    #[test]
    fn test_multiple_spaces_between_words() {
        // "A" then multiple spaces then "B"
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char(" ", 20.0, 100.0, 25.0, 112.0),
            make_char(" ", 25.0, 100.0, 30.0, 112.0),
            make_char("B", 30.0, 100.0, 40.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "A");
        assert_eq!(words[1].text, "B");
    }

    #[test]
    fn test_leading_spaces_ignored() {
        let chars = vec![
            make_char(" ", 5.0, 100.0, 10.0, 112.0),
            make_char("A", 10.0, 100.0, 20.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "A");
    }

    #[test]
    fn test_trailing_spaces_ignored() {
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char(" ", 20.0, 100.0, 25.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "A");
    }

    #[test]
    fn test_overlapping_chars_grouped() {
        // Overlapping characters (negative gap) should still group
        let chars = vec![
            make_char("f", 10.0, 100.0, 20.0, 112.0),
            make_char("i", 18.0, 100.0, 25.0, 112.0), // gap = 18-20 = -2 (overlap)
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "fi");
    }

    #[test]
    fn test_duplicate_chars_at_same_position_grouped() {
        // Duplicate characters at the same position (e.g., bold rendering trick)
        // should be grouped into one word, not split.
        // This is a common pattern in PDFs that create bold text by overlaying.
        let chars = vec![
            make_char("D", 117.6, 99.2, 130.6, 117.2),
            make_char("D", 117.6, 99.2, 130.6, 117.2), // exact duplicate
            make_char("u", 130.6, 99.2, 140.6, 117.2),
            make_char("u", 130.6, 99.2, 140.6, 117.2), // exact duplicate
            make_char("p", 140.6, 99.2, 150.5, 117.2),
            make_char("p", 140.6, 99.2, 150.5, 117.2), // exact duplicate
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Duplicate chars should form one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        assert_eq!(words[0].text, "DDuupp");
    }

    #[test]
    fn test_duplicate_chars_with_slight_offset_grouped() {
        // Duplicate characters at slightly offset positions (horizontal shift effect)
        // should still be grouped into one word.
        let chars = vec![
            make_char("H", 117.6, 344.1, 130.6, 362.1),
            make_char("H", 123.3, 344.1, 136.3, 362.1), // shifted ~5.7pt
            make_char("o", 130.6, 344.1, 140.6, 362.1),
            make_char("o", 136.3, 344.1, 146.2, 362.1), // shifted
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Offset duplicate chars should form one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        assert_eq!(words[0].text, "HHoo");
    }

    #[test]
    fn test_three_words_on_one_line() {
        // "The quick fox" — three words separated by spaces
        let chars = vec![
            make_char("T", 10.0, 100.0, 20.0, 112.0),
            make_char("h", 20.0, 100.0, 28.0, 112.0),
            make_char("e", 28.0, 100.0, 36.0, 112.0),
            make_char(" ", 36.0, 100.0, 40.0, 112.0),
            make_char("q", 40.0, 100.0, 48.0, 112.0),
            make_char("u", 48.0, 100.0, 56.0, 112.0),
            make_char("i", 56.0, 100.0, 60.0, 112.0),
            make_char("c", 60.0, 100.0, 68.0, 112.0),
            make_char("k", 68.0, 100.0, 76.0, 112.0),
            make_char(" ", 76.0, 100.0, 80.0, 112.0),
            make_char("f", 80.0, 100.0, 88.0, 112.0),
            make_char("o", 88.0, 100.0, 96.0, 112.0),
            make_char("x", 96.0, 100.0, 104.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "The");
        assert_eq!(words[1].text, "quick");
        assert_eq!(words[2].text, "fox");
    }

    #[test]
    fn test_multiline_sorting() {
        // Chars from two lines given interleaved — should sort by top then x0
        let chars = vec![
            make_char("C", 10.0, 120.0, 20.0, 132.0), // line 2
            make_char("A", 10.0, 100.0, 20.0, 112.0), // line 1
            make_char("D", 20.0, 120.0, 30.0, 132.0), // line 2
            make_char("B", 20.0, 100.0, 30.0, 112.0), // line 1
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "AB");
        assert_eq!(words[1].text, "CD");
    }

    // --- CJK word grouping tests (US-020) ---

    /// Helper to create a CJK character (full-width, typically 12pt wide).
    fn make_cjk_char(text: &str, x0: f64, top: f64, width: f64, height: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x0 + width, top + height),
            fontname: "SimSun".to_string(),
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
    fn test_chinese_text_grouping() {
        // "中国人" — 3 consecutive CJK characters, each 12pt wide with small gaps
        // With default x_tolerance=3, a gap of 1 between 12pt-wide chars should group
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("国", 23.0, 100.0, 12.0, 12.0), // gap = 23-22 = 1
            make_cjk_char("人", 36.0, 100.0, 12.0, 12.0), // gap = 36-35 = 1
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "中国人");
    }

    #[test]
    fn test_chinese_text_with_larger_gap_uses_char_width_tolerance() {
        // CJK chars with gap=8, which exceeds default x_tolerance=3
        // but CJK-aware logic should use char width (12) as tolerance
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("国", 30.0, 100.0, 12.0, 12.0), // gap = 30-22 = 8 > 3 but < 12
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "CJK chars within char-width tolerance should group"
        );
        assert_eq!(words[0].text, "中国");
    }

    #[test]
    fn test_chinese_text_large_gap_splits() {
        // CJK chars with gap=15, exceeding char width (12)
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("国", 37.0, 100.0, 12.0, 12.0), // gap = 37-22 = 15 > 12
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            2,
            "CJK chars beyond char-width tolerance should split"
        );
        assert_eq!(words[0].text, "中");
        assert_eq!(words[1].text, "国");
    }

    #[test]
    fn test_japanese_mixed_text() {
        // "日本語abc" — CJK followed by Latin
        let chars = vec![
            make_cjk_char("日", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("本", 23.0, 100.0, 12.0, 12.0), // gap=1
            make_cjk_char("語", 36.0, 100.0, 12.0, 12.0), // gap=1
            make_char("a", 49.0, 100.0, 55.0, 112.0),     // gap=1
            make_char("b", 55.0, 100.0, 61.0, 112.0),     // gap=0
            make_char("c", 61.0, 100.0, 67.0, 112.0),     // gap=0
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "日本語abc");
    }

    #[test]
    fn test_korean_text_grouping() {
        // "한글" — 2 Korean characters
        let chars = vec![
            make_cjk_char("한", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("글", 23.0, 100.0, 12.0, 12.0), // gap=1
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "한글");
    }

    #[test]
    fn test_mixed_cjk_latin_with_gap() {
        // "Hello" then gap then "中国" — should be two words
        let chars = vec![
            make_char("H", 10.0, 100.0, 18.0, 112.0),
            make_char("e", 18.0, 100.0, 24.0, 112.0),
            make_char("l", 24.0, 100.0, 28.0, 112.0),
            make_char("l", 28.0, 100.0, 32.0, 112.0),
            make_char("o", 32.0, 100.0, 38.0, 112.0),
            // gap of 20 (well beyond any tolerance)
            make_cjk_char("中", 58.0, 100.0, 12.0, 12.0),
            make_cjk_char("国", 71.0, 100.0, 12.0, 12.0), // gap=1
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[1].text, "中国");
    }

    #[test]
    fn test_cjk_transition_to_latin_uses_cjk_tolerance() {
        // CJK char followed by Latin char with gap=5 (> default 3, but < CJK width 12)
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_char("A", 27.0, 100.0, 33.0, 112.0), // gap = 27-22 = 5
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "CJK-to-Latin transition should use CJK tolerance"
        );
        assert_eq!(words[0].text, "中A");
    }

    #[test]
    fn test_vertical_text_chinese() {
        // Vertical text: chars stacked top-to-bottom in a column
        // "中国人" flowing vertically at x=100
        let chars = vec![
            make_cjk_char("中", 100.0, 10.0, 12.0, 12.0),
            make_cjk_char("国", 100.0, 23.0, 12.0, 12.0), // y_gap = 23-22 = 1
            make_cjk_char("人", 100.0, 36.0, 12.0, 12.0), // y_gap = 36-35 = 1
        ];
        let opts = WordOptions {
            text_direction: TextDirection::Ttb,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "中国人");
    }

    #[test]
    fn test_vertical_text_two_columns() {
        // Two vertical columns: column 1 at x=100, column 2 at x=70
        // Vertical text reads right-to-left (column1 first, column2 second)
        let chars = vec![
            // Column 1 (right side, x=100)
            make_cjk_char("一", 100.0, 10.0, 12.0, 12.0),
            make_cjk_char("二", 100.0, 23.0, 12.0, 12.0),
            // Column 2 (left side, x=70)
            make_cjk_char("三", 70.0, 10.0, 12.0, 12.0),
            make_cjk_char("四", 70.0, 23.0, 12.0, 12.0),
        ];
        let opts = WordOptions {
            text_direction: TextDirection::Ttb,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 2);
        // Right column first in reading order (right-to-left)
        assert_eq!(words[0].text, "一二");
        assert_eq!(words[1].text, "三四");
    }

    #[test]
    fn test_vertical_text_with_gap() {
        // Vertical CJK chars with large vertical gap
        let chars = vec![
            make_cjk_char("上", 100.0, 10.0, 12.0, 12.0),
            make_cjk_char("下", 100.0, 40.0, 12.0, 12.0), // y_gap = 40-22 = 18 > 12
        ];
        let opts = WordOptions {
            text_direction: TextDirection::Ttb,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(
            words.len(),
            2,
            "Vertical CJK chars with large gap should split"
        );
        assert_eq!(words[0].text, "上");
        assert_eq!(words[1].text, "下");
    }

    // --- Ligature expansion tests (US-088) ---

    #[test]
    fn test_expand_ligatures_default_true() {
        let opts = WordOptions::default();
        assert!(opts.expand_ligatures);
    }

    #[test]
    fn test_fi_ligature_expanded_in_word() {
        // "ﬁ" (U+FB01) followed by "nd" → "find"
        let chars = vec![
            make_char("\u{FB01}", 10.0, 100.0, 22.0, 112.0),
            make_char("n", 22.0, 100.0, 30.0, 112.0),
            make_char("d", 30.0, 100.0, 38.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "find");
    }

    #[test]
    fn test_ligature_preserved_when_disabled() {
        // With expand_ligatures=false, ligature should pass through unchanged
        let chars = vec![
            make_char("\u{FB01}", 10.0, 100.0, 22.0, 112.0),
            make_char("n", 22.0, 100.0, 30.0, 112.0),
            make_char("d", 30.0, 100.0, 38.0, 112.0),
        ];
        let opts = WordOptions {
            expand_ligatures: false,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "\u{FB01}nd");
    }

    #[test]
    fn test_all_seven_ligatures_expanded() {
        // Test each of the 7 Latin ligatures U+FB00-U+FB06
        let ligatures = vec![
            ("\u{FB00}", "ff"),        // ff
            ("\u{FB01}", "fi"),        // fi
            ("\u{FB02}", "fl"),        // fl
            ("\u{FB03}", "ffi"),       // ffi
            ("\u{FB04}", "ffl"),       // ffl
            ("\u{FB05}", "\u{017F}t"), // long s + t (ſt)
            ("\u{FB06}", "st"),        // st
        ];
        for (lig, expanded) in ligatures {
            let chars = vec![make_char(lig, 10.0, 100.0, 22.0, 112.0)];
            let words = WordExtractor::extract(&chars, &WordOptions::default());
            assert_eq!(
                words[0].text, expanded,
                "Ligature {} should expand to {:?}",
                lig, expanded
            );
        }
    }

    #[test]
    fn test_multiple_ligatures_in_one_word() {
        // "oﬃce" with ffi ligature → "office"
        let chars = vec![
            make_char("o", 10.0, 100.0, 18.0, 112.0),
            make_char("\u{FB03}", 18.0, 100.0, 30.0, 112.0), // ffi
            make_char("c", 30.0, 100.0, 38.0, 112.0),
            make_char("e", 38.0, 100.0, 46.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words[0].text, "office");
    }

    #[test]
    fn test_cjk_with_space_splits() {
        // CJK chars separated by a space character should still split on the space
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            Char {
                text: " ".to_string(),
                bbox: BBox::new(22.0, 100.0, 25.0, 112.0),
                fontname: "SimSun".to_string(),
                size: 12.0,
                doctop: 100.0,
                upright: true,
                direction: TextDirection::Ltr,
                stroking_color: None,
                non_stroking_color: None,
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                char_code: 32,
                mcid: None,
                tag: None,
            },
            make_cjk_char("国", 25.0, 100.0, 12.0, 12.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "中");
        assert_eq!(words[1].text, "国");
    }
}
