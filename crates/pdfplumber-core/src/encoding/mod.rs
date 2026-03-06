//! Standard PDF text encodings and encoding resolution.
//!
//! Implements WinAnsiEncoding, MacRomanEncoding, MacExpertEncoding,
//! StandardEncoding, Differences array handling, and the encoding
//! resolution order: ToUnicode > explicit Encoding > implicit/default.

use std::collections::HashMap;

/// A named standard PDF encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardEncoding {
    /// WinAnsiEncoding — Windows code page 1252 superset.
    WinAnsi,
    /// MacRomanEncoding — Classic Mac OS Roman.
    MacRoman,
    /// MacExpertEncoding — Mac expert glyph set (small caps, fractions, etc.).
    MacExpert,
    /// StandardEncoding — Adobe standard Latin encoding.
    Standard,
}

impl StandardEncoding {
    /// Look up the Unicode character for a given byte code in this encoding.
    ///
    /// Returns `None` for undefined code points (e.g., 0x80–0x8F in WinAnsi
    /// that are undefined, or codes with no mapping).
    pub fn decode(&self, code: u8) -> Option<char> {
        let table = match self {
            StandardEncoding::WinAnsi => &WIN_ANSI_TABLE,
            StandardEncoding::MacRoman => &MAC_ROMAN_TABLE,
            StandardEncoding::MacExpert => &MAC_EXPERT_TABLE,
            StandardEncoding::Standard => &STANDARD_TABLE,
        };
        table[code as usize]
    }

    /// Decode a byte string into a Unicode string using this encoding.
    ///
    /// Bytes with no mapping are replaced with U+FFFD (replacement character).
    pub fn decode_bytes(&self, bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|&b| self.decode(b).unwrap_or('\u{FFFD}'))
            .collect()
    }
}

/// An encoding table that may be a standard encoding modified by a Differences array.
///
/// This represents the /Encoding entry in a PDF font dictionary, which can be:
/// - A name referring to a standard encoding (e.g., /WinAnsiEncoding)
/// - A dictionary with /BaseEncoding + /Differences array
#[derive(Debug, Clone)]
pub struct FontEncoding {
    /// The base encoding table (256 entries, indexed by character code).
    table: [Option<char>; 256],
}

impl FontEncoding {
    /// Create a `FontEncoding` from a standard encoding.
    pub fn from_standard(encoding: StandardEncoding) -> Self {
        let table = match encoding {
            StandardEncoding::WinAnsi => WIN_ANSI_TABLE,
            StandardEncoding::MacRoman => MAC_ROMAN_TABLE,
            StandardEncoding::MacExpert => MAC_EXPERT_TABLE,
            StandardEncoding::Standard => STANDARD_TABLE,
        };
        Self { table }
    }

    /// Create a `FontEncoding` from a standard encoding with Differences applied.
    ///
    /// The `differences` slice contains pairs of `(code, character)` that override
    /// the base encoding. This matches the PDF /Differences array format:
    /// `[code1 /name1 /name2 ... codeN /nameN ...]` where each code starts a run
    /// of consecutive overrides.
    pub fn from_standard_with_differences(
        encoding: StandardEncoding,
        differences: &[(u8, char)],
    ) -> Self {
        let mut enc = Self::from_standard(encoding);
        enc.apply_differences(differences);
        enc
    }

    /// Create a `FontEncoding` from a custom table (256 entries).
    pub fn from_table(table: [Option<char>; 256]) -> Self {
        Self { table }
    }

    /// Apply Differences array overrides to this encoding.
    pub fn apply_differences(&mut self, differences: &[(u8, char)]) {
        for &(code, ch) in differences {
            self.table[code as usize] = Some(ch);
        }
    }

    /// Decode a single byte code to a Unicode character.
    pub fn decode(&self, code: u8) -> Option<char> {
        self.table[code as usize]
    }

    /// Decode a byte string into a Unicode string.
    ///
    /// Bytes with no mapping are replaced with U+FFFD (replacement character).
    pub fn decode_bytes(&self, bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|&b| self.decode(b).unwrap_or('\u{FFFD}'))
            .collect()
    }
}

/// Resolved encoding for a font, following PDF encoding resolution order.
///
/// Resolution order:
/// 1. ToUnicode CMap (highest priority — direct CID/code → Unicode mapping)
/// 2. Explicit /Encoding from font dictionary (standard name or dictionary with Differences)
/// 3. Implicit/default encoding (typically StandardEncoding for Type1, identity for TrueType)
#[derive(Debug, Clone)]
pub struct EncodingResolver {
    /// ToUnicode mappings (code → Unicode string). Highest priority.
    to_unicode: Option<HashMap<u16, String>>,
    /// Explicit font encoding (from /Encoding entry). Second priority.
    font_encoding: Option<FontEncoding>,
    /// Default/fallback encoding. Lowest priority.
    default_encoding: FontEncoding,
}

impl EncodingResolver {
    /// Create a resolver with only a default encoding.
    pub fn new(default_encoding: FontEncoding) -> Self {
        Self {
            to_unicode: None,
            font_encoding: None,
            default_encoding,
        }
    }

    /// Set the ToUnicode CMap mappings (highest priority).
    pub fn with_to_unicode(mut self, to_unicode: HashMap<u16, String>) -> Self {
        self.to_unicode = Some(to_unicode);
        self
    }

    /// Set the explicit font encoding (second priority).
    pub fn with_font_encoding(mut self, encoding: FontEncoding) -> Self {
        self.font_encoding = Some(encoding);
        self
    }

    /// Resolve a character code to a Unicode string.
    ///
    /// Follows the resolution order:
    /// 1. ToUnicode CMap (if present and has mapping for this code)
    /// 2. Explicit font encoding (if present)
    /// 3. Default encoding
    ///
    /// Returns `None` only if no encoding level has a mapping.
    pub fn resolve(&self, code: u16) -> Option<String> {
        // 1. ToUnicode CMap (highest priority)
        if let Some(ref to_unicode) = self.to_unicode {
            if let Some(s) = to_unicode.get(&code) {
                return Some(s.clone());
            }
        }

        // For single-byte codes, try font encoding and default
        if code <= 255 {
            let byte = code as u8;

            // 2. Explicit font encoding
            if let Some(ref enc) = self.font_encoding {
                if let Some(ch) = enc.decode(byte) {
                    return Some(ch.to_string());
                }
            }

            // 3. Default encoding
            if let Some(ch) = self.default_encoding.decode(byte) {
                return Some(ch.to_string());
            }
        }

        None
    }

    /// Decode a byte string using the resolution chain.
    ///
    /// Each byte is resolved independently. Unresolved bytes become U+FFFD.
    pub fn decode_bytes(&self, bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|&b| {
                self.resolve(b as u16)
                    .unwrap_or_else(|| "\u{FFFD}".to_string())
            })
            .collect()
    }
}


mod glyph_names;
pub use glyph_names::glyph_name_to_char;
use glyph_names::{MAC_EXPERT_TABLE, MAC_ROMAN_TABLE, STANDARD_TABLE, WIN_ANSI_TABLE};

#[cfg(test)]
mod tests;
