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
mod tests;
