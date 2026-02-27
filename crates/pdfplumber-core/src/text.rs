use crate::geometry::BBox;

/// A single character extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
pub struct Char {
    /// The text content of this character.
    pub text: String,
    /// Bounding box in top-left origin coordinates.
    pub bbox: BBox,
    /// Font name.
    pub fontname: String,
    /// Font size in points.
    pub size: f64,
}

/// Text flow direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDirection {
    /// Left-to-right (default for Latin, CJK horizontal).
    #[default]
    Ltr,
    /// Right-to-left (Arabic, Hebrew).
    Rtl,
    /// Top-to-bottom (CJK vertical writing).
    Ttb,
    /// Bottom-to-top.
    Btt,
}

/// Returns `true` if the character is a CJK ideograph, syllable, or kana.
///
/// Covers the main Unicode blocks used by Chinese, Japanese, and Korean text:
/// - CJK Unified Ideographs (U+4E00–U+9FFF)
/// - CJK Extension A (U+3400–U+4DBF)
/// - CJK Extension B (U+20000–U+2A6DF)
/// - CJK Compatibility Ideographs (U+F900–U+FAFF)
/// - Hiragana (U+3040–U+309F)
/// - Katakana (U+30A0–U+30FF)
/// - Hangul Syllables (U+AC00–U+D7AF)
/// - Hangul Jamo (U+1100–U+11FF)
/// - Bopomofo (U+3100–U+312F)
/// - CJK Radicals Supplement (U+2E80–U+2EFF)
/// - Kangxi Radicals (U+2F00–U+2FDF)
pub fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{1100}'..='\u{11FF}' // Hangul Jamo
        | '\u{3100}'..='\u{312F}' // Bopomofo
        | '\u{2E80}'..='\u{2EFF}' // CJK Radicals Supplement
        | '\u{2F00}'..='\u{2FDF}' // Kangxi Radicals
        | '\u{20000}'..='\u{2A6DF}' // CJK Extension B
    )
}

/// Returns `true` if the first character of the text is CJK.
pub fn is_cjk_text(text: &str) -> bool {
    text.chars().next().is_some_and(is_cjk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_creation() {
        let ch = Char {
            text: "A".to_string(),
            bbox: BBox::new(10.0, 20.0, 20.0, 32.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
        };
        assert_eq!(ch.text, "A");
        assert_eq!(ch.bbox.x0, 10.0);
        assert_eq!(ch.fontname, "Helvetica");
        assert_eq!(ch.size, 12.0);
    }

    #[test]
    fn test_text_direction_default() {
        let dir = TextDirection::default();
        assert_eq!(dir, TextDirection::Ltr);
    }

    #[test]
    fn test_is_cjk_chinese() {
        assert!(is_cjk('中'));
        assert!(is_cjk('国'));
        assert!(is_cjk('人'));
    }

    #[test]
    fn test_is_cjk_japanese_hiragana() {
        assert!(is_cjk('あ'));
        assert!(is_cjk('い'));
    }

    #[test]
    fn test_is_cjk_japanese_katakana() {
        assert!(is_cjk('ア'));
        assert!(is_cjk('イ'));
    }

    #[test]
    fn test_is_cjk_korean() {
        assert!(is_cjk('한'));
        assert!(is_cjk('글'));
    }

    #[test]
    fn test_is_cjk_latin() {
        assert!(!is_cjk('A'));
        assert!(!is_cjk('z'));
        assert!(!is_cjk('0'));
        assert!(!is_cjk(' '));
    }

    #[test]
    fn test_is_cjk_text() {
        assert!(is_cjk_text("中文"));
        assert!(is_cjk_text("한글"));
        assert!(!is_cjk_text("Hello"));
        assert!(!is_cjk_text(""));
    }
}
