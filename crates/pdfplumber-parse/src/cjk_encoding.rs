//! CJK encoding support for predefined CMap encodings.
//!
//! Handles decoding of CJK-encoded byte strings in PDF content streams when
//! fonts use predefined CMaps like GBK-EUC-H, ETen-B5-H, 90ms-RKSJ-H, etc.
//! These encodings use variable-length byte sequences (1 or 2 bytes per char)
//! that require encoding-aware decoding for correct text extraction.

use encoding_rs::Encoding;

/// A decoded character from a CJK-encoded byte string.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedChar {
    /// The raw character code from the byte stream (1 or 2 bytes combined).
    pub char_code: u32,
    /// The Unicode string for this character.
    pub unicode: String,
    /// Number of bytes consumed from the input (1 or 2).
    pub byte_len: usize,
}

/// Detect the `encoding_rs` encoding from a predefined CMap name.
///
/// Returns the corresponding `encoding_rs::Encoding` for known CJK CMap names.
/// Returns `None` for Identity-H/V or unknown CMap names.
pub fn encoding_for_cmap(cmap_name: &str) -> Option<&'static Encoding> {
    // Strip -H/-V suffix for matching
    let base = cmap_name
        .strip_suffix("-H")
        .or_else(|| cmap_name.strip_suffix("-V"))
        .unwrap_or(cmap_name);

    match base {
        // Chinese Simplified: GBK/GB2312 encoding
        "GBK-EUC" | "GB-EUC" | "UniGB-UCS2" | "UniGB-UTF16" => Some(encoding_rs::GBK),

        // Chinese Traditional: Big5 encoding
        "B5pc" | "ETen-B5" | "HKscs-B5" | "UniCNS-UCS2" | "UniCNS-UTF16" => Some(encoding_rs::BIG5),

        // Japanese: Shift-JIS encoding
        "90ms-RKSJ" | "90pv-RKSJ" | "83pv-RKSJ" | "78-RKSJ" | "Add-RKSJ" | "Ext-RKSJ" => {
            Some(encoding_rs::SHIFT_JIS)
        }

        // Japanese: EUC-JP encoding
        "EUC" if cmap_name.contains("JIS") || cmap_name.contains("Japan") => {
            Some(encoding_rs::EUC_JP)
        }

        // Korean: EUC-KR encoding
        "KSC-EUC" | "KSCms-UHC" | "UniKS-UCS2" | "UniKS-UTF16" => Some(encoding_rs::EUC_KR),

        // Identity or unknown — not a legacy CJK encoding
        _ => None,
    }
}

/// Decode a CJK-encoded byte string into individual characters with Unicode text.
///
/// For each character in the byte string:
/// - Determines the byte length (1 or 2 bytes) based on the encoding
/// - Extracts the raw character code
/// - Converts to Unicode using `encoding_rs`
///
/// Returns a vector of decoded characters, each with its char code, Unicode text,
/// and byte length.
pub fn decode_cjk_string(bytes: &[u8], encoding: &'static Encoding) -> Vec<DecodedChar> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let byte = bytes[i];

        // Determine if this is a single-byte or double-byte character
        let (char_code, byte_len) = if is_lead_byte(byte, encoding) && i + 1 < bytes.len() {
            // Two-byte character
            let code = u32::from(byte) << 8 | u32::from(bytes[i + 1]);
            (code, 2)
        } else {
            // Single-byte character
            (u32::from(byte), 1)
        };

        // Decode the bytes to Unicode using encoding_rs
        let char_bytes = &bytes[i..i + byte_len];
        let unicode = decode_bytes_to_unicode(char_bytes, encoding);

        result.push(DecodedChar {
            char_code,
            unicode,
            byte_len,
        });

        i += byte_len;
    }

    result
}

/// Check if a byte is a lead byte (first byte of a 2-byte sequence) for the given encoding.
fn is_lead_byte(byte: u8, encoding: &'static Encoding) -> bool {
    if encoding == encoding_rs::GBK {
        // GBK: lead byte range 0x81-0xFE
        (0x81..=0xFE).contains(&byte)
    } else if encoding == encoding_rs::BIG5 {
        // Big5: lead byte range 0x81-0xFE
        (0x81..=0xFE).contains(&byte)
    } else if encoding == encoding_rs::SHIFT_JIS {
        // Shift-JIS: lead byte ranges 0x81-0x9F and 0xE0-0xFC
        (0x81..=0x9F).contains(&byte) || (0xE0..=0xFC).contains(&byte)
    } else if encoding == encoding_rs::EUC_JP {
        // EUC-JP: lead byte range 0xA1-0xFE (and 0x8E for half-width katakana)
        (0xA1..=0xFE).contains(&byte) || byte == 0x8E
    } else if encoding == encoding_rs::EUC_KR {
        // EUC-KR: lead byte range 0x81-0xFE
        (0x81..=0xFE).contains(&byte)
    } else {
        false
    }
}

/// Decode a single character's bytes to a Unicode string using encoding_rs.
fn decode_bytes_to_unicode(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (decoded, _, _) = encoding.decode(bytes);
    decoded.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== encoding_for_cmap tests ==========

    #[test]
    fn gbk_euc_h_returns_gbk_encoding() {
        let enc = encoding_for_cmap("GBK-EUC-H");
        assert_eq!(enc, Some(encoding_rs::GBK));
    }

    #[test]
    fn gbk_euc_v_returns_gbk_encoding() {
        let enc = encoding_for_cmap("GBK-EUC-V");
        assert_eq!(enc, Some(encoding_rs::GBK));
    }

    #[test]
    fn gb_euc_h_returns_gbk_encoding() {
        let enc = encoding_for_cmap("GB-EUC-H");
        assert_eq!(enc, Some(encoding_rs::GBK));
    }

    #[test]
    fn b5pc_h_returns_big5_encoding() {
        let enc = encoding_for_cmap("B5pc-H");
        assert_eq!(enc, Some(encoding_rs::BIG5));
    }

    #[test]
    fn eten_b5_h_returns_big5_encoding() {
        let enc = encoding_for_cmap("ETen-B5-H");
        assert_eq!(enc, Some(encoding_rs::BIG5));
    }

    #[test]
    fn rksj_h_returns_shift_jis_encoding() {
        let enc = encoding_for_cmap("90ms-RKSJ-H");
        assert_eq!(enc, Some(encoding_rs::SHIFT_JIS));
    }

    #[test]
    fn ksc_euc_h_returns_euc_kr_encoding() {
        let enc = encoding_for_cmap("KSC-EUC-H");
        assert_eq!(enc, Some(encoding_rs::EUC_KR));
    }

    #[test]
    fn identity_h_returns_none() {
        assert_eq!(encoding_for_cmap("Identity-H"), None);
    }

    #[test]
    fn identity_v_returns_none() {
        assert_eq!(encoding_for_cmap("Identity-V"), None);
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(encoding_for_cmap("SomeCustomEncoding"), None);
    }

    // ========== is_lead_byte tests ==========

    #[test]
    fn gbk_lead_byte_detection() {
        // 0x81-0xFE are lead bytes
        assert!(is_lead_byte(0x81, encoding_rs::GBK));
        assert!(is_lead_byte(0xB9, encoding_rs::GBK));
        assert!(is_lead_byte(0xFE, encoding_rs::GBK));
        // 0x00-0x80 are NOT lead bytes
        assert!(!is_lead_byte(0x00, encoding_rs::GBK));
        assert!(!is_lead_byte(0x41, encoding_rs::GBK)); // 'A'
        assert!(!is_lead_byte(0x80, encoding_rs::GBK));
    }

    #[test]
    fn shift_jis_lead_byte_detection() {
        assert!(is_lead_byte(0x81, encoding_rs::SHIFT_JIS));
        assert!(is_lead_byte(0x9F, encoding_rs::SHIFT_JIS));
        assert!(is_lead_byte(0xE0, encoding_rs::SHIFT_JIS));
        assert!(is_lead_byte(0xFC, encoding_rs::SHIFT_JIS));
        // Not lead bytes
        assert!(!is_lead_byte(0x41, encoding_rs::SHIFT_JIS));
        assert!(!is_lead_byte(0xA0, encoding_rs::SHIFT_JIS)); // half-width katakana
    }

    // ========== decode_cjk_string tests ==========

    #[test]
    fn decode_gbk_chinese_chars() {
        // 关 = GBK 0xB9D8, 于 = GBK 0xD3DA
        let bytes = vec![0xB9, 0xD8, 0xD3, 0xDA];
        let decoded = decode_cjk_string(&bytes, encoding_rs::GBK);

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].unicode, "关");
        assert_eq!(decoded[0].char_code, 0xB9D8);
        assert_eq!(decoded[0].byte_len, 2);
        assert_eq!(decoded[1].unicode, "于");
        assert_eq!(decoded[1].char_code, 0xD3DA);
        assert_eq!(decoded[1].byte_len, 2);
    }

    #[test]
    fn decode_gbk_mixed_ascii_and_chinese() {
        // "A" (0x41) followed by 关 (0xB9D8)
        let bytes = vec![0x41, 0xB9, 0xD8];
        let decoded = decode_cjk_string(&bytes, encoding_rs::GBK);

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].unicode, "A");
        assert_eq!(decoded[0].char_code, 0x41);
        assert_eq!(decoded[0].byte_len, 1);
        assert_eq!(decoded[1].unicode, "关");
        assert_eq!(decoded[1].char_code, 0xB9D8);
        assert_eq!(decoded[1].byte_len, 2);
    }

    #[test]
    fn decode_gbk_ascii_only() {
        let bytes = b"Hello";
        let decoded = decode_cjk_string(bytes, encoding_rs::GBK);

        assert_eq!(decoded.len(), 5);
        assert_eq!(decoded[0].unicode, "H");
        assert_eq!(decoded[0].char_code, 0x48);
        assert_eq!(decoded[0].byte_len, 1);
    }

    #[test]
    fn decode_gbk_empty() {
        let decoded = decode_cjk_string(&[], encoding_rs::GBK);
        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_gbk_space() {
        // Space should be single byte
        let bytes = vec![0x20];
        let decoded = decode_cjk_string(&bytes, encoding_rs::GBK);

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].unicode, " ");
        assert_eq!(decoded[0].char_code, 0x20);
        assert_eq!(decoded[0].byte_len, 1);
    }

    #[test]
    fn decode_gbk_full_sentence() {
        // 浙 = 0xD5E3, 江 = 0xBDAD
        let bytes = vec![0xD5, 0xE3, 0xBD, 0xAD];
        let decoded = decode_cjk_string(&bytes, encoding_rs::GBK);

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].unicode, "浙");
        assert_eq!(decoded[1].unicode, "江");
    }
}
