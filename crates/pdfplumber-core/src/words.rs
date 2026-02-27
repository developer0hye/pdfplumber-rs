use crate::geometry::BBox;
use crate::text::Char;

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
}

impl Default for WordOptions {
    fn default() -> Self {
        Self {
            x_tolerance: 3.0,
            y_tolerance: 3.0,
            keep_blank_chars: false,
            use_text_flow: false,
        }
    }
}

/// A word extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// The text content of this word.
    pub text: String,
    /// Bounding box encompassing all constituent characters.
    pub bbox: BBox,
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
    /// - By default, whitespace characters split words. Set `keep_blank_chars`
    ///   to include them.
    /// - By default, characters are sorted spatially (top-to-bottom, left-to-right).
    ///   Set `use_text_flow` to preserve PDF content stream order.
    pub fn extract(chars: &[Char], options: &WordOptions) -> Vec<Word> {
        if chars.is_empty() {
            return Vec::new();
        }

        let mut sorted_chars: Vec<&Char> = chars.iter().collect();
        if !options.use_text_flow {
            sorted_chars.sort_by(|a, b| {
                a.bbox
                    .top
                    .partial_cmp(&b.bbox.top)
                    .unwrap()
                    .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
            });
        }

        let mut words = Vec::new();
        let mut current_chars: Vec<Char> = Vec::new();

        for &ch in &sorted_chars {
            let is_blank = ch.text.chars().all(|c| c.is_whitespace());

            // If this is a blank and we're not keeping blanks, finish current word
            if is_blank && !options.keep_blank_chars {
                if !current_chars.is_empty() {
                    words.push(Self::make_word(&current_chars));
                    current_chars.clear();
                }
                continue;
            }

            if current_chars.is_empty() {
                current_chars.push(ch.clone());
                continue;
            }

            let last = current_chars.last().unwrap();
            let x_gap = ch.bbox.x0 - last.bbox.x1;
            let y_diff = (ch.bbox.top - last.bbox.top).abs();

            if x_gap > options.x_tolerance || y_diff > options.y_tolerance {
                // Gap too large — start a new word
                words.push(Self::make_word(&current_chars));
                current_chars.clear();
            }

            current_chars.push(ch.clone());
        }

        if !current_chars.is_empty() {
            words.push(Self::make_word(&current_chars));
        }

        words
    }

    fn make_word(chars: &[Char]) -> Word {
        let text: String = chars.iter().map(|c| c.text.as_str()).collect();
        let bbox = chars
            .iter()
            .map(|c| c.bbox)
            .reduce(|a, b| a.union(&b))
            .expect("make_word called with non-empty chars");
        Word {
            text,
            bbox,
            chars: chars.to_vec(),
        }
    }
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
        }
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
        // Characters with varying heights
        let chars = vec![
            make_char("A", 10.0, 98.0, 20.0, 112.0),
            make_char("b", 20.0, 100.0, 28.0, 110.0),
            make_char("C", 28.0, 97.0, 38.0, 113.0),
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
        // Chars in PDF content stream order (reverse of spatial)
        let chars = vec![
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("A", 10.0, 100.0, 20.0, 112.0),
        ];
        let opts = WordOptions {
            use_text_flow: true,
            ..WordOptions::default()
        };
        let words = WordExtractor::extract(&chars, &opts);
        // With text_flow, order preserved: "B" first, "A" second
        // x_gap = A.x0(10) - B.x1(30) = -20 <= 3, so they group
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "BA");
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
}
