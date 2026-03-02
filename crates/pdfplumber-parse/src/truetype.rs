//! Minimal TrueType font table parsing for glyph width extraction.
//!
//! Parses the `hmtx` (horizontal metrics) table from embedded TrueType font
//! data (/FontFile2 streams) to extract per-glyph advance widths. Also reads
//! `hhea` and `maxp` tables for required metadata.
//!
//! This is intentionally minimal — only enough parsing to extract advance
//! widths, not full font outline data.

/// Parsed glyph widths from a TrueType font's hmtx table.
///
/// Advance widths are in font design units (typically 1/1000 of em).
#[derive(Debug, Clone)]
pub struct TrueTypeWidths {
    /// Per-glyph advance widths indexed by glyph ID.
    advance_widths: Vec<u16>,
    /// Units per em from the head table (typically 1000 or 2048).
    units_per_em: u16,
}

impl TrueTypeWidths {
    /// Get the advance width for a glyph ID, scaled to 1000 units per em.
    ///
    /// Returns `None` if the glyph ID is out of range.
    pub fn get_width(&self, glyph_id: u16) -> Option<f64> {
        let raw = self.advance_widths.get(glyph_id as usize)?;
        if self.units_per_em == 0 {
            return None;
        }
        // Scale to 1000 units per em (PDF glyph space convention)
        Some(f64::from(*raw) * 1000.0 / f64::from(self.units_per_em))
    }

    /// Number of glyphs in the font.
    pub fn num_glyphs(&self) -> usize {
        self.advance_widths.len()
    }

    /// Units per em from the head table.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }
}

/// A TrueType table directory entry.
#[derive(Debug, Clone, Copy)]
struct TableRecord {
    offset: u32,
    length: u32,
}

/// Read a big-endian u16 from a byte slice at the given offset.
fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

/// Read a big-endian u32 from a byte slice at the given offset.
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// Parse the TrueType offset table and table directory to find a table by tag.
fn find_table(data: &[u8], tag: &[u8; 4]) -> Option<TableRecord> {
    // Offset table: sfVersion(4) + numTables(2) + searchRange(2)
    //               + entrySelector(2) + rangeShift(2) = 12 bytes
    let num_tables = read_u16(data, 4)? as usize;

    // Table directory starts at offset 12
    for i in 0..num_tables {
        let entry_offset = 12 + i * 16;
        if entry_offset + 16 > data.len() {
            return None;
        }
        let entry_tag = [
            data[entry_offset],
            data[entry_offset + 1],
            data[entry_offset + 2],
            data[entry_offset + 3],
        ];
        if &entry_tag == tag {
            let offset = read_u32(data, entry_offset + 8)?;
            let length = read_u32(data, entry_offset + 12)?;
            return Some(TableRecord { offset, length });
        }
    }
    None
}

/// Parse the `head` table to extract `unitsPerEm`.
///
/// head table layout: version(4) + fontRevision(4) + checksumAdjustment(4)
///   + magicNumber(4) + flags(2) + unitsPerEm(2) = at offset 18
fn parse_head_units_per_em(data: &[u8]) -> Option<u16> {
    let head = find_table(data, b"head")?;
    let off = head.offset as usize;
    // unitsPerEm is at offset 18 within the head table
    if head.length < 20 {
        return None;
    }
    read_u16(data, off + 18)
}

/// Parse the `hhea` table to extract `numberOfHMetrics`.
///
/// hhea table: 36 bytes total.
/// numberOfHMetrics is at offset 34 within the table.
fn parse_hhea_num_h_metrics(data: &[u8]) -> Option<u16> {
    let hhea = find_table(data, b"hhea")?;
    let off = hhea.offset as usize;
    if hhea.length < 36 {
        return None;
    }
    read_u16(data, off + 34)
}

/// Parse the `maxp` table to extract `numGlyphs`.
///
/// maxp table: version(4) + numGlyphs(2) (minimum 6 bytes).
fn parse_maxp_num_glyphs(data: &[u8]) -> Option<u16> {
    let maxp = find_table(data, b"maxp")?;
    let off = maxp.offset as usize;
    if maxp.length < 6 {
        return None;
    }
    read_u16(data, off + 4)
}

/// Parse TrueType font data to extract per-glyph advance widths.
///
/// Reads the `hmtx`, `hhea`, `maxp`, and `head` tables from raw TrueType
/// font data (typically from a /FontFile2 PDF stream).
///
/// Returns `None` if the data is not valid TrueType or required tables
/// are missing.
pub fn parse_truetype_widths(data: &[u8]) -> Option<TrueTypeWidths> {
    // Minimum TrueType data: offset table (12) + at least one table record (16)
    if data.len() < 28 {
        return None;
    }

    // Validate sfVersion: 0x00010000 (TrueType) or 0x74727565 ('true')
    let sf_version = read_u32(data, 0)?;
    if sf_version != 0x00010000 && sf_version != 0x74727565 {
        return None;
    }

    let units_per_em = parse_head_units_per_em(data)?;
    let num_h_metrics = parse_hhea_num_h_metrics(data)? as usize;
    let num_glyphs = parse_maxp_num_glyphs(data)? as usize;

    if num_h_metrics == 0 || num_glyphs == 0 {
        return None;
    }

    let hmtx = find_table(data, b"hmtx")?;
    let hmtx_off = hmtx.offset as usize;

    // Each longHorMetric is 4 bytes (u16 advanceWidth + i16 lsb)
    let long_metrics_size = num_h_metrics * 4;
    if hmtx_off + long_metrics_size > data.len() {
        return None;
    }

    let mut advance_widths = Vec::with_capacity(num_glyphs);

    // Read longHorMetric records
    for i in 0..num_h_metrics {
        let w = read_u16(data, hmtx_off + i * 4)?;
        advance_widths.push(w);
    }

    // Remaining glyphs share the last advance width
    if num_glyphs > num_h_metrics {
        let last_width = *advance_widths.last()?;
        for _ in num_h_metrics..num_glyphs {
            advance_widths.push(last_width);
        }
    }

    Some(TrueTypeWidths {
        advance_widths,
        units_per_em,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build minimal TrueType data with head, hhea, maxp, and hmtx tables.
    ///
    /// `widths` contains advance widths for `num_h_metrics` longHorMetric entries.
    /// `num_glyphs` is the total number of glyphs (remaining glyphs inherit the
    /// last advance width).
    fn build_truetype_data(
        units_per_em: u16,
        num_h_metrics: u16,
        num_glyphs: u16,
        widths: &[u16],
    ) -> Vec<u8> {
        assert_eq!(widths.len(), num_h_metrics as usize);

        // We'll create 4 tables: head, hhea, maxp, hmtx
        let num_tables: u16 = 4;

        // Table sizes
        let head_len: u32 = 54; // minimal head table
        let hhea_len: u32 = 36;
        let maxp_len: u32 = 6;
        let hmtx_len: u32 = (num_h_metrics as u32) * 4; // only longHorMetric, no extra lsb

        // Offsets: offset table (12) + 4 table records (4 * 16 = 64) = 76
        let dir_end: u32 = 12 + num_tables as u32 * 16;
        let head_off = dir_end;
        let hhea_off = head_off + head_len;
        let maxp_off = hhea_off + hhea_len;
        let hmtx_off = maxp_off + maxp_len;
        let total_len = hmtx_off + hmtx_len;

        let mut buf = vec![0u8; total_len as usize];

        // --- Offset table ---
        // sfVersion = 0x00010000 (TrueType)
        buf[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        buf[4..6].copy_from_slice(&num_tables.to_be_bytes());
        // searchRange, entrySelector, rangeShift — not needed for parsing
        buf[6..8].copy_from_slice(&0u16.to_be_bytes());
        buf[8..10].copy_from_slice(&0u16.to_be_bytes());
        buf[10..12].copy_from_slice(&0u16.to_be_bytes());

        // --- Table directory ---
        let tables: [(&[u8; 4], u32, u32); 4] = [
            (b"head", head_off, head_len),
            (b"hhea", hhea_off, hhea_len),
            (b"maxp", maxp_off, maxp_len),
            (b"hmtx", hmtx_off, hmtx_len),
        ];
        for (i, (tag, off, len)) in tables.iter().enumerate() {
            let entry = 12 + i * 16;
            buf[entry..entry + 4].copy_from_slice(*tag);
            // checksum = 0 (not validated)
            buf[entry + 4..entry + 8].copy_from_slice(&0u32.to_be_bytes());
            buf[entry + 8..entry + 12].copy_from_slice(&off.to_be_bytes());
            buf[entry + 12..entry + 16].copy_from_slice(&len.to_be_bytes());
        }

        // --- head table ---
        // version = 0x00010000 (first 4 bytes)
        buf[head_off as usize..head_off as usize + 4].copy_from_slice(&0x00010000u32.to_be_bytes());
        // unitsPerEm at offset 18
        buf[head_off as usize + 18..head_off as usize + 20]
            .copy_from_slice(&units_per_em.to_be_bytes());

        // --- hhea table ---
        buf[hhea_off as usize..hhea_off as usize + 4].copy_from_slice(&0x00010000u32.to_be_bytes());
        // numberOfHMetrics at offset 34
        buf[hhea_off as usize + 34..hhea_off as usize + 36]
            .copy_from_slice(&num_h_metrics.to_be_bytes());

        // --- maxp table ---
        buf[maxp_off as usize..maxp_off as usize + 4].copy_from_slice(&0x00005000u32.to_be_bytes()); // version 0.5
        buf[maxp_off as usize + 4..maxp_off as usize + 6]
            .copy_from_slice(&num_glyphs.to_be_bytes());

        // --- hmtx table ---
        for (i, &w) in widths.iter().enumerate() {
            let pos = hmtx_off as usize + i * 4;
            buf[pos..pos + 2].copy_from_slice(&w.to_be_bytes());
            // lsb = 0
            buf[pos + 2..pos + 4].copy_from_slice(&0i16.to_be_bytes());
        }

        buf
    }

    // ========== TDD Red Phase: Unit tests for TrueType parsing ==========

    #[test]
    fn parse_valid_truetype_basic() {
        // 3 glyphs, all with explicit widths, 1000 upem
        let data = build_truetype_data(1000, 3, 3, &[250, 500, 750]);
        let widths = parse_truetype_widths(&data).expect("should parse valid TrueType");

        assert_eq!(widths.num_glyphs(), 3);
        assert_eq!(widths.units_per_em(), 1000);
        // With 1000 upem, raw widths = scaled widths
        assert!((widths.get_width(0).unwrap() - 250.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 500.0).abs() < 0.01);
        assert!((widths.get_width(2).unwrap() - 750.0).abs() < 0.01);
    }

    #[test]
    fn parse_truetype_upem_2048() {
        // Common TrueType upem: 2048
        let data = build_truetype_data(2048, 2, 2, &[1024, 512]);
        let widths = parse_truetype_widths(&data).expect("should parse");

        // 1024 * 1000 / 2048 = 500.0
        assert!((widths.get_width(0).unwrap() - 500.0).abs() < 0.01);
        // 512 * 1000 / 2048 = 250.0
        assert!((widths.get_width(1).unwrap() - 250.0).abs() < 0.01);
    }

    #[test]
    fn parse_truetype_inherited_widths() {
        // 5 glyphs but only 2 longHorMetric entries.
        // Glyphs 2-4 inherit the last advance width (600).
        let data = build_truetype_data(1000, 2, 5, &[300, 600]);
        let widths = parse_truetype_widths(&data).expect("should parse");

        assert_eq!(widths.num_glyphs(), 5);
        assert!((widths.get_width(0).unwrap() - 300.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 600.0).abs() < 0.01);
        // Inherited
        assert!((widths.get_width(2).unwrap() - 600.0).abs() < 0.01);
        assert!((widths.get_width(3).unwrap() - 600.0).abs() < 0.01);
        assert!((widths.get_width(4).unwrap() - 600.0).abs() < 0.01);
    }

    #[test]
    fn parse_truetype_out_of_range_glyph() {
        let data = build_truetype_data(1000, 2, 2, &[400, 800]);
        let widths = parse_truetype_widths(&data).expect("should parse");

        assert!(widths.get_width(2).is_none());
        assert!(widths.get_width(100).is_none());
    }

    #[test]
    fn parse_truetype_empty_data() {
        assert!(parse_truetype_widths(&[]).is_none());
    }

    #[test]
    fn parse_truetype_truncated_data() {
        assert!(parse_truetype_widths(&[0; 10]).is_none());
    }

    #[test]
    fn parse_truetype_invalid_sf_version() {
        let mut data = build_truetype_data(1000, 1, 1, &[500]);
        // Set sfVersion to something invalid
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
        assert!(parse_truetype_widths(&data).is_none());
    }

    #[test]
    fn parse_truetype_mac_sf_version() {
        let mut data = build_truetype_data(1000, 1, 1, &[500]);
        // Set sfVersion to 'true' (Mac TrueType)
        data[0..4].copy_from_slice(&0x74727565u32.to_be_bytes());
        let widths = parse_truetype_widths(&data).expect("should parse Mac TrueType");
        assert!((widths.get_width(0).unwrap() - 500.0).abs() < 0.01);
    }

    #[test]
    fn parse_truetype_zero_upem() {
        let data = build_truetype_data(0, 1, 1, &[500]);
        // With 0 upem, get_width should return None (avoid div by zero)
        let widths = parse_truetype_widths(&data);
        // The parser may return Some, but get_width should handle 0 upem
        if let Some(w) = widths {
            assert!(w.get_width(0).is_none());
        }
    }

    #[test]
    fn parse_truetype_single_glyph() {
        let data = build_truetype_data(1000, 1, 1, &[600]);
        let widths = parse_truetype_widths(&data).expect("should parse");
        assert_eq!(widths.num_glyphs(), 1);
        assert!((widths.get_width(0).unwrap() - 600.0).abs() < 0.01);
    }

    #[test]
    fn find_table_returns_none_for_missing() {
        let data = build_truetype_data(1000, 1, 1, &[500]);
        // 'GSUB' table doesn't exist in our minimal data
        assert!(find_table(&data, b"GSUB").is_none());
    }

    #[test]
    fn find_table_finds_existing() {
        let data = build_truetype_data(1000, 1, 1, &[500]);
        let record = find_table(&data, b"hmtx");
        assert!(record.is_some());
        let r = record.unwrap();
        assert!(r.length > 0);
        assert!(r.offset > 0);
    }
}
