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
            let code = u32::from(string_bytes[i]) << 8 | u32::from(string_bytes[i + 1]);
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
mod tests {
    use super::*;

    /// Constant width function: returns 600 glyph units for all characters.
    /// This simulates a monospaced font where each glyph is 0.6 em wide.
    fn constant_width(_char_code: u32) -> f64 {
        600.0
    }

    /// Variable width function for testing different widths per character.
    fn variable_width(char_code: u32) -> f64 {
        match char_code {
            32 => 250.0, // space
            65 => 722.0, // A
            66 => 667.0, // B
            _ => 500.0,  // default
        }
    }

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    // --- RawChar construction ---

    #[test]
    fn raw_char_construction() {
        let rc = RawChar {
            char_code: 65,
            displacement: 7.2,
            text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 700.0],
        };
        assert_eq!(rc.char_code, 65);
        assert_approx(rc.displacement, 7.2);
        assert_eq!(rc.text_matrix, [1.0, 0.0, 0.0, 1.0, 72.0, 700.0]);
    }

    #[test]
    fn raw_char_clone() {
        let rc = RawChar {
            char_code: 65,
            displacement: 7.2,
            text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 700.0],
        };
        let cloned = rc.clone();
        assert_eq!(rc, cloned);
    }

    // --- TjElement ---

    #[test]
    fn tj_element_string_variant() {
        let elem = TjElement::String(vec![65, 66, 67]);
        if let TjElement::String(bytes) = &elem {
            assert_eq!(bytes, &[65, 66, 67]);
        } else {
            panic!("expected String variant");
        }
    }

    #[test]
    fn tj_element_adjustment_variant() {
        let elem = TjElement::Adjustment(-120.0);
        if let TjElement::Adjustment(adj) = &elem {
            assert_approx(*adj, -120.0);
        } else {
            panic!("expected Adjustment variant");
        }
    }

    // --- Tj operator: show_string ---

    #[test]
    fn tj_empty_string() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 12.0);

        let chars = show_string(&mut ts, &[], &constant_width);
        assert!(chars.is_empty());
    }

    #[test]
    fn tj_single_char() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 12.0);
        ts.move_text_position(72.0, 700.0);

        let chars = show_string(&mut ts, &[65], &constant_width); // 'A'
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].char_code, 65);
        // Text matrix captured at render position
        assert_eq!(chars[0].text_matrix, [1.0, 0.0, 0.0, 1.0, 72.0, 700.0]);
        // displacement = (600/1000 * 12 + 0 + 0) * 1.0 = 7.2
        assert_approx(chars[0].displacement, 7.2);
    }

    #[test]
    fn tj_multiple_chars() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // "AB" = bytes [65, 66]
        let chars = show_string(&mut ts, &[65, 66], &constant_width);
        assert_eq!(chars.len(), 2);

        // First char at position (100, 500)
        assert_eq!(chars[0].char_code, 65);
        assert_approx(chars[0].text_matrix[4], 100.0);

        // Second char: displaced by (600/1000 * 10) * 1.0 = 6.0
        assert_eq!(chars[1].char_code, 66);
        assert_approx(chars[1].text_matrix[4], 106.0);
    }

    #[test]
    fn tj_with_char_spacing() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_char_spacing(2.0);
        ts.move_text_position(100.0, 500.0);

        let chars = show_string(&mut ts, &[65, 66], &constant_width);

        // First char displacement: (600/1000 * 10 + 2.0) * 1.0 = 8.0
        assert_approx(chars[0].displacement, 8.0);
        // Second char starts at 100 + 8 = 108
        assert_approx(chars[1].text_matrix[4], 108.0);
    }

    #[test]
    fn tj_word_spacing_applied_only_for_space() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_word_spacing(5.0);
        ts.move_text_position(100.0, 500.0);

        // Space (32) gets word spacing; 'A' (65) does not
        let chars = show_string(&mut ts, &[32, 65], &constant_width);

        // Space: (600/1000 * 10 + 0 + 5.0) * 1.0 = 11.0
        assert_approx(chars[0].displacement, 11.0);
        assert_eq!(chars[0].char_code, 32);

        // 'A': (600/1000 * 10 + 0 + 0) * 1.0 = 6.0
        assert_approx(chars[1].displacement, 6.0);
        assert_eq!(chars[1].char_code, 65);
    }

    #[test]
    fn tj_with_h_scaling() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_h_scaling(50.0); // 50%
        ts.move_text_position(100.0, 500.0);

        let chars = show_string(&mut ts, &[65], &constant_width);

        // displacement: (600/1000 * 10 + 0) * 0.5 = 3.0
        assert_approx(chars[0].displacement, 3.0);
    }

    #[test]
    fn tj_combined_spacing_and_scaling() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_char_spacing(1.0);
        ts.set_word_spacing(3.0);
        ts.set_h_scaling(200.0); // 200%
        ts.move_text_position(0.0, 0.0);

        // Space char: (600/1000 * 10 + 1.0 + 3.0) * 2.0 = (6 + 1 + 3) * 2 = 20.0
        let chars = show_string(&mut ts, &[32], &constant_width);
        assert_approx(chars[0].displacement, 20.0);

        // Non-space: (600/1000 * 10 + 1.0 + 0) * 2.0 = (6 + 1) * 2 = 14.0
        let chars = show_string(&mut ts, &[65], &constant_width);
        assert_approx(chars[0].displacement, 14.0);
    }

    #[test]
    fn tj_advances_text_position() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        show_string(&mut ts, &[65, 66], &constant_width);

        // After 2 chars: 100 + 6.0 + 6.0 = 112.0
        assert_approx(ts.text_matrix().e, 112.0);
    }

    #[test]
    fn tj_does_not_change_line_matrix() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        let line_matrix_before = *ts.line_matrix();
        show_string(&mut ts, &[65, 66, 67], &constant_width);

        // Line matrix should not change during Tj
        assert_eq!(*ts.line_matrix(), line_matrix_before);
    }

    #[test]
    fn tj_with_variable_widths() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(0.0, 0.0);

        // 'A' (722), space (250), 'B' (667)
        let chars = show_string(&mut ts, &[65, 32, 66], &variable_width);

        assert_eq!(chars.len(), 3);
        // A: (722/1000 * 10) * 1.0 = 7.22
        assert_approx(chars[0].displacement, 7.22);
        // space: (250/1000 * 10) * 1.0 = 2.5
        assert_approx(chars[1].displacement, 2.5);
        // B: (667/1000 * 10) * 1.0 = 6.67
        assert_approx(chars[2].displacement, 6.67);

        // Verify positions
        assert_approx(chars[0].text_matrix[4], 0.0);
        assert_approx(chars[1].text_matrix[4], 7.22);
        assert_approx(chars[2].text_matrix[4], 9.72); // 7.22 + 2.5
    }

    #[test]
    fn tj_with_scaled_text_matrix() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 1.0); // font_size = 1 (scaling via Tm)
        // Text matrix with 12x scaling (simulates 12pt font via matrix)
        ts.set_text_matrix(12.0, 0.0, 0.0, 12.0, 72.0, 700.0);

        let chars = show_string(&mut ts, &[65], &constant_width);

        assert_eq!(chars[0].text_matrix, [12.0, 0.0, 0.0, 12.0, 72.0, 700.0]);
        // displacement = (600/1000 * 1.0) * 1.0 = 0.6
        assert_approx(chars[0].displacement, 0.6);
        // advance_text_position(0.6) pre-multiplies [1 0 0 1 0.6 0] × [12 0 0 12 72 700]
        // new_e = 0.6 * 12 + 72 = 79.2
        assert_approx(ts.text_matrix().e, 79.2);
    }

    // --- TJ operator: show_string_with_positioning ---

    #[test]
    fn tj_array_empty() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);

        let chars = show_string_with_positioning(&mut ts, &[], &constant_width);
        assert!(chars.is_empty());
    }

    #[test]
    fn tj_array_strings_only() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        let elements = vec![
            TjElement::String(vec![65]), // "A"
            TjElement::String(vec![66]), // "B"
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].char_code, 65);
        assert_eq!(chars[1].char_code, 66);
        // Same as two consecutive Tj calls
        assert_approx(chars[0].text_matrix[4], 100.0);
        assert_approx(chars[1].text_matrix[4], 106.0);
    }

    #[test]
    fn tj_array_with_negative_adjustment_adds_space() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // [(A) -200 (B)]
        // -200 means move right: tx = -(-200)/1000 * 10 * 1.0 = +2.0
        let elements = vec![
            TjElement::String(vec![65]),
            TjElement::Adjustment(-200.0),
            TjElement::String(vec![66]),
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert_eq!(chars.len(), 2);
        assert_approx(chars[0].text_matrix[4], 100.0);
        // A advance (6.0) + adjustment (+2.0) = 8.0 offset
        assert_approx(chars[1].text_matrix[4], 108.0);
    }

    #[test]
    fn tj_array_with_positive_adjustment_tightens() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // [(A) 200 (B)] — positive adjustment moves LEFT (kerning/tightening)
        let elements = vec![
            TjElement::String(vec![65]),
            TjElement::Adjustment(200.0),
            TjElement::String(vec![66]),
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert_eq!(chars.len(), 2);
        // A at 100, advance 6.0, then adjustment -(200/1000)*10 = -2.0
        // B at 100 + 6.0 - 2.0 = 104.0
        assert_approx(chars[1].text_matrix[4], 104.0);
    }

    #[test]
    fn tj_array_adjustment_only() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // TJ array with only adjustments (no characters)
        let elements = vec![
            TjElement::Adjustment(-500.0), // move right by 5.0
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert!(chars.is_empty());
        // Position should be advanced by -(-500)/1000 * 10 = +5.0
        assert_approx(ts.text_matrix().e, 105.0);
    }

    #[test]
    fn tj_array_multi_byte_strings() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(0.0, 0.0);

        // [(AB) -100 (CD)]
        let elements = vec![
            TjElement::String(vec![65, 66]),
            TjElement::Adjustment(-100.0),
            TjElement::String(vec![67, 68]),
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert_eq!(chars.len(), 4);
        assert_eq!(chars[0].char_code, 65); // A
        assert_eq!(chars[1].char_code, 66); // B
        assert_eq!(chars[2].char_code, 67); // C
        assert_eq!(chars[3].char_code, 68); // D

        // A at 0, B at 6, adjustment +1.0, C at 13.0, D at 19.0
        assert_approx(chars[0].text_matrix[4], 0.0);
        assert_approx(chars[1].text_matrix[4], 6.0);
        assert_approx(chars[2].text_matrix[4], 13.0); // 6 + 6 + 1
        assert_approx(chars[3].text_matrix[4], 19.0); // 13 + 6
    }

    #[test]
    fn tj_array_adjustment_with_h_scaling() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_h_scaling(50.0); // 50%
        ts.move_text_position(100.0, 500.0);

        // [(A) -1000 (B)] — adjustment of -1000 thousandths
        let elements = vec![
            TjElement::String(vec![65]),
            TjElement::Adjustment(-1000.0),
            TjElement::String(vec![66]),
        ];
        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        // A displacement: (600/1000 * 10) * 0.5 = 3.0
        assert_approx(chars[0].displacement, 3.0);
        // Adjustment: -(-1000/1000) * 10 * 0.5 = 5.0
        // B at: 100 + 3.0 + 5.0 = 108.0
        assert_approx(chars[1].text_matrix[4], 108.0);
    }

    // --- ' operator: quote_show_string ---

    #[test]
    fn quote_moves_to_next_line_then_shows() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        let chars = quote_show_string(&mut ts, &[65], &constant_width);

        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].char_code, 65);
        // T* moved to (72, 700 - 14) = (72, 686)
        assert_approx(chars[0].text_matrix[4], 72.0);
        assert_approx(chars[0].text_matrix[5], 686.0);
    }

    #[test]
    fn quote_empty_string() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        let chars = quote_show_string(&mut ts, &[], &constant_width);

        assert!(chars.is_empty());
        // T* should still have moved the position
        assert_approx(ts.text_matrix().e, 72.0);
        assert_approx(ts.text_matrix().f, 686.0);
    }

    #[test]
    fn quote_updates_line_matrix() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        quote_show_string(&mut ts, &[65], &constant_width);

        // Line matrix should reflect the T* move
        assert_approx(ts.line_matrix().e, 72.0);
        assert_approx(ts.line_matrix().f, 686.0);
    }

    // --- " operator: double_quote_show_string ---

    #[test]
    fn double_quote_sets_spacing_then_shows() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        let chars = double_quote_show_string(&mut ts, 3.0, 1.0, &[65], &constant_width);

        assert_eq!(chars.len(), 1);
        // Word spacing and char spacing should be set
        assert_approx(ts.word_spacing, 3.0);
        assert_approx(ts.char_spacing, 1.0);
        // T* moved to (72, 686), then showed 'A'
        assert_approx(chars[0].text_matrix[4], 72.0);
        assert_approx(chars[0].text_matrix[5], 686.0);
        // displacement includes the new char_spacing: (600/1000 * 10 + 1.0) * 1.0 = 7.0
        assert_approx(chars[0].displacement, 7.0);
    }

    #[test]
    fn double_quote_word_spacing_applies_to_space() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        // Show a space character — word spacing should apply
        let chars = double_quote_show_string(&mut ts, 5.0, 0.0, &[32], &constant_width);

        // displacement: (600/1000 * 10 + 0 + 5.0) * 1.0 = 11.0
        assert_approx(chars[0].displacement, 11.0);
    }

    // --- Position tracking across multiple operators ---

    #[test]
    fn position_tracking_across_multiple_tj() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // First Tj: "AB"
        let _chars1 = show_string(&mut ts, &[65, 66], &constant_width);
        // Second Tj: "CD"
        let chars2 = show_string(&mut ts, &[67, 68], &constant_width);

        // After "AB": position = 100 + 6 + 6 = 112
        // C at 112, D at 118
        assert_approx(chars2[0].text_matrix[4], 112.0);
        assert_approx(chars2[1].text_matrix[4], 118.0);
    }

    #[test]
    fn position_tracking_tj_then_quote() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(14.0);
        ts.move_text_position(72.0, 700.0);

        // First line: Tj "A"
        show_string(&mut ts, &[65], &constant_width);

        // Next line via ': "B"
        let chars = quote_show_string(&mut ts, &[66], &constant_width);

        // T* moves to (72, 686) — x resets to line start
        assert_approx(chars[0].text_matrix[4], 72.0);
        assert_approx(chars[0].text_matrix[5], 686.0);
    }

    #[test]
    fn position_tracking_multiple_quote_lines() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.set_leading(12.0);
        ts.move_text_position(72.0, 700.0);

        // Three lines using quote operator
        let chars1 = quote_show_string(&mut ts, &[65], &constant_width);
        let chars2 = quote_show_string(&mut ts, &[66], &constant_width);
        let chars3 = quote_show_string(&mut ts, &[67], &constant_width);

        // Line 1 at y = 700 - 12 = 688
        assert_approx(chars1[0].text_matrix[5], 688.0);
        // Line 2 at y = 688 - 12 = 676
        assert_approx(chars2[0].text_matrix[5], 676.0);
        // Line 3 at y = 676 - 12 = 664
        assert_approx(chars3[0].text_matrix[5], 664.0);
    }

    // --- Realistic sequences ---

    #[test]
    fn realistic_text_block_sequence() {
        let mut ts = TextState::new();

        // Setup
        ts.set_font("Helvetica".to_string(), 12.0);
        ts.set_leading(14.0);

        // BT
        ts.begin_text();

        // 72 700 Td
        ts.move_text_position(72.0, 700.0);

        // (Hello) Tj
        let hello = show_string(&mut ts, b"Hello", &constant_width);
        assert_eq!(hello.len(), 5);
        assert_eq!(hello[0].char_code, b'H' as u32);
        assert_eq!(hello[4].char_code, b'o' as u32);
        assert_approx(hello[0].text_matrix[4], 72.0);

        // T* — move to next line
        ts.move_to_next_line();

        // (World) Tj
        let world = show_string(&mut ts, b"World", &constant_width);
        assert_approx(world[0].text_matrix[4], 72.0);
        assert_approx(world[0].text_matrix[5], 686.0); // 700 - 14

        // ET
        ts.end_text();
    }

    #[test]
    fn realistic_tj_array_kerned_text() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("Times-Roman".to_string(), 12.0);
        ts.move_text_position(72.0, 700.0);

        // [(T) 80 (o) -15 (da) 10 (y)] — typical kerned "Today"
        let elements = vec![
            TjElement::String(vec![b'T']),
            TjElement::Adjustment(80.0), // tighten To pair
            TjElement::String(vec![b'o']),
            TjElement::Adjustment(-15.0), // loosen od pair
            TjElement::String(vec![b'd', b'a']),
            TjElement::Adjustment(10.0), // tighten ay pair
            TjElement::String(vec![b'y']),
        ];

        let chars = show_string_with_positioning(&mut ts, &elements, &constant_width);

        assert_eq!(chars.len(), 5);
        assert_eq!(chars[0].char_code, b'T' as u32);
        assert_eq!(chars[1].char_code, b'o' as u32);
        assert_eq!(chars[2].char_code, b'd' as u32);
        assert_eq!(chars[3].char_code, b'a' as u32);
        assert_eq!(chars[4].char_code, b'y' as u32);

        // T at 72.0
        assert_approx(chars[0].text_matrix[4], 72.0);
        // After T (7.2) + adjustment -(80/1000)*12 = -0.96
        // o at 72 + 7.2 - 0.96 = 78.24
        assert_approx(chars[1].text_matrix[4], 78.24);
    }

    #[test]
    fn zero_width_font_produces_zero_displacement() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        let zero_width = |_: u32| 0.0;
        let chars = show_string(&mut ts, &[65, 66], &zero_width);

        assert_eq!(chars.len(), 2);
        assert_approx(chars[0].displacement, 0.0);
        assert_approx(chars[1].displacement, 0.0);
        // Both chars at same position since no advancement
        assert_approx(chars[0].text_matrix[4], 100.0);
        assert_approx(chars[1].text_matrix[4], 100.0);
    }

    #[test]
    fn zero_font_size_produces_only_spacing_displacement() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 0.0); // zero font size
        ts.set_char_spacing(2.0);
        ts.move_text_position(100.0, 500.0);

        let chars = show_string(&mut ts, &[65], &constant_width);

        // displacement: (600/1000 * 0 + 2.0) * 1.0 = 2.0
        assert_approx(chars[0].displacement, 2.0);
    }

    // --- CID font 2-byte character codes: show_string_cid ---

    #[test]
    fn cid_show_string_two_byte_codes() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 12.0);
        ts.move_text_position(72.0, 700.0);

        // Two 2-byte characters: 0x4E2D (中) and 0x6587 (文)
        let bytes = vec![0x4E, 0x2D, 0x65, 0x87];
        let chars = show_string_cid(&mut ts, &bytes, &constant_width);

        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].char_code, 0x4E2D);
        assert_eq!(chars[1].char_code, 0x6587);
    }

    #[test]
    fn cid_show_string_empty() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 12.0);

        let chars = show_string_cid(&mut ts, &[], &constant_width);
        assert!(chars.is_empty());
    }

    #[test]
    fn cid_show_string_odd_byte_length() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 12.0);

        // 3 bytes: first two form 0x4E2D, last byte is 0x41
        let bytes = vec![0x4E, 0x2D, 0x41];
        let chars = show_string_cid(&mut ts, &bytes, &constant_width);

        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].char_code, 0x4E2D);
        assert_eq!(chars[1].char_code, 0x41);
    }

    #[test]
    fn cid_show_string_single_two_byte_code() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // Single 2-byte character: 0x0041 (should be 'A' in Unicode)
        let bytes = vec![0x00, 0x41];
        let chars = show_string_cid(&mut ts, &bytes, &constant_width);

        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].char_code, 0x0041);
        assert_eq!(chars[0].text_matrix, [1.0, 0.0, 0.0, 1.0, 100.0, 500.0]);
        // displacement = (600/1000 * 10 + 0 + 0) * 1.0 = 6.0
        assert_approx(chars[0].displacement, 6.0);
    }

    #[test]
    fn cid_show_string_advances_position() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(100.0, 500.0);

        // Two 2-byte codes
        let bytes = vec![0x4E, 0x2D, 0x65, 0x87];
        let chars = show_string_cid(&mut ts, &bytes, &constant_width);

        assert_eq!(chars.len(), 2);
        assert_approx(chars[0].text_matrix[4], 100.0);
        // Second char advanced by 6.0 (600/1000 * 10)
        assert_approx(chars[1].text_matrix[4], 106.0);
    }

    #[test]
    fn cid_show_string_with_variable_widths() {
        let mut ts = TextState::new();
        ts.begin_text();
        ts.set_font("F1".to_string(), 10.0);
        ts.move_text_position(0.0, 0.0);

        // Custom width function for CID codes
        let cid_width = |code: u32| -> f64 {
            match code {
                0x4E2D => 1000.0, // full-width CJK
                0x6587 => 1000.0,
                _ => 500.0,
            }
        };

        let bytes = vec![0x4E, 0x2D, 0x65, 0x87];
        let chars = show_string_cid(&mut ts, &bytes, &cid_width);

        // 0x4E2D width: (1000/1000 * 10) = 10.0
        assert_approx(chars[0].displacement, 10.0);
        // 0x6587 at position 10.0
        assert_approx(chars[1].text_matrix[4], 10.0);
        assert_approx(chars[1].displacement, 10.0);
    }
}
