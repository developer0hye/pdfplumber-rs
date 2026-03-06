//! ToUnicode CMap parser for mapping character codes to Unicode strings.
//!
//! Parses CMap data embedded in PDF `/ToUnicode` streams to convert glyph codes
//! to Unicode text. Supports `beginbfchar`/`endbfchar` (single mappings) and
//! `beginbfrange`/`endbfrange` (range mappings) with UTF-16BE encoded values.

use std::collections::HashMap;

use crate::error::BackendError;

/// A parsed ToUnicode CMap that maps character codes to Unicode strings.
///
/// Character codes are typically 1 or 2 bytes from the PDF font encoding.
/// Unicode values may be single characters or multi-character strings
/// (e.g., ligatures like "fi" → "fi").
#[derive(Debug, Clone)]
pub struct CMap {
    /// Mapping from character code to Unicode string.
    mappings: HashMap<u32, String>,
    /// When true, unmapped codes are interpreted as Unicode code points directly.
    /// Set when a ToUnicode CMap uses a full-range Identity cidrange
    /// (e.g., `begincidrange <0000> <FFFF> 0 endcidrange`).
    identity: bool,
}

impl CMap {
    /// Parse a ToUnicode CMap from its raw byte content.
    ///
    /// Extracts `beginbfchar`/`endbfchar` and `beginbfrange`/`endbfrange`
    /// sections to build the character code → Unicode mapping table.
    ///
    /// As a fallback, also parses `begincidrange`/`endcidrange` sections,
    /// treating CID values as Unicode code points. This handles ToUnicode
    /// CMaps that use CID-style operators for Identity mappings (e.g.,
    /// `begincidrange <0000> <FFFF> 0 endcidrange`).
    pub fn parse(data: &[u8]) -> Result<Self, BackendError> {
        let text = String::from_utf8_lossy(data);
        let mut mappings = HashMap::new();

        // Parse all beginbfchar...endbfchar sections
        let mut search_from = 0;
        while let Some(start) = text[search_from..].find("beginbfchar") {
            let section_start = search_from + start + "beginbfchar".len();
            if let Some(end) = text[section_start..].find("endbfchar") {
                let section = &text[section_start..section_start + end];
                parse_bfchar_section(section, &mut mappings)?;
                search_from = section_start + end + "endbfchar".len();
            } else {
                break;
            }
        }

        // Parse all beginbfrange...endbfrange sections
        search_from = 0;
        while let Some(start) = text[search_from..].find("beginbfrange") {
            let section_start = search_from + start + "beginbfrange".len();
            if let Some(end) = text[section_start..].find("endbfrange") {
                let section = &text[section_start..section_start + end];
                parse_bfrange_section(section, &mut mappings)?;
                search_from = section_start + end + "endbfrange".len();
            } else {
                break;
            }
        }

        // Fallback: if no bfchar/bfrange mappings found, try cidrange/cidchar
        // sections. Some ToUnicode CMaps use CID-style operators for Identity
        // mappings where code == CID == Unicode code point.
        let mut identity = false;
        if mappings.is_empty() {
            identity = parse_cidrange_as_unicode(&text, &mut mappings);
        }

        Ok(CMap { mappings, identity })
    }

    /// Look up the Unicode string for a character code.
    ///
    /// Returns `None` if the code has no mapping in this CMap.
    /// For Identity CMaps, returns `None` (the caller should use
    /// `char::from_u32` as fallback; see [`is_identity`]).
    pub fn lookup(&self, code: u32) -> Option<&str> {
        self.mappings.get(&code).map(|s| s.as_str())
    }

    /// Returns true if this CMap uses Identity mapping (code == Unicode).
    ///
    /// When true, unmapped codes should be interpreted as Unicode code points
    /// via `char::from_u32` rather than treated as unmapped.
    pub fn is_identity(&self) -> bool {
        self.identity
    }

    /// Look up the Unicode string for a character code, with fallback.
    ///
    /// If no mapping is found, returns U+FFFD (REPLACEMENT CHARACTER).
    pub fn lookup_or_replacement(&self, code: u32) -> String {
        self.lookup(code)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "\u{FFFD}".to_string())
    }

    /// Returns the number of mappings in this CMap.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Returns true if this CMap has no mappings and is not an Identity CMap.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty() && !self.identity
    }
}

/// A parsed CID CMap that maps character codes to CIDs.
///
/// Used by predefined CMaps (e.g., Adobe-Japan1) and embedded CID CMaps
/// that use `begincidchar`/`endcidchar` and `begincidrange`/`endcidrange`
/// sections. Unlike [`CMap`] which maps to Unicode strings, this maps
/// character codes to numeric CID values.
#[derive(Debug, Clone)]
pub struct CidCMap {
    /// Mapping from character code to CID.
    cid_mappings: HashMap<u32, u32>,
    /// CMap name (e.g., "Adobe-Japan1-6").
    name: Option<String>,
    /// Writing mode: 0 = horizontal, 1 = vertical.
    writing_mode: u8,
}

impl CidCMap {
    /// Parse a CID CMap from its raw byte content.
    ///
    /// Extracts `begincidchar`/`endcidchar` and `begincidrange`/`endcidrange`
    /// sections to build the character code → CID mapping table.
    pub fn parse(data: &[u8]) -> Result<Self, BackendError> {
        let text = String::from_utf8_lossy(data);
        let mut cid_mappings = HashMap::new();

        // Parse CMap name
        let name = parse_cmap_name(&text);

        // Parse writing mode (/WMode)
        let writing_mode = parse_writing_mode(&text);

        // Parse all begincidchar...endcidchar sections
        let mut search_from = 0;
        while let Some(start) = text[search_from..].find("begincidchar") {
            let section_start = search_from + start + "begincidchar".len();
            if let Some(end) = text[section_start..].find("endcidchar") {
                let section = &text[section_start..section_start + end];
                parse_cidchar_section(section, &mut cid_mappings)?;
                search_from = section_start + end + "endcidchar".len();
            } else {
                break;
            }
        }

        // Parse all begincidrange...endcidrange sections
        search_from = 0;
        while let Some(start) = text[search_from..].find("begincidrange") {
            let section_start = search_from + start + "begincidrange".len();
            if let Some(end) = text[section_start..].find("endcidrange") {
                let section = &text[section_start..section_start + end];
                parse_cidrange_section(section, &mut cid_mappings)?;
                search_from = section_start + end + "endcidrange".len();
            } else {
                break;
            }
        }

        Ok(CidCMap {
            cid_mappings,
            name,
            writing_mode,
        })
    }

    /// Look up the CID for a character code.
    pub fn lookup(&self, code: u32) -> Option<u32> {
        self.cid_mappings.get(&code).copied()
    }

    /// Returns the number of mappings in this CID CMap.
    pub fn len(&self) -> usize {
        self.cid_mappings.len()
    }

    /// Returns true if this CID CMap has no mappings.
    pub fn is_empty(&self) -> bool {
        self.cid_mappings.is_empty()
    }

    /// CMap name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Writing mode: 0 = horizontal, 1 = vertical.
    pub fn writing_mode(&self) -> u8 {
        self.writing_mode
    }
}

/// Parse begincidrange/endcidrange sections as Unicode mappings.
///
/// Used as a fallback for ToUnicode CMaps that use CID-style operators
/// instead of bfchar/bfrange. Interprets the CID value as a Unicode
/// code point (code + CID_start → char). This handles Identity ToUnicode
/// CMaps like `begincidrange <0000> <FFFF> 0 endcidrange`.
///
/// Returns `true` if a full-range Identity mapping (0-FFFF with CID start 0)
/// was detected, indicating the CMap is an Identity CMap. Full-range mappings
/// are not materialized into the HashMap to avoid excessive memory usage.
fn parse_cidrange_as_unicode(text: &str, mappings: &mut HashMap<u32, String>) -> bool {
    let mut found_identity = false;
    let mut search_from = 0;
    while let Some(start) = text[search_from..].find("begincidrange") {
        let section_start = search_from + start + "begincidrange".len();
        if let Some(end) = text[section_start..].find("endcidrange") {
            let section = &text[section_start..section_start + end];
            for line in section.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || !trimmed.contains('<') {
                    continue;
                }
                let tokens = extract_hex_tokens(trimmed);
                if tokens.len() < 2 {
                    continue;
                }
                let Ok(src_low) = parse_hex_code(tokens[0]) else {
                    continue;
                };
                let Ok(src_high) = parse_hex_code(tokens[1]) else {
                    continue;
                };
                let after_last_hex = trimmed
                    .rfind('>')
                    .map(|pos| &trimmed[pos + 1..])
                    .unwrap_or("");
                let Ok(cid_start) = after_last_hex.trim().parse::<u32>() else {
                    continue;
                };
                // Full-range Identity mapping: mark flag instead of materializing
                if src_low == 0 && src_high >= 0xFFFF && cid_start == 0 {
                    found_identity = true;
                    continue;
                }
                for offset in 0..=(src_high.saturating_sub(src_low)) {
                    let unicode_cp = cid_start + offset;
                    if let Some(ch) = char::from_u32(unicode_cp) {
                        mappings.insert(src_low + offset, ch.to_string());
                    }
                }
            }
            search_from = section_start + end + "endcidrange".len();
        } else {
            break;
        }
    }
    found_identity
}

/// Parse a begincidchar...endcidchar section.
///
/// Each line has format: `<srcCode> CID`
fn parse_cidchar_section(
    section: &str,
    mappings: &mut HashMap<u32, u32>,
) -> Result<(), BackendError> {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('<') {
            continue;
        }

        let tokens = extract_hex_tokens(trimmed);
        if tokens.is_empty() {
            continue;
        }
        let src_code = parse_hex_code(tokens[0])?;

        // CID is a decimal number after the hex token
        let after_hex = trimmed
            .rfind('>')
            .map(|pos| &trimmed[pos + 1..])
            .unwrap_or("");
        if let Ok(cid) = after_hex.trim().parse::<u32>() {
            mappings.insert(src_code, cid);
        }
    }
    Ok(())
}

/// Parse a begincidrange...endcidrange section.
///
/// Each line has format: `<srcLow> <srcHigh> CID_start`
fn parse_cidrange_section(
    section: &str,
    mappings: &mut HashMap<u32, u32>,
) -> Result<(), BackendError> {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('<') {
            continue;
        }

        let tokens = extract_hex_tokens(trimmed);
        if tokens.len() < 2 {
            continue;
        }
        let src_low = parse_hex_code(tokens[0])?;
        let src_high = parse_hex_code(tokens[1])?;

        // CID start is a decimal number after the last hex token
        let after_last_hex = trimmed
            .rfind('>')
            .map(|pos| &trimmed[pos + 1..])
            .unwrap_or("");
        if let Ok(cid_start) = after_last_hex.trim().parse::<u32>() {
            for offset in 0..=(src_high.saturating_sub(src_low)) {
                mappings.insert(src_low + offset, cid_start + offset);
            }
        }
    }
    Ok(())
}

/// Parse /CMapName from CMap data.
fn parse_cmap_name(text: &str) -> Option<String> {
    // Look for "/CMapName /SomeName def"
    let idx = text.find("/CMapName")?;
    let rest = &text[idx + "/CMapName".len()..];
    let rest = rest.trim_start();
    if let Some(rest) = rest.strip_prefix('/') {
        let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        Some(rest[..end].to_string())
    } else {
        None
    }
}

/// Parse /WMode from CMap data.
fn parse_writing_mode(text: &str) -> u8 {
    // Look for "/WMode N def"
    if let Some(idx) = text.find("/WMode") {
        let rest = &text[idx + "/WMode".len()..];
        let rest = rest.trim_start();
        if let Some(ch) = rest.chars().next() {
            if ch == '1' {
                return 1;
            }
        }
    }
    0 // default horizontal
}

/// Parse a hex string like "0041" into a u32 character code.
fn parse_hex_code(hex: &str) -> Result<u32, BackendError> {
    u32::from_str_radix(hex, 16)
        .map_err(|e| BackendError::Parse(format!("invalid hex code '{hex}': {e}")))
}

/// Decode a hex string as UTF-16BE bytes into a Unicode string.
///
/// The hex string represents UTF-16BE encoded code units. For BMP characters,
/// this is a single 2-byte value. For supplementary characters, this is a
/// surrogate pair (4 bytes). For multi-character mappings (ligatures), this
/// can be multiple 2-byte values.
fn decode_utf16be_hex(hex: &str) -> Result<String, BackendError> {
    if hex.len() % 4 != 0 {
        // Pad to even number of hex digits (groups of 4 for UTF-16 code units)
        // For 2-digit hex like "41", treat as single-byte padded to "0041"
        if hex.len() == 2 {
            let padded = format!("00{hex}");
            return decode_utf16be_hex(&padded);
        }
        return Err(BackendError::Parse(format!(
            "UTF-16BE hex string must have length divisible by 4, got '{hex}' (len={})",
            hex.len()
        )));
    }

    // Parse hex string into u16 code units
    let mut code_units = Vec::with_capacity(hex.len() / 4);
    for chunk in hex.as_bytes().chunks(4) {
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| BackendError::Parse(format!("invalid UTF-8 in hex: {e}")))?;
        let unit = u16::from_str_radix(chunk_str, 16).map_err(|e| {
            BackendError::Parse(format!("invalid hex in UTF-16BE '{chunk_str}': {e}"))
        })?;
        code_units.push(unit);
    }

    // Decode UTF-16BE code units to String
    String::from_utf16(&code_units)
        .map_err(|e| BackendError::Parse(format!("invalid UTF-16BE sequence: {e}")))
}

/// Extract all `<hex>` tokens from a line of text.
fn extract_hex_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        if let Some(end) = rest[start + 1..].find('>') {
            let hex = &rest[start + 1..start + 1 + end];
            tokens.push(hex);
            rest = &rest[start + 1 + end + 1..];
        } else {
            break;
        }
    }
    tokens
}

/// Parse a beginbfchar...endbfchar section.
///
/// Each entry has format: `<srcCode> <dstUnicode>`.
/// Entries may be on separate lines or concatenated on a single line
/// (common in CJK PDFs like issue9262).
fn parse_bfchar_section(
    section: &str,
    mappings: &mut HashMap<u32, String>,
) -> Result<(), BackendError> {
    let tokens = extract_hex_tokens(section);
    // Process tokens in pairs: (srcCode, dstUnicode)
    let mut i = 0;
    while i + 1 < tokens.len() {
        let src_code = parse_hex_code(tokens[i])?;
        let unicode_str = decode_utf16be_hex(tokens[i + 1])?;
        mappings.insert(src_code, unicode_str);
        i += 2;
    }
    Ok(())
}

/// Parse a beginbfrange...endbfrange section.
///
/// Each entry has format: `<srcLow> <srcHigh> <dstStart>`
/// or: `<srcLow> <srcHigh> [<str1> <str2> ...]`
///
/// Entries may be on separate lines or concatenated on a single line.
/// When the section contains array entries (`[...]`), those are parsed
/// with bracket-aware logic; otherwise all hex tokens are processed
/// as triples of (srcLow, srcHigh, dstStart).
fn parse_bfrange_section(
    section: &str,
    mappings: &mut HashMap<u32, String>,
) -> Result<(), BackendError> {
    if section.contains('[') {
        // Array form present — parse with bracket-aware line splitting.
        // Split into logical entries by finding [...] boundaries.
        parse_bfrange_with_arrays(section, mappings)
    } else {
        // Standard form only — process all hex tokens in triples.
        let tokens = extract_hex_tokens(section);
        let mut i = 0;
        while i + 2 < tokens.len() {
            let src_low = parse_hex_code(tokens[i])?;
            let src_high = parse_hex_code(tokens[i + 1])?;
            let dst_start = parse_hex_code(tokens[i + 2])?;
            for offset in 0..=(src_high.saturating_sub(src_low)) {
                let code = src_low + offset;
                let unicode_cp = dst_start + offset;
                if let Some(ch) = char::from_u32(unicode_cp) {
                    mappings.insert(code, ch.to_string());
                }
            }
            i += 3;
        }
        Ok(())
    }
}

/// Parse bfrange entries that may contain array destinations `[...]`.
fn parse_bfrange_with_arrays(
    section: &str,
    mappings: &mut HashMap<u32, String>,
) -> Result<(), BackendError> {
    let mut rest = section;
    while !rest.is_empty() {
        // Skip whitespace
        rest = rest.trim_start();
        if rest.is_empty() || !rest.contains('<') {
            break;
        }

        // Extract srcLow and srcHigh
        let src_low_token = match next_hex_token(rest) {
            Some((tok, remaining)) => {
                rest = remaining;
                tok
            }
            None => break,
        };
        let src_high_token = match next_hex_token(rest) {
            Some((tok, remaining)) => {
                rest = remaining;
                tok
            }
            None => break,
        };
        let src_low = parse_hex_code(src_low_token)?;
        let src_high = parse_hex_code(src_high_token)?;

        rest = rest.trim_start();

        if rest.starts_with('[') {
            // Array form: [<str1> <str2> ...]
            let bracket_end = rest.find(']').unwrap_or(rest.len());
            let array_content = &rest[1..bracket_end];
            let dst_tokens = extract_hex_tokens(array_content);
            for (i, dst_hex) in dst_tokens.iter().enumerate() {
                let code = src_low + i as u32;
                if code > src_high {
                    break;
                }
                let unicode_str = decode_utf16be_hex(dst_hex)?;
                mappings.insert(code, unicode_str);
            }
            rest = if bracket_end < rest.len() {
                &rest[bracket_end + 1..]
            } else {
                ""
            };
        } else {
            // Standard form: <dstStart>
            let dst_token = match next_hex_token(rest) {
                Some((tok, remaining)) => {
                    rest = remaining;
                    tok
                }
                None => break,
            };
            let dst_start = parse_hex_code(dst_token)?;
            for offset in 0..=(src_high.saturating_sub(src_low)) {
                let code = src_low + offset;
                let unicode_cp = dst_start + offset;
                if let Some(ch) = char::from_u32(unicode_cp) {
                    mappings.insert(code, ch.to_string());
                }
            }
        }
    }
    Ok(())
}

/// Extract the next `<hex>` token from text, returning the hex content
/// and the remaining text after the closing `>`.
fn next_hex_token(text: &str) -> Option<(&str, &str)> {
    let start = text.find('<')?;
    let end = text[start + 1..].find('>')?;
    let hex = &text[start + 1..start + 1 + end];
    let remaining = &text[start + 1 + end + 1..];
    Some((hex, remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CMap construction and basic lookup ---

    #[test]
    fn empty_cmap_returns_none() {
        let cmap = CMap::parse(b"").unwrap();
        assert!(cmap.is_empty());
        assert_eq!(cmap.len(), 0);
        assert_eq!(cmap.lookup(0x0041), None);
    }

    #[test]
    fn lookup_or_replacement_returns_fffd_for_missing() {
        let cmap = CMap::parse(b"").unwrap();
        assert_eq!(cmap.lookup_or_replacement(0x0041), "\u{FFFD}");
    }

    // --- beginbfchar / endbfchar ---

    #[test]
    fn bfchar_single_mapping() {
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
    }

    #[test]
    fn bfchar_multiple_mappings() {
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            <0042> <0042>\n\
            <0043> <0043>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn bfchar_single_byte_source_code() {
        // 1-byte source code
        let data = b"\
            beginbfchar\n\
            <41> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x41), Some("A"));
    }

    #[test]
    fn bfchar_remapped_codes() {
        // Code 0x01 maps to 'A' (0x0041)
        let data = b"\
            beginbfchar\n\
            <01> <0041>\n\
            <02> <0042>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x01), Some("A"));
        assert_eq!(cmap.lookup(0x02), Some("B"));
    }

    #[test]
    fn bfchar_multi_char_unicode_ligature() {
        // fi ligature → "fi" (two Unicode characters)
        let data = b"\
            beginbfchar\n\
            <FB01> <00660069>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0xFB01), Some("fi"));
    }

    #[test]
    fn bfchar_non_bmp_character() {
        // U+1F600 (😀) encoded as UTF-16BE surrogate pair: D83D DE00
        let data = b"\
            beginbfchar\n\
            <0001> <D83DDE00>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("\u{1F600}"));
    }

    #[test]
    fn bfchar_with_surrounding_cmap_boilerplate() {
        let data = b"\
            /CIDInit /ProcSet findresource begin\n\
            12 dict begin\n\
            begincmap\n\
            /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
            /CMapName /Adobe-Identity-UCS def\n\
            /CMapType 2 def\n\
            1 begincodespacerange\n\
            <0000> <FFFF>\n\
            endcodespacerange\n\
            2 beginbfchar\n\
            <0041> <0041>\n\
            <0042> <0042>\n\
            endbfchar\n\
            endcmap\n\
            CMapName currentdict /CMap defineresource pop\n\
            end\n\
            end\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.len(), 2);
    }

    // --- beginbfrange / endbfrange ---

    #[test]
    fn bfrange_simple_range() {
        let data = b"\
            beginbfrange\n\
            <0041> <0043> <0041>\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn bfrange_offset_mapping() {
        // Source codes 0x01-0x03 map to U+0041-U+0043
        let data = b"\
            beginbfrange\n\
            <01> <03> <0041>\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x01), Some("A"));
        assert_eq!(cmap.lookup(0x02), Some("B"));
        assert_eq!(cmap.lookup(0x03), Some("C"));
    }

    #[test]
    fn bfrange_single_code_range() {
        // Range with low == high (single mapping)
        let data = b"\
            beginbfrange\n\
            <0041> <0041> <0061>\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("a")); // U+0061 = 'a'
        assert_eq!(cmap.len(), 1);
    }

    #[test]
    fn bfrange_multiple_ranges() {
        let data = b"\
            beginbfrange\n\
            <0041> <0043> <0041>\n\
            <0061> <0063> <0061>\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.lookup(0x0061), Some("a"));
        assert_eq!(cmap.lookup(0x0063), Some("c"));
        assert_eq!(cmap.len(), 6);
    }

    #[test]
    fn bfrange_with_array_destination() {
        // Range with array of individual Unicode strings
        let data = b"\
            beginbfrange\n\
            <0041> <0043> [<0058> <0059> <005A>]\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("X"));
        assert_eq!(cmap.lookup(0x0042), Some("Y"));
        assert_eq!(cmap.lookup(0x0043), Some("Z"));
    }

    // --- Combined bfchar + bfrange ---

    #[test]
    fn combined_bfchar_and_bfrange() {
        let data = b"\
            2 beginbfchar\n\
            <0001> <0041>\n\
            <0002> <0042>\n\
            endbfchar\n\
            1 beginbfrange\n\
            <0003> <0005> <0043>\n\
            endbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("A"));
        assert_eq!(cmap.lookup(0x0002), Some("B"));
        assert_eq!(cmap.lookup(0x0003), Some("C"));
        assert_eq!(cmap.lookup(0x0004), Some("D"));
        assert_eq!(cmap.lookup(0x0005), Some("E"));
        assert_eq!(cmap.len(), 5);
    }

    // --- Multiple bfchar/bfrange sections ---

    #[test]
    fn multiple_bfchar_sections() {
        let data = b"\
            1 beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n\
            1 beginbfchar\n\
            <0042> <0042>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.len(), 2);
    }

    // --- UTF-16BE encoding ---

    #[test]
    fn utf16be_basic_latin() {
        // ASCII 'A' is 0x0041 in UTF-16BE
        let data = b"\
            beginbfchar\n\
            <41> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x41), Some("A"));
    }

    #[test]
    fn utf16be_cjk_character() {
        // U+4E2D (中) in UTF-16BE is 4E2D
        let data = b"\
            beginbfchar\n\
            <01> <4E2D>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x01), Some("中"));
    }

    #[test]
    fn utf16be_surrogate_pair() {
        // U+10400 (𐐀) = D801 DC00 in UTF-16BE
        let data = b"\
            beginbfchar\n\
            <01> <D801DC00>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x01), Some("\u{10400}"));
    }

    // --- Edge cases ---

    #[test]
    fn whitespace_variations() {
        // Tabs and extra whitespace
        let data = b"\
            beginbfchar\n\
            \t<0041>\t<0041>\t\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
    }

    #[test]
    fn crlf_line_endings() {
        let data = b"beginbfchar\r\n<0041> <0041>\r\nendbfchar\r\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
    }

    #[test]
    fn missing_mapping_returns_none() {
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x9999), None);
    }

    #[test]
    fn lookup_or_replacement_with_valid_mapping() {
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup_or_replacement(0x0041), "A");
    }

    #[test]
    fn lookup_or_replacement_with_missing_mapping() {
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup_or_replacement(0x9999), "\u{FFFD}");
    }

    // --- CidCMap tests ---

    #[test]
    fn cid_cmap_empty() {
        let cmap = CidCMap::parse(b"").unwrap();
        assert!(cmap.is_empty());
        assert_eq!(cmap.len(), 0);
        assert_eq!(cmap.lookup(0), None);
    }

    #[test]
    fn cid_cmap_cidchar_single() {
        let data = b"\
            begincidchar\n\
            <0041> 100\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some(100));
    }

    #[test]
    fn cid_cmap_cidchar_multiple() {
        let data = b"\
            begincidchar\n\
            <0041> 100\n\
            <0042> 101\n\
            <0043> 102\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some(100));
        assert_eq!(cmap.lookup(0x0042), Some(101));
        assert_eq!(cmap.lookup(0x0043), Some(102));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn cid_cmap_cidrange_simple() {
        let data = b"\
            begincidrange\n\
            <0041> <0043> 100\n\
            endcidrange\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some(100));
        assert_eq!(cmap.lookup(0x0042), Some(101));
        assert_eq!(cmap.lookup(0x0043), Some(102));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn cid_cmap_cidrange_single_code() {
        let data = b"\
            begincidrange\n\
            <0041> <0041> 50\n\
            endcidrange\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some(50));
        assert_eq!(cmap.len(), 1);
    }

    #[test]
    fn cid_cmap_combined_cidchar_and_cidrange() {
        let data = b"\
            1 begincidchar\n\
            <0001> 1\n\
            endcidchar\n\
            1 begincidrange\n\
            <0010> <0012> 100\n\
            endcidrange\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some(1));
        assert_eq!(cmap.lookup(0x0010), Some(100));
        assert_eq!(cmap.lookup(0x0011), Some(101));
        assert_eq!(cmap.lookup(0x0012), Some(102));
        assert_eq!(cmap.len(), 4);
    }

    #[test]
    fn cid_cmap_parses_name() {
        let data = b"\
            /CMapName /Adobe-Japan1-6 def\n\
            begincidrange\n\
            <0041> <0043> 100\n\
            endcidrange\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.name(), Some("Adobe-Japan1-6"));
    }

    #[test]
    fn cid_cmap_parses_writing_mode_horizontal() {
        let data = b"\
            /WMode 0 def\n\
            begincidchar\n\
            <0041> 1\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.writing_mode(), 0);
    }

    #[test]
    fn cid_cmap_parses_writing_mode_vertical() {
        let data = b"\
            /WMode 1 def\n\
            begincidchar\n\
            <0041> 1\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.writing_mode(), 1);
    }

    #[test]
    fn cid_cmap_default_writing_mode_horizontal() {
        let data = b"\
            begincidchar\n\
            <0041> 1\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.writing_mode(), 0);
    }

    #[test]
    fn cid_cmap_with_full_boilerplate() {
        let data = b"\
            /CIDInit /ProcSet findresource begin\n\
            12 dict begin\n\
            begincmap\n\
            /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 6 >> def\n\
            /CMapName /Adobe-Japan1-6 def\n\
            /CMapType 1 def\n\
            /WMode 0 def\n\
            1 begincodespacerange\n\
            <0000> <FFFF>\n\
            endcodespacerange\n\
            2 begincidchar\n\
            <0041> 100\n\
            <0042> 101\n\
            endcidchar\n\
            1 begincidrange\n\
            <0100> <010F> 200\n\
            endcidrange\n\
            endcmap\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.name(), Some("Adobe-Japan1-6"));
        assert_eq!(cmap.writing_mode(), 0);
        assert_eq!(cmap.lookup(0x0041), Some(100));
        assert_eq!(cmap.lookup(0x0042), Some(101));
        assert_eq!(cmap.lookup(0x0100), Some(200));
        assert_eq!(cmap.lookup(0x010F), Some(215)); // 200 + 15
        assert_eq!(cmap.len(), 18); // 2 + 16
    }

    #[test]
    fn cid_cmap_missing_lookup_returns_none() {
        let data = b"\
            begincidchar\n\
            <0041> 100\n\
            endcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x9999), None);
    }

    // --- No-newline format (US-183-1: issue9262-style concatenated bfchar) ---

    #[test]
    fn bfchar_no_newlines_concatenated_entries() {
        // Real-world pattern from issue9262_reduced.pdf: all entries on a single line
        // with no newline separators between pairs.
        let data = b"beginbfchar\n<0002> <000D><0144> <01C2><0155> <01F5>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0002), Some("\r")); // U+000D = carriage return
        assert_eq!(cmap.lookup(0x0144), Some("\u{01C2}")); // ǂ
        assert_eq!(cmap.lookup(0x0155), Some("\u{01F5}")); // ǵ
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn bfchar_fully_concatenated_no_whitespace() {
        // All entries concatenated with no whitespace at all
        let data = b"beginbfchar\n<0041><0041><0042><0042><0043><0043>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn bfrange_no_newlines_concatenated_entries() {
        // Range entries concatenated on a single line
        let data = b"beginbfrange\n<0041> <0043> <0041><0061> <0063> <0061>\nendbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.lookup(0x0061), Some("a"));
        assert_eq!(cmap.lookup(0x0063), Some("c"));
        assert_eq!(cmap.len(), 6);
    }

    #[test]
    fn bfchar_mixed_newline_and_concatenated() {
        // Mix of newline-separated and concatenated entries
        let data = b"beginbfchar\n<0041> <0041>\n<0042> <0042><0043> <0043>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("A"));
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.len(), 3);
    }

    // --- Identity CMap via cidrange ---

    #[test]
    fn cidrange_identity_full_range() {
        // Full-range Identity cidrange: <0000> <FFFF> 0
        // Should set identity flag, not materialize 65536 entries
        let data = b"\
            begincidrange\n\
            <0000> <FFFF> 0\n\
            endcidrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(cmap.is_identity());
        assert_eq!(cmap.mappings.len(), 0); // No materialized entries
        assert!(!cmap.is_empty()); // Not empty because identity is set
    }

    #[test]
    fn cidrange_partial_range_materialized() {
        // Partial cidrange: should be materialized, not identity
        let data = b"\
            begincidrange\n\
            <0041> <0043> 65\n\
            endcidrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(!cmap.is_identity());
        assert_eq!(cmap.lookup(0x0041), Some("A")); // 65 = 'A'
        assert_eq!(cmap.lookup(0x0042), Some("B"));
        assert_eq!(cmap.lookup(0x0043), Some("C"));
        assert_eq!(cmap.len(), 3);
    }

    #[test]
    fn bfchar_cmap_is_not_identity() {
        // Normal bfchar CMap should not be identity
        let data = b"\
            beginbfchar\n\
            <0041> <0041>\n\
            endbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(!cmap.is_identity());
    }

    // =========================================================================
    // Wave 2: Edge cases and adversarial inputs
    // =========================================================================

    // --- Empty / degenerate inputs ---

    #[test]
    fn empty_cmap_data() {
        let cmap = CMap::parse(b"").unwrap();
        assert!(cmap.is_empty());
        assert_eq!(cmap.len(), 0);
        assert!(!cmap.is_identity());
    }

    #[test]
    fn cmap_no_sections_only_boilerplate() {
        let data = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\nendcmap\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(cmap.is_empty());
    }

    #[test]
    fn empty_bfchar_section() {
        let data = b"beginbfchar\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.len(), 0);
    }

    #[test]
    fn empty_bfrange_section() {
        let data = b"beginbfrange\nendbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.len(), 0);
    }

    #[test]
    fn empty_cidcmap_data() {
        let cmap = CidCMap::parse(b"").unwrap();
        assert!(cmap.is_empty());
        assert_eq!(cmap.len(), 0);
        assert_eq!(cmap.name(), None);
        assert_eq!(cmap.writing_mode(), 0);
    }

    // --- lookup_or_replacement ---

    #[test]
    fn lookup_or_replacement_found() {
        let data = b"beginbfchar\n<0041> <0041>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup_or_replacement(0x0041), "A");
    }

    #[test]
    fn lookup_or_replacement_not_found() {
        let data = b"beginbfchar\n<0041> <0041>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup_or_replacement(0x9999), "\u{FFFD}");
    }

    // --- Surrogate pairs (characters beyond BMP) ---

    #[test]
    fn utf16be_surrogate_pair_emoji() {
        // U+1F600 (😀) in UTF-16BE is D83D DE00
        let data = b"beginbfchar\n<0001> <D83DDE00>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("😀"));
    }

    #[test]
    fn utf16be_supplementary_cjk() {
        // U+20000 (𠀀, CJK Extension B) in UTF-16BE is D840 DC00
        let data = b"beginbfchar\n<0001> <D840DC00>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("\u{20000}"));
    }

    // --- Multi-character mappings (ligatures) ---

    #[test]
    fn bfchar_multi_char_mapping() {
        // Some fonts map a single code to multiple Unicode chars
        // <0001> → "fi" (f=0046, i=0069)
        let data = b"beginbfchar\n<0001> <00660069>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("fi"));
    }

    #[test]
    fn bfchar_three_char_mapping() {
        // <0001> → "ffi" (f=0066, f=0066, i=0069)
        let data = b"beginbfchar\n<0001> <006600660069>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0001), Some("ffi"));
    }

    // --- bfrange with array: edge cases ---

    #[test]
    fn bfrange_array_single_element() {
        let data = b"beginbfrange\n<0041> <0041> [<005A>]\nendbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.lookup(0x0041), Some("Z"));
        assert_eq!(cmap.len(), 1);
    }

    // --- Overwriting: later entry wins ---

    #[test]
    fn bfchar_duplicate_code_later_wins() {
        let data = b"beginbfchar\n<0041> <0041>\n<0041> <0042>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        // HashMap: last insert wins
        assert_eq!(cmap.lookup(0x0041), Some("B"));
    }

    // --- cidrange with non-zero start ---

    #[test]
    fn cidrange_non_zero_cid_start() {
        let data = b"begincidrange\n<0000> <0002> 65\nendcidrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(!cmap.is_identity());
        // code 0 → CID 65 → U+0041 = 'A'
        assert_eq!(cmap.lookup(0x0000), Some("A"));
        assert_eq!(cmap.lookup(0x0001), Some("B"));
        assert_eq!(cmap.lookup(0x0002), Some("C"));
    }

    // --- CidCMap edge cases ---

    #[test]
    fn cidcmap_empty_cidchar_section() {
        let data = b"begincidchar\nendcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert!(cmap.is_empty());
    }

    #[test]
    fn cidcmap_name_with_spaces() {
        // CMap name parsing should handle various formats
        let data = b"/CMapName /My-Custom-CMap def\nbegincidchar\n<0001> 1\nendcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert_eq!(cmap.name(), Some("My-Custom-CMap"));
    }

    // --- Property: bfrange produces exactly (high - low + 1) mappings ---

    #[test]
    fn bfrange_exact_count() {
        let data = b"beginbfrange\n<0010> <001F> <0041>\nendbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.len(), 16); // 0x1F - 0x10 + 1 = 16
        // First and last
        assert_eq!(cmap.lookup(0x0010), Some("A")); // 0x0041
        assert_eq!(cmap.lookup(0x001F), Some("P")); // 0x0041 + 15 = 0x0050 = 'P'
    }

    // --- Property: is_empty vs len consistency ---

    #[test]
    fn cmap_is_empty_consistency() {
        let data = b"beginbfchar\n<0041> <0041>\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert!(!cmap.is_empty());
        assert!(cmap.len() > 0);
    }

    #[test]
    fn cidcmap_is_empty_consistency() {
        let data = b"begincidchar\n<0041> 1\nendcidchar\n";
        let cmap = CidCMap::parse(data).unwrap();
        assert!(!cmap.is_empty());
        assert!(cmap.len() > 0);
    }

    // --- Garbage tolerance: lines without hex tokens should be skipped ---

    #[test]
    fn bfchar_skips_garbage_lines() {
        let data = b"beginbfchar\nthis is garbage\n<0041> <0041>\nmore garbage\nendbfchar\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.len(), 1);
        assert_eq!(cmap.lookup(0x0041), Some("A"));
    }

    #[test]
    fn bfrange_skips_garbage_lines() {
        let data = b"beginbfrange\ngarbage\n<0041> <0043> <0041>\nmore garbage\nendbfrange\n";
        let cmap = CMap::parse(data).unwrap();
        assert_eq!(cmap.len(), 3);
    }
}
