//! Text rendering operators (Tj, TJ, ', ") for the content stream interpreter.
//!
//! Processes text-showing operators that produce character glyph output,
//! advancing the text position within the [`TextState`].
//!
//! At this stage, character codes are raw byte values — Unicode mapping
//! (US-012) and font metrics for accurate widths (US-013) come later.

use crate::text_state::TextState;

/// A raw character extracted from a text rendering operator.
///
/// Captures the character code and text state snapshot at the moment
/// of rendering. Unicode mapping and precise font metrics will refine
/// this data in later processing stages.
#[derive(Debug, Clone, PartialEq)]
pub struct RawChar {
    /// The character code from the PDF string byte.
    pub char_code: u32,
    /// The displacement in text space units used to advance the text position.
    ///
    /// Calculated as: `((w0 / 1000) * font_size + char_spacing + word_spacing_if_space) * h_scaling`.
    pub displacement: f64,
    /// Snapshot of the text matrix at the moment this character was rendered.
    pub text_matrix: [f64; 6],
}

/// An element of a TJ array operand.
///
/// TJ arrays contain a mix of strings (to show) and numeric adjustments
/// (for kerning/spacing).
#[derive(Debug, Clone, PartialEq)]
pub enum TjElement {
    /// A string of bytes to show (each byte is a character code).
    String(Vec<u8>),
    /// A numeric adjustment in thousandths of a unit of text space.
    /// Positive values move left (tighten), negative move right (loosen).
    Adjustment(f64),
}

/// `Tj` operator: show a string.
///
/// Each byte in `string_bytes` is treated as a single character code.
/// For each byte:
/// 1. Snapshot the current text matrix as the character's render position
/// 2. Look up the glyph width via `get_width(char_code)` (in glyph space, 1/1000 units)
/// 3. Calculate text-space displacement and advance the text position
///
/// Returns a [`Vec<RawChar>`] with one entry per byte.
pub fn show_string(
    text_state: &mut TextState,
    string_bytes: &[u8],
    get_width: &dyn Fn(u32) -> f64,
) -> Vec<RawChar> {
    let mut chars = Vec::with_capacity(string_bytes.len());

    for &byte in string_bytes {
        let char_code = u32::from(byte);

        // Snapshot the text matrix before advancing
        let text_matrix = text_state.text_matrix_array();

        // Calculate displacement in text space
        let w0 = get_width(char_code);
        let font_size = text_state.font_size;
        let char_spacing = text_state.char_spacing;
        let word_spacing = if char_code == 32 {
            text_state.word_spacing
        } else {
            0.0
        };
        let h_scaling = text_state.h_scaling_normalized();

        let tx = ((w0 / 1000.0) * font_size + char_spacing + word_spacing) * h_scaling;

        chars.push(RawChar {
            char_code,
            displacement: tx,
            text_matrix,
        });

        // Advance text position
        text_state.advance_text_position(tx);
    }

    chars
}

/// `TJ` operator: show strings with positioning adjustments.
///
/// Processes an array of [`TjElement`]s. Strings are rendered like `Tj`;
/// numeric adjustments shift the text position (in thousandths of a unit
/// of text space). Positive adjustments move left, negative move right.
pub fn show_string_with_positioning(
    text_state: &mut TextState,
    elements: &[TjElement],
    get_width: &dyn Fn(u32) -> f64,
) -> Vec<RawChar> {
    let mut chars = Vec::new();

    for element in elements {
        match element {
            TjElement::String(bytes) => {
                let mut sub_chars = show_string(text_state, bytes, get_width);
                chars.append(&mut sub_chars);
            }
            TjElement::Adjustment(adj) => {
                // PDF spec: positive adjustment moves left, negative moves right
                // tx = -(adj / 1000) * font_size * h_scaling
                let font_size = text_state.font_size;
                let h_scaling = text_state.h_scaling_normalized();
                let tx = -(adj / 1000.0) * font_size * h_scaling;
                text_state.advance_text_position(tx);
            }
        }
    }

    chars
}

/// `Tj` operator for CID fonts: show a string using 2-byte character codes.
///
/// For CID fonts (Type0/composite), each character code is formed from two
/// consecutive bytes in big-endian order. If the byte string has an odd length,
/// the last byte is treated as a single-byte code.
pub fn show_string_cid(
    text_state: &mut TextState,
    string_bytes: &[u8],
    get_width: &dyn Fn(u32) -> f64,
) -> Vec<RawChar> {
    let mut chars = Vec::with_capacity(string_bytes.len() / 2);
    let mut i = 0;

    while i < string_bytes.len() {
        let char_code = if i + 1 < string_bytes.len() {
            let code = (u32::from(string_bytes[i]) << 8) | u32::from(string_bytes[i + 1]);
            i += 2;
            code
        } else {
            let code = u32::from(string_bytes[i]);
            i += 1;
            code
        };

        // Snapshot the text matrix before advancing
        let text_matrix = text_state.text_matrix_array();

        // Calculate displacement in text space
        let w0 = get_width(char_code);
        let font_size = text_state.font_size;
        let char_spacing = text_state.char_spacing;
        let word_spacing = if char_code == 32 {
            text_state.word_spacing
        } else {
            0.0
        };
        let h_scaling = text_state.h_scaling_normalized();

        let tx = ((w0 / 1000.0) * font_size + char_spacing + word_spacing) * h_scaling;

        chars.push(RawChar {
            char_code,
            displacement: tx,
            text_matrix,
        });

        // Advance text position
        text_state.advance_text_position(tx);
    }

    chars
}

/// `TJ` operator with CID mode: show strings with positioning adjustments.
///
/// Like [`show_string_with_positioning`] but when `cid_mode` is true, string
/// bytes are decoded as 2-byte character codes (for CID/Type0 fonts).
pub fn show_string_with_positioning_mode(
    text_state: &mut TextState,
    elements: &[TjElement],
    get_width: &dyn Fn(u32) -> f64,
    cid_mode: bool,
) -> Vec<RawChar> {
    let mut chars = Vec::new();

    for element in elements {
        match element {
            TjElement::String(bytes) => {
                let mut sub_chars = if cid_mode {
                    show_string_cid(text_state, bytes, get_width)
                } else {
                    show_string(text_state, bytes, get_width)
                };
                chars.append(&mut sub_chars);
            }
            TjElement::Adjustment(adj) => {
                // PDF spec: positive adjustment moves left, negative moves right
                let font_size = text_state.font_size;
                let h_scaling = text_state.h_scaling_normalized();
                let tx = -(adj / 1000.0) * font_size * h_scaling;
                text_state.advance_text_position(tx);
            }
        }
    }

    chars
}

/// `'` (single quote) operator: move to next line and show a string.
///
/// Equivalent to `T*` followed by `Tj`.
pub fn quote_show_string(
    text_state: &mut TextState,
    string_bytes: &[u8],
    get_width: &dyn Fn(u32) -> f64,
) -> Vec<RawChar> {
    text_state.move_to_next_line(); // T*
    show_string(text_state, string_bytes, get_width) // Tj
}

/// `"` (double quote) operator: set spacing, move to next line, and show a string.
///
/// Equivalent to: `aw Tw`, `ac Tc`, then `string '`.
pub fn double_quote_show_string(
    text_state: &mut TextState,
    word_spacing: f64,
    char_spacing: f64,
    string_bytes: &[u8],
    get_width: &dyn Fn(u32) -> f64,
) -> Vec<RawChar> {
    text_state.set_word_spacing(word_spacing); // aw Tw
    text_state.set_char_spacing(char_spacing); // ac Tc
    quote_show_string(text_state, string_bytes, get_width) // string '
}


#[cfg(test)]
mod tests;
