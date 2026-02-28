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

/// Resolve a PDF glyph name to its Unicode character.
///
/// Handles:
/// - `uniXXXX` and `uniXXXXXXXX` forms
/// - Common Adobe glyph names (Latin-1 + common symbols)
/// - Single ASCII character names
pub fn glyph_name_to_char(name: &str) -> Option<char> {
    // Handle uniXXXX form
    if let Some(hex) = name.strip_prefix("uni") {
        if hex.len() == 4 || hex.len() == 8 {
            if let Ok(code) = u32::from_str_radix(hex, 16) {
                return char::from_u32(code);
            }
        }
    }

    // Lookup in common glyph names
    GLYPH_NAME_MAP
        .binary_search_by_key(&name, |&(n, _)| n)
        .ok()
        .map(|i| GLYPH_NAME_MAP[i].1)
}

/// Common Adobe glyph names mapped to Unicode characters.
/// Sorted alphabetically for binary search.
static GLYPH_NAME_MAP: &[(&str, char)] = &[
    ("A", 'A'),
    ("AE", '\u{00C6}'),
    ("Aacute", '\u{00C1}'),
    ("Acircumflex", '\u{00C2}'),
    ("Adieresis", '\u{00C4}'),
    ("Agrave", '\u{00C0}'),
    ("Aring", '\u{00C5}'),
    ("Atilde", '\u{00C3}'),
    ("B", 'B'),
    ("C", 'C'),
    ("Ccedilla", '\u{00C7}'),
    ("D", 'D'),
    ("E", 'E'),
    ("Eacute", '\u{00C9}'),
    ("Ecircumflex", '\u{00CA}'),
    ("Edieresis", '\u{00CB}'),
    ("Egrave", '\u{00C8}'),
    ("Eth", '\u{00D0}'),
    ("Euro", '\u{20AC}'),
    ("F", 'F'),
    ("G", 'G'),
    ("H", 'H'),
    ("I", 'I'),
    ("Iacute", '\u{00CD}'),
    ("Icircumflex", '\u{00CE}'),
    ("Idieresis", '\u{00CF}'),
    ("Igrave", '\u{00CC}'),
    ("J", 'J'),
    ("K", 'K'),
    ("L", 'L'),
    ("M", 'M'),
    ("N", 'N'),
    ("Ntilde", '\u{00D1}'),
    ("O", 'O'),
    ("OE", '\u{0152}'),
    ("Oacute", '\u{00D3}'),
    ("Ocircumflex", '\u{00D4}'),
    ("Odieresis", '\u{00D6}'),
    ("Ograve", '\u{00D2}'),
    ("Oslash", '\u{00D8}'),
    ("Otilde", '\u{00D5}'),
    ("P", 'P'),
    ("Q", 'Q'),
    ("R", 'R'),
    ("S", 'S'),
    ("Scaron", '\u{0160}'),
    ("T", 'T'),
    ("Thorn", '\u{00DE}'),
    ("U", 'U'),
    ("Uacute", '\u{00DA}'),
    ("Ucircumflex", '\u{00DB}'),
    ("Udieresis", '\u{00DC}'),
    ("Ugrave", '\u{00D9}'),
    ("V", 'V'),
    ("W", 'W'),
    ("X", 'X'),
    ("Y", 'Y'),
    ("Yacute", '\u{00DD}'),
    ("Ydieresis", '\u{0178}'),
    ("Z", 'Z'),
    ("Zcaron", '\u{017D}'),
    ("a", 'a'),
    ("aacute", '\u{00E1}'),
    ("acircumflex", '\u{00E2}'),
    ("acute", '\u{00B4}'),
    ("adieresis", '\u{00E4}'),
    ("ae", '\u{00E6}'),
    ("agrave", '\u{00E0}'),
    ("ampersand", '&'),
    ("aring", '\u{00E5}'),
    ("asciicircum", '^'),
    ("asciitilde", '~'),
    ("asterisk", '*'),
    ("at", '@'),
    ("atilde", '\u{00E3}'),
    ("b", 'b'),
    ("backslash", '\\'),
    ("bar", '|'),
    ("braceleft", '{'),
    ("braceright", '}'),
    ("bracketleft", '['),
    ("bracketright", ']'),
    ("brokenbar", '\u{00A6}'),
    ("bullet", '\u{2022}'),
    ("c", 'c'),
    ("ccedilla", '\u{00E7}'),
    ("cedilla", '\u{00B8}'),
    ("cent", '\u{00A2}'),
    ("colon", ':'),
    ("comma", ','),
    ("copyright", '\u{00A9}'),
    ("currency", '\u{00A4}'),
    ("d", 'd'),
    ("dagger", '\u{2020}'),
    ("daggerdbl", '\u{2021}'),
    ("degree", '\u{00B0}'),
    ("dieresis", '\u{00A8}'),
    ("divide", '\u{00F7}'),
    ("dollar", '$'),
    ("e", 'e'),
    ("eacute", '\u{00E9}'),
    ("ecircumflex", '\u{00EA}'),
    ("edieresis", '\u{00EB}'),
    ("egrave", '\u{00E8}'),
    ("eight", '8'),
    ("ellipsis", '\u{2026}'),
    ("emdash", '\u{2014}'),
    ("endash", '\u{2013}'),
    ("equal", '='),
    ("eth", '\u{00F0}'),
    ("exclam", '!'),
    ("exclamdown", '\u{00A1}'),
    ("f", 'f'),
    ("fi", '\u{FB01}'),
    ("five", '5'),
    ("fl", '\u{FB02}'),
    ("florin", '\u{0192}'),
    ("four", '4'),
    ("fraction", '\u{2044}'),
    ("g", 'g'),
    ("germandbls", '\u{00DF}'),
    ("grave", '`'),
    ("greater", '>'),
    ("guillemotleft", '\u{00AB}'),
    ("guillemotright", '\u{00BB}'),
    ("guilsinglleft", '\u{2039}'),
    ("guilsinglright", '\u{203A}'),
    ("h", 'h'),
    ("hyphen", '-'),
    ("i", 'i'),
    ("iacute", '\u{00ED}'),
    ("icircumflex", '\u{00EE}'),
    ("idieresis", '\u{00EF}'),
    ("igrave", '\u{00EC}'),
    ("j", 'j'),
    ("k", 'k'),
    ("l", 'l'),
    ("less", '<'),
    ("logicalnot", '\u{00AC}'),
    ("m", 'm'),
    ("macron", '\u{00AF}'),
    ("minus", '\u{2212}'),
    ("mu", '\u{00B5}'),
    ("multiply", '\u{00D7}'),
    ("n", 'n'),
    ("nine", '9'),
    ("ntilde", '\u{00F1}'),
    ("numbersign", '#'),
    ("o", 'o'),
    ("oacute", '\u{00F3}'),
    ("ocircumflex", '\u{00F4}'),
    ("odieresis", '\u{00F6}'),
    ("oe", '\u{0153}'),
    ("ograve", '\u{00F2}'),
    ("one", '1'),
    ("onehalf", '\u{00BD}'),
    ("onequarter", '\u{00BC}'),
    ("onesuperior", '\u{00B9}'),
    ("ordfeminine", '\u{00AA}'),
    ("ordmasculine", '\u{00BA}'),
    ("oslash", '\u{00F8}'),
    ("otilde", '\u{00F5}'),
    ("p", 'p'),
    ("paragraph", '\u{00B6}'),
    ("parenleft", '('),
    ("parenright", ')'),
    ("percent", '%'),
    ("period", '.'),
    ("periodcentered", '\u{00B7}'),
    ("perthousand", '\u{2030}'),
    ("plus", '+'),
    ("plusminus", '\u{00B1}'),
    ("q", 'q'),
    ("question", '?'),
    ("questiondown", '\u{00BF}'),
    ("quotedbl", '"'),
    ("quotedblbase", '\u{201E}'),
    ("quotedblleft", '\u{201C}'),
    ("quotedblright", '\u{201D}'),
    ("quoteleft", '\u{2018}'),
    ("quoteright", '\u{2019}'),
    ("quotesinglbase", '\u{201A}'),
    ("quotesingle", '\''),
    ("r", 'r'),
    ("registered", '\u{00AE}'),
    ("s", 's'),
    ("scaron", '\u{0161}'),
    ("section", '\u{00A7}'),
    ("semicolon", ';'),
    ("seven", '7'),
    ("six", '6'),
    ("slash", '/'),
    ("space", ' '),
    ("sterling", '\u{00A3}'),
    ("t", 't'),
    ("thorn", '\u{00FE}'),
    ("three", '3'),
    ("threequarters", '\u{00BE}'),
    ("threesuperior", '\u{00B3}'),
    ("tilde", '\u{02DC}'),
    ("trademark", '\u{2122}'),
    ("two", '2'),
    ("twosuperior", '\u{00B2}'),
    ("u", 'u'),
    ("uacute", '\u{00FA}'),
    ("ucircumflex", '\u{00FB}'),
    ("udieresis", '\u{00FC}'),
    ("ugrave", '\u{00F9}'),
    ("underscore", '_'),
    ("v", 'v'),
    ("w", 'w'),
    ("x", 'x'),
    ("y", 'y'),
    ("yacute", '\u{00FD}'),
    ("ydieresis", '\u{00FF}'),
    ("yen", '\u{00A5}'),
    ("z", 'z'),
    ("zcaron", '\u{017E}'),
    ("zero", '0'),
];

// =============================================================================
// Encoding tables
// =============================================================================

/// WinAnsiEncoding — based on Windows code page 1252.
///
/// Codes 0x00–0x1F are control characters (mapped to Unicode controls).
/// Codes 0x20–0x7E match ASCII. Codes 0x80–0xFF include extended Latin characters.
/// Some codes (0x81, 0x8D, 0x8F, 0x90, 0x9D) are undefined in the PDF spec.
static WIN_ANSI_TABLE: [Option<char>; 256] = {
    let mut t = [None; 256];
    // 0x00–0x1F: C0 controls
    t[0x00] = Some('\0');
    t[0x01] = Some('\u{0001}');
    t[0x02] = Some('\u{0002}');
    t[0x03] = Some('\u{0003}');
    t[0x04] = Some('\u{0004}');
    t[0x05] = Some('\u{0005}');
    t[0x06] = Some('\u{0006}');
    t[0x07] = Some('\u{0007}');
    t[0x08] = Some('\u{0008}');
    t[0x09] = Some('\t');
    t[0x0A] = Some('\n');
    t[0x0B] = Some('\u{000B}');
    t[0x0C] = Some('\u{000C}');
    t[0x0D] = Some('\r');
    t[0x0E] = Some('\u{000E}');
    t[0x0F] = Some('\u{000F}');
    t[0x10] = Some('\u{0010}');
    t[0x11] = Some('\u{0011}');
    t[0x12] = Some('\u{0012}');
    t[0x13] = Some('\u{0013}');
    t[0x14] = Some('\u{0014}');
    t[0x15] = Some('\u{0015}');
    t[0x16] = Some('\u{0016}');
    t[0x17] = Some('\u{0017}');
    t[0x18] = Some('\u{0018}');
    t[0x19] = Some('\u{0019}');
    t[0x1A] = Some('\u{001A}');
    t[0x1B] = Some('\u{001B}');
    t[0x1C] = Some('\u{001C}');
    t[0x1D] = Some('\u{001D}');
    t[0x1E] = Some('\u{001E}');
    t[0x1F] = Some('\u{001F}');
    // 0x20–0x7E: ASCII printable
    t[0x20] = Some(' ');
    t[0x21] = Some('!');
    t[0x22] = Some('"');
    t[0x23] = Some('#');
    t[0x24] = Some('$');
    t[0x25] = Some('%');
    t[0x26] = Some('&');
    t[0x27] = Some('\'');
    t[0x28] = Some('(');
    t[0x29] = Some(')');
    t[0x2A] = Some('*');
    t[0x2B] = Some('+');
    t[0x2C] = Some(',');
    t[0x2D] = Some('-');
    t[0x2E] = Some('.');
    t[0x2F] = Some('/');
    t[0x30] = Some('0');
    t[0x31] = Some('1');
    t[0x32] = Some('2');
    t[0x33] = Some('3');
    t[0x34] = Some('4');
    t[0x35] = Some('5');
    t[0x36] = Some('6');
    t[0x37] = Some('7');
    t[0x38] = Some('8');
    t[0x39] = Some('9');
    t[0x3A] = Some(':');
    t[0x3B] = Some(';');
    t[0x3C] = Some('<');
    t[0x3D] = Some('=');
    t[0x3E] = Some('>');
    t[0x3F] = Some('?');
    t[0x40] = Some('@');
    t[0x41] = Some('A');
    t[0x42] = Some('B');
    t[0x43] = Some('C');
    t[0x44] = Some('D');
    t[0x45] = Some('E');
    t[0x46] = Some('F');
    t[0x47] = Some('G');
    t[0x48] = Some('H');
    t[0x49] = Some('I');
    t[0x4A] = Some('J');
    t[0x4B] = Some('K');
    t[0x4C] = Some('L');
    t[0x4D] = Some('M');
    t[0x4E] = Some('N');
    t[0x4F] = Some('O');
    t[0x50] = Some('P');
    t[0x51] = Some('Q');
    t[0x52] = Some('R');
    t[0x53] = Some('S');
    t[0x54] = Some('T');
    t[0x55] = Some('U');
    t[0x56] = Some('V');
    t[0x57] = Some('W');
    t[0x58] = Some('X');
    t[0x59] = Some('Y');
    t[0x5A] = Some('Z');
    t[0x5B] = Some('[');
    t[0x5C] = Some('\\');
    t[0x5D] = Some(']');
    t[0x5E] = Some('^');
    t[0x5F] = Some('_');
    t[0x60] = Some('`');
    t[0x61] = Some('a');
    t[0x62] = Some('b');
    t[0x63] = Some('c');
    t[0x64] = Some('d');
    t[0x65] = Some('e');
    t[0x66] = Some('f');
    t[0x67] = Some('g');
    t[0x68] = Some('h');
    t[0x69] = Some('i');
    t[0x6A] = Some('j');
    t[0x6B] = Some('k');
    t[0x6C] = Some('l');
    t[0x6D] = Some('m');
    t[0x6E] = Some('n');
    t[0x6F] = Some('o');
    t[0x70] = Some('p');
    t[0x71] = Some('q');
    t[0x72] = Some('r');
    t[0x73] = Some('s');
    t[0x74] = Some('t');
    t[0x75] = Some('u');
    t[0x76] = Some('v');
    t[0x77] = Some('w');
    t[0x78] = Some('x');
    t[0x79] = Some('y');
    t[0x7A] = Some('z');
    t[0x7B] = Some('{');
    t[0x7C] = Some('|');
    t[0x7D] = Some('}');
    t[0x7E] = Some('~');
    t[0x7F] = None; // DELETE (undefined in WinAnsi)
    // 0x80–0x9F: Windows-1252 extensions
    t[0x80] = Some('\u{20AC}'); // Euro sign
    // 0x81 undefined
    t[0x82] = Some('\u{201A}'); // Single low-9 quotation mark
    t[0x83] = Some('\u{0192}'); // Latin small letter f with hook
    t[0x84] = Some('\u{201E}'); // Double low-9 quotation mark
    t[0x85] = Some('\u{2026}'); // Horizontal ellipsis
    t[0x86] = Some('\u{2020}'); // Dagger
    t[0x87] = Some('\u{2021}'); // Double dagger
    t[0x88] = Some('\u{02C6}'); // Modifier letter circumflex accent
    t[0x89] = Some('\u{2030}'); // Per mille sign
    t[0x8A] = Some('\u{0160}'); // Latin capital letter S with caron
    t[0x8B] = Some('\u{2039}'); // Single left-pointing angle quotation mark
    t[0x8C] = Some('\u{0152}'); // Latin capital ligature OE
    // 0x8D undefined
    t[0x8E] = Some('\u{017D}'); // Latin capital letter Z with caron
    // 0x8F undefined
    // 0x90 undefined
    t[0x91] = Some('\u{2018}'); // Left single quotation mark
    t[0x92] = Some('\u{2019}'); // Right single quotation mark
    t[0x93] = Some('\u{201C}'); // Left double quotation mark
    t[0x94] = Some('\u{201D}'); // Right double quotation mark
    t[0x95] = Some('\u{2022}'); // Bullet
    t[0x96] = Some('\u{2013}'); // En dash
    t[0x97] = Some('\u{2014}'); // Em dash
    t[0x98] = Some('\u{02DC}'); // Small tilde
    t[0x99] = Some('\u{2122}'); // Trade mark sign
    t[0x9A] = Some('\u{0161}'); // Latin small letter s with caron
    t[0x9B] = Some('\u{203A}'); // Single right-pointing angle quotation mark
    t[0x9C] = Some('\u{0153}'); // Latin small ligature oe
    // 0x9D undefined
    t[0x9E] = Some('\u{017E}'); // Latin small letter z with caron
    t[0x9F] = Some('\u{0178}'); // Latin capital letter Y with diaeresis
    // 0xA0–0xFF: ISO 8859-1 upper half
    t[0xA0] = Some('\u{00A0}'); // No-break space
    t[0xA1] = Some('\u{00A1}'); // Inverted exclamation mark
    t[0xA2] = Some('\u{00A2}'); // Cent sign
    t[0xA3] = Some('\u{00A3}'); // Pound sign
    t[0xA4] = Some('\u{00A4}'); // Currency sign
    t[0xA5] = Some('\u{00A5}'); // Yen sign
    t[0xA6] = Some('\u{00A6}'); // Broken bar
    t[0xA7] = Some('\u{00A7}'); // Section sign
    t[0xA8] = Some('\u{00A8}'); // Diaeresis
    t[0xA9] = Some('\u{00A9}'); // Copyright sign
    t[0xAA] = Some('\u{00AA}'); // Feminine ordinal indicator
    t[0xAB] = Some('\u{00AB}'); // Left-pointing double angle quotation mark
    t[0xAC] = Some('\u{00AC}'); // Not sign
    t[0xAD] = Some('\u{00AD}'); // Soft hyphen
    t[0xAE] = Some('\u{00AE}'); // Registered sign
    t[0xAF] = Some('\u{00AF}'); // Macron
    t[0xB0] = Some('\u{00B0}'); // Degree sign
    t[0xB1] = Some('\u{00B1}'); // Plus-minus sign
    t[0xB2] = Some('\u{00B2}'); // Superscript two
    t[0xB3] = Some('\u{00B3}'); // Superscript three
    t[0xB4] = Some('\u{00B4}'); // Acute accent
    t[0xB5] = Some('\u{00B5}'); // Micro sign
    t[0xB6] = Some('\u{00B6}'); // Pilcrow sign
    t[0xB7] = Some('\u{00B7}'); // Middle dot
    t[0xB8] = Some('\u{00B8}'); // Cedilla
    t[0xB9] = Some('\u{00B9}'); // Superscript one
    t[0xBA] = Some('\u{00BA}'); // Masculine ordinal indicator
    t[0xBB] = Some('\u{00BB}'); // Right-pointing double angle quotation mark
    t[0xBC] = Some('\u{00BC}'); // Vulgar fraction one quarter
    t[0xBD] = Some('\u{00BD}'); // Vulgar fraction one half
    t[0xBE] = Some('\u{00BE}'); // Vulgar fraction three quarters
    t[0xBF] = Some('\u{00BF}'); // Inverted question mark
    t[0xC0] = Some('\u{00C0}'); // Latin capital letter A with grave
    t[0xC1] = Some('\u{00C1}'); // Latin capital letter A with acute
    t[0xC2] = Some('\u{00C2}'); // Latin capital letter A with circumflex
    t[0xC3] = Some('\u{00C3}'); // Latin capital letter A with tilde
    t[0xC4] = Some('\u{00C4}'); // Latin capital letter A with diaeresis
    t[0xC5] = Some('\u{00C5}'); // Latin capital letter A with ring above
    t[0xC6] = Some('\u{00C6}'); // Latin capital letter AE
    t[0xC7] = Some('\u{00C7}'); // Latin capital letter C with cedilla
    t[0xC8] = Some('\u{00C8}'); // Latin capital letter E with grave
    t[0xC9] = Some('\u{00C9}'); // Latin capital letter E with acute
    t[0xCA] = Some('\u{00CA}'); // Latin capital letter E with circumflex
    t[0xCB] = Some('\u{00CB}'); // Latin capital letter E with diaeresis
    t[0xCC] = Some('\u{00CC}'); // Latin capital letter I with grave
    t[0xCD] = Some('\u{00CD}'); // Latin capital letter I with acute
    t[0xCE] = Some('\u{00CE}'); // Latin capital letter I with circumflex
    t[0xCF] = Some('\u{00CF}'); // Latin capital letter I with diaeresis
    t[0xD0] = Some('\u{00D0}'); // Latin capital letter Eth
    t[0xD1] = Some('\u{00D1}'); // Latin capital letter N with tilde
    t[0xD2] = Some('\u{00D2}'); // Latin capital letter O with grave
    t[0xD3] = Some('\u{00D3}'); // Latin capital letter O with acute
    t[0xD4] = Some('\u{00D4}'); // Latin capital letter O with circumflex
    t[0xD5] = Some('\u{00D5}'); // Latin capital letter O with tilde
    t[0xD6] = Some('\u{00D6}'); // Latin capital letter O with diaeresis
    t[0xD7] = Some('\u{00D7}'); // Multiplication sign
    t[0xD8] = Some('\u{00D8}'); // Latin capital letter O with stroke
    t[0xD9] = Some('\u{00D9}'); // Latin capital letter U with grave
    t[0xDA] = Some('\u{00DA}'); // Latin capital letter U with acute
    t[0xDB] = Some('\u{00DB}'); // Latin capital letter U with circumflex
    t[0xDC] = Some('\u{00DC}'); // Latin capital letter U with diaeresis
    t[0xDD] = Some('\u{00DD}'); // Latin capital letter Y with acute
    t[0xDE] = Some('\u{00DE}'); // Latin capital letter Thorn
    t[0xDF] = Some('\u{00DF}'); // Latin small letter sharp s
    t[0xE0] = Some('\u{00E0}'); // Latin small letter a with grave
    t[0xE1] = Some('\u{00E1}'); // Latin small letter a with acute
    t[0xE2] = Some('\u{00E2}'); // Latin small letter a with circumflex
    t[0xE3] = Some('\u{00E3}'); // Latin small letter a with tilde
    t[0xE4] = Some('\u{00E4}'); // Latin small letter a with diaeresis
    t[0xE5] = Some('\u{00E5}'); // Latin small letter a with ring above
    t[0xE6] = Some('\u{00E6}'); // Latin small letter ae
    t[0xE7] = Some('\u{00E7}'); // Latin small letter c with cedilla
    t[0xE8] = Some('\u{00E8}'); // Latin small letter e with grave
    t[0xE9] = Some('\u{00E9}'); // Latin small letter e with acute
    t[0xEA] = Some('\u{00EA}'); // Latin small letter e with circumflex
    t[0xEB] = Some('\u{00EB}'); // Latin small letter e with diaeresis
    t[0xEC] = Some('\u{00EC}'); // Latin small letter i with grave
    t[0xED] = Some('\u{00ED}'); // Latin small letter i with acute
    t[0xEE] = Some('\u{00EE}'); // Latin small letter i with circumflex
    t[0xEF] = Some('\u{00EF}'); // Latin small letter i with diaeresis
    t[0xF0] = Some('\u{00F0}'); // Latin small letter eth
    t[0xF1] = Some('\u{00F1}'); // Latin small letter n with tilde
    t[0xF2] = Some('\u{00F2}'); // Latin small letter o with grave
    t[0xF3] = Some('\u{00F3}'); // Latin small letter o with acute
    t[0xF4] = Some('\u{00F4}'); // Latin small letter o with circumflex
    t[0xF5] = Some('\u{00F5}'); // Latin small letter o with tilde
    t[0xF6] = Some('\u{00F6}'); // Latin small letter o with diaeresis
    t[0xF7] = Some('\u{00F7}'); // Division sign
    t[0xF8] = Some('\u{00F8}'); // Latin small letter o with stroke
    t[0xF9] = Some('\u{00F9}'); // Latin small letter u with grave
    t[0xFA] = Some('\u{00FA}'); // Latin small letter u with acute
    t[0xFB] = Some('\u{00FB}'); // Latin small letter u with circumflex
    t[0xFC] = Some('\u{00FC}'); // Latin small letter u with diaeresis
    t[0xFD] = Some('\u{00FD}'); // Latin small letter y with acute
    t[0xFE] = Some('\u{00FE}'); // Latin small letter thorn
    t[0xFF] = Some('\u{00FF}'); // Latin small letter y with diaeresis
    t
};

/// MacRomanEncoding — Classic Macintosh character set.
static MAC_ROMAN_TABLE: [Option<char>; 256] = {
    let mut t = [None; 256];
    // 0x00–0x7E: Same as ASCII
    t[0x00] = Some('\0');
    t[0x01] = Some('\u{0001}');
    t[0x02] = Some('\u{0002}');
    t[0x03] = Some('\u{0003}');
    t[0x04] = Some('\u{0004}');
    t[0x05] = Some('\u{0005}');
    t[0x06] = Some('\u{0006}');
    t[0x07] = Some('\u{0007}');
    t[0x08] = Some('\u{0008}');
    t[0x09] = Some('\t');
    t[0x0A] = Some('\n');
    t[0x0B] = Some('\u{000B}');
    t[0x0C] = Some('\u{000C}');
    t[0x0D] = Some('\r');
    t[0x0E] = Some('\u{000E}');
    t[0x0F] = Some('\u{000F}');
    t[0x10] = Some('\u{0010}');
    t[0x11] = Some('\u{0011}');
    t[0x12] = Some('\u{0012}');
    t[0x13] = Some('\u{0013}');
    t[0x14] = Some('\u{0014}');
    t[0x15] = Some('\u{0015}');
    t[0x16] = Some('\u{0016}');
    t[0x17] = Some('\u{0017}');
    t[0x18] = Some('\u{0018}');
    t[0x19] = Some('\u{0019}');
    t[0x1A] = Some('\u{001A}');
    t[0x1B] = Some('\u{001B}');
    t[0x1C] = Some('\u{001C}');
    t[0x1D] = Some('\u{001D}');
    t[0x1E] = Some('\u{001E}');
    t[0x1F] = Some('\u{001F}');
    t[0x20] = Some(' ');
    t[0x21] = Some('!');
    t[0x22] = Some('"');
    t[0x23] = Some('#');
    t[0x24] = Some('$');
    t[0x25] = Some('%');
    t[0x26] = Some('&');
    t[0x27] = Some('\'');
    t[0x28] = Some('(');
    t[0x29] = Some(')');
    t[0x2A] = Some('*');
    t[0x2B] = Some('+');
    t[0x2C] = Some(',');
    t[0x2D] = Some('-');
    t[0x2E] = Some('.');
    t[0x2F] = Some('/');
    t[0x30] = Some('0');
    t[0x31] = Some('1');
    t[0x32] = Some('2');
    t[0x33] = Some('3');
    t[0x34] = Some('4');
    t[0x35] = Some('5');
    t[0x36] = Some('6');
    t[0x37] = Some('7');
    t[0x38] = Some('8');
    t[0x39] = Some('9');
    t[0x3A] = Some(':');
    t[0x3B] = Some(';');
    t[0x3C] = Some('<');
    t[0x3D] = Some('=');
    t[0x3E] = Some('>');
    t[0x3F] = Some('?');
    t[0x40] = Some('@');
    t[0x41] = Some('A');
    t[0x42] = Some('B');
    t[0x43] = Some('C');
    t[0x44] = Some('D');
    t[0x45] = Some('E');
    t[0x46] = Some('F');
    t[0x47] = Some('G');
    t[0x48] = Some('H');
    t[0x49] = Some('I');
    t[0x4A] = Some('J');
    t[0x4B] = Some('K');
    t[0x4C] = Some('L');
    t[0x4D] = Some('M');
    t[0x4E] = Some('N');
    t[0x4F] = Some('O');
    t[0x50] = Some('P');
    t[0x51] = Some('Q');
    t[0x52] = Some('R');
    t[0x53] = Some('S');
    t[0x54] = Some('T');
    t[0x55] = Some('U');
    t[0x56] = Some('V');
    t[0x57] = Some('W');
    t[0x58] = Some('X');
    t[0x59] = Some('Y');
    t[0x5A] = Some('Z');
    t[0x5B] = Some('[');
    t[0x5C] = Some('\\');
    t[0x5D] = Some(']');
    t[0x5E] = Some('^');
    t[0x5F] = Some('_');
    t[0x60] = Some('`');
    t[0x61] = Some('a');
    t[0x62] = Some('b');
    t[0x63] = Some('c');
    t[0x64] = Some('d');
    t[0x65] = Some('e');
    t[0x66] = Some('f');
    t[0x67] = Some('g');
    t[0x68] = Some('h');
    t[0x69] = Some('i');
    t[0x6A] = Some('j');
    t[0x6B] = Some('k');
    t[0x6C] = Some('l');
    t[0x6D] = Some('m');
    t[0x6E] = Some('n');
    t[0x6F] = Some('o');
    t[0x70] = Some('p');
    t[0x71] = Some('q');
    t[0x72] = Some('r');
    t[0x73] = Some('s');
    t[0x74] = Some('t');
    t[0x75] = Some('u');
    t[0x76] = Some('v');
    t[0x77] = Some('w');
    t[0x78] = Some('x');
    t[0x79] = Some('y');
    t[0x7A] = Some('z');
    t[0x7B] = Some('{');
    t[0x7C] = Some('|');
    t[0x7D] = Some('}');
    t[0x7E] = Some('~');
    t[0x7F] = None; // DELETE
    // 0x80–0xFF: MacRoman extended characters
    t[0x80] = Some('\u{00C4}'); // Ä
    t[0x81] = Some('\u{00C5}'); // Å
    t[0x82] = Some('\u{00C7}'); // Ç
    t[0x83] = Some('\u{00C9}'); // É
    t[0x84] = Some('\u{00D1}'); // Ñ
    t[0x85] = Some('\u{00D6}'); // Ö
    t[0x86] = Some('\u{00DC}'); // Ü
    t[0x87] = Some('\u{00E1}'); // á
    t[0x88] = Some('\u{00E0}'); // à
    t[0x89] = Some('\u{00E2}'); // â
    t[0x8A] = Some('\u{00E4}'); // ä
    t[0x8B] = Some('\u{00E3}'); // ã
    t[0x8C] = Some('\u{00E5}'); // å
    t[0x8D] = Some('\u{00E7}'); // ç
    t[0x8E] = Some('\u{00E9}'); // é
    t[0x8F] = Some('\u{00E8}'); // è
    t[0x90] = Some('\u{00EA}'); // ê
    t[0x91] = Some('\u{00EB}'); // ë
    t[0x92] = Some('\u{00ED}'); // í
    t[0x93] = Some('\u{00EC}'); // ì
    t[0x94] = Some('\u{00EE}'); // î
    t[0x95] = Some('\u{00EF}'); // ï
    t[0x96] = Some('\u{00F1}'); // ñ
    t[0x97] = Some('\u{00F3}'); // ó
    t[0x98] = Some('\u{00F2}'); // ò
    t[0x99] = Some('\u{00F4}'); // ô
    t[0x9A] = Some('\u{00F6}'); // ö
    t[0x9B] = Some('\u{00F5}'); // õ
    t[0x9C] = Some('\u{00FA}'); // ú
    t[0x9D] = Some('\u{00F9}'); // ù
    t[0x9E] = Some('\u{00FB}'); // û
    t[0x9F] = Some('\u{00FC}'); // ü
    t[0xA0] = Some('\u{2020}'); // †
    t[0xA1] = Some('\u{00B0}'); // °
    t[0xA2] = Some('\u{00A2}'); // ¢
    t[0xA3] = Some('\u{00A3}'); // £
    t[0xA4] = Some('\u{00A7}'); // §
    t[0xA5] = Some('\u{2022}'); // •
    t[0xA6] = Some('\u{00B6}'); // ¶
    t[0xA7] = Some('\u{00DF}'); // ß
    t[0xA8] = Some('\u{00AE}'); // ®
    t[0xA9] = Some('\u{00A9}'); // ©
    t[0xAA] = Some('\u{2122}'); // ™
    t[0xAB] = Some('\u{00B4}'); // ´
    t[0xAC] = Some('\u{00A8}'); // ¨
    t[0xAD] = Some('\u{2260}'); // ≠
    t[0xAE] = Some('\u{00C6}'); // Æ
    t[0xAF] = Some('\u{00D8}'); // Ø
    t[0xB0] = Some('\u{221E}'); // ∞
    t[0xB1] = Some('\u{00B1}'); // ±
    t[0xB2] = Some('\u{2264}'); // ≤
    t[0xB3] = Some('\u{2265}'); // ≥
    t[0xB4] = Some('\u{00A5}'); // ¥
    t[0xB5] = Some('\u{00B5}'); // µ
    t[0xB6] = Some('\u{2202}'); // ∂
    t[0xB7] = Some('\u{2211}'); // ∑
    t[0xB8] = Some('\u{220F}'); // ∏
    t[0xB9] = Some('\u{03C0}'); // π
    t[0xBA] = Some('\u{222B}'); // ∫
    t[0xBB] = Some('\u{00AA}'); // ª
    t[0xBC] = Some('\u{00BA}'); // º
    t[0xBD] = Some('\u{2126}'); // Ω
    t[0xBE] = Some('\u{00E6}'); // æ
    t[0xBF] = Some('\u{00F8}'); // ø
    t[0xC0] = Some('\u{00BF}'); // ¿
    t[0xC1] = Some('\u{00A1}'); // ¡
    t[0xC2] = Some('\u{00AC}'); // ¬
    t[0xC3] = Some('\u{221A}'); // √
    t[0xC4] = Some('\u{0192}'); // ƒ
    t[0xC5] = Some('\u{2248}'); // ≈
    t[0xC6] = Some('\u{2206}'); // ∆
    t[0xC7] = Some('\u{00AB}'); // «
    t[0xC8] = Some('\u{00BB}'); // »
    t[0xC9] = Some('\u{2026}'); // …
    t[0xCA] = Some('\u{00A0}'); // non-breaking space
    t[0xCB] = Some('\u{00C0}'); // À
    t[0xCC] = Some('\u{00C3}'); // Ã
    t[0xCD] = Some('\u{00D5}'); // Õ
    t[0xCE] = Some('\u{0152}'); // Œ
    t[0xCF] = Some('\u{0153}'); // œ
    t[0xD0] = Some('\u{2013}'); // –
    t[0xD1] = Some('\u{2014}'); // —
    t[0xD2] = Some('\u{201C}'); // "
    t[0xD3] = Some('\u{201D}'); // "
    t[0xD4] = Some('\u{2018}'); // '
    t[0xD5] = Some('\u{2019}'); // '
    t[0xD6] = Some('\u{00F7}'); // ÷
    t[0xD7] = Some('\u{25CA}'); // ◊
    t[0xD8] = Some('\u{00FF}'); // ÿ
    t[0xD9] = Some('\u{0178}'); // Ÿ
    t[0xDA] = Some('\u{2044}'); // ⁄
    t[0xDB] = Some('\u{20AC}'); // €
    t[0xDC] = Some('\u{2039}'); // ‹
    t[0xDD] = Some('\u{203A}'); // ›
    t[0xDE] = Some('\u{FB01}'); // fi
    t[0xDF] = Some('\u{FB02}'); // fl
    t[0xE0] = Some('\u{2021}'); // ‡
    t[0xE1] = Some('\u{00B7}'); // ·
    t[0xE2] = Some('\u{201A}'); // ‚
    t[0xE3] = Some('\u{201E}'); // „
    t[0xE4] = Some('\u{2030}'); // ‰
    t[0xE5] = Some('\u{00C2}'); // Â
    t[0xE6] = Some('\u{00CA}'); // Ê
    t[0xE7] = Some('\u{00C1}'); // Á
    t[0xE8] = Some('\u{00CB}'); // Ë
    t[0xE9] = Some('\u{00C8}'); // È
    t[0xEA] = Some('\u{00CD}'); // Í
    t[0xEB] = Some('\u{00CE}'); // Î
    t[0xEC] = Some('\u{00CF}'); // Ï
    t[0xED] = Some('\u{00CC}'); // Ì
    t[0xEE] = Some('\u{00D3}'); // Ó
    t[0xEF] = Some('\u{00D4}'); // Ô
    t[0xF0] = Some('\u{F8FF}'); // Apple logo (private use)
    t[0xF1] = Some('\u{00D2}'); // Ò
    t[0xF2] = Some('\u{00DA}'); // Ú
    t[0xF3] = Some('\u{00DB}'); // Û
    t[0xF4] = Some('\u{00D9}'); // Ù
    t[0xF5] = Some('\u{0131}'); // ı (dotless i)
    t[0xF6] = Some('\u{02C6}'); // ˆ
    t[0xF7] = Some('\u{02DC}'); // ˜
    t[0xF8] = Some('\u{00AF}'); // ¯
    t[0xF9] = Some('\u{02D8}'); // ˘
    t[0xFA] = Some('\u{02D9}'); // ˙
    t[0xFB] = Some('\u{02DA}'); // ˚
    t[0xFC] = Some('\u{00B8}'); // ¸
    t[0xFD] = Some('\u{02DD}'); // ˝
    t[0xFE] = Some('\u{02DB}'); // ˛
    t[0xFF] = Some('\u{02C7}'); // ˇ
    t
};

/// MacExpertEncoding — Mac expert character set (small caps, fractions, etc.).
///
/// This encoding is used for expert glyph sets in Type1 fonts. Many positions
/// contain special typographic characters like small caps, fractions, and
/// old-style numerals. Undefined positions are None.
static MAC_EXPERT_TABLE: [Option<char>; 256] = {
    let mut t = [None; 256];
    t[0x20] = Some(' '); // space
    // Expert glyphs — small caps, fractions, special characters
    t[0x21] = Some('\u{F721}'); // exclamsmall
    t[0x22] = Some('\u{F6C9}'); // Hungarumlautsmall
    t[0x23] = Some('\u{F7A2}'); // centoldstyle
    t[0x24] = Some('\u{F724}'); // dollaroldstyle
    t[0x25] = Some('\u{F6DC}'); // dollarsuperior
    t[0x26] = Some('\u{F726}'); // ampersandsmall
    t[0x27] = Some('\u{F7B4}'); // Acutesmall
    t[0x28] = Some('\u{207D}'); // parenleftsuperior
    t[0x29] = Some('\u{207E}'); // parenrightsuperior
    t[0x2A] = Some('\u{2025}'); // twodotenleader
    t[0x2B] = Some('\u{2024}'); // onedotenleader
    t[0x2C] = Some(','); // comma
    t[0x2D] = Some('\u{002D}'); // hyphen
    t[0x2E] = Some('.'); // period
    t[0x2F] = Some('\u{2044}'); // fraction
    t[0x30] = Some('\u{F730}'); // zerooldstyle
    t[0x31] = Some('\u{F731}'); // oneoldstyle
    t[0x32] = Some('\u{F732}'); // twooldstyle
    t[0x33] = Some('\u{F733}'); // threeoldstyle
    t[0x34] = Some('\u{F734}'); // fouroldstyle
    t[0x35] = Some('\u{F735}'); // fiveoldstyle
    t[0x36] = Some('\u{F736}'); // sixoldstyle
    t[0x37] = Some('\u{F737}'); // sevenoldstyle
    t[0x38] = Some('\u{F738}'); // eightoldstyle
    t[0x39] = Some('\u{F739}'); // nineoldstyle
    t[0x3A] = Some(':'); // colon
    t[0x3B] = Some(';'); // semicolon
    // 0x3C–0x3E undefined
    t[0x3F] = Some('\u{F73F}'); // questionsmall
    // 0x40 undefined
    // 0x41–0x5A: undefined
    // 0x5B–0x60: various
    t[0x5B] = Some('\u{F6E2}'); // commainferior
    // 0x5C undefined
    // 0x5D undefined
    // 0x5E undefined
    // 0x5F undefined
    // 0x60 undefined
    t[0x61] = Some('\u{F6F1}'); // Asmall
    t[0x62] = Some('\u{F6F2}'); // Bsmall
    // Small caps A-Z (using Adobe PUA range as per PDF spec)
    t[0x63] = Some('\u{F7A3}'); // Csmall
    t[0x64] = Some('\u{F6F4}'); // Dsmall
    t[0x65] = Some('\u{F6F5}'); // Esmall
    t[0x66] = Some('\u{F6F6}'); // Fsmall
    t[0x67] = Some('\u{F6F7}'); // Gsmall
    t[0x68] = Some('\u{F6F8}'); // Hsmall
    t[0x69] = Some('\u{F6E3}'); // Ismall
    t[0x6A] = Some('\u{F6FA}'); // Jsmall
    t[0x6B] = Some('\u{F6FB}'); // Ksmall
    t[0x6C] = Some('\u{F6FC}'); // Lsmall
    t[0x6D] = Some('\u{F6FD}'); // Msmall
    t[0x6E] = Some('\u{F6FE}'); // Nsmall
    t[0x6F] = Some('\u{F6FF}'); // Osmall
    t[0x70] = Some('\u{F700}'); // Psmall
    t[0x71] = Some('\u{F701}'); // Qsmall
    t[0x72] = Some('\u{F702}'); // Rsmall
    t[0x73] = Some('\u{F703}'); // Ssmall
    t[0x74] = Some('\u{F704}'); // Tsmall
    t[0x75] = Some('\u{F705}'); // Usmall
    t[0x76] = Some('\u{F706}'); // Vsmall
    t[0x77] = Some('\u{F707}'); // Wsmall
    t[0x78] = Some('\u{F708}'); // Xsmall
    t[0x79] = Some('\u{F709}'); // Ysmall
    t[0x7A] = Some('\u{F70A}'); // Zsmall
    t[0x7B] = Some('\u{20A1}'); // colonmonetary
    t[0x7C] = Some('\u{F6DC}'); // onefitted
    t[0x7D] = Some('\u{F6DD}'); // rupiah
    t[0x7E] = Some('\u{F6DE}'); // Tildesmall
    // 0x7F undefined
    // 0x80–0x86 undefined
    t[0x87] = Some('\u{F6E4}'); // exclamdownsmall
    t[0x88] = Some('\u{F7A8}'); // Dieresissmall
    // 0x89 undefined
    t[0x8A] = Some('\u{F6E5}'); // centinferior
    t[0x8B] = Some('\u{F6E6}'); // Lslashsmall
    // 0x8C undefined
    t[0x8D] = Some('\u{F7AF}'); // Macronsmall
    // 0x8E–0x8F undefined
    t[0x90] = Some('\u{F6E7}'); // Scaronsmall
    // 0x91–0x92 undefined
    t[0x93] = Some('\u{F6E8}'); // Zcaronsmall
    // 0x94 undefined
    t[0x95] = Some('\u{F6EA}'); // Dieresissmall (alternate)
    t[0x96] = Some('\u{F7B8}'); // Cedillasmall
    // 0x97–0x99 undefined
    t[0x9A] = Some('\u{F6EB}'); // OEsmall
    t[0x9B] = Some('\u{F6EC}'); // figuredash
    // 0x9C undefined
    t[0x9D] = Some('\u{F6ED}'); // habornarrowi (variant)
    // 0x9E–0x9F undefined
    t[0xA0] = Some('\u{F6EE}'); // spacehackcyrillic (variant)
    t[0xA1] = Some('\u{F6EF}'); // Agravesmall
    t[0xA2] = Some('\u{F6F0}'); // Aacutesmall
    t[0xA3] = Some('\u{F7A3}'); // Acircumflexsmall
    t[0xA4] = Some('\u{F7A4}'); // Atildesmall
    t[0xA5] = Some('\u{F7A5}'); // Adieresissmall
    t[0xA6] = Some('\u{F7A6}'); // Aringsmall
    t[0xA7] = Some('\u{F7A7}'); // AEsmall
    t[0xA8] = Some('\u{F7A9}'); // Ccedillasmall
    t[0xA9] = Some('\u{F7AA}'); // Egravesmall
    t[0xAA] = Some('\u{F7AB}'); // Eacutesmall
    t[0xAB] = Some('\u{F7AC}'); // Ecircumflexsmall
    t[0xAC] = Some('\u{F7AD}'); // Edieresissmall
    t[0xAD] = Some('\u{F7AE}'); // Igravesmall
    t[0xAE] = Some('\u{F7AF}'); // Iacutesmall
    t[0xAF] = Some('\u{F7B0}'); // Icircumflexsmall
    t[0xB0] = Some('\u{F7B1}'); // Idieresissmall
    t[0xB1] = Some('\u{F7B2}'); // Ethsmall
    t[0xB2] = Some('\u{F7B3}'); // Ntildesmall
    t[0xB3] = Some('\u{F7B4}'); // Ogravesmall
    t[0xB4] = Some('\u{F7B5}'); // Oacutesmall
    t[0xB5] = Some('\u{F7B6}'); // Ocircumflexsmall
    t[0xB6] = Some('\u{F7B7}'); // Otildesmall
    t[0xB7] = Some('\u{F7B8}'); // Odieresissmall
    t[0xB8] = Some('\u{F7B9}'); // OEsmall (alternate)
    t[0xB9] = Some('\u{F7BA}'); // Oslashsmall
    t[0xBA] = Some('\u{F7BB}'); // Ugravesmall
    t[0xBB] = Some('\u{F7BC}'); // Uacutesmall
    t[0xBC] = Some('\u{F7BD}'); // Ucircumflexsmall
    t[0xBD] = Some('\u{F7BE}'); // Udieresissmall
    t[0xBE] = Some('\u{F7BF}'); // Yacutesmall
    t[0xBF] = Some('\u{F7C0}'); // Thornsmall
    t[0xC0] = Some('\u{F7C1}'); // Ydieresissmall
    t[0xC1] = Some('\u{2153}'); // onethird
    t[0xC2] = Some('\u{2154}'); // twothirds
    // 0xC3 undefined
    t[0xC4] = Some('\u{215B}'); // oneeighth
    t[0xC5] = Some('\u{215C}'); // threeeighths
    t[0xC6] = Some('\u{215D}'); // fiveeighths
    t[0xC7] = Some('\u{215E}'); // seveneighths
    t[0xC8] = Some('\u{2070}'); // zerosuperior
    // 0xC9 undefined
    t[0xCA] = Some('\u{F6F3}'); // foursuperior
    // 0xCB–0xCC undefined
    t[0xCD] = Some('\u{2074}'); // foursuperior (standard)
    // 0xCE undefined
    t[0xCF] = Some('\u{2075}'); // fivesuperior
    // 0xD0 undefined
    t[0xD1] = Some('\u{2076}'); // sixsuperior
    t[0xD2] = Some('\u{2077}'); // sevensuperior
    t[0xD3] = Some('\u{2078}'); // eightsuperior
    t[0xD4] = Some('\u{2079}'); // ninesuperior
    t[0xD5] = Some('\u{2080}'); // zeroinferior
    t[0xD6] = Some('\u{2081}'); // oneinferior
    t[0xD7] = Some('\u{2082}'); // twoinferior
    t[0xD8] = Some('\u{2083}'); // threeinferior
    t[0xD9] = Some('\u{2084}'); // fourinferior
    t[0xDA] = Some('\u{2085}'); // fiveinferior
    t[0xDB] = Some('\u{2086}'); // sixinferior
    t[0xDC] = Some('\u{2087}'); // seveninferior
    t[0xDD] = Some('\u{2088}'); // eightinferior
    t[0xDE] = Some('\u{2089}'); // nineinferior
    // 0xDF undefined
    t[0xE0] = Some('\u{2215}'); // centinferior (variant) / division slash
    t[0xE1] = Some('\u{F6F4}'); // ff ligature (PUA)
    t[0xE2] = Some('\u{F6F5}'); // fi ligature (PUA)
    t[0xE3] = Some('\u{F6F6}'); // fl ligature (PUA)
    t[0xE4] = Some('\u{F6F7}'); // ffi ligature (PUA)
    t[0xE5] = Some('\u{F6F8}'); // ffl ligature (PUA)
    t[0xE6] = Some('\u{F7E6}'); // parenleftinferior
    // 0xE7 undefined
    t[0xE8] = Some('\u{F7E8}'); // parenrightinferior
    // 0xE9 undefined
    t[0xEA] = Some('\u{F6F9}'); // Circumflexsmall
    t[0xEB] = Some('\u{F6FA}'); // habornarrow
    // 0xEC undefined
    t[0xED] = Some('\u{F6E9}'); // colonmonetary
    // 0xEE–0xEF undefined
    t[0xF0] = Some('\u{F7F0}'); // Gravesmall
    t[0xF1] = Some('\u{00BC}'); // onequarter
    t[0xF2] = Some('\u{00BD}'); // onehalf
    t[0xF3] = Some('\u{00BE}'); // threequarters
    t[0xF4] = Some('\u{215F}'); // fraction1 (variant)
    // 0xF5–0xF7 undefined
    t[0xF8] = Some('\u{F6FA}'); // Commasmall (variant)
    // 0xF9–0xFF undefined
    t
};

/// StandardEncoding — Adobe standard Latin character encoding (PDF Reference Table D.1).
///
/// This is the default encoding for Type1 fonts when no explicit /Encoding is specified.
static STANDARD_TABLE: [Option<char>; 256] = {
    let mut t = [None; 256];
    // 0x20–0x7E: mostly ASCII but with some differences
    t[0x20] = Some(' '); // space
    t[0x21] = Some('!'); // exclam
    t[0x22] = Some('"'); // quotedbl
    t[0x23] = Some('#'); // numbersign
    t[0x24] = Some('$'); // dollar
    t[0x25] = Some('%'); // percent
    t[0x26] = Some('&'); // ampersand
    t[0x27] = Some('\u{2019}'); // quoteright (right single quote, NOT ASCII apostrophe)
    t[0x28] = Some('('); // parenleft
    t[0x29] = Some(')'); // parenright
    t[0x2A] = Some('*'); // asterisk
    t[0x2B] = Some('+'); // plus
    t[0x2C] = Some(','); // comma
    t[0x2D] = Some('-'); // hyphen
    t[0x2E] = Some('.'); // period
    t[0x2F] = Some('/'); // slash
    t[0x30] = Some('0'); // zero
    t[0x31] = Some('1'); // one
    t[0x32] = Some('2'); // two
    t[0x33] = Some('3'); // three
    t[0x34] = Some('4'); // four
    t[0x35] = Some('5'); // five
    t[0x36] = Some('6'); // six
    t[0x37] = Some('7'); // seven
    t[0x38] = Some('8'); // eight
    t[0x39] = Some('9'); // nine
    t[0x3A] = Some(':'); // colon
    t[0x3B] = Some(';'); // semicolon
    t[0x3C] = Some('<'); // less
    t[0x3D] = Some('='); // equal
    t[0x3E] = Some('>'); // greater
    t[0x3F] = Some('?'); // question
    t[0x40] = Some('@'); // at
    t[0x41] = Some('A');
    t[0x42] = Some('B');
    t[0x43] = Some('C');
    t[0x44] = Some('D');
    t[0x45] = Some('E');
    t[0x46] = Some('F');
    t[0x47] = Some('G');
    t[0x48] = Some('H');
    t[0x49] = Some('I');
    t[0x4A] = Some('J');
    t[0x4B] = Some('K');
    t[0x4C] = Some('L');
    t[0x4D] = Some('M');
    t[0x4E] = Some('N');
    t[0x4F] = Some('O');
    t[0x50] = Some('P');
    t[0x51] = Some('Q');
    t[0x52] = Some('R');
    t[0x53] = Some('S');
    t[0x54] = Some('T');
    t[0x55] = Some('U');
    t[0x56] = Some('V');
    t[0x57] = Some('W');
    t[0x58] = Some('X');
    t[0x59] = Some('Y');
    t[0x5A] = Some('Z');
    t[0x5B] = Some('['); // bracketleft
    t[0x5C] = Some('\\'); // backslash
    t[0x5D] = Some(']'); // bracketright
    t[0x5E] = Some('^'); // asciicircum
    t[0x5F] = Some('_'); // underscore
    t[0x60] = Some('\u{2018}'); // quoteleft (left single quote, NOT ASCII grave)
    t[0x61] = Some('a');
    t[0x62] = Some('b');
    t[0x63] = Some('c');
    t[0x64] = Some('d');
    t[0x65] = Some('e');
    t[0x66] = Some('f');
    t[0x67] = Some('g');
    t[0x68] = Some('h');
    t[0x69] = Some('i');
    t[0x6A] = Some('j');
    t[0x6B] = Some('k');
    t[0x6C] = Some('l');
    t[0x6D] = Some('m');
    t[0x6E] = Some('n');
    t[0x6F] = Some('o');
    t[0x70] = Some('p');
    t[0x71] = Some('q');
    t[0x72] = Some('r');
    t[0x73] = Some('s');
    t[0x74] = Some('t');
    t[0x75] = Some('u');
    t[0x76] = Some('v');
    t[0x77] = Some('w');
    t[0x78] = Some('x');
    t[0x79] = Some('y');
    t[0x7A] = Some('z');
    t[0x7B] = Some('{'); // braceleft
    t[0x7C] = Some('|'); // bar
    t[0x7D] = Some('}'); // braceright
    t[0x7E] = Some('~'); // asciitilde
    // 0x7F undefined
    // 0x80–0x9F: undefined in StandardEncoding
    // 0xA0–0xFF: Extended characters
    t[0xA1] = Some('\u{00A1}'); // exclamdown
    t[0xA2] = Some('\u{00A2}'); // cent
    t[0xA3] = Some('\u{00A3}'); // sterling
    t[0xA4] = Some('\u{2044}'); // fraction
    t[0xA5] = Some('\u{00A5}'); // yen
    t[0xA6] = Some('\u{0192}'); // florin
    t[0xA7] = Some('\u{00A7}'); // section
    t[0xA8] = Some('\u{00A4}'); // currency
    t[0xA9] = Some('\''); // quotesingle (ASCII apostrophe)
    t[0xAA] = Some('\u{201C}'); // quotedblleft
    t[0xAB] = Some('\u{00AB}'); // guillemotleft
    t[0xAC] = Some('\u{2039}'); // guilsinglleft
    t[0xAD] = Some('\u{203A}'); // guilsinglright
    t[0xAE] = Some('\u{FB01}'); // fi
    t[0xAF] = Some('\u{FB02}'); // fl
    // 0xB0 undefined
    t[0xB1] = Some('\u{2013}'); // endash
    t[0xB2] = Some('\u{2020}'); // dagger
    t[0xB3] = Some('\u{2021}'); // daggerdbl
    t[0xB4] = Some('\u{00B7}'); // periodcentered
    // 0xB5 undefined
    t[0xB6] = Some('\u{00B6}'); // paragraph
    t[0xB7] = Some('\u{2022}'); // bullet
    t[0xB8] = Some('\u{201A}'); // quotesinglbase
    t[0xB9] = Some('\u{201E}'); // quotedblbase
    t[0xBA] = Some('\u{201D}'); // quotedblright
    t[0xBB] = Some('\u{00BB}'); // guillemotright
    t[0xBC] = Some('\u{2026}'); // ellipsis
    t[0xBD] = Some('\u{2030}'); // perthousand
    // 0xBE undefined
    t[0xBF] = Some('\u{00BF}'); // questiondown
    // 0xC0 undefined
    t[0xC1] = Some('\u{0060}'); // grave
    t[0xC2] = Some('\u{00B4}'); // acute
    t[0xC3] = Some('\u{02C6}'); // circumflex
    t[0xC4] = Some('\u{02DC}'); // tilde
    t[0xC5] = Some('\u{00AF}'); // macron
    t[0xC6] = Some('\u{02D8}'); // breve
    t[0xC7] = Some('\u{02D9}'); // dotaccent
    t[0xC8] = Some('\u{00A8}'); // dieresis
    // 0xC9 undefined
    t[0xCA] = Some('\u{02DA}'); // ring
    t[0xCB] = Some('\u{00B8}'); // cedilla
    // 0xCC undefined
    t[0xCD] = Some('\u{02DD}'); // hungarumlaut
    t[0xCE] = Some('\u{02DB}'); // ogonek
    t[0xCF] = Some('\u{02C7}'); // caron
    t[0xD0] = Some('\u{2014}'); // emdash
    // 0xD1–0xDF undefined
    // 0xE0 undefined
    t[0xE1] = Some('\u{00C6}'); // AE
    // 0xE2 undefined
    t[0xE3] = Some('\u{00AA}'); // ordfeminine
    // 0xE4–0xE7 undefined
    t[0xE8] = Some('\u{0141}'); // Lslash
    t[0xE9] = Some('\u{00D8}'); // Oslash
    t[0xEA] = Some('\u{0152}'); // OE
    t[0xEB] = Some('\u{00BA}'); // ordmasculine
    // 0xEC–0xEF undefined
    // 0xF0 undefined
    t[0xF1] = Some('\u{00E6}'); // ae
    // 0xF2 undefined
    // 0xF3 undefined
    // 0xF4 undefined
    t[0xF5] = Some('\u{0131}'); // dotlessi
    // 0xF6 undefined
    // 0xF7 undefined
    t[0xF8] = Some('\u{0142}'); // lslash
    t[0xF9] = Some('\u{00F8}'); // oslash
    t[0xFA] = Some('\u{0153}'); // oe
    t[0xFB] = Some('\u{00DF}'); // germandbls
    // 0xFC–0xFF undefined
    t
};

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
