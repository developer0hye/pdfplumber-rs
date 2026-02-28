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
}

impl CMap {
    /// Parse a ToUnicode CMap from its raw byte content.
    ///
    /// Extracts `beginbfchar`/`endbfchar` and `beginbfrange`/`endbfrange`
    /// sections to build the character code → Unicode mapping table.
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

        Ok(CMap { mappings })
    }

    /// Look up the Unicode string for a character code.
    ///
    /// Returns `None` if the code has no mapping in this CMap.
    pub fn lookup(&self, code: u32) -> Option<&str> {
        self.mappings.get(&code).map(|s| s.as_str())
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

    /// Returns true if this CMap has no mappings.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
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
/// Each line has format: `<srcCode> <dstUnicode>`
fn parse_bfchar_section(
    section: &str,
    mappings: &mut HashMap<u32, String>,
) -> Result<(), BackendError> {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('<') {
            continue;
        }

        let tokens = extract_hex_tokens(trimmed);
        if tokens.len() >= 2 {
            let src_code = parse_hex_code(tokens[0])?;
            let unicode_str = decode_utf16be_hex(tokens[1])?;
            mappings.insert(src_code, unicode_str);
        }
    }
    Ok(())
}

/// Parse a beginbfrange...endbfrange section.
///
/// Each line has format: `<srcLow> <srcHigh> <dstStart>`
/// or: `<srcLow> <srcHigh> [<str1> <str2> ...]`
fn parse_bfrange_section(
    section: &str,
    mappings: &mut HashMap<u32, String>,
) -> Result<(), BackendError> {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.contains('<') {
            continue;
        }

        // Check if destination is an array: [<hex> <hex> ...]
        if let Some(bracket_start) = trimmed.find('[') {
            // Array form: <srcLow> <srcHigh> [<str1> <str2> ...]
            let before_bracket = &trimmed[..bracket_start];
            let src_tokens = extract_hex_tokens(before_bracket);
            if src_tokens.len() < 2 {
                continue;
            }
            let src_low = parse_hex_code(src_tokens[0])?;
            let src_high = parse_hex_code(src_tokens[1])?;

            // Extract hex tokens from inside the brackets
            let bracket_end = trimmed.rfind(']').unwrap_or(trimmed.len());
            let array_content = &trimmed[bracket_start + 1..bracket_end];
            let dst_tokens = extract_hex_tokens(array_content);

            for (i, dst_hex) in dst_tokens.iter().enumerate() {
                let code = src_low + i as u32;
                if code > src_high {
                    break;
                }
                let unicode_str = decode_utf16be_hex(dst_hex)?;
                mappings.insert(code, unicode_str);
            }
        } else {
            // Standard form: <srcLow> <srcHigh> <dstStart>
            let tokens = extract_hex_tokens(trimmed);
            if tokens.len() < 3 {
                continue;
            }
            let src_low = parse_hex_code(tokens[0])?;
            let src_high = parse_hex_code(tokens[1])?;
            let dst_start = parse_hex_code(tokens[2])?;

            for offset in 0..=(src_high - src_low) {
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
}
