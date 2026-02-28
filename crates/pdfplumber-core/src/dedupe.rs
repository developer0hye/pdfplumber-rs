//! Duplicate character deduplication.
//!
//! Removes duplicate overlapping characters that some PDF generators output
//! (for bold effect or due to bugs). Reference: pdfplumber(Py) `Page.dedupe_chars()`.

use crate::text::Char;

/// Options for duplicate character detection and removal.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DedupeOptions {
    /// Maximum distance (in points) between character positions to consider
    /// them as duplicates. Default: `1.0`.
    pub tolerance: f64,
    /// Additional character attributes that must match for two characters to be
    /// considered duplicates. Default: `["fontname", "size"]`.
    ///
    /// Supported attribute names: `"fontname"`, `"size"`, `"upright"`,
    /// `"stroking_color"`, `"non_stroking_color"`.
    pub extra_attrs: Vec<String>,
}

impl Default for DedupeOptions {
    fn default() -> Self {
        Self {
            tolerance: 1.0,
            extra_attrs: vec!["fontname".to_string(), "size".to_string()],
        }
    }
}

/// Returns whether two characters match on the given attribute name.
fn attrs_match(a: &Char, b: &Char, attr: &str) -> bool {
    match attr {
        "fontname" => a.fontname == b.fontname,
        "size" => (a.size - b.size).abs() < f64::EPSILON,
        "upright" => a.upright == b.upright,
        "stroking_color" => a.stroking_color == b.stroking_color,
        "non_stroking_color" => a.non_stroking_color == b.non_stroking_color,
        _ => true, // Unknown attributes are ignored (treated as matching)
    }
}

/// Returns whether two characters are duplicates according to the given options.
///
/// Two characters are considered duplicates if:
/// 1. They have the same text content
/// 2. Their positions (x0, top) are within the tolerance
/// 3. All specified extra attributes match
fn is_duplicate(a: &Char, b: &Char, options: &DedupeOptions) -> bool {
    // Must have the same text
    if a.text != b.text {
        return false;
    }

    // Positions must be within tolerance
    let dx = (a.bbox.x0 - b.bbox.x0).abs();
    let dy = (a.bbox.top - b.bbox.top).abs();
    if dx > options.tolerance || dy > options.tolerance {
        return false;
    }

    // All extra attributes must match
    options
        .extra_attrs
        .iter()
        .all(|attr| attrs_match(a, b, attr))
}

/// Remove duplicate overlapping characters from a slice.
///
/// Iterates through characters in order, keeping the first occurrence and
/// discarding subsequent duplicates. Two characters are duplicates if their
/// positions overlap within `tolerance` and the specified `extra_attrs` match.
///
/// The original slice is not modified; a new `Vec<Char>` is returned.
pub fn dedupe_chars(chars: &[Char], options: &DedupeOptions) -> Vec<Char> {
    let mut kept: Vec<Char> = Vec::with_capacity(chars.len());

    for ch in chars {
        let dominated = kept.iter().any(|k| is_duplicate(k, ch, options));
        if !dominated {
            kept.push(ch.clone());
        }
    }

    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::BBox;
    use crate::painting::Color;
    use crate::text::TextDirection;

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "Helvetica".to_string(),
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

    fn make_char_with_font(text: &str, x0: f64, top: f64, fontname: &str, size: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x0 + 10.0, top + 12.0),
            fontname: fontname.to_string(),
            size,
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
    fn test_overlapping_identical_chars_deduped() {
        // Two "A" chars at nearly the same position — should be deduped to one
        let chars = vec![
            make_char("A", 10.0, 20.0, 20.0, 32.0),
            make_char("A", 10.5, 20.3, 20.5, 32.3), // within tolerance=1.0
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "A");
        // First occurrence is kept
        assert!((result[0].bbox.x0 - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_non_overlapping_chars_preserved() {
        // Two "A" chars at different positions — should NOT be deduped
        let chars = vec![
            make_char("A", 10.0, 20.0, 20.0, 32.0),
            make_char("A", 50.0, 20.0, 60.0, 32.0), // far apart
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_different_text_not_deduped() {
        // "A" and "B" at the same position — should NOT be deduped
        let chars = vec![
            make_char("A", 10.0, 20.0, 20.0, 32.0),
            make_char("B", 10.0, 20.0, 20.0, 32.0),
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_different_font_not_deduped() {
        // Same text, same position, but different font — should NOT be deduped
        let chars = vec![
            make_char_with_font("A", 10.0, 20.0, "Helvetica", 12.0),
            make_char_with_font("A", 10.0, 20.0, "Times-Roman", 12.0),
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_different_size_not_deduped() {
        // Same text, same position, same font, different size — should NOT be deduped
        let chars = vec![
            make_char_with_font("A", 10.0, 20.0, "Helvetica", 12.0),
            make_char_with_font("A", 10.0, 20.0, "Helvetica", 14.0),
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_custom_tolerance() {
        // Two chars 2.5 points apart — not deduped with default tolerance=1.0
        // but deduped with tolerance=3.0
        let chars = vec![
            make_char("A", 10.0, 20.0, 20.0, 32.0),
            make_char("A", 12.5, 20.0, 22.5, 32.0),
        ];

        let default_result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(
            default_result.len(),
            2,
            "Default tolerance should not merge these"
        );

        let wide_result = dedupe_chars(
            &chars,
            &DedupeOptions {
                tolerance: 3.0,
                ..DedupeOptions::default()
            },
        );
        assert_eq!(wide_result.len(), 1, "Wide tolerance should merge these");
    }

    #[test]
    fn test_empty_extra_attrs() {
        // With no extra_attrs, only text + position matter
        // Different font chars at same position should be deduped
        let chars = vec![
            make_char_with_font("A", 10.0, 20.0, "Helvetica", 12.0),
            make_char_with_font("A", 10.0, 20.0, "Times-Roman", 14.0),
        ];

        let result = dedupe_chars(
            &chars,
            &DedupeOptions {
                tolerance: 1.0,
                extra_attrs: vec![],
            },
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_multiple_duplicates_keep_first() {
        // Three identical chars — should keep only the first
        let chars = vec![
            make_char("A", 10.0, 20.0, 20.0, 32.0),
            make_char("A", 10.2, 20.1, 20.2, 32.1),
            make_char("A", 10.4, 20.2, 20.4, 32.2),
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 1);
        assert!((result[0].bbox.x0 - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mixed_chars_only_duplicates_removed() {
        // "H" "e" "l" "l" "o" with "H" duplicated
        let chars = vec![
            make_char("H", 10.0, 20.0, 20.0, 32.0),
            make_char("H", 10.1, 20.0, 20.1, 32.0), // duplicate of first H
            make_char("e", 20.0, 20.0, 30.0, 32.0),
            make_char("l", 30.0, 20.0, 40.0, 32.0),
            make_char("l", 40.0, 20.0, 50.0, 32.0), // NOT a dup (different position)
            make_char("o", 50.0, 20.0, 60.0, 32.0),
        ];

        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 5);
        let texts: Vec<&str> = result.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, vec!["H", "e", "l", "l", "o"]);
    }

    #[test]
    fn test_empty_input() {
        let result = dedupe_chars(&[], &DedupeOptions::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_char() {
        let chars = vec![make_char("A", 10.0, 20.0, 20.0, 32.0)];
        let result = dedupe_chars(&chars, &DedupeOptions::default());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_color_as_extra_attr() {
        // Two chars at same position, same text, same font, but different fill color
        let mut c1 = make_char("A", 10.0, 20.0, 20.0, 32.0);
        c1.non_stroking_color = Some(Color::Rgb(1.0, 0.0, 0.0));
        let mut c2 = make_char("A", 10.0, 20.0, 20.0, 32.0);
        c2.non_stroking_color = Some(Color::Rgb(0.0, 0.0, 1.0));

        // With default extra_attrs (fontname, size) → deduped (colors not checked)
        let result = dedupe_chars(&[c1.clone(), c2.clone()], &DedupeOptions::default());
        assert_eq!(result.len(), 1);

        // With non_stroking_color in extra_attrs → not deduped
        let result = dedupe_chars(
            &[c1, c2],
            &DedupeOptions {
                tolerance: 1.0,
                extra_attrs: vec![
                    "fontname".to_string(),
                    "size".to_string(),
                    "non_stroking_color".to_string(),
                ],
            },
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_default_options() {
        let opts = DedupeOptions::default();
        assert!((opts.tolerance - 1.0).abs() < f64::EPSILON);
        assert_eq!(opts.extra_attrs, vec!["fontname", "size"]);
    }
}
