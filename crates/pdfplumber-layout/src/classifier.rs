//! Font and text classifier helpers.
//!
//! Derives body baseline font size from a page's character set and classifies
//! text blocks as headings, body text, or captions.

use std::collections::HashMap;

use pdfplumber_core::Char;

/// Minimum ratio of a block's mean font size to body baseline to qualify as a heading.
const HEADING_SIZE_RATIO: f64 = 1.15;

/// Maximum character count for a block to qualify as a heading (headings are short).
const HEADING_MAX_CHARS: usize = 160;

/// Font classification result for a sequence of characters.
#[derive(Debug, Clone, PartialEq)]
pub struct FontProfile {
    /// Most common (modal) font size across the chars — body baseline.
    pub body_size: f64,
    /// Mean font size for this specific sequence.
    pub mean_size: f64,
    /// Whether any char fontname contains a bold indicator.
    pub has_bold: bool,
    /// Whether any char fontname contains an italic indicator.
    pub has_italic: bool,
}

impl FontProfile {
    /// Build a FontProfile from a slice of chars.
    #[allow(dead_code)] // public API for external callers, not used within the crate
    pub fn from_chars(chars: &[Char]) -> Self {
        if chars.is_empty() {
            return FontProfile {
                body_size: 10.0,
                mean_size: 10.0,
                has_bold: false,
                has_italic: false,
            };
        }
        let mean_size = chars.iter().map(|c| c.size).sum::<f64>() / chars.len() as f64;
        let has_bold = chars
            .iter()
            .any(|c| is_bold_fontname(&c.fontname));
        let has_italic = chars
            .iter()
            .any(|c| is_italic_fontname(&c.fontname));
        // body_size is filled in after whole-page analysis; default to mean
        FontProfile {
            body_size: mean_size,
            mean_size,
            has_bold,
            has_italic,
        }
    }
}

/// Compute the body baseline font size for a page.
///
/// Uses the modal bucket: rounds all sizes to 0.5-pt bins and picks the bucket
/// with the most characters. Returns 10.0 if chars is empty.
pub fn compute_body_baseline(chars: &[Char]) -> f64 {
    if chars.is_empty() {
        return 10.0;
    }
    let mut buckets: HashMap<u32, usize> = HashMap::new();
    for c in chars {
        // Round to nearest 0.5 pt bucket, stored as integer (size * 2)
        let key = (c.size * 2.0).round() as u32;
        *buckets.entry(key).or_insert(0) += 1;
    }
    let modal_key = buckets
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(k, _)| k)
        .unwrap_or(20); // default 10.0 pt
    modal_key as f64 / 2.0
}

/// Returns true if `fontname` indicates a bold font.
pub fn is_bold_fontname(fontname: &str) -> bool {
    let lower = fontname.to_lowercase();
    lower.contains("bold") || lower.contains("-bd") || lower.contains("heavy")
}

/// Returns true if `fontname` indicates an italic font.
#[allow(dead_code)] // public API for external callers, used by FontProfile::from_chars
pub fn is_italic_fontname(fontname: &str) -> bool {
    let lower = fontname.to_lowercase();
    lower.contains("italic") || lower.contains("oblique") || lower.contains("-it")
}

/// Classify whether a text block (described by its chars and total text length)
/// is a heading candidate, given the page body baseline.
///
/// Returns true if the block qualifies as a heading.
pub fn is_heading_candidate(
    block_chars: &[Char],
    text_len: usize,
    body_baseline: f64,
) -> bool {
    if block_chars.is_empty() || text_len == 0 {
        return false;
    }
    // Headings are short
    if text_len > HEADING_MAX_CHARS {
        return false;
    }
    let mean_size = block_chars.iter().map(|c| c.size).sum::<f64>() / block_chars.len() as f64;
    let is_large = mean_size >= body_baseline * HEADING_SIZE_RATIO;
    let has_bold = block_chars.iter().any(|c| is_bold_fontname(&c.fontname));
    // Must be either larger than body OR bold (or both)
    is_large || has_bold
}

/// Compute the mean font size for a slice of chars.
pub fn mean_font_size(chars: &[Char]) -> f64 {
    if chars.is_empty() {
        return 0.0;
    }
    chars.iter().map(|c| c.size).sum::<f64>() / chars.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, TextDirection};

    fn make_char(size: f64, fontname: &str) -> Char {
        Char {
            text: "A".to_string(),
            bbox: BBox::new(0.0, 0.0, size * 0.6, size),
            fontname: fontname.to_string(),
            size,
            doctop: 0.0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 65,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn body_baseline_modal_bucket() {
        // 100 chars at 10pt, 10 chars at 18pt — baseline should be 10
        let mut chars: Vec<Char> = (0..100).map(|_| make_char(10.0, "Helvetica")).collect();
        chars.extend((0..10).map(|_| make_char(18.0, "Helvetica-Bold")));
        let baseline = compute_body_baseline(&chars);
        assert!((baseline - 10.0).abs() < 0.5, "expected ~10pt, got {baseline}");
    }

    #[test]
    fn body_baseline_empty() {
        assert_eq!(compute_body_baseline(&[]), 10.0);
    }

    #[test]
    fn is_bold_fontname_detects_bold() {
        assert!(is_bold_fontname("Helvetica-Bold"));
        assert!(is_bold_fontname("TimesNewRoman,Bold"));
        assert!(is_bold_fontname("ArialHeavy"));
        assert!(!is_bold_fontname("Helvetica"));
    }

    #[test]
    fn is_italic_fontname_detects_italic() {
        assert!(is_italic_fontname("Helvetica-Italic"));
        assert!(is_italic_fontname("Times-Oblique"));
        assert!(!is_italic_fontname("Helvetica"));
    }

    #[test]
    fn heading_candidate_large_size() {
        let chars: Vec<Char> = (0..5).map(|_| make_char(18.0, "Helvetica")).collect();
        // body baseline 10pt → 18 is 1.8x → heading
        assert!(is_heading_candidate(&chars, 20, 10.0));
    }

    #[test]
    fn heading_candidate_bold() {
        let chars: Vec<Char> = (0..5).map(|_| make_char(11.0, "Helvetica-Bold")).collect();
        // size barely above baseline, but bold → heading
        assert!(is_heading_candidate(&chars, 20, 10.0));
    }

    #[test]
    fn heading_candidate_too_long() {
        let chars: Vec<Char> = (0..5).map(|_| make_char(18.0, "Helvetica")).collect();
        // text_len > HEADING_MAX_CHARS → not a heading
        assert!(!is_heading_candidate(&chars, 200, 10.0));
    }

    #[test]
    fn heading_candidate_body_size_regular() {
        let chars: Vec<Char> = (0..5).map(|_| make_char(10.0, "Helvetica")).collect();
        assert!(!is_heading_candidate(&chars, 40, 10.0));
    }

    #[test]
    fn mean_font_size_empty() {
        assert_eq!(mean_font_size(&[]), 0.0);
    }

    #[test]
    fn mean_font_size_uniform() {
        let chars: Vec<Char> = (0..4).map(|_| make_char(12.0, "Helvetica")).collect();
        assert!((mean_font_size(&chars) - 12.0).abs() < 0.001);
    }

    #[test]
    fn mean_font_size_mixed() {
        let chars = vec![make_char(10.0, "Helvetica"), make_char(14.0, "Helvetica")];
        assert!((mean_font_size(&chars) - 12.0).abs() < 0.001);
    }
}
