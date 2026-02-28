use crate::geometry::BBox;
use crate::painting::Color;

/// A single character extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Char {
    /// The text content of this character.
    pub text: String,
    /// Bounding box in top-left origin coordinates.
    pub bbox: BBox,
    /// Font name.
    pub fontname: String,
    /// Font size in points.
    pub size: f64,
    /// Distance from the top of the first page (accumulates across pages).
    pub doctop: f64,
    /// Whether the character is upright (not rotated).
    pub upright: bool,
    /// Text direction for this character.
    pub direction: TextDirection,
    /// Stroking (outline) color, if any.
    pub stroking_color: Option<Color>,
    /// Non-stroking (fill) color, if any.
    pub non_stroking_color: Option<Color>,
    /// Current transformation matrix `[a, b, c, d, e, f]` at time of rendering.
    pub ctm: [f64; 6],
    /// Raw character code from the PDF content stream.
    pub char_code: u32,
    /// Marked content identifier linking this character to a structure tree element.
    /// Set when the character is inside a marked-content sequence with an MCID.
    pub mcid: Option<u32>,
    /// Structure tag for this character (e.g., "P", "H1", "Span").
    /// Derived from the structure tree element that references this character's MCID.
    pub tag: Option<String>,
}

impl Char {
    /// Resolve the non-stroking (fill) color to RGB.
    ///
    /// Converts the `non_stroking_color` to `Color::Rgb` if possible.
    /// Returns `None` if no color is set or conversion is not possible
    /// (e.g., `Color::Other` with unknown color space).
    pub fn resolved_color(&self) -> Option<Color> {
        self.non_stroking_color
            .as_ref()
            .and_then(|c| c.to_rgb())
            .map(|(r, g, b)| Color::Rgb(r, g, b))
    }
}

/// Text flow direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    fn test_char_creation_basic() {
        let ch = Char {
            text: "A".to_string(),
            bbox: BBox::new(10.0, 20.0, 20.0, 32.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
            doctop: 20.0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 65,
            mcid: None,
            tag: None,
        };
        assert_eq!(ch.text, "A");
        assert_eq!(ch.bbox.x0, 10.0);
        assert_eq!(ch.fontname, "Helvetica");
        assert_eq!(ch.size, 12.0);
        assert_eq!(ch.doctop, 20.0);
        assert!(ch.upright);
        assert_eq!(ch.direction, TextDirection::Ltr);
        assert_eq!(ch.stroking_color, None);
        assert_eq!(ch.non_stroking_color, Some(Color::Gray(0.0)));
        assert_eq!(ch.ctm, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(ch.char_code, 65);
        assert_eq!(ch.mcid, None);
        assert_eq!(ch.tag, None);
    }

    #[test]
    fn test_char_with_colors() {
        let ch = Char {
            text: "B".to_string(),
            bbox: BBox::new(30.0, 20.0, 40.0, 32.0),
            fontname: "Times-Roman".to_string(),
            size: 14.0,
            doctop: 820.0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: Some(Color::Rgb(1.0, 0.0, 0.0)),
            non_stroking_color: Some(Color::Cmyk(0.0, 1.0, 1.0, 0.0)),
            ctm: [2.0, 0.0, 0.0, 2.0, 100.0, 200.0],
            char_code: 66,
            mcid: Some(3),
            tag: Some("P".to_string()),
        };
        assert_eq!(ch.stroking_color, Some(Color::Rgb(1.0, 0.0, 0.0)));
        assert_eq!(ch.non_stroking_color, Some(Color::Cmyk(0.0, 1.0, 1.0, 0.0)));
        assert_eq!(ch.ctm[4], 100.0);
        assert_eq!(ch.ctm[5], 200.0);
        assert_eq!(ch.doctop, 820.0);
        assert_eq!(ch.mcid, Some(3));
        assert_eq!(ch.tag.as_deref(), Some("P"));
    }

    #[test]
    fn test_char_rotated_text() {
        let ch = Char {
            text: "R".to_string(),
            bbox: BBox::new(50.0, 100.0, 62.0, 110.0),
            fontname: "Courier".to_string(),
            size: 10.0,
            doctop: 100.0,
            upright: false,
            direction: TextDirection::Ttb,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [0.0, 1.0, -1.0, 0.0, 50.0, 100.0],
            char_code: 82,
            mcid: None,
            tag: None,
        };
        assert!(!ch.upright);
        assert_eq!(ch.direction, TextDirection::Ttb);
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

    // --- Char::resolved_color tests ---

    fn make_char(non_stroking: Option<Color>) -> Char {
        Char {
            text: "A".to_string(),
            bbox: BBox::new(0.0, 0.0, 10.0, 10.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
            doctop: 0.0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: non_stroking,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 65,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn test_resolved_color_gray_to_rgb() {
        let ch = make_char(Some(Color::Gray(0.5)));
        let resolved = ch.resolved_color();
        assert_eq!(resolved, Some(Color::Rgb(0.5, 0.5, 0.5)));
    }

    #[test]
    fn test_resolved_color_rgb_identity() {
        let ch = make_char(Some(Color::Rgb(1.0, 0.0, 0.0)));
        let resolved = ch.resolved_color();
        assert_eq!(resolved, Some(Color::Rgb(1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_resolved_color_cmyk_to_rgb() {
        let ch = make_char(Some(Color::Cmyk(0.0, 0.0, 0.0, 0.0)));
        let resolved = ch.resolved_color();
        assert_eq!(resolved, Some(Color::Rgb(1.0, 1.0, 1.0)));
    }

    #[test]
    fn test_resolved_color_none() {
        let ch = make_char(None);
        let resolved = ch.resolved_color();
        assert_eq!(resolved, None);
    }

    #[test]
    fn test_resolved_color_other_returns_none() {
        let ch = make_char(Some(Color::Other(vec![0.1])));
        let resolved = ch.resolved_color();
        assert_eq!(resolved, None);
    }
}
