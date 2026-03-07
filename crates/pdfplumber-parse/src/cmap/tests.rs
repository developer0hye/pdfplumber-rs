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
