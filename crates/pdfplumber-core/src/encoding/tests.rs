#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // WinAnsiEncoding tests
    // =========================================================================

    #[test]
    fn win_ansi_ascii_printable() {
        let enc = StandardEncoding::WinAnsi;
        assert_eq!(enc.decode(0x20), Some(' '));
        assert_eq!(enc.decode(0x41), Some('A'));
        assert_eq!(enc.decode(0x5A), Some('Z'));
        assert_eq!(enc.decode(0x61), Some('a'));
        assert_eq!(enc.decode(0x7A), Some('z'));
        assert_eq!(enc.decode(0x30), Some('0'));
        assert_eq!(enc.decode(0x39), Some('9'));
    }

    #[test]
    fn win_ansi_extended_characters() {
        let enc = StandardEncoding::WinAnsi;
        // Windows-1252 extensions (0x80–0x9F)
        assert_eq!(enc.decode(0x80), Some('\u{20AC}')); // Euro
        assert_eq!(enc.decode(0x85), Some('\u{2026}')); // Ellipsis
        assert_eq!(enc.decode(0x93), Some('\u{201C}')); // Left double quote
        assert_eq!(enc.decode(0x94), Some('\u{201D}')); // Right double quote
        assert_eq!(enc.decode(0x96), Some('\u{2013}')); // En dash
        assert_eq!(enc.decode(0x97), Some('\u{2014}')); // Em dash
        assert_eq!(enc.decode(0x99), Some('\u{2122}')); // Trademark
    }

    #[test]
    fn win_ansi_undefined_codes() {
        let enc = StandardEncoding::WinAnsi;
        assert_eq!(enc.decode(0x81), None); // Undefined
        assert_eq!(enc.decode(0x8D), None); // Undefined
        assert_eq!(enc.decode(0x8F), None); // Undefined
        assert_eq!(enc.decode(0x90), None); // Undefined
        assert_eq!(enc.decode(0x9D), None); // Undefined
    }

    #[test]
    fn win_ansi_latin_extended() {
        let enc = StandardEncoding::WinAnsi;
        // ISO 8859-1 upper half
        assert_eq!(enc.decode(0xC0), Some('\u{00C0}')); // À
        assert_eq!(enc.decode(0xC9), Some('\u{00C9}')); // É
        assert_eq!(enc.decode(0xE9), Some('\u{00E9}')); // é
        assert_eq!(enc.decode(0xF1), Some('\u{00F1}')); // ñ
        assert_eq!(enc.decode(0xFC), Some('\u{00FC}')); // ü
        assert_eq!(enc.decode(0xFF), Some('\u{00FF}')); // ÿ
    }

    #[test]
    fn win_ansi_decode_bytes() {
        let enc = StandardEncoding::WinAnsi;
        // "Hello" in ASCII/WinAnsi
        let result = enc.decode_bytes(&[0x48, 0x65, 0x6C, 0x6C, 0x6F]);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn win_ansi_decode_bytes_with_extended() {
        let enc = StandardEncoding::WinAnsi;
        // "café" — 'é' is 0xE9 in WinAnsi
        let result = enc.decode_bytes(&[0x63, 0x61, 0x66, 0xE9]);
        assert_eq!(result, "caf\u{00E9}");
    }

    #[test]
    fn win_ansi_decode_bytes_with_undefined() {
        let enc = StandardEncoding::WinAnsi;
        // Byte 0x81 is undefined, should become U+FFFD
        let result = enc.decode_bytes(&[0x41, 0x81, 0x42]);
        assert_eq!(result, "A\u{FFFD}B");
    }

    // =========================================================================
    // MacRomanEncoding tests
    // =========================================================================

    #[test]
    fn mac_roman_ascii_printable() {
        let enc = StandardEncoding::MacRoman;
        assert_eq!(enc.decode(0x41), Some('A'));
        assert_eq!(enc.decode(0x61), Some('a'));
        assert_eq!(enc.decode(0x20), Some(' '));
    }

    #[test]
    fn mac_roman_extended_characters() {
        let enc = StandardEncoding::MacRoman;
        assert_eq!(enc.decode(0x80), Some('\u{00C4}')); // Ä
        assert_eq!(enc.decode(0x83), Some('\u{00C9}')); // É
        assert_eq!(enc.decode(0x84), Some('\u{00D1}')); // Ñ
        assert_eq!(enc.decode(0x87), Some('\u{00E1}')); // á
        assert_eq!(enc.decode(0x8E), Some('\u{00E9}')); // é
        assert_eq!(enc.decode(0x96), Some('\u{00F1}')); // ñ
        assert_eq!(enc.decode(0xCA), Some('\u{00A0}')); // non-breaking space
        assert_eq!(enc.decode(0xD2), Some('\u{201C}')); // "
        assert_eq!(enc.decode(0xD3), Some('\u{201D}')); // "
        assert_eq!(enc.decode(0xDB), Some('\u{20AC}')); // €
    }

    #[test]
    fn mac_roman_special_symbols() {
        let enc = StandardEncoding::MacRoman;
        assert_eq!(enc.decode(0xA5), Some('\u{2022}')); // Bullet
        assert_eq!(enc.decode(0xB0), Some('\u{221E}')); // Infinity
        assert_eq!(enc.decode(0xB9), Some('\u{03C0}')); // Pi
        assert_eq!(enc.decode(0xC5), Some('\u{2248}')); // Almost equal
        assert_eq!(enc.decode(0xDE), Some('\u{FB01}')); // fi ligature
        assert_eq!(enc.decode(0xDF), Some('\u{FB02}')); // fl ligature
    }

    #[test]
    fn mac_roman_decode_bytes() {
        let enc = StandardEncoding::MacRoman;
        // "Ñ" is 0x84 in MacRoman
        let result = enc.decode_bytes(&[0x84]);
        assert_eq!(result, "\u{00D1}");
    }

    // =========================================================================
    // MacExpertEncoding tests
    // =========================================================================

    #[test]
    fn mac_expert_fractions() {
        let enc = StandardEncoding::MacExpert;
        assert_eq!(enc.decode(0xF1), Some('\u{00BC}')); // ¼
        assert_eq!(enc.decode(0xF2), Some('\u{00BD}')); // ½
        assert_eq!(enc.decode(0xF3), Some('\u{00BE}')); // ¾
        assert_eq!(enc.decode(0xC1), Some('\u{2153}')); // ⅓
        assert_eq!(enc.decode(0xC2), Some('\u{2154}')); // ⅔
    }

    #[test]
    fn mac_expert_superscripts_subscripts() {
        let enc = StandardEncoding::MacExpert;
        // Superscripts
        assert_eq!(enc.decode(0x28), Some('\u{207D}')); // parenleftsuperior
        assert_eq!(enc.decode(0x29), Some('\u{207E}')); // parenrightsuperior
        // Subscripts/Inferiors
        assert_eq!(enc.decode(0xD5), Some('\u{2080}')); // zeroinferior
        assert_eq!(enc.decode(0xD6), Some('\u{2081}')); // oneinferior
        assert_eq!(enc.decode(0xDE), Some('\u{2089}')); // nineinferior
    }

    #[test]
    fn mac_expert_space_and_basic() {
        let enc = StandardEncoding::MacExpert;
        assert_eq!(enc.decode(0x20), Some(' '));
        assert_eq!(enc.decode(0x2C), Some(','));
        assert_eq!(enc.decode(0x2E), Some('.'));
        assert_eq!(enc.decode(0x2F), Some('\u{2044}')); // fraction slash
    }

    #[test]
    fn mac_expert_undefined_codes() {
        let enc = StandardEncoding::MacExpert;
        // Low codes should be undefined (except 0x20)
        assert_eq!(enc.decode(0x00), None);
        assert_eq!(enc.decode(0x01), None);
        assert_eq!(enc.decode(0x10), None);
    }

    // =========================================================================
    // StandardEncoding tests
    // =========================================================================

    #[test]
    fn standard_ascii_letters() {
        let enc = StandardEncoding::Standard;
        assert_eq!(enc.decode(0x41), Some('A'));
        assert_eq!(enc.decode(0x5A), Some('Z'));
        assert_eq!(enc.decode(0x61), Some('a'));
        assert_eq!(enc.decode(0x7A), Some('z'));
    }

    #[test]
    fn standard_differs_from_ascii() {
        let enc = StandardEncoding::Standard;
        // Key differences from ASCII:
        // 0x27 is quoteright (U+2019), not ASCII apostrophe
        assert_eq!(enc.decode(0x27), Some('\u{2019}'));
        // 0x60 is quoteleft (U+2018), not ASCII grave accent
        assert_eq!(enc.decode(0x60), Some('\u{2018}'));
    }

    #[test]
    fn standard_extended_characters() {
        let enc = StandardEncoding::Standard;
        assert_eq!(enc.decode(0xA1), Some('\u{00A1}')); // ¡
        assert_eq!(enc.decode(0xA4), Some('\u{2044}')); // fraction
        assert_eq!(enc.decode(0xAE), Some('\u{FB01}')); // fi
        assert_eq!(enc.decode(0xAF), Some('\u{FB02}')); // fl
        assert_eq!(enc.decode(0xB1), Some('\u{2013}')); // endash
        assert_eq!(enc.decode(0xD0), Some('\u{2014}')); // emdash
        assert_eq!(enc.decode(0xE1), Some('\u{00C6}')); // AE
        assert_eq!(enc.decode(0xF1), Some('\u{00E6}')); // ae
        assert_eq!(enc.decode(0xFA), Some('\u{0153}')); // oe
        assert_eq!(enc.decode(0xFB), Some('\u{00DF}')); // germandbls
    }

    #[test]
    fn standard_undefined_ranges() {
        let enc = StandardEncoding::Standard;
        // 0x80–0x9F are all undefined in StandardEncoding
        for code in 0x80..=0x9F {
            assert_eq!(enc.decode(code), None, "code 0x{code:02X} should be None");
        }
    }

    #[test]
    fn standard_diacritics() {
        let enc = StandardEncoding::Standard;
        assert_eq!(enc.decode(0xC1), Some('\u{0060}')); // grave
        assert_eq!(enc.decode(0xC2), Some('\u{00B4}')); // acute
        assert_eq!(enc.decode(0xC3), Some('\u{02C6}')); // circumflex
        assert_eq!(enc.decode(0xC4), Some('\u{02DC}')); // tilde
        assert_eq!(enc.decode(0xCA), Some('\u{02DA}')); // ring
        assert_eq!(enc.decode(0xCF), Some('\u{02C7}')); // caron
    }

    // =========================================================================
    // FontEncoding tests (Differences array)
    // =========================================================================

    #[test]
    fn font_encoding_from_standard() {
        let enc = FontEncoding::from_standard(StandardEncoding::WinAnsi);
        assert_eq!(enc.decode(0x41), Some('A'));
        assert_eq!(enc.decode(0x80), Some('\u{20AC}'));
    }

    #[test]
    fn font_encoding_differences_override() {
        // Create WinAnsi encoding with differences that override some positions
        let differences = vec![
            (0x41, '\u{0391}'), // Override 'A' (0x41) with Greek Alpha
            (0x42, '\u{0392}'), // Override 'B' (0x42) with Greek Beta
        ];
        let enc =
            FontEncoding::from_standard_with_differences(StandardEncoding::WinAnsi, &differences);

        // Overridden positions
        assert_eq!(enc.decode(0x41), Some('\u{0391}')); // Alpha instead of A
        assert_eq!(enc.decode(0x42), Some('\u{0392}')); // Beta instead of B

        // Non-overridden positions remain unchanged
        assert_eq!(enc.decode(0x43), Some('C'));
        assert_eq!(enc.decode(0x80), Some('\u{20AC}')); // Euro unchanged
    }

    #[test]
    fn font_encoding_differences_fill_undefined() {
        // Fill in a previously undefined code
        let differences = vec![
            (0x81, '\u{2603}'), // Fill undefined 0x81 with snowman
        ];
        let enc =
            FontEncoding::from_standard_with_differences(StandardEncoding::WinAnsi, &differences);

        assert_eq!(enc.decode(0x81), Some('\u{2603}')); // Was None, now snowman
    }

    #[test]
    fn font_encoding_apply_differences_incrementally() {
        let mut enc = FontEncoding::from_standard(StandardEncoding::Standard);
        assert_eq!(enc.decode(0x27), Some('\u{2019}')); // quoteright

        // Apply differences
        enc.apply_differences(&[(0x27, '\'')]); // Override to ASCII apostrophe
        assert_eq!(enc.decode(0x27), Some('\'')); // Now ASCII apostrophe
    }

    #[test]
    fn font_encoding_decode_bytes() {
        let differences = vec![(0xE9, '\u{00E9}')]; // é at 0xE9
        let enc =
            FontEncoding::from_standard_with_differences(StandardEncoding::Standard, &differences);
        let result = enc.decode_bytes(&[0x63, 0x61, 0x66, 0xE9]); // c, a, f, é
        assert_eq!(result, "caf\u{00E9}");
    }

    #[test]
    fn font_encoding_from_custom_table() {
        let mut table = [None; 256];
        table[0x41] = Some('X');
        table[0x42] = Some('Y');
        let enc = FontEncoding::from_table(table);
        assert_eq!(enc.decode(0x41), Some('X'));
        assert_eq!(enc.decode(0x42), Some('Y'));
        assert_eq!(enc.decode(0x43), None);
    }

    // =========================================================================
    // EncodingResolver tests
    // =========================================================================

    #[test]
    fn resolver_default_only() {
        let resolver =
            EncodingResolver::new(FontEncoding::from_standard(StandardEncoding::WinAnsi));

        assert_eq!(resolver.resolve(0x41), Some("A".to_string()));
        assert_eq!(resolver.resolve(0x80), Some("\u{20AC}".to_string()));
        assert_eq!(resolver.resolve(0x81), None); // Undefined
    }

    #[test]
    fn resolver_font_encoding_over_default() {
        let default_enc = FontEncoding::from_standard(StandardEncoding::Standard);
        let font_enc = FontEncoding::from_standard(StandardEncoding::WinAnsi);

        let resolver = EncodingResolver::new(default_enc).with_font_encoding(font_enc);

        // WinAnsi 0x27 = ASCII apostrophe, Standard 0x27 = quoteright
        // Font encoding should win over default
        assert_eq!(resolver.resolve(0x27), Some("'".to_string()));
    }

    #[test]
    fn resolver_to_unicode_highest_priority() {
        let mut to_unicode = HashMap::new();
        to_unicode.insert(0x41, "X".to_string()); // Override 'A' → 'X'

        let resolver =
            EncodingResolver::new(FontEncoding::from_standard(StandardEncoding::WinAnsi))
                .with_to_unicode(to_unicode);

        // ToUnicode overrides everything
        assert_eq!(resolver.resolve(0x41), Some("X".to_string()));
        // But non-overridden codes still fall through
        assert_eq!(resolver.resolve(0x42), Some("B".to_string()));
    }

    #[test]
    fn resolver_full_chain() {
        let mut to_unicode = HashMap::new();
        to_unicode.insert(0x01, "TOUNICODE".to_string());

        let mut font_table = [None; 256];
        font_table[0x02] = Some('F'); // Only in font encoding
        let font_enc = FontEncoding::from_table(font_table);

        let default_enc = FontEncoding::from_standard(StandardEncoding::WinAnsi);

        let resolver = EncodingResolver::new(default_enc)
            .with_font_encoding(font_enc)
            .with_to_unicode(to_unicode);

        // Level 1: ToUnicode
        assert_eq!(resolver.resolve(0x01), Some("TOUNICODE".to_string()));
        // Level 2: Font encoding
        assert_eq!(resolver.resolve(0x02), Some("F".to_string()));
        // Level 3: Default (WinAnsi)
        assert_eq!(resolver.resolve(0x41), Some("A".to_string()));
        // No mapping at any level
        assert_eq!(resolver.resolve(0x81), None); // WinAnsi undefined, no font/tounicode
    }

    #[test]
    fn resolver_to_unicode_multi_char() {
        let mut to_unicode = HashMap::new();
        // ToUnicode can map to multi-character strings (e.g., ligatures)
        to_unicode.insert(0xFB01, "fi".to_string());

        let resolver =
            EncodingResolver::new(FontEncoding::from_standard(StandardEncoding::WinAnsi))
                .with_to_unicode(to_unicode);

        // Multi-byte code
        assert_eq!(resolver.resolve(0xFB01), Some("fi".to_string()));
    }

    #[test]
    fn resolver_decode_bytes() {
        let mut to_unicode = HashMap::new();
        to_unicode.insert(0x41, "X".to_string()); // A → X

        let resolver =
            EncodingResolver::new(FontEncoding::from_standard(StandardEncoding::WinAnsi))
                .with_to_unicode(to_unicode);

        let result = resolver.decode_bytes(&[0x41, 0x42, 0x43]);
        assert_eq!(result, "XBC"); // A overridden to X, B and C from default
    }

    #[test]
    fn resolver_decode_bytes_with_undefined() {
        let resolver =
            EncodingResolver::new(FontEncoding::from_standard(StandardEncoding::WinAnsi));

        let result = resolver.decode_bytes(&[0x41, 0x81, 0x42]);
        assert_eq!(result, "A\u{FFFD}B"); // 0x81 is undefined in WinAnsi
    }

    // =========================================================================
    // Cross-encoding comparison tests
    // =========================================================================

    #[test]
    fn ascii_range_consistent_except_standard() {
        // WinAnsi and MacRoman should agree on 0x20–0x7E (ASCII printable)
        for code in 0x20..=0x7E_u8 {
            let win = StandardEncoding::WinAnsi.decode(code);
            let mac = StandardEncoding::MacRoman.decode(code);
            assert_eq!(win, mac, "WinAnsi and MacRoman disagree at 0x{code:02X}");
        }
    }

    #[test]
    fn standard_encoding_quote_marks_differ() {
        // StandardEncoding uses curly quotes where others use straight
        assert_eq!(StandardEncoding::Standard.decode(0x27), Some('\u{2019}')); // curly
        assert_eq!(StandardEncoding::WinAnsi.decode(0x27), Some('\'')); // straight
        assert_eq!(StandardEncoding::Standard.decode(0x60), Some('\u{2018}')); // curly
        assert_eq!(StandardEncoding::WinAnsi.decode(0x60), Some('`')); // straight
    }

    #[test]
    fn all_encodings_have_space() {
        assert_eq!(StandardEncoding::WinAnsi.decode(0x20), Some(' '));
        assert_eq!(StandardEncoding::MacRoman.decode(0x20), Some(' '));
        assert_eq!(StandardEncoding::MacExpert.decode(0x20), Some(' '));
        assert_eq!(StandardEncoding::Standard.decode(0x20), Some(' '));
    }
}
