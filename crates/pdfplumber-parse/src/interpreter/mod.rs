//! Content stream interpreter.
//!
//! Interprets tokenized PDF content stream operators, maintaining graphics and
//! text state, and emitting events to a [`ContentHandler`]. Handles Form XObject
//! recursion via the `Do` operator.

use std::collections::HashMap;

use crate::cid_font::{
    CidFontMetrics, extract_cid_font_metrics, get_descendant_font, get_type0_encoding,
    is_type0_font, parse_predefined_cmap_name, strip_subset_prefix,
};
use crate::cjk_encoding;
use crate::cmap::CMap;
use crate::color_space::resolve_color_space_name;
use crate::error::BackendError;
use crate::font_metrics::{FontMetrics, extract_font_metrics};
use crate::handler::{CharEvent, ContentHandler, ImageEvent, PaintOp, PathEvent};
use crate::interpreter_state::InterpreterState;
use crate::lopdf_backend::object_to_f64;
use crate::text_renderer::{
    TjElement, show_string, show_string_cid, show_string_with_positioning_mode,
};
use crate::text_state::TextState;
use crate::tokenizer::{Operand, Operator, tokenize_lenient};
use pdfplumber_core::{
    ExtractOptions, ExtractWarning, ExtractWarningCode, FillRule, FontEncoding, PathBuilder,
    StandardEncoding, glyph_name_to_char,
};

/// Cached font information for the interpreter.
struct CachedFont {
    metrics: FontMetrics,
    cmap: Option<CMap>,
    base_name: String,
    /// CID font metrics (present for Type0/CID fonts).
    cid_metrics: Option<CidFontMetrics>,
    /// Whether this is a CID (composite/Type0) font.
    is_cid_font: bool,
    /// Writing mode: 0 = horizontal, 1 = vertical.
    /// Used in US-041 for vertical writing mode support.
    writing_mode: u8,
    /// Font encoding from the /Encoding entry (for simple fonts).
    encoding: Option<FontEncoding>,
    /// CJK encoding for predefined CMap encodings (e.g., GBK-EUC-H).
    /// When present, used for variable-length byte decoding and Unicode conversion.
    cjk_encoding: Option<&'static encoding_rs::Encoding>,
    /// Whether the font uses Identity-H or Identity-V encoding CMap.
    /// When true and CIDSystemInfo ordering is "Identity", CID values
    /// are treated as Unicode codepoints in the fallback chain.
    is_identity_encoding: bool,
}

/// Entry on the marked content stack, tracking BMC/BDC nesting.
#[derive(Debug, Clone)]
struct MarkedContentEntry {
    /// Tag name (e.g., "P", "Span", "Artifact").
    tag: String,
    /// Marked content identifier from BDC properties, if present.
    mcid: Option<u32>,
}

/// Extract MCID from a BDC operator's properties operand (inline dictionary).
fn extract_mcid_from_operands(op: &Operator) -> Option<u32> {
    for operand in &op.operands {
        if let Operand::Dictionary(entries) = operand {
            for (key, value) in entries {
                if key == "MCID" {
                    return match value {
                        Operand::Integer(i) => Some(*i as u32),
                        Operand::Real(f) => Some(*f as u32),
                        _ => None,
                    };
                }
            }
        }
    }
    None
}

/// Extract the tag name from a BMC/BDC operator's operands.
fn extract_tag_name(op: &Operator) -> Option<String> {
    op.operands.first().and_then(|o| match o {
        Operand::Name(name) => Some(name.clone()),
        _ => None,
    })
}

/// Interpret a content stream and emit events to the handler.
///
/// Processes tokenized PDF operators, updates graphics/text state, and calls
/// handler methods for text, path, and image events. Handles Form XObject
/// recursion via the `Do` operator.
///
/// # Arguments
///
/// * `doc` - The lopdf document (for resolving references)
/// * `stream_bytes` - Decoded content stream bytes
/// * `resources` - Resources dictionary for this scope
/// * `handler` - Event callback handler
/// * `options` - Resource limits and settings
/// * `depth` - Current recursion depth (0 for page-level)
/// * `gstate` - Current graphics/interpreter state
/// * `tstate` - Current text state
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpret_content_stream(
    doc: &lopdf::Document,
    stream_bytes: &[u8],
    resources: &lopdf::Dictionary,
    handler: &mut dyn ContentHandler,
    options: &ExtractOptions,
    depth: usize,
    gstate: &mut InterpreterState,
    tstate: &mut TextState,
) -> Result<(), BackendError> {
    if depth > options.max_recursion_depth {
        return Err(BackendError::Interpreter(format!(
            "Form XObject recursion depth {} exceeds limit {}",
            depth, options.max_recursion_depth
        )));
    }

    let (operators, tokenize_warnings) = tokenize_lenient(stream_bytes);
    for warning_msg in &tokenize_warnings {
        handler.on_warning(ExtractWarning::with_code(
            ExtractWarningCode::MalformedObject,
            warning_msg.clone(),
        ));
        #[cfg(feature = "tracing")]
        tracing::warn!(warning = %warning_msg, "content stream tokenization error (recovered)");
    }
    let mut font_cache: HashMap<String, CachedFont> = HashMap::new();
    let mut path_builder = PathBuilder::new(*gstate.ctm());
    let mut marked_content_stack: Vec<MarkedContentEntry> = Vec::new();

    for (op_index, op) in operators.iter().enumerate() {
        match op.name.as_str() {
            // --- Graphics state operators ---
            "q" => gstate.save_state_with_text(tstate.save_snapshot()),
            "Q" => {
                if let Some(Some(snapshot)) = gstate.restore_state_with_text() {
                    tstate.restore_snapshot(snapshot);
                }
                path_builder.set_ctm(*gstate.ctm());
            }
            "cm" => {
                if op.operands.len() >= 6 {
                    let a = get_f64(&op.operands, 0).unwrap_or(1.0);
                    let b = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let c = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let d = get_f64(&op.operands, 3).unwrap_or(1.0);
                    let e = get_f64(&op.operands, 4).unwrap_or(0.0);
                    let f = get_f64(&op.operands, 5).unwrap_or(0.0);
                    gstate.concat_matrix(a, b, c, d, e, f);
                    path_builder.set_ctm(*gstate.ctm());
                }
            }
            "w" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    gstate.set_line_width(v);
                }
            }
            "d" => {
                // Dash pattern: [array] phase d
                if op.operands.len() >= 2 {
                    if let Operand::Array(ref arr) = op.operands[0] {
                        let dash_array: Vec<f64> = arr
                            .iter()
                            .filter_map(|o| match o {
                                Operand::Integer(i) => Some(*i as f64),
                                Operand::Real(f) => Some(*f),
                                _ => None,
                            })
                            .collect();
                        let phase = get_f64(&op.operands, 1).unwrap_or(0.0);
                        gstate.set_dash_pattern(dash_array, phase);
                    }
                }
            }
            "gs" => {
                // Extended Graphics State
                if let Some(Operand::Name(name)) = op.operands.first() {
                    apply_ext_gstate(doc, resources, gstate, name);
                }
            }
            "J" | "j" | "M" | "i" | "ri" => {
                // Line cap, line join, miter limit, flatness, rendering intent
                // Not yet fully implemented — ignore
            }

            // --- Color operators ---
            "G" => {
                if let Some(g) = get_f32(&op.operands, 0) {
                    gstate.set_stroking_gray(g);
                }
            }
            "g" => {
                if let Some(g) = get_f32(&op.operands, 0) {
                    gstate.set_non_stroking_gray(g);
                }
            }
            "RG" => {
                if op.operands.len() >= 3 {
                    let r = get_f32(&op.operands, 0).unwrap_or(0.0);
                    let g = get_f32(&op.operands, 1).unwrap_or(0.0);
                    let b = get_f32(&op.operands, 2).unwrap_or(0.0);
                    gstate.set_stroking_rgb(r, g, b);
                }
            }
            "rg" => {
                if op.operands.len() >= 3 {
                    let r = get_f32(&op.operands, 0).unwrap_or(0.0);
                    let g = get_f32(&op.operands, 1).unwrap_or(0.0);
                    let b = get_f32(&op.operands, 2).unwrap_or(0.0);
                    gstate.set_non_stroking_rgb(r, g, b);
                }
            }
            "K" => {
                if op.operands.len() >= 4 {
                    let c = get_f32(&op.operands, 0).unwrap_or(0.0);
                    let m = get_f32(&op.operands, 1).unwrap_or(0.0);
                    let y = get_f32(&op.operands, 2).unwrap_or(0.0);
                    let k = get_f32(&op.operands, 3).unwrap_or(0.0);
                    gstate.set_stroking_cmyk(c, m, y, k);
                }
            }
            "k" => {
                if op.operands.len() >= 4 {
                    let c = get_f32(&op.operands, 0).unwrap_or(0.0);
                    let m = get_f32(&op.operands, 1).unwrap_or(0.0);
                    let y = get_f32(&op.operands, 2).unwrap_or(0.0);
                    let k = get_f32(&op.operands, 3).unwrap_or(0.0);
                    gstate.set_non_stroking_cmyk(c, m, y, k);
                }
            }
            "CS" => {
                if let Some(Operand::Name(name)) = op.operands.first() {
                    if let Some(cs) = resolve_color_space_name(name, doc, resources) {
                        gstate.set_stroking_color_space(cs);
                    }
                }
            }
            "cs" => {
                if let Some(Operand::Name(name)) = op.operands.first() {
                    if let Some(cs) = resolve_color_space_name(name, doc, resources) {
                        gstate.set_non_stroking_color_space(cs);
                    }
                }
            }
            "SC" | "SCN" => {
                let components: Vec<f32> = op.operands.iter().filter_map(operand_to_f32).collect();
                gstate.set_stroking_color(&components);
            }
            "sc" | "scn" => {
                let components: Vec<f32> = op.operands.iter().filter_map(operand_to_f32).collect();
                gstate.set_non_stroking_color(&components);
            }

            // --- Text state operators ---
            "BT" => tstate.begin_text(),
            "ET" => tstate.end_text(),
            "Tf" => {
                if op.operands.len() >= 2 {
                    let font_name = operand_to_name(&op.operands[0]);
                    let size = get_f64(&op.operands, 1).unwrap_or(0.0);
                    tstate.set_font(font_name.clone(), size);
                    load_font_if_needed(
                        doc,
                        resources,
                        &font_name,
                        &mut font_cache,
                        handler,
                        options,
                        op_index,
                    );
                }
            }
            "Tm" => {
                if op.operands.len() >= 6 {
                    let a = get_f64(&op.operands, 0).unwrap_or(1.0);
                    let b = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let c = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let d = get_f64(&op.operands, 3).unwrap_or(1.0);
                    let e = get_f64(&op.operands, 4).unwrap_or(0.0);
                    let f = get_f64(&op.operands, 5).unwrap_or(0.0);
                    tstate.set_text_matrix(a, b, c, d, e, f);
                }
            }
            "Td" => {
                if op.operands.len() >= 2 {
                    let tx = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let ty = get_f64(&op.operands, 1).unwrap_or(0.0);
                    tstate.move_text_position(tx, ty);
                }
            }
            "TD" => {
                if op.operands.len() >= 2 {
                    let tx = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let ty = get_f64(&op.operands, 1).unwrap_or(0.0);
                    tstate.move_text_position_and_set_leading(tx, ty);
                }
            }
            "T*" => tstate.move_to_next_line(),
            "Tc" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    tstate.set_char_spacing(v);
                }
            }
            "Tw" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    tstate.set_word_spacing(v);
                }
            }
            "Tz" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    tstate.set_h_scaling(v);
                }
            }
            "TL" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    tstate.set_leading(v);
                }
            }
            "Tr" => {
                if let Some(v) = get_i64(&op.operands, 0) {
                    if let Some(mode) = crate::text_state::TextRenderMode::from_i64(v) {
                        tstate.set_render_mode(mode);
                    }
                }
            }
            "Ts" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    tstate.set_rise(v);
                }
            }

            // --- Text rendering operators ---
            "Tj" => {
                handle_tj(
                    tstate,
                    gstate,
                    handler,
                    &op.operands,
                    &font_cache,
                    &marked_content_stack,
                );
            }
            "TJ" => {
                handle_tj_array(
                    tstate,
                    gstate,
                    handler,
                    &op.operands,
                    &font_cache,
                    &marked_content_stack,
                );
            }
            "'" => {
                // T* then Tj
                tstate.move_to_next_line();
                handle_tj(
                    tstate,
                    gstate,
                    handler,
                    &op.operands,
                    &font_cache,
                    &marked_content_stack,
                );
            }
            "\"" => {
                // aw ac (string) "
                if op.operands.len() >= 3 {
                    if let Some(aw) = get_f64(&op.operands, 0) {
                        tstate.set_word_spacing(aw);
                    }
                    if let Some(ac) = get_f64(&op.operands, 1) {
                        tstate.set_char_spacing(ac);
                    }
                    tstate.move_to_next_line();
                    // Show the string (3rd operand)
                    let string_operands = vec![op.operands[2].clone()];
                    handle_tj(
                        tstate,
                        gstate,
                        handler,
                        &string_operands,
                        &font_cache,
                        &marked_content_stack,
                    );
                }
            }

            // --- XObject operator ---
            "Do" => {
                if let Some(Operand::Name(name)) = op.operands.first() {
                    if let Err(e) = handle_do(
                        doc, resources, handler, options, depth, gstate, tstate, name,
                    ) {
                        // Resource limit errors (e.g., recursion depth) must propagate
                        if matches!(&e, BackendError::Interpreter(msg) if msg.contains("recursion depth"))
                        {
                            return Err(e);
                        }
                        let msg = format!(
                            "Do operator for XObject '{}' failed (recovered): {}",
                            name, e
                        );
                        handler.on_warning(ExtractWarning::with_code(
                            ExtractWarningCode::MalformedObject,
                            &msg,
                        ));
                        #[cfg(feature = "tracing")]
                        tracing::warn!(xobject = %name, error = %e, "Do operator failed (recovered)");
                    }
                }
            }

            // --- Path construction operators ---
            "m" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 2 {
                    let x = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y = get_f64(&op.operands, 1).unwrap_or(0.0);
                    path_builder.move_to(x, y);
                }
            }
            "l" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 2 {
                    let x = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y = get_f64(&op.operands, 1).unwrap_or(0.0);
                    path_builder.line_to(x, y);
                }
            }
            "c" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 6 {
                    let x1 = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y1 = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let x2 = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let y2 = get_f64(&op.operands, 3).unwrap_or(0.0);
                    let x3 = get_f64(&op.operands, 4).unwrap_or(0.0);
                    let y3 = get_f64(&op.operands, 5).unwrap_or(0.0);
                    path_builder.curve_to(x1, y1, x2, y2, x3, y3);
                }
            }
            "v" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 4 {
                    let x2 = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y2 = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let x3 = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let y3 = get_f64(&op.operands, 3).unwrap_or(0.0);
                    path_builder.curve_to_v(x2, y2, x3, y3);
                }
            }
            "y" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 4 {
                    let x1 = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y1 = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let x3 = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let y3 = get_f64(&op.operands, 3).unwrap_or(0.0);
                    path_builder.curve_to_y(x1, y1, x3, y3);
                }
            }
            "re" => {
                path_builder.set_ctm(*gstate.ctm());
                if op.operands.len() >= 4 {
                    let x = get_f64(&op.operands, 0).unwrap_or(0.0);
                    let y = get_f64(&op.operands, 1).unwrap_or(0.0);
                    let w = get_f64(&op.operands, 2).unwrap_or(0.0);
                    let h = get_f64(&op.operands, 3).unwrap_or(0.0);
                    path_builder.rectangle(x, y, w, h);
                }
            }
            "h" => {
                // closepath
                path_builder.close_path();
            }

            // --- Path painting operators ---
            "S" => {
                let painted = path_builder.stroke(gstate.graphics_state());
                emit_path_event(handler, gstate, &painted, PaintOp::Stroke, None);
            }
            "s" => {
                let painted = path_builder.close_and_stroke(gstate.graphics_state());
                emit_path_event(handler, gstate, &painted, PaintOp::Stroke, None);
            }
            "f" | "F" => {
                let painted = path_builder.fill(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::Fill,
                    Some(FillRule::NonZeroWinding),
                );
            }
            "f*" => {
                let painted = path_builder.fill_even_odd(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::Fill,
                    Some(FillRule::EvenOdd),
                );
            }
            "B" => {
                let painted = path_builder.fill_and_stroke(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::FillAndStroke,
                    Some(FillRule::NonZeroWinding),
                );
            }
            "B*" => {
                let painted = path_builder.fill_even_odd_and_stroke(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::FillAndStroke,
                    Some(FillRule::EvenOdd),
                );
            }
            "b" => {
                let painted = path_builder.close_fill_and_stroke(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::FillAndStroke,
                    Some(FillRule::NonZeroWinding),
                );
            }
            "b*" => {
                let painted = path_builder.close_fill_even_odd_and_stroke(gstate.graphics_state());
                emit_path_event(
                    handler,
                    gstate,
                    &painted,
                    PaintOp::FillAndStroke,
                    Some(FillRule::EvenOdd),
                );
            }
            "n" => {
                path_builder.end_path();
            }

            // --- Clipping operators (no-op for extraction) ---
            "W" | "W*" => {}

            // --- Marked content operators ---
            "BMC" => {
                let tag = extract_tag_name(op).unwrap_or_default();
                marked_content_stack.push(MarkedContentEntry { tag, mcid: None });
            }
            "BDC" => {
                let tag = extract_tag_name(op).unwrap_or_default();
                let mcid = extract_mcid_from_operands(op);
                marked_content_stack.push(MarkedContentEntry { tag, mcid });
            }
            "EMC" => {
                marked_content_stack.pop();
            }
            "MP" | "DP" => {}

            // --- Inline image operator ---
            "BI" => {
                handle_inline_image(op, op_index, gstate, handler);
            }

            // Other operators — skip with optional tracing
            _other => {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    operator = _other,
                    "skipping unrecognized content stream operator"
                );
            }
        }
    }

    Ok(())
}

// --- Operand extraction helpers ---

fn get_f64(operands: &[Operand], index: usize) -> Option<f64> {
    operands.get(index).and_then(|o| match o {
        Operand::Integer(i) => Some(*i as f64),
        Operand::Real(f) => Some(*f),
        _ => None,
    })
}

fn get_f32(operands: &[Operand], index: usize) -> Option<f32> {
    get_f64(operands, index).map(|v| v as f32)
}

fn get_i64(operands: &[Operand], index: usize) -> Option<i64> {
    operands.get(index).and_then(|o| match o {
        Operand::Integer(i) => Some(*i),
        Operand::Real(f) => Some(*f as i64),
        _ => None,
    })
}

fn operand_to_f32(o: &Operand) -> Option<f32> {
    match o {
        Operand::Integer(i) => Some(*i as f32),
        Operand::Real(f) => Some(*f as f32),
        _ => None,
    }
}

fn operand_to_name(o: &Operand) -> String {
    match o {
        Operand::Name(n) => n.clone(),
        _ => String::new(),
    }
}

fn operand_to_string_bytes(o: &Operand) -> Option<&[u8]> {
    match o {
        Operand::LiteralString(s) | Operand::HexString(s) => Some(s),
        _ => None,
    }
}

pub(super) fn operand_to_u32(op: &Operand) -> Option<u32> {
    match op {
        Operand::Integer(i) => Some(*i as u32),
        Operand::Real(f) => Some(*f as u32),
        _ => None,
    }
}

// --- Font loading ---


mod events;
mod font;
mod text;
mod xobjects;

use events::{apply_ext_gstate, emit_char_events, emit_path_event};
use font::{get_width_fn, load_font_if_needed};
use text::{handle_tj, handle_tj_array, show_string_cjk, show_string_cid_vertical, show_string_with_positioning_cjk, show_string_with_positioning_vertical};
use xobjects::{handle_do, handle_form_xobject, handle_image_xobject, handle_inline_image};

#[cfg(test)]
mod tests;
