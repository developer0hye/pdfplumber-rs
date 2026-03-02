//! Minimal CFF (Compact Font Format) parsing for glyph width extraction.
//!
//! Parses CFF font data from /FontFile3 streams (subtype Type1C) to extract
//! per-glyph advance widths. Only enough of the CFF format is parsed to
//! extract widths — full outline parsing is not implemented.
//!
//! Reference: Adobe Technical Note #5176 (CFF specification) and
//! Adobe Technical Note #5177 (Type 2 Charstring Format).

/// Parsed glyph widths from a CFF font program.
#[derive(Debug, Clone)]
pub struct CffWidths {
    /// Per-glyph advance widths in 1/1000 text space units, indexed by glyph ID.
    widths: Vec<f64>,
}

impl CffWidths {
    /// Get the advance width for a glyph ID.
    ///
    /// Returns `None` if the glyph ID is out of range.
    pub fn get_width(&self, glyph_id: u16) -> Option<f64> {
        self.widths.get(glyph_id as usize).copied()
    }

    /// Number of glyphs in the font.
    pub fn num_glyphs(&self) -> usize {
        self.widths.len()
    }
}

/// Parse CFF font data to extract per-glyph advance widths.
///
/// Reads the CFF header, Top DICT, Private DICT, and CharStrings INDEX
/// to extract widths from the Type 2 CharString programs.
///
/// Returns `None` if the data is not valid CFF or required structures
/// are missing.
pub fn parse_cff_widths(data: &[u8]) -> Option<CffWidths> {
    // Minimum CFF: header (4) + Name INDEX (2 for empty) = 6 bytes
    if data.len() < 6 {
        return None;
    }

    // CFF Header: major(1), minor(1), hdrSize(1), offSize(1)
    let major = data[0];
    if major != 1 {
        return None; // Only CFF version 1 supported
    }
    let hdr_size = data[2] as usize;
    if hdr_size < 4 || hdr_size > data.len() {
        return None;
    }

    // Skip Name INDEX (starts at hdrSize)
    let name_idx_start = hdr_size;
    let name_idx_end = skip_index(data, name_idx_start)?;

    // Parse Top DICT INDEX
    let (top_dicts, _top_dict_end) = parse_index(data, name_idx_start_offset(name_idx_end))?;
    if top_dicts.is_empty() {
        return None;
    }

    // Parse the first Top DICT (CFF typically has one font)
    let top_dict = parse_dict(&top_dicts[0])?;

    // Get CharStrings offset from Top DICT (operator 17)
    let charstrings_offset = top_dict.charstrings_offset? as usize;

    // Get Private DICT location from Top DICT (operator 18: size, offset)
    let (private_size, private_offset) = top_dict.private?;
    let private_size = private_size as usize;
    let private_offset = private_offset as usize;

    // Parse Private DICT
    if private_offset + private_size > data.len() {
        return None;
    }
    let private_dict_data = &data[private_offset..private_offset + private_size];
    let private_dict = parse_private_dict(private_dict_data)?;

    let default_width_x = private_dict.default_width_x;
    let nominal_width_x = private_dict.nominal_width_x;

    // Parse CharStrings INDEX
    if charstrings_offset >= data.len() {
        return None;
    }
    let (charstrings, _) = parse_index(data, charstrings_offset)?;

    // Extract width from each charstring
    let mut widths = Vec::with_capacity(charstrings.len());
    for cs_data in &charstrings {
        let width = extract_charstring_width(cs_data, default_width_x, nominal_width_x);
        widths.push(width);
    }

    Some(CffWidths { widths })
}

// ============================================================
// Internal parsing helpers
// ============================================================

/// Top DICT parsed values relevant for width extraction.
#[derive(Debug, Default)]
struct TopDictInfo {
    /// Offset to CharStrings INDEX (operator 17).
    charstrings_offset: Option<f64>,
    /// Private DICT (size, offset) (operator 18).
    private: Option<(f64, f64)>,
}

/// Private DICT parsed values relevant for width extraction.
#[derive(Debug)]
struct PrivateDictInfo {
    /// Default width for glyphs without explicit width (operator 20).
    default_width_x: f64,
    /// Nominal width added to charstring width values (operator 21).
    nominal_width_x: f64,
}

impl Default for PrivateDictInfo {
    fn default() -> Self {
        Self {
            default_width_x: 0.0,
            nominal_width_x: 0.0,
        }
    }
}

/// Helper: the name_idx_start_offset is just the identity (for readability).
fn name_idx_start_offset(offset: usize) -> usize {
    offset
}

/// Skip an INDEX structure and return the offset past it.
fn skip_index(data: &[u8], offset: usize) -> Option<usize> {
    if offset + 2 > data.len() {
        return None;
    }
    let count = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
    if count == 0 {
        return Some(offset + 2);
    }
    if offset + 3 > data.len() {
        return None;
    }
    let off_size = data[offset + 2] as usize;
    if off_size == 0 || off_size > 4 {
        return None;
    }

    // Read the last offset to determine total data size
    let offsets_start = offset + 3;
    let last_offset_pos = offsets_start + count * off_size;
    if last_offset_pos + off_size > data.len() {
        return None;
    }
    let last_offset = read_offset(data, last_offset_pos, off_size)?;
    // Data starts after the offset array; last_offset is 1-based
    let data_start = offsets_start + (count + 1) * off_size;
    Some(data_start + last_offset - 1)
}

/// Parse an INDEX structure and return (list of byte slices, offset past INDEX).
fn parse_index(data: &[u8], offset: usize) -> Option<(Vec<Vec<u8>>, usize)> {
    if offset + 2 > data.len() {
        return None;
    }
    let count = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
    if count == 0 {
        return Some((Vec::new(), offset + 2));
    }
    if offset + 3 > data.len() {
        return None;
    }
    let off_size = data[offset + 2] as usize;
    if off_size == 0 || off_size > 4 {
        return None;
    }

    let offsets_start = offset + 3;
    // Read all (count + 1) offsets
    let mut offsets = Vec::with_capacity(count + 1);
    for i in 0..=count {
        let pos = offsets_start + i * off_size;
        let off = read_offset(data, pos, off_size)?;
        offsets.push(off);
    }

    let data_start = offsets_start + (count + 1) * off_size;
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let start = data_start + offsets[i] - 1; // offsets are 1-based
        let end = data_start + offsets[i + 1] - 1;
        if end > data.len() || start > end {
            return None;
        }
        items.push(data[start..end].to_vec());
    }

    let total_end = data_start + offsets[count] - 1;
    Some((items, total_end))
}

/// Read an offset value of `off_size` bytes (1-4) at `pos`.
fn read_offset(data: &[u8], pos: usize, off_size: usize) -> Option<usize> {
    if pos + off_size > data.len() {
        return None;
    }
    let mut val: usize = 0;
    for i in 0..off_size {
        val = (val << 8) | data[pos + i] as usize;
    }
    Some(val)
}

/// Parse a CFF DICT from raw bytes, extracting Top DICT values.
fn parse_dict(data: &[u8]) -> Option<TopDictInfo> {
    let mut info = TopDictInfo::default();
    let mut operand_stack: Vec<f64> = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let b0 = data[pos];
        match b0 {
            // Operators
            0..=11 | 13..=21 => {
                match b0 {
                    17 => {
                        // CharStrings offset
                        if let Some(&v) = operand_stack.last() {
                            info.charstrings_offset = Some(v);
                        }
                    }
                    18 => {
                        // Private: size, offset
                        if operand_stack.len() >= 2 {
                            let size = operand_stack[operand_stack.len() - 2];
                            let offset = operand_stack[operand_stack.len() - 1];
                            info.private = Some((size, offset));
                        }
                    }
                    _ => {} // Other operators: ignore
                }
                operand_stack.clear();
                pos += 1;
            }
            12 => {
                // 2-byte operator (escape)
                operand_stack.clear();
                pos += 2;
            }
            // Operands
            28 => {
                // 3-byte integer: b1<<8 | b2
                if pos + 2 >= data.len() {
                    return None;
                }
                let val = i16::from_be_bytes([data[pos + 1], data[pos + 2]]) as f64;
                operand_stack.push(val);
                pos += 3;
            }
            29 => {
                // 5-byte integer: b1<<24 | b2<<16 | b3<<8 | b4
                if pos + 4 >= data.len() {
                    return None;
                }
                let val = i32::from_be_bytes([
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                ]) as f64;
                operand_stack.push(val);
                pos += 5;
            }
            30 => {
                // Real number (nibble-encoded)
                let (val, consumed) = parse_real_number(data, pos)?;
                operand_stack.push(val);
                pos += consumed;
            }
            32..=246 => {
                // 1-byte integer: b0 - 139
                operand_stack.push(b0 as f64 - 139.0);
                pos += 1;
            }
            247..=250 => {
                // 2-byte positive: (b0-247)*256 + b1 + 108
                if pos + 1 >= data.len() {
                    return None;
                }
                let val = (b0 as f64 - 247.0) * 256.0 + data[pos + 1] as f64 + 108.0;
                operand_stack.push(val);
                pos += 2;
            }
            251..=254 => {
                // 2-byte negative: -(b0-251)*256 - b1 - 108
                if pos + 1 >= data.len() {
                    return None;
                }
                let val = -(b0 as f64 - 251.0) * 256.0 - data[pos + 1] as f64 - 108.0;
                operand_stack.push(val);
                pos += 2;
            }
            _ => {
                // 22..=27, 255: reserved / invalid in DICT context
                pos += 1;
            }
        }
    }

    Some(info)
}

/// Parse a Private DICT from raw bytes.
fn parse_private_dict(data: &[u8]) -> Option<PrivateDictInfo> {
    let mut info = PrivateDictInfo::default();
    let mut operand_stack: Vec<f64> = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let b0 = data[pos];
        match b0 {
            // Operators
            0..=11 | 13..=21 => {
                match b0 {
                    20 => {
                        // defaultWidthX
                        if let Some(&v) = operand_stack.last() {
                            info.default_width_x = v;
                        }
                    }
                    21 => {
                        // nominalWidthX
                        if let Some(&v) = operand_stack.last() {
                            info.nominal_width_x = v;
                        }
                    }
                    _ => {}
                }
                operand_stack.clear();
                pos += 1;
            }
            12 => {
                operand_stack.clear();
                pos += 2;
            }
            28 => {
                if pos + 2 >= data.len() {
                    return None;
                }
                let val = i16::from_be_bytes([data[pos + 1], data[pos + 2]]) as f64;
                operand_stack.push(val);
                pos += 3;
            }
            29 => {
                if pos + 4 >= data.len() {
                    return None;
                }
                let val = i32::from_be_bytes([
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                ]) as f64;
                operand_stack.push(val);
                pos += 5;
            }
            30 => {
                let (val, consumed) = parse_real_number(data, pos)?;
                operand_stack.push(val);
                pos += consumed;
            }
            32..=246 => {
                operand_stack.push(b0 as f64 - 139.0);
                pos += 1;
            }
            247..=250 => {
                if pos + 1 >= data.len() {
                    return None;
                }
                let val = (b0 as f64 - 247.0) * 256.0 + data[pos + 1] as f64 + 108.0;
                operand_stack.push(val);
                pos += 2;
            }
            251..=254 => {
                if pos + 1 >= data.len() {
                    return None;
                }
                let val = -(b0 as f64 - 251.0) * 256.0 - data[pos + 1] as f64 - 108.0;
                operand_stack.push(val);
                pos += 2;
            }
            _ => {
                pos += 1;
            }
        }
    }

    Some(info)
}

/// Parse a nibble-encoded real number (operator 30 in DICT).
///
/// Returns (value, bytes consumed including the 30 prefix byte).
fn parse_real_number(data: &[u8], start: usize) -> Option<(f64, usize)> {
    // Skip the 30 prefix
    let mut pos = start + 1;
    let mut nibbles = Vec::new();
    let mut done = false;

    while pos < data.len() && !done {
        let byte = data[pos];
        for shift in [4, 0] {
            let nibble = (byte >> shift) & 0x0F;
            if nibble == 0x0F {
                done = true;
                break;
            }
            nibbles.push(nibble);
        }
        pos += 1;
    }

    // Convert nibbles to string then parse
    let mut s = String::new();
    for &n in &nibbles {
        match n {
            0..=9 => s.push(char::from(b'0' + n)),
            0x0A => s.push('.'),
            0x0B => s.push('E'),
            0x0C => {
                s.push('E');
                s.push('-');
            }
            0x0E => s.push('-'),
            _ => return None, // 0x0D is reserved
        }
    }

    let val: f64 = s.parse().ok()?;
    Some((val, pos - start))
}

/// Extract the width from a Type 2 CharString program.
///
/// Per Adobe TN #5177: the width is an optional first operand before the
/// first stack-clearing operator. If present, the actual width is
/// `nominal_width_x + charstring_width`. If absent, `default_width_x`.
fn extract_charstring_width(data: &[u8], default_width_x: f64, nominal_width_x: f64) -> f64 {
    let mut stack: Vec<f64> = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let b0 = data[pos];
        match b0 {
            // Type 2 CharString operators that clear the stack
            1 | 3 | 18 | 23 => {
                // hstem(1), vstem(3), hstemhm(18), vstemhm(23)
                // Expected: even number of args (pairs). If odd, first is width.
                if stack.len() % 2 == 1 {
                    return nominal_width_x + stack[0];
                }
                return default_width_x;
            }
            4 | 22 => {
                // vmoveto(4), hmoveto(22)
                // Expected: 1 arg. If 2, first is width.
                if stack.len() > 1 {
                    return nominal_width_x + stack[0];
                }
                return default_width_x;
            }
            14 => {
                // endchar
                // Expected: 0 args. If 1, it's width.
                if !stack.is_empty() {
                    return nominal_width_x + stack[0];
                }
                return default_width_x;
            }
            21 => {
                // rmoveto
                // Expected: 2 args. If 3, first is width.
                if stack.len() > 2 {
                    return nominal_width_x + stack[0];
                }
                return default_width_x;
            }
            // hintmask(19), cntrmask(20) — like stems, check for odd args
            19 | 20 => {
                if stack.len() % 2 == 1 {
                    return nominal_width_x + stack[0];
                }
                return default_width_x;
            }
            // 2-byte operator escape (12 xx)
            12 => {
                // All escape operators clear the stack; no width info expected
                return default_width_x;
            }

            // ---- Operands ----
            28 => {
                if pos + 2 >= data.len() {
                    break;
                }
                let val = i16::from_be_bytes([data[pos + 1], data[pos + 2]]) as f64;
                stack.push(val);
                pos += 3;
                continue;
            }
            // Other operators (drawing, etc.) — width determination is done
            5..=11 | 13 | 15..=17 | 24..=27 | 29..=31 => {
                return default_width_x;
            }
            255 => {
                // Fixed-point 16.16
                if pos + 4 >= data.len() {
                    break;
                }
                let fixed = i32::from_be_bytes([
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                ]);
                stack.push(fixed as f64 / 65536.0);
                pos += 5;
                continue;
            }
            32..=246 => {
                stack.push(b0 as f64 - 139.0);
                pos += 1;
                continue;
            }
            247..=250 => {
                if pos + 1 >= data.len() {
                    break;
                }
                let val = (b0 as f64 - 247.0) * 256.0 + data[pos + 1] as f64 + 108.0;
                stack.push(val);
                pos += 2;
                continue;
            }
            251..=254 => {
                if pos + 1 >= data.len() {
                    break;
                }
                let val = -(b0 as f64 - 251.0) * 256.0 - data[pos + 1] as f64 - 108.0;
                stack.push(val);
                pos += 2;
                continue;
            }
            _ => {
                pos += 1;
                continue;
            }
        }
    }

    // Reached end of data without a stack-clearing operator
    default_width_x
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // ========== Test helpers ==========

    /// Public test helper for building CFF data from other test modules.
    pub fn build_cff_data_for_test(
        default_width_x: i32,
        nominal_width_x: i32,
        glyph_widths: &[i32],
    ) -> Vec<u8> {
        build_cff_data(default_width_x, nominal_width_x, glyph_widths)
    }

    /// Encode a CFF integer operand in DICT format.
    fn encode_dict_int(val: i32) -> Vec<u8> {
        if (-107..=107).contains(&val) {
            vec![(val + 139) as u8]
        } else if (108..=1131).contains(&val) {
            let adjusted = val - 108;
            let b0 = (adjusted / 256 + 247) as u8;
            let b1 = (adjusted % 256) as u8;
            vec![b0, b1]
        } else if (-1131..=-108).contains(&val) {
            let adjusted = -val - 108;
            let b0 = (adjusted / 256 + 251) as u8;
            let b1 = (adjusted % 256) as u8;
            vec![b0, b1]
        } else if (-32768..=32767).contains(&val) {
            let bytes = (val as i16).to_be_bytes();
            vec![28, bytes[0], bytes[1]]
        } else {
            let bytes = val.to_be_bytes();
            vec![29, bytes[0], bytes[1], bytes[2], bytes[3]]
        }
    }

    /// Build a CFF INDEX from a list of byte arrays.
    fn build_index(items: &[&[u8]]) -> Vec<u8> {
        let count = items.len() as u16;
        if count == 0 {
            return vec![0, 0]; // empty INDEX
        }

        // Calculate offsets (1-based)
        let mut offsets: Vec<usize> = vec![1];
        for item in items {
            offsets.push(offsets.last().unwrap() + item.len());
        }

        // Determine offSize (use 1, 2, or 4 bytes)
        let max_offset = *offsets.last().unwrap();
        let off_size: u8 = if max_offset <= 255 {
            1
        } else if max_offset <= 65535 {
            2
        } else {
            4
        };

        let mut buf = Vec::new();
        // count (2 bytes)
        buf.extend_from_slice(&count.to_be_bytes());
        // offSize (1 byte)
        buf.push(off_size);
        // offset array
        for &off in &offsets {
            match off_size {
                1 => buf.push(off as u8),
                2 => buf.extend_from_slice(&(off as u16).to_be_bytes()),
                4 => buf.extend_from_slice(&(off as u32).to_be_bytes()),
                _ => unreachable!(),
            }
        }
        // data
        for item in items {
            buf.extend_from_slice(item);
        }
        buf
    }

    /// Build a minimal CFF font with specified widths.
    ///
    /// Creates a CFF with:
    /// - One font (in Name INDEX)
    /// - Top DICT pointing to CharStrings and Private
    /// - Private DICT with defaultWidthX and nominalWidthX
    /// - CharStrings with simple charstrings encoding the widths
    fn build_cff_data(
        default_width_x: i32,
        nominal_width_x: i32,
        glyph_widths: &[i32], // actual widths in 1/1000
    ) -> Vec<u8> {
        // Build CharStrings: each glyph is either:
        //   - endchar (if width == default_width_x)
        //   - (width - nominal_width_x) endchar (if width != default_width_x)
        let mut charstrings: Vec<Vec<u8>> = Vec::new();
        for &w in glyph_widths {
            let mut cs = Vec::new();
            if w == default_width_x {
                // No width operand, just endchar
                cs.push(14); // endchar
            } else {
                // Width operand + endchar
                let width_val = w - nominal_width_x;
                cs.extend_from_slice(&encode_dict_int(width_val));
                cs.push(14); // endchar
            }
            charstrings.push(cs);
        }

        // Build Private DICT
        let mut private_dict = Vec::new();
        if default_width_x != 0 {
            private_dict.extend_from_slice(&encode_dict_int(default_width_x));
            private_dict.push(20); // defaultWidthX operator
        }
        if nominal_width_x != 0 {
            private_dict.extend_from_slice(&encode_dict_int(nominal_width_x));
            private_dict.push(21); // nominalWidthX operator
        }
        if private_dict.is_empty() {
            // Need at least something; put a dummy
            private_dict.extend_from_slice(&encode_dict_int(0));
            private_dict.push(20);
        }

        // Build CharStrings INDEX
        let cs_refs: Vec<&[u8]> = charstrings.iter().map(|cs| cs.as_slice()).collect();
        let charstrings_index = build_index(&cs_refs);

        // We need to know offsets, so build in stages:
        // CFF structure:
        //   Header (4 bytes)
        //   Name INDEX
        //   Top DICT INDEX
        //   String INDEX (empty)
        //   Global Subr INDEX (empty)
        //   CharStrings INDEX
        //   Private DICT

        let header = vec![1u8, 0, 4, 1]; // major=1, minor=0, hdrSize=4, offSize=1

        let name_index = build_index(&[b"TestFont"]);

        let string_index = build_index(&[]); // empty
        let global_subr_index = build_index(&[]); // empty

        // Calculate offsets for CharStrings and Private
        // First pass: build Top DICT with placeholder offsets, measure sizes
        // We'll use a two-pass approach:

        // Phase 1: compute sizes without Top DICT content
        let header_size = header.len();
        let name_size = name_index.len();

        // We need to know Top DICT INDEX size to compute subsequent offsets.
        // Build a dummy Top DICT first, then adjust.
        // Top DICT needs: charstrings_offset (op 17), private size+offset (op 18)

        // Estimate: Top DICT will have at most ~20 bytes
        // Let's build it properly with a two-pass approach.

        // For simplicity: manually compute offsets.
        // After header + name + top_dict + string + global_subr, charstrings start.
        // Then private DICT follows charstrings.

        // Build top_dict content (will need to adjust offsets)
        let build_top_dict = |cs_offset: i32, priv_size: i32, priv_offset: i32| -> Vec<u8> {
            let mut td = Vec::new();
            // CharStrings offset
            td.extend_from_slice(&encode_dict_int(cs_offset));
            td.push(17);
            // Private: size offset
            td.extend_from_slice(&encode_dict_int(priv_size));
            td.extend_from_slice(&encode_dict_int(priv_offset));
            td.push(18);
            td
        };

        // First estimate: use large placeholder values to determine Top DICT INDEX size
        let td_est = build_top_dict(9999, private_dict.len() as i32, 99999);
        let td_refs_est: Vec<&[u8]> = vec![td_est.as_slice()];
        let top_dict_index_est = build_index(&td_refs_est);

        let cs_offset_est = header_size
            + name_size
            + top_dict_index_est.len()
            + string_index.len()
            + global_subr_index.len();

        let priv_offset_est = cs_offset_est + charstrings_index.len();

        // Now build with real values
        let td = build_top_dict(
            cs_offset_est as i32,
            private_dict.len() as i32,
            priv_offset_est as i32,
        );
        let td_refs: Vec<&[u8]> = vec![td.as_slice()];
        let top_dict_index = build_index(&td_refs);

        // Verify our size estimate was correct
        let actual_cs_offset = header_size
            + name_size
            + top_dict_index.len()
            + string_index.len()
            + global_subr_index.len();

        let actual_priv_offset = actual_cs_offset + charstrings_index.len();

        // If sizes differ from estimate, rebuild (should rarely happen)
        let (top_dict_index, charstrings_offset_final, private_offset_final) = if actual_cs_offset
            != cs_offset_est
            || actual_priv_offset != priv_offset_est
        {
            let td2 = build_top_dict(
                actual_cs_offset as i32,
                private_dict.len() as i32,
                actual_priv_offset as i32,
            );
            let td2_refs: Vec<&[u8]> = vec![td2.as_slice()];
            let tdi2 = build_index(&td2_refs);
            let cs2 =
                header_size + name_size + tdi2.len() + string_index.len() + global_subr_index.len();
            let pr2 = cs2 + charstrings_index.len();
            (tdi2, cs2, pr2)
        } else {
            (top_dict_index, actual_cs_offset, actual_priv_offset)
        };

        // If sizes still differ after rebuild, do one more iteration
        let (top_dict_index, _cs_off, _priv_off) = {
            let verify_cs = header_size
                + name_size
                + top_dict_index.len()
                + string_index.len()
                + global_subr_index.len();
            let verify_priv = verify_cs + charstrings_index.len();
            if verify_cs != charstrings_offset_final || verify_priv != private_offset_final {
                let td3 = build_top_dict(
                    verify_cs as i32,
                    private_dict.len() as i32,
                    verify_priv as i32,
                );
                let td3_refs: Vec<&[u8]> = vec![td3.as_slice()];
                let tdi3 = build_index(&td3_refs);
                let cs3 = header_size
                    + name_size
                    + tdi3.len()
                    + string_index.len()
                    + global_subr_index.len();
                let pr3 = cs3 + charstrings_index.len();
                (tdi3, cs3, pr3)
            } else {
                (
                    top_dict_index,
                    charstrings_offset_final,
                    private_offset_final,
                )
            }
        };

        // Assemble
        let mut buf = Vec::new();
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&name_index);
        buf.extend_from_slice(&top_dict_index);
        buf.extend_from_slice(&string_index);
        buf.extend_from_slice(&global_subr_index);
        buf.extend_from_slice(&charstrings_index);
        buf.extend_from_slice(&private_dict);

        buf
    }

    // ========== TDD: Unit tests for CFF INDEX parsing ==========

    #[test]
    fn parse_empty_index() {
        let data = build_index(&[]);
        let (items, end) = parse_index(&data, 0).expect("should parse empty INDEX");
        assert!(items.is_empty());
        assert_eq!(end, 2); // count(2) only
    }

    #[test]
    fn parse_single_item_index() {
        let data = build_index(&[b"hello"]);
        let (items, _) = parse_index(&data, 0).expect("should parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], b"hello");
    }

    #[test]
    fn parse_multi_item_index() {
        let data = build_index(&[b"abc", b"de", b"f"]);
        let (items, _) = parse_index(&data, 0).expect("should parse");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], b"abc");
        assert_eq!(items[1], b"de");
        assert_eq!(items[2], b"f");
    }

    #[test]
    fn skip_index_correct_end() {
        let data = build_index(&[b"test", b"data"]);
        let end_skip = skip_index(&data, 0).expect("should skip");
        let (_, end_parse) = parse_index(&data, 0).expect("should parse");
        assert_eq!(end_skip, end_parse);
    }

    // ========== TDD: Unit tests for DICT number encoding ==========

    #[test]
    fn encode_dict_int_small_positive() {
        assert_eq!(encode_dict_int(0), vec![139]);
        assert_eq!(encode_dict_int(100), vec![239]);
        assert_eq!(encode_dict_int(107), vec![246]);
    }

    #[test]
    fn encode_dict_int_small_negative() {
        assert_eq!(encode_dict_int(-1), vec![138]);
        assert_eq!(encode_dict_int(-107), vec![32]);
    }

    #[test]
    fn encode_dict_int_medium_positive() {
        // 108..=1131
        let enc = encode_dict_int(108);
        assert_eq!(enc.len(), 2);
        assert_eq!(enc[0], 247);
    }

    #[test]
    fn encode_dict_int_medium_negative() {
        let enc = encode_dict_int(-108);
        assert_eq!(enc.len(), 2);
        assert_eq!(enc[0], 251);
    }

    #[test]
    fn encode_dict_int_large() {
        let enc = encode_dict_int(5000);
        assert_eq!(enc[0], 28); // 3-byte format
        assert_eq!(enc.len(), 3);
    }

    #[test]
    fn encode_dict_int_very_large() {
        let enc = encode_dict_int(100000);
        assert_eq!(enc[0], 29); // 5-byte format
        assert_eq!(enc.len(), 5);
    }

    // ========== TDD: Unit tests for real number parsing ==========

    #[test]
    fn parse_real_simple() {
        // Encode 3.14: nibbles 3, '.', 1, 4, 0xF
        // byte 1: 0x3A (3, '.')
        // byte 2: 0x14 (1, 4)
        // byte 3: 0xFF (terminator)
        let data = [30u8, 0x3A, 0x14, 0xFF];
        let (val, consumed) = parse_real_number(&data, 0).expect("should parse");
        assert!((val - 3.14).abs() < 0.001);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn parse_real_negative() {
        // Encode -2.5: nibbles '-', 2, '.', 5, 0xF
        // 0xE = '-', 0x2 = 2, 0xA = '.', 0x5 = 5
        // byte 1: 0xE2 (-, 2)
        // byte 2: 0xA5 (., 5)
        // byte 3: 0xFF (terminator)
        let data = [30u8, 0xE2, 0xA5, 0xFF];
        let (val, consumed) = parse_real_number(&data, 0).expect("should parse");
        assert!((val - (-2.5)).abs() < 0.001);
        assert_eq!(consumed, 4);
    }

    // ========== TDD: Unit tests for CharString width extraction ==========

    #[test]
    fn charstring_width_endchar_only() {
        // endchar with no args → default width
        let data = [14u8]; // endchar
        let w = extract_charstring_width(&data, 500.0, 0.0);
        assert!((w - 500.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_with_explicit_width_endchar() {
        // width=100, endchar → nominal + 100
        let mut cs = encode_dict_int(100);
        cs.push(14); // endchar
        let w = extract_charstring_width(&cs, 500.0, 200.0);
        assert!((w - 300.0).abs() < 0.01); // nominal(200) + 100
    }

    #[test]
    fn charstring_width_hmoveto_no_width() {
        // hmoveto with 1 arg (dx) → default width
        let mut cs = encode_dict_int(50); // dx
        cs.push(22); // hmoveto
        let w = extract_charstring_width(&cs, 600.0, 0.0);
        assert!((w - 600.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_hmoveto_with_width() {
        // width + dx, hmoveto → nominal + width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(300)); // width
        cs.extend_from_slice(&encode_dict_int(50)); // dx
        cs.push(22); // hmoveto
        let w = extract_charstring_width(&cs, 0.0, 200.0);
        assert!((w - 500.0).abs() < 0.01); // nominal(200) + 300
    }

    #[test]
    fn charstring_width_vmoveto_with_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(-100)); // width
        cs.extend_from_slice(&encode_dict_int(50)); // dy
        cs.push(4); // vmoveto
        let w = extract_charstring_width(&cs, 0.0, 500.0);
        assert!((w - 400.0).abs() < 0.01); // nominal(500) + (-100)
    }

    #[test]
    fn charstring_width_rmoveto_no_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(10)); // dx
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(21); // rmoveto
        let w = extract_charstring_width(&cs, 700.0, 0.0);
        assert!((w - 700.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_rmoveto_with_width() {
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(50)); // width
        cs.extend_from_slice(&encode_dict_int(10)); // dx
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(21); // rmoveto
        let w = extract_charstring_width(&cs, 0.0, 100.0);
        assert!((w - 150.0).abs() < 0.01); // nominal(100) + 50
    }

    #[test]
    fn charstring_width_hstem_no_width() {
        // hstem with 2 args (pair) → default width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(10)); // y
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(1); // hstem
        let w = extract_charstring_width(&cs, 400.0, 0.0);
        assert!((w - 400.0).abs() < 0.01);
    }

    #[test]
    fn charstring_width_hstem_with_width() {
        // hstem with 3 args (width + pair) → nominal + width
        let mut cs = Vec::new();
        cs.extend_from_slice(&encode_dict_int(200)); // width
        cs.extend_from_slice(&encode_dict_int(10)); // y
        cs.extend_from_slice(&encode_dict_int(20)); // dy
        cs.push(1); // hstem
        let w = extract_charstring_width(&cs, 0.0, 300.0);
        assert!((w - 500.0).abs() < 0.01); // nominal(300) + 200
    }

    // ========== TDD: Full CFF parsing integration tests ==========

    #[test]
    fn parse_cff_basic_widths() {
        // 3 glyphs with widths 250, 500, 750 (default=0, nominal=0)
        let data = build_cff_data(0, 0, &[250, 500, 750]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 3);
        assert!((widths.get_width(0).unwrap() - 250.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 500.0).abs() < 0.01);
        assert!((widths.get_width(2).unwrap() - 750.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_with_default_width() {
        // Glyphs with default width (all same width = defaultWidthX)
        let data = build_cff_data(600, 0, &[600, 600, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 3);
        for gid in 0..3 {
            assert!(
                (widths.get_width(gid).unwrap() - 600.0).abs() < 0.01,
                "glyph {} width mismatch",
                gid
            );
        }
    }

    #[test]
    fn parse_cff_with_nominal_width() {
        // nominal=200, widths: 300 (encoded as 100), 500 (encoded as 300)
        let data = build_cff_data(0, 200, &[300, 500]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 2);
        assert!((widths.get_width(0).unwrap() - 300.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 500.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_mixed_default_and_explicit() {
        // default=500, nominal=400
        // Glyph 0: width=500 (default, no operand)
        // Glyph 1: width=600 (encoded as 600-400=200)
        let data = build_cff_data(500, 400, &[500, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");

        assert_eq!(widths.num_glyphs(), 2);
        assert!((widths.get_width(0).unwrap() - 500.0).abs() < 0.01);
        assert!((widths.get_width(1).unwrap() - 600.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_single_glyph() {
        let data = build_cff_data(0, 0, &[800]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert_eq!(widths.num_glyphs(), 1);
        assert!((widths.get_width(0).unwrap() - 800.0).abs() < 0.01);
    }

    #[test]
    fn parse_cff_out_of_range_glyph() {
        let data = build_cff_data(0, 0, &[500, 600]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert!(widths.get_width(2).is_none());
        assert!(widths.get_width(100).is_none());
    }

    #[test]
    fn parse_cff_empty_data() {
        assert!(parse_cff_widths(&[]).is_none());
    }

    #[test]
    fn parse_cff_truncated_header() {
        assert!(parse_cff_widths(&[1, 0]).is_none());
    }

    #[test]
    fn parse_cff_invalid_version() {
        // Version 2 (not supported)
        let mut data = build_cff_data(0, 0, &[500]);
        data[0] = 2; // major version 2
        assert!(parse_cff_widths(&data).is_none());
    }

    #[test]
    fn parse_cff_negative_width() {
        // Some fonts have negative widths (rare but valid)
        let data = build_cff_data(0, 0, &[-100]);
        let widths = parse_cff_widths(&data).expect("should parse CFF");
        assert!((widths.get_width(0).unwrap() - (-100.0)).abs() < 0.01);
    }
}
