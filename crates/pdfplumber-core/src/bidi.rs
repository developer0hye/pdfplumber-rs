//! Unicode Bidirectional (BiDi) text direction analysis.
//!
//! Applies the Unicode Bidirectional Algorithm (UAX #9) to determine per-character
//! text direction for Arabic, Hebrew, and mixed BiDi content. Characters with
//! strong RTL Unicode properties are tagged with [`TextDirection::Rtl`] so the
//! word extractor can group them correctly.
//!
//! Uses the [`unicode_bidi`] crate for BiDi level resolution.

use unicode_bidi::BidiInfo;

use crate::text::{Char, TextDirection};

/// Apply Unicode BiDi direction analysis to extracted characters.
///
/// Groups characters into lines by vertical proximity, then runs the Unicode
/// BiDi algorithm (UAX #9) on each line to determine per-character direction.
/// Characters resolved to RTL (odd BiDi level) have their `direction` updated
/// to [`TextDirection::Rtl`].
///
/// Only overrides direction for upright horizontal text. Characters already
/// assigned vertical directions (Ttb/Btt) are left unchanged.
///
/// # Arguments
///
/// * `chars` - Characters extracted from a PDF page with initial direction
///   from the text rendering matrix.
/// * `y_tolerance` - Maximum vertical distance to group characters into the
///   same line (default: 3.0 points).
pub fn apply_bidi_directions(chars: &[Char], y_tolerance: f64) -> Vec<Char> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Quick check: if no character has a strong RTL Unicode property,
    // skip BiDi analysis entirely for performance.
    let has_potential_rtl = chars.iter().any(|c| c.text.chars().any(is_strong_rtl));

    if !has_potential_rtl {
        return chars.to_vec();
    }

    let mut result = chars.to_vec();

    // Group chars into lines by vertical proximity
    let line_groups = group_chars_into_lines(&result, y_tolerance);

    for group in &line_groups {
        // Build the text string for this line (in left-to-right spatial order)
        let mut sorted_indices: Vec<usize> = group.clone();
        sorted_indices.sort_by(|&a, &b| {
            result[a]
                .bbox
                .x0
                .partial_cmp(&result[b].bbox.x0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let line_text: String = sorted_indices.iter().map(|&i| &*result[i].text).collect();

        if line_text.is_empty() {
            continue;
        }

        // Run the Unicode BiDi algorithm on the line text
        let bidi_info = BidiInfo::new(&line_text, None);

        // Map BiDi levels back to each character
        let mut text_offset = 0;
        for &char_idx in &sorted_indices {
            let text_len = result[char_idx].text.len();
            let is_horizontal_upright = result[char_idx].upright
                && !matches!(
                    result[char_idx].direction,
                    TextDirection::Ttb | TextDirection::Btt
                );

            if !is_horizontal_upright {
                text_offset += text_len;
                continue;
            }

            // Determine the BiDi level for this character's position in the line
            if text_offset < bidi_info.levels.len() {
                let level = bidi_info.levels[text_offset];
                if level.is_rtl() {
                    result[char_idx].direction = TextDirection::Rtl;
                }
            }

            text_offset += text_len;
        }
    }

    result
}

/// Returns `true` if the character has a strong RTL Unicode bidi class.
///
/// Covers Arabic, Hebrew, and other RTL scripts per Unicode standard.
fn is_strong_rtl(ch: char) -> bool {
    matches!(ch,
        // Arabic
        '\u{0600}'..='\u{06FF}' |   // Arabic
        '\u{0750}'..='\u{077F}' |   // Arabic Supplement
        '\u{08A0}'..='\u{08FF}' |   // Arabic Extended-A
        '\u{FB50}'..='\u{FDFF}' |   // Arabic Presentation Forms-A
        '\u{FE70}'..='\u{FEFF}' |   // Arabic Presentation Forms-B
        // Hebrew
        '\u{0590}'..='\u{05FF}' |   // Hebrew
        '\u{FB1D}'..='\u{FB4F}' |   // Hebrew Presentation Forms
        // Other RTL scripts
        '\u{0700}'..='\u{074F}' |   // Syriac
        '\u{0780}'..='\u{07BF}' |   // Thaana
        '\u{07C0}'..='\u{07FF}' |   // NKo
        '\u{0800}'..='\u{083F}' |   // Samaritan
        '\u{0840}'..='\u{085F}' |   // Mandaic
        '\u{1EE00}'..='\u{1EEFF}'   // Arabic Mathematical Alphabetic Symbols
    )
}

/// Group character indices into lines based on vertical proximity.
///
/// Characters whose vertical centers are within `y_tolerance` of each other
/// are grouped into the same line.
fn group_chars_into_lines(chars: &[Char], y_tolerance: f64) -> Vec<Vec<usize>> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Sort indices by vertical center
    let mut indices: Vec<usize> = (0..chars.len()).collect();
    indices.sort_by(|&a, &b| {
        let center_a = (chars[a].bbox.top + chars[a].bbox.bottom) / 2.0;
        let center_b = (chars[b].bbox.top + chars[b].bbox.bottom) / 2.0;
        center_a
            .partial_cmp(&center_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = vec![indices[0]];
    let mut current_center = (chars[indices[0]].bbox.top + chars[indices[0]].bbox.bottom) / 2.0;

    for &idx in &indices[1..] {
        let center = (chars[idx].bbox.top + chars[idx].bbox.bottom) / 2.0;
        if (center - current_center).abs() <= y_tolerance {
            current_group.push(idx);
        } else {
            groups.push(current_group);
            current_group = vec![idx];
            current_center = center;
        }
    }
    groups.push(current_group);

    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::BBox;
    use crate::painting::Color;

    /// Helper to create a test Char with specific text and position.
    fn make_char_at(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    // ===== Test: Empty input returns empty output =====

    #[test]
    fn empty_chars_returns_empty() {
        let result = apply_bidi_directions(&[], 3.0);
        assert!(result.is_empty());
    }

    // ===== Test: LTR-only text is unchanged =====

    #[test]
    fn ltr_only_text_unchanged() {
        let chars = vec![
            make_char_at("H", 10.0, 0.0, 20.0, 12.0),
            make_char_at("e", 20.0, 0.0, 30.0, 12.0),
            make_char_at("l", 30.0, 0.0, 35.0, 12.0),
            make_char_at("l", 35.0, 0.0, 40.0, 12.0),
            make_char_at("o", 40.0, 0.0, 50.0, 12.0),
        ];
        let result = apply_bidi_directions(&chars, 3.0);
        assert_eq!(result.len(), 5);
        for ch in &result {
            assert_eq!(
                ch.direction,
                TextDirection::Ltr,
                "LTR text should remain LTR"
            );
        }
    }

    // ===== Test: Arabic text is tagged as RTL =====

    #[test]
    fn arabic_text_tagged_rtl() {
        // Arabic word "العربية" (al-arabiyyah) characters positioned left-to-right in PDF
        let chars = vec![
            make_char_at("ا", 56.0, 60.0, 61.0, 74.0),
            make_char_at("ل", 61.0, 60.0, 66.0, 74.0),
            make_char_at("ع", 66.0, 60.0, 71.0, 74.0),
            make_char_at("ر", 71.0, 60.0, 76.0, 74.0),
            make_char_at("ب", 76.0, 60.0, 81.0, 74.0),
            make_char_at("ي", 81.0, 60.0, 86.0, 74.0),
            make_char_at("ة", 86.0, 60.0, 91.0, 74.0),
        ];

        let result = apply_bidi_directions(&chars, 3.0);

        for (i, ch) in result.iter().enumerate() {
            assert_eq!(
                ch.direction,
                TextDirection::Rtl,
                "Arabic char '{}' at index {} should be RTL",
                ch.text,
                i
            );
        }
    }

    // ===== Test: Hebrew text is tagged as RTL =====

    #[test]
    fn hebrew_text_tagged_rtl() {
        // Hebrew word "שלום" (shalom)
        let chars = vec![
            make_char_at("ש", 10.0, 0.0, 20.0, 12.0),
            make_char_at("ל", 20.0, 0.0, 30.0, 12.0),
            make_char_at("ו", 30.0, 0.0, 40.0, 12.0),
            make_char_at("ם", 40.0, 0.0, 50.0, 12.0),
        ];

        let result = apply_bidi_directions(&chars, 3.0);

        for ch in &result {
            assert_eq!(
                ch.direction,
                TextDirection::Rtl,
                "Hebrew char '{}' should be RTL",
                ch.text
            );
        }
    }

    // ===== Test: Mixed BiDi text preserves directions =====

    #[test]
    fn mixed_bidi_text_correct_directions() {
        // Mixed: "Hello مرحبا World"
        let chars = vec![
            // LTR: "Hello "
            make_char_at("H", 10.0, 0.0, 18.0, 12.0),
            make_char_at("e", 18.0, 0.0, 26.0, 12.0),
            make_char_at("l", 26.0, 0.0, 30.0, 12.0),
            make_char_at("l", 30.0, 0.0, 34.0, 12.0),
            make_char_at("o", 34.0, 0.0, 42.0, 12.0),
            make_char_at(" ", 42.0, 0.0, 46.0, 12.0),
            // RTL: "مرحبا"
            make_char_at("م", 46.0, 0.0, 54.0, 12.0),
            make_char_at("ر", 54.0, 0.0, 62.0, 12.0),
            make_char_at("ح", 62.0, 0.0, 70.0, 12.0),
            make_char_at("ب", 70.0, 0.0, 78.0, 12.0),
            make_char_at("ا", 78.0, 0.0, 86.0, 12.0),
            // LTR: " World"
            make_char_at(" ", 86.0, 0.0, 90.0, 12.0),
            make_char_at("W", 90.0, 0.0, 100.0, 12.0),
            make_char_at("o", 100.0, 0.0, 108.0, 12.0),
            make_char_at("r", 108.0, 0.0, 114.0, 12.0),
            make_char_at("l", 114.0, 0.0, 118.0, 12.0),
            make_char_at("d", 118.0, 0.0, 126.0, 12.0),
        ];

        let result = apply_bidi_directions(&chars, 3.0);

        // "Hello" chars should remain LTR
        for i in 0..5 {
            assert_eq!(
                result[i].direction,
                TextDirection::Ltr,
                "Latin char '{}' should be LTR",
                result[i].text
            );
        }
        // Arabic chars should be RTL
        for i in 6..11 {
            assert_eq!(
                result[i].direction,
                TextDirection::Rtl,
                "Arabic char '{}' should be RTL",
                result[i].text
            );
        }
        // "World" chars should remain LTR
        for i in 12..17 {
            assert_eq!(
                result[i].direction,
                TextDirection::Ltr,
                "Latin char '{}' should be LTR",
                result[i].text
            );
        }
    }

    // ===== Test: Vertical text direction is preserved =====

    #[test]
    fn vertical_text_direction_preserved() {
        let mut chars = vec![
            make_char_at("ا", 10.0, 0.0, 20.0, 12.0),
            make_char_at("ل", 20.0, 0.0, 30.0, 12.0),
        ];
        // Mark as vertical text
        chars[0].direction = TextDirection::Ttb;
        chars[0].upright = false;
        chars[1].direction = TextDirection::Ttb;
        chars[1].upright = false;

        let result = apply_bidi_directions(&chars, 3.0);

        for ch in &result {
            assert_eq!(
                ch.direction,
                TextDirection::Ttb,
                "Vertical text direction should be preserved"
            );
        }
    }

    // ===== Test: Multi-line grouping =====

    #[test]
    fn multi_line_grouped_separately() {
        // Line 1: Arabic at y=0
        // Line 2: Latin at y=20
        let chars = vec![
            make_char_at("ا", 10.0, 0.0, 20.0, 12.0),
            make_char_at("ل", 20.0, 0.0, 30.0, 12.0),
            make_char_at("H", 10.0, 20.0, 20.0, 32.0),
            make_char_at("i", 20.0, 20.0, 25.0, 32.0),
        ];

        let result = apply_bidi_directions(&chars, 3.0);

        // Arabic line should be RTL
        assert_eq!(result[0].direction, TextDirection::Rtl);
        assert_eq!(result[1].direction, TextDirection::Rtl);
        // Latin line should be LTR
        assert_eq!(result[2].direction, TextDirection::Ltr);
        assert_eq!(result[3].direction, TextDirection::Ltr);
    }

    // ===== Test: Other fields are preserved =====

    #[test]
    fn other_fields_preserved() {
        let mut ch = make_char_at("ع", 10.0, 0.0, 20.0, 12.0);
        ch.fontname = "Arabic-Font".to_string();
        ch.size = 14.0;
        ch.char_code = 1593; // U+0639

        let result = apply_bidi_directions(&[ch], 3.0);

        assert_eq!(result[0].fontname, "Arabic-Font");
        assert_eq!(result[0].size, 14.0);
        assert_eq!(result[0].char_code, 1593);
        assert_eq!(result[0].direction, TextDirection::Rtl);
    }

    // ===== Test: is_strong_rtl utility =====

    #[test]
    fn is_strong_rtl_arabic() {
        assert!(is_strong_rtl('ع'));
        assert!(is_strong_rtl('ب'));
        assert!(is_strong_rtl('ة'));
    }

    #[test]
    fn is_strong_rtl_hebrew() {
        assert!(is_strong_rtl('ש'));
        assert!(is_strong_rtl('ל'));
        assert!(is_strong_rtl('ם'));
    }

    #[test]
    fn is_strong_rtl_latin() {
        assert!(!is_strong_rtl('A'));
        assert!(!is_strong_rtl('z'));
        assert!(!is_strong_rtl('0'));
        assert!(!is_strong_rtl(' '));
    }

    // ===== Test: Line grouping =====

    #[test]
    fn group_chars_into_lines_basic() {
        let chars = vec![
            make_char_at("A", 10.0, 0.0, 20.0, 12.0),  // line 1
            make_char_at("B", 20.0, 0.5, 30.0, 12.5),  // line 1 (within tolerance)
            make_char_at("C", 10.0, 20.0, 20.0, 32.0), // line 2
        ];

        let groups = group_chars_into_lines(&chars, 3.0);
        assert_eq!(groups.len(), 2, "should have 2 lines");
        assert_eq!(groups[0].len(), 2, "line 1 should have 2 chars");
        assert_eq!(groups[1].len(), 1, "line 2 should have 1 char");
    }
}
