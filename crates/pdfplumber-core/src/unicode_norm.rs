//! Unicode normalization for extracted text.
//!
//! Provides [`UnicodeNorm`] enum for selecting normalization form and
//! [`normalize_chars`] for applying normalization to character text.

use unicode_normalization::UnicodeNormalization;

use crate::text::Char;

/// Unicode normalization form to apply to extracted text.
///
/// Different PDF generators may produce different Unicode representations
/// for the same visual text (e.g., composed vs. decomposed accented chars).
/// Normalizing ensures consistent text output regardless of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnicodeNorm {
    /// No normalization (default).
    #[default]
    None,
    /// Canonical Decomposition, followed by Canonical Composition (NFC).
    Nfc,
    /// Canonical Decomposition (NFD).
    Nfd,
    /// Compatibility Decomposition, followed by Canonical Composition (NFKC).
    Nfkc,
    /// Compatibility Decomposition (NFKD).
    Nfkd,
}

impl UnicodeNorm {
    /// Apply this normalization form to the given string.
    ///
    /// Returns the input unchanged if normalization is `None`.
    pub fn normalize(&self, text: &str) -> String {
        match self {
            UnicodeNorm::None => text.to_string(),
            UnicodeNorm::Nfc => text.nfc().collect(),
            UnicodeNorm::Nfd => text.nfd().collect(),
            UnicodeNorm::Nfkc => text.nfkc().collect(),
            UnicodeNorm::Nfkd => text.nfkd().collect(),
        }
    }
}

/// Apply Unicode normalization to the text of each character.
///
/// Returns a new `Vec<Char>` with normalized text values. All other
/// fields (bbox, fontname, size, etc.) are preserved unchanged.
/// If normalization is `None`, the chars are returned as clones.
pub fn normalize_chars(chars: &[Char], norm: &UnicodeNorm) -> Vec<Char> {
    if *norm == UnicodeNorm::None {
        return chars.to_vec();
    }

    chars
        .iter()
        .map(|ch| {
            let mut normalized = ch.clone();
            normalized.text = norm.normalize(&ch.text);
            normalized
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::BBox;
    use crate::text::TextDirection;

    fn make_char(text: &str) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(0.0, 0.0, 10.0, 12.0),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: 0.0,
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
    fn unicode_norm_default_is_none() {
        assert_eq!(UnicodeNorm::default(), UnicodeNorm::None);
    }

    #[test]
    fn normalize_none_returns_unchanged() {
        let text = "caf\u{0065}\u{0301}"; // "café" in NFD (e + combining acute)
        let result = UnicodeNorm::None.normalize(text);
        assert_eq!(result, text);
    }

    #[test]
    fn normalize_nfc_composes_characters() {
        // NFD: "e" + combining acute accent → NFC: "é"
        let decomposed = "caf\u{0065}\u{0301}";
        let result = UnicodeNorm::Nfc.normalize(decomposed);
        assert_eq!(result, "caf\u{00E9}"); // "café" with composed é
    }

    #[test]
    fn normalize_nfd_decomposes_characters() {
        // NFC: "é" → NFD: "e" + combining acute accent
        let composed = "caf\u{00E9}";
        let result = UnicodeNorm::Nfd.normalize(composed);
        assert_eq!(result, "caf\u{0065}\u{0301}");
    }

    #[test]
    fn normalize_nfkc_decomposes_compatibility_and_composes() {
        // NFKC: "ﬁ" (U+FB01 LATIN SMALL LIGATURE FI) → "fi"
        let ligature = "\u{FB01}";
        let result = UnicodeNorm::Nfkc.normalize(ligature);
        assert_eq!(result, "fi");
    }

    #[test]
    fn normalize_nfkd_decomposes_compatibility() {
        // NFKD: "ﬁ" (U+FB01) → "fi"
        let ligature = "\u{FB01}";
        let result = UnicodeNorm::Nfkd.normalize(ligature);
        assert_eq!(result, "fi");
    }

    #[test]
    fn normalize_nfkc_fullwidth_to_ascii() {
        // Fullwidth "Ａ" (U+FF21) → "A"
        let fullwidth = "\u{FF21}";
        let result = UnicodeNorm::Nfkc.normalize(fullwidth);
        assert_eq!(result, "A");
    }

    #[test]
    fn normalize_chars_none_preserves_original() {
        let chars = vec![make_char("caf\u{0065}\u{0301}"), make_char("hello")];
        let result = normalize_chars(&chars, &UnicodeNorm::None);
        assert_eq!(result[0].text, "caf\u{0065}\u{0301}");
        assert_eq!(result[1].text, "hello");
    }

    #[test]
    fn normalize_chars_nfc_composes() {
        let chars = vec![
            make_char("caf\u{0065}\u{0301}"), // decomposed é
            make_char("hello"),
        ];
        let result = normalize_chars(&chars, &UnicodeNorm::Nfc);
        assert_eq!(result[0].text, "caf\u{00E9}"); // composed é
        assert_eq!(result[1].text, "hello"); // unchanged
    }

    #[test]
    fn normalize_chars_preserves_other_fields() {
        let mut ch = make_char("\u{FB01}"); // ﬁ ligature
        ch.fontname = "SpecialFont".to_string();
        ch.size = 14.0;
        ch.bbox = BBox::new(10.0, 20.0, 30.0, 40.0);

        let result = normalize_chars(&[ch], &UnicodeNorm::Nfkc);
        assert_eq!(result[0].text, "fi"); // normalized
        assert_eq!(result[0].fontname, "SpecialFont"); // preserved
        assert!((result[0].size - 14.0).abs() < f64::EPSILON); // preserved
        assert_eq!(result[0].bbox, BBox::new(10.0, 20.0, 30.0, 40.0)); // preserved
    }

    #[test]
    fn normalize_chars_empty_input() {
        let result = normalize_chars(&[], &UnicodeNorm::Nfc);
        assert!(result.is_empty());
    }
}
