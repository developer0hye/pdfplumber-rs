//! Text string rendering for CJK and vertical writing modes.
//!
//! Supplements the core `text_renderer` module with show_string variants for
//! CJK encodings, vertical writing modes, and TJ array positioning.

use std::collections::HashMap;
use crate::cjk_encoding;
use crate::error::BackendError;
use crate::handler::{CharEvent, ContentHandler};
use crate::interpreter_state::InterpreterState;
use crate::text_renderer::{TjElement, show_string, show_string_cid, show_string_with_positioning_mode, RawChar};
use crate::text_state::TextState;
use crate::tokenizer::Operand;
use pdfplumber_core::{ExtractOptions, ExtractWarning, ExtractWarningCode, FontEncoding};
use super::{CachedFont, MarkedContentEntry, get_f64, get_i64, operand_to_name, operand_to_string_bytes, operand_to_f32};
use super::font::get_width_fn;
use super::events::emit_char_events;

pub(super) fn show_string_cjk(
    text_state: &mut TextState,
    string_bytes: &[u8],
    get_width: &dyn Fn(u32) -> f64,
    encoding: &'static encoding_rs::Encoding,
) -> Vec<crate::text_renderer::RawChar> {
    let decoded = cjk_encoding::decode_cjk_string(string_bytes, encoding);
    let mut chars = Vec::with_capacity(decoded.len());

    for dc in decoded {
        let text_matrix = text_state.text_matrix_array();
        let w0 = get_width(dc.char_code);
        let font_size = text_state.font_size;
        let char_spacing = text_state.char_spacing;
        let word_spacing = if dc.char_code == 32 {
            text_state.word_spacing
        } else {
            0.0
        };
        let h_scaling = text_state.h_scaling_normalized();
        let tx = ((w0 / 1000.0) * font_size + char_spacing + word_spacing) * h_scaling;

        chars.push(crate::text_renderer::RawChar {
            char_code: dc.char_code,
            displacement: tx,
            text_matrix,
        });

        text_state.advance_text_position(tx);
    }

    chars
}

/// Show a CID string in vertical writing mode (WMode=1).
///
/// Uses vertical advance (w1y from W2/DW2) instead of horizontal advance.
/// Text position advances vertically (downward in PDF coordinates).
pub(super) fn show_string_cid_vertical(
    text_state: &mut TextState,
    string_bytes: &[u8],
    get_vertical_advance: &dyn Fn(u32) -> f64,
) -> Vec<crate::text_renderer::RawChar> {
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

        let text_matrix = text_state.text_matrix_array();

        // For vertical mode, use the full em square (1000 glyph units) as the
        // displacement for bbox width. In vertical writing, the glyph occupies
        // the full em square horizontally, matching pdfminer's bbox behavior.
        let displacement = 1000.0;
        let font_size = text_state.font_size;

        chars.push(crate::text_renderer::RawChar {
            char_code,
            displacement,
            text_matrix,
        });

        // Vertical advance: ty = (w1y / 1000) * font_size
        // w1y is typically negative (e.g., -1000), so ty is negative → moves down in PDF coords
        let w1y = get_vertical_advance(char_code);
        let ty = (w1y / 1000.0) * font_size;
        text_state.advance_text_position_vertical(ty);
    }

    chars
}

/// TJ operator with CJK-aware byte decoding.
///
/// Like `show_string_with_positioning_mode` but uses CJK variable-length byte
/// decoding when a CJK encoding is provided. Falls back to 2-byte CID mode when
/// encoding is `None`.
pub(super) fn show_string_with_positioning_cjk(
    text_state: &mut TextState,
    elements: &[TjElement],
    get_width: &dyn Fn(u32) -> f64,
    encoding: Option<&'static encoding_rs::Encoding>,
) -> Vec<crate::text_renderer::RawChar> {
    let mut chars = Vec::new();

    for element in elements {
        match element {
            TjElement::String(bytes) => {
                let mut sub_chars = if let Some(enc) = encoding {
                    show_string_cjk(text_state, bytes, get_width, enc)
                } else {
                    show_string_cid(text_state, bytes, get_width)
                };
                chars.append(&mut sub_chars);
            }
            TjElement::Adjustment(adj) => {
                let font_size = text_state.font_size;
                let h_scaling = text_state.h_scaling_normalized();
                let tx = -(adj / 1000.0) * font_size * h_scaling;
                text_state.advance_text_position(tx);
            }
        }
    }

    chars
}

/// TJ operator for vertical writing mode (WMode=1).
///
/// Adjustments in TJ arrays affect the vertical position instead of horizontal.
pub(super) fn show_string_with_positioning_vertical(
    text_state: &mut TextState,
    elements: &[TjElement],
    get_vertical_advance: &dyn Fn(u32) -> f64,
) -> Vec<crate::text_renderer::RawChar> {
    let mut chars = Vec::new();

    for element in elements {
        match element {
            TjElement::String(bytes) => {
                let mut sub_chars =
                    show_string_cid_vertical(text_state, bytes, get_vertical_advance);
                chars.append(&mut sub_chars);
            }
            TjElement::Adjustment(adj) => {
                // For vertical mode, TJ adjustments affect the vertical position
                let font_size = text_state.font_size;
                let ty = -(adj / 1000.0) * font_size;
                text_state.advance_text_position_vertical(ty);
            }
        }
    }

    chars
}

/// Build a vertical advance lookup function for a cached font (WMode=1).
/// Returns w1y from W2/DW2 metrics.
fn get_vertical_advance_fn(cached: Option<&CachedFont>) -> Box<dyn Fn(u32) -> f64 + '_> {
    match cached {
        Some(cf) if cf.is_cid_font => {
            if let Some(ref cid_met) = cf.cid_metrics {
                Box::new(move |code: u32| cid_met.get_vertical_w1(code))
            } else {
                Box::new(|_: u32| -1000.0) // default w1
            }
        }
        _ => Box::new(|_: u32| -1000.0), // default w1
    }
}

pub(super) fn handle_tj(
    tstate: &mut TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    operands: &[Operand],
    font_cache: &HashMap<String, CachedFont>,
    marked_content_stack: &[MarkedContentEntry],
) {
    let string_bytes = match operands.first().and_then(operand_to_string_bytes) {
        Some(bytes) => bytes,
        None => return,
    };

    let cached = font_cache.get(&tstate.font_name);
    let width_fn = get_width_fn(cached);
    let is_vertical = cached.is_some_and(|c| c.writing_mode == 1);

    let raw_chars = if is_vertical {
        let vert_fn = get_vertical_advance_fn(cached);
        show_string_cid_vertical(tstate, string_bytes, &*vert_fn)
    } else if let Some(enc) = cached.and_then(|c| c.cjk_encoding) {
        show_string_cjk(tstate, string_bytes, &*width_fn, enc)
    } else if cached.is_some_and(|c| c.is_cid_font) {
        show_string_cid(tstate, string_bytes, &*width_fn)
    } else {
        show_string(tstate, string_bytes, &*width_fn)
    };

    emit_char_events(
        raw_chars,
        tstate,
        gstate,
        handler,
        cached,
        marked_content_stack,
    );
}

pub(super) fn handle_tj_array(
    tstate: &mut TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    operands: &[Operand],
    font_cache: &HashMap<String, CachedFont>,
    marked_content_stack: &[MarkedContentEntry],
) {
    let array = match operands.first() {
        Some(Operand::Array(arr)) => arr,
        _ => return,
    };

    // Convert Operand array to TjElement array
    let elements: Vec<TjElement> = array
        .iter()
        .filter_map(|o| match o {
            Operand::LiteralString(s) | Operand::HexString(s) => Some(TjElement::String(s.clone())),
            Operand::Integer(i) => Some(TjElement::Adjustment(*i as f64)),
            Operand::Real(f) => Some(TjElement::Adjustment(*f)),
            _ => None,
        })
        .collect();

    let cached = font_cache.get(&tstate.font_name);
    let width_fn = get_width_fn(cached);
    let is_vertical = cached.is_some_and(|c| c.writing_mode == 1);

    let raw_chars = if is_vertical {
        let vert_fn = get_vertical_advance_fn(cached);
        show_string_with_positioning_vertical(tstate, &elements, &*vert_fn)
    } else {
        let cjk_enc = cached.and_then(|c| c.cjk_encoding);
        let is_cid = cached.is_some_and(|c| c.is_cid_font);
        if cjk_enc.is_some() || is_cid {
            show_string_with_positioning_cjk(tstate, &elements, &*width_fn, cjk_enc)
        } else {
            show_string_with_positioning_mode(tstate, &elements, &*width_fn, false)
        }
    };

    emit_char_events(
        raw_chars,
        tstate,
        gstate,
        handler,
        cached,
        marked_content_stack,
    );
}

