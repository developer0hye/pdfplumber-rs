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
    fn test_chinese_text_with_larger_gap_splits() {
        // CJK chars with gap=8, which exceeds default x_tolerance=3.
        // Python pdfplumber uses flat x_tolerance for all chars (no CJK expansion),
        // so gap=8 > 3 → split into separate words.
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_cjk_char("国", 30.0, 100.0, 12.0, 12.0), // gap = 30-22 = 8 > 3
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            2,
            "CJK chars beyond x_tolerance should split (matching Python)"
        );
        assert_eq!(words[0].text, "中");
        assert_eq!(words[1].text, "国");
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
    fn test_cjk_transition_to_latin_splits_beyond_tolerance() {
        // CJK char followed by Latin char with gap=5 (> default x_tolerance=3).
        // Python pdfplumber uses flat x_tolerance for all chars, so gap=5 > 3 → split.
        let chars = vec![
            make_cjk_char("中", 10.0, 100.0, 12.0, 12.0),
            make_char("A", 27.0, 100.0, 33.0, 112.0), // gap = 27-22 = 5 > 3
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            2,
            "CJK-to-Latin beyond x_tolerance should split (matching Python)"
        );
        assert_eq!(words[0].text, "中");
        assert_eq!(words[1].text, "A");
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

    // --- Line clustering tests (US-168-3) ---

    #[test]
    fn test_mixed_y_positions_on_same_visual_line_clustered() {
        // Simulates CJK + Latin mixing where digits are at a slightly different y.
        // Python pdfplumber clusters chars by y (within y_tolerance) before
        // sorting by x, so "2018" is interleaved between CJK chars.
        // Without clustering, global (top, x0) sort separates them.
        //
        // Layout: "公司2018年度" - CJK at top=46.0, digits at top=47.3
        let chars = vec![
            make_cjk_char("公", 282.0, 46.0, 9.0, 9.0), // x: 282-291
            make_cjk_char("司", 291.0, 46.0, 9.0, 9.0), // x: 291-300
            // "2018" at slightly different y (top=47.3, gap ~2.3pt from "司")
            make_char("2", 302.2, 47.3, 306.7, 56.3),
            make_char("0", 306.7, 47.3, 311.2, 56.3),
            make_char("1", 311.2, 47.3, 315.7, 56.3),
            make_char("8", 315.7, 47.3, 320.2, 56.3),
            // CJK resumes at top=46.0 (gap ~2.2pt from "8")
            make_cjk_char("年", 322.4, 46.0, 9.0, 9.0),
            make_cjk_char("度", 331.4, 46.0, 9.0, 9.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Chars at slightly different y on same visual line should cluster into one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        assert_eq!(words[0].text, "公司2018年度");
    }

    #[test]
    fn test_different_y_beyond_tolerance_split() {
        // Chars at y=100 and y=120 (y_diff=20 >> y_tolerance=3) should be separate words.
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
            make_char("C", 10.0, 120.0, 20.0, 132.0),
            make_char("D", 20.0, 120.0, 30.0, 132.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "AB");
        assert_eq!(words[1].text, "CD");
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

    // --- Per-char direction tests (US-181-1) ---

    /// Helper to create a vertical (Ttb) char — same x, stacked vertically.
    fn make_vertical_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: false,
            direction: TextDirection::Ttb,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [0.0, -1.0, 1.0, 0.0, x0, top],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn test_per_char_ttb_direction_groups_vertical_chars() {
        // Simulates B-1191 from senate-expenditures.pdf:
        // 6 chars stacked vertically at the same x, direction=Ttb.
        // With per-char direction, these should group into one word
        // even when the global text_direction is Ltr (default).
        let chars = vec![
            make_vertical_char("B", 731.07, 286.62, 742.89, 295.15),
            make_vertical_char("-", 731.07, 295.15, 742.89, 299.09),
            make_vertical_char("1", 731.07, 299.09, 742.89, 305.66),
            make_vertical_char("1", 731.07, 305.66, 742.89, 312.23),
            make_vertical_char("9", 731.07, 312.23, 742.89, 318.80),
            make_vertical_char("1", 731.07, 318.80, 742.89, 325.37),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Ttb chars should group into one word with per-char direction, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        assert_eq!(words[0].text, "B-1191");
        assert_eq!(words[0].direction, TextDirection::Ttb);
    }

    #[test]
    fn test_per_char_mixed_ltr_and_ttb_on_same_page() {
        // Page with mostly horizontal (Ltr) text plus one vertical (Ttb) word.
        // Global text_direction = Ltr (default).
        // Horizontal chars should use horizontal splitting; vertical chars should
        // use vertical splitting based on their per-char direction.
        let mut chars = vec![
            // Horizontal word "Hi" at top of page
            make_char("H", 10.0, 50.0, 20.0, 62.0),
            make_char("i", 20.0, 50.0, 26.0, 62.0),
            // Vertical word "AB" far away on the right
            make_vertical_char("A", 700.0, 200.0, 712.0, 210.0),
            make_vertical_char("B", 700.0, 210.0, 712.0, 220.0),
        ];
        // Ensure horizontal chars have Ltr direction
        chars[0].direction = TextDirection::Ltr;
        chars[1].direction = TextDirection::Ltr;
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2, "Should have 2 words (Hi + AB)");
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[0].direction, TextDirection::Ltr);
        assert_eq!(words[1].text, "AB");
        assert_eq!(words[1].direction, TextDirection::Ttb);
    }

    #[test]
    fn test_per_char_direction_transition_splits_word() {
        // A horizontal char followed by a vertical char should split
        // because they have different directions, even if spatially close.
        let chars = vec![
            make_char("A", 100.0, 100.0, 110.0, 112.0),          // Ltr
            make_vertical_char("B", 100.0, 112.0, 112.0, 122.0), // Ttb, below A
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            2,
            "Chars with different directions should split into separate words"
        );
    }

    #[test]
    fn test_per_char_ltr_chars_unaffected() {
        // All Ltr chars should behave exactly as before (no regression).
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
    }

    // --- Per-char Rtl direction tests (US-181-2) ---

    /// Helper to create an Rtl char — horizontally mirrored (as in 180° rotation).
    fn make_rtl_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: true,
            direction: TextDirection::Rtl,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [-1.0, 0.0, 0.0, -1.0, x0, top],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    /// Helper to create a Btt char — vertically stacked bottom-to-top (as in 270° rotation).
    fn make_btt_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: false,
            direction: TextDirection::Btt,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [0.0, 1.0, -1.0, 0.0, x0, top],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn test_per_char_rtl_direction_groups_correctly() {
        // Simulates 180° rotated text "Hello":
        // Chars are positioned right-to-left (first char 'H' has largest x0).
        // Rtl chars from TRM rotation are sorted spatially LTR (ascending x0),
        // matching Python pdfplumber behavior. Text appears reversed ("olleH").
        let chars = vec![
            make_rtl_char("H", 540.0, 100.0, 548.0, 112.0),
            make_rtl_char("e", 532.0, 100.0, 540.0, 112.0),
            make_rtl_char("l", 526.0, 100.0, 532.0, 112.0),
            make_rtl_char("l", 520.0, 100.0, 526.0, 112.0),
            make_rtl_char("o", 512.0, 100.0, 520.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Rtl chars should group into one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        // Spatial LTR order: o(512), l(520), l(526), e(532), H(540) → "olleH"
        assert_eq!(words[0].text, "olleH");
        assert_eq!(words[0].direction, TextDirection::Rtl);
    }

    #[test]
    fn test_per_char_rtl_two_words() {
        // "Hello World" with Rtl direction — two words separated by space.
        // In 180° rotation, first word at right, second word at left.
        // Sorted spatially LTR (ascending x0), matching Python pdfplumber.
        let chars = vec![
            // "Hello" at right side
            make_rtl_char("H", 540.0, 100.0, 548.0, 112.0),
            make_rtl_char("e", 532.0, 100.0, 540.0, 112.0),
            make_rtl_char("l", 526.0, 100.0, 532.0, 112.0),
            make_rtl_char("l", 520.0, 100.0, 526.0, 112.0),
            make_rtl_char("o", 512.0, 100.0, 520.0, 112.0),
            // space
            make_rtl_char(" ", 508.0, 100.0, 512.0, 112.0),
            // "World" at left side
            make_rtl_char("W", 500.0, 100.0, 508.0, 112.0),
            make_rtl_char("o", 492.0, 100.0, 500.0, 112.0),
            make_rtl_char("r", 486.0, 100.0, 492.0, 112.0),
            make_rtl_char("l", 480.0, 100.0, 486.0, 112.0),
            make_rtl_char("d", 474.0, 100.0, 480.0, 112.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2);
        // Spatial LTR: d(474)..W(500) → "dlroW", o(512)..H(540) → "olleH"
        assert_eq!(words[0].text, "dlroW");
        assert_eq!(words[1].text, "olleH");
    }

    #[test]
    fn test_per_char_btt_direction_groups_correctly() {
        // Simulates 270° rotated text "Hello":
        // Chars stacked vertically at same x, reading bottom-to-top.
        // First char 'H' has largest y (bottom of page), last char at top.
        // Btt chars are merged with Ttb and sorted spatially top-to-bottom
        // (ascending top), matching Python pdfplumber behavior.
        let chars = vec![
            make_btt_char("H", 72.0, 540.0, 84.0, 548.0),
            make_btt_char("e", 72.0, 532.0, 84.0, 540.0),
            make_btt_char("l", 72.0, 526.0, 84.0, 532.0),
            make_btt_char("l", 72.0, 520.0, 84.0, 526.0),
            make_btt_char("o", 72.0, 512.0, 84.0, 520.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Btt chars should group into one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        // Spatial top-to-bottom: o(512), l(520), l(526), e(532), H(540) → "olleH"
        assert_eq!(words[0].text, "olleH");
        assert_eq!(words[0].direction, TextDirection::Btt);
    }

    #[test]
    fn test_per_char_mixed_ltr_and_rtl_on_same_page() {
        // Page with Ltr and Rtl chars — Rtl is merged with Ltr for
        // spatial sorting (matching Python pdfplumber behavior).
        let chars = vec![
            // Ltr word "Hi"
            make_char("H", 10.0, 50.0, 20.0, 62.0),
            make_char("i", 20.0, 50.0, 26.0, 62.0),
            // Rtl chars "AB" (A at right=540, B at left=532)
            make_rtl_char("A", 540.0, 200.0, 548.0, 212.0),
            make_rtl_char("B", 532.0, 200.0, 540.0, 212.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2, "Should have 2 words (Hi + BA)");
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[0].direction, TextDirection::Ltr);
        // Spatial LTR: B(532), A(540) → "BA"
        assert_eq!(words[1].text, "BA");
        assert_eq!(words[1].direction, TextDirection::Rtl);
    }

    // --- 180°/270° rotation word grouping tests (US-205-5) ---

    #[test]
    fn test_180_rotation_word_grouping_matches_python() {
        // 180° rotated text "Dummy PDF file" produces reversed words in Python.
        // Chars have direction=Rtl from TRM, upright=true.
        // Python sorts spatially LTR → "elif FDP ymmuD".
        let chars = vec![
            // "Dummy" chars, x0 decreasing (Rtl physical layout)
            make_rtl_char("D", 526.5, 754.7, 538.2, 770.8),
            make_rtl_char("u", 516.7, 754.7, 526.5, 770.8),
            make_rtl_char("m", 502.4, 754.7, 516.7, 770.8),
            make_rtl_char("m", 488.1, 754.7, 502.4, 770.8),
            make_rtl_char("y", 479.1, 754.7, 488.1, 770.8),
            // space
            make_rtl_char(" ", 474.6, 754.7, 479.1, 770.8),
            // "PDF" chars
            make_rtl_char("P", 463.9, 754.7, 474.7, 770.8),
            make_rtl_char("D", 454.6, 754.7, 463.9, 770.8),
            make_rtl_char("F", 442.5, 754.7, 454.6, 770.8),
            // space
            make_rtl_char(" ", 438.0, 754.7, 442.5, 770.8),
            // "file" chars
            make_rtl_char("f", 432.6, 754.7, 438.0, 770.8),
            make_rtl_char("i", 425.5, 754.7, 432.6, 770.8),
            make_rtl_char("l", 421.0, 754.7, 425.5, 770.8),
            make_rtl_char("e", 414.7, 754.7, 421.0, 770.8),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 3, "Should have 3 words");
        // Spatial LTR order produces reversed text for each word
        assert_eq!(words[0].text, "elif");
        assert_eq!(words[1].text, "FDP");
        assert_eq!(words[2].text, "ymmuD");
    }

    #[test]
    fn test_270_rotation_word_grouping_matches_python() {
        // 270° rotated text "Dummy PDF file" arranged vertically.
        // Chars have direction=Ttb from TRM, upright=false.
        // Python sorts top-to-bottom → "elif FDP ymmuD" (each word reversed).
        let chars = vec![
            // "Dummy" chars at same x, top decreasing
            make_vertical_char("D", 71.1, 526.5, 87.2, 538.2),
            make_vertical_char("u", 71.1, 516.7, 87.2, 526.5),
            make_vertical_char("m", 71.1, 502.4, 87.2, 516.7),
            make_vertical_char("m", 71.1, 488.1, 87.2, 502.4),
            make_vertical_char("y", 71.1, 479.1, 87.2, 488.1),
            // space
            make_vertical_char(" ", 71.1, 474.6, 87.2, 479.1),
            // "PDF" chars
            make_vertical_char("P", 71.1, 463.9, 87.2, 474.7),
            make_vertical_char("D", 71.1, 454.6, 87.2, 463.9),
            make_vertical_char("F", 71.1, 442.5, 87.2, 454.6),
            // space
            make_vertical_char(" ", 71.1, 438.0, 87.2, 442.5),
            // "file" chars
            make_vertical_char("f", 71.1, 432.6, 87.2, 438.0),
            make_vertical_char("i", 71.1, 425.5, 87.2, 432.6),
            make_vertical_char("l", 71.1, 421.0, 87.2, 425.5),
            make_vertical_char("e", 71.1, 414.7, 87.2, 421.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 3, "Should have 3 words");
        // Sorted top-to-bottom: e(414.7), l, i, f → "elif"; F, D, P → "FDP"; y, m, m, u, D → "ymmuD"
        assert_eq!(words[0].text, "elif");
        assert_eq!(words[1].text, "FDP");
        assert_eq!(words[2].text, "ymmuD");
    }

    // --- Arabic diacritical mark combining tests (US-185-2) ---

    /// Helper to create an Arabic BiDi-RTL char (ascending x0 layout).
    fn make_arabic_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "Arabic-Font".to_string(),
            size: 12.0,
            doctop: top,
            upright: true,
            direction: TextDirection::Rtl,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn test_arabic_diacritic_combined_with_base_overlapping() {
        // Arabic base char followed by diacritical mark (damma U+064F)
        // with overlapping bounding boxes — should form one word.
        let chars = vec![
            make_arabic_char("آ", 120.8, 75.0, 131.0, 101.0),
            make_arabic_char("\u{064F}", 120.8, 75.0, 126.2, 101.0), // damma, overlaps
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Overlapping diacritic should combine with base char"
        );
        assert_eq!(words[0].text, "آ\u{064F}");
    }

    #[test]
    fn test_arabic_diacritic_combined_with_base_small_gap() {
        // Arabic base char followed by diacritical mark with a gap slightly
        // exceeding x_tolerance — should still combine because it's a diacritical mark.
        let chars = vec![
            make_arabic_char("ب", 10.0, 100.0, 20.0, 112.0),
            make_arabic_char("\u{064E}", 24.0, 100.0, 28.0, 112.0), // fatha, gap=4 > x_tolerance=3
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Arabic diacritical mark should combine with base char even with small gap"
        );
        assert_eq!(words[0].text, "ب\u{064E}");
    }

    #[test]
    fn test_arabic_shadda_combined_with_base() {
        // Arabic shadda (U+0651) — a common diacritical mark for consonant doubling.
        let chars = vec![
            make_arabic_char("ت", 10.0, 100.0, 20.0, 112.0),
            make_arabic_char("\u{0651}", 25.0, 100.0, 30.0, 112.0), // shadda, gap=5
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1, "Shadda should combine with base char");
    }

    #[test]
    fn test_arabic_multiple_diacritics_on_one_base() {
        // Arabic base with shadda + fatha (common combination like "تَّ").
        let chars = vec![
            make_arabic_char("ت", 10.0, 100.0, 20.0, 112.0),
            make_arabic_char("\u{0651}", 24.0, 100.0, 28.0, 112.0), // shadda
            make_arabic_char("\u{064E}", 24.0, 96.0, 28.0, 108.0),  // fatha (above shadda)
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Multiple diacritics should combine with base char"
        );
    }

    #[test]
    fn test_arabic_presentation_forms_word_grouping() {
        // Arabic presentation forms (U+FE70-U+FEFF) from FC60_Times.pdf pattern.
        // These chars have ascending x0 (BiDi RTL in PDF visual order).
        let chars = vec![
            make_arabic_char("ﺎ", 108.5, 75.0, 114.5, 101.0),
            make_arabic_char("ﱠ", 114.5, 75.0, 120.0, 101.0),
            make_arabic_char("ﺘ", 114.5, 75.0, 120.8, 101.0),
            make_arabic_char("\u{064F}", 120.8, 75.0, 126.2, 101.0), // damma
            make_arabic_char("آ", 120.8, 75.0, 131.0, 101.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(
            words.len(),
            1,
            "Arabic presentation forms should group into one word, got: {:?}",
            words.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_arabic_word_with_space_splits() {
        // Two Arabic words separated by a space.
        let chars = vec![
            make_arabic_char("ب", 90.0, 75.0, 108.5, 101.0),
            make_arabic_char(" ", 108.5, 75.0, 114.5, 101.0), // space
            make_arabic_char("آ", 114.5, 75.0, 131.0, 101.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 2, "Arabic words should split on space");
    }

    #[test]
    fn test_bidi_rtl_word_text_order() {
        // BiDi RTL chars in ascending x0 (PDF visual order).
        // Word text should be the concatenation in sorted order.
        let chars = vec![
            make_arabic_char("ب", 90.0, 75.0, 108.5, 101.0),
            make_arabic_char("ﺎ", 108.5, 75.0, 114.5, 101.0),
            make_arabic_char("ﺘ", 114.5, 75.0, 120.8, 101.0),
        ];
        let words = WordExtractor::extract(&chars, &WordOptions::default());
        assert_eq!(words.len(), 1);
        // BiDi RTL chars in ascending x0 → sorted ascending → text is visual order
        assert_eq!(words[0].text, "بﺎﺘ");
        assert_eq!(words[0].direction, TextDirection::Rtl);
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
