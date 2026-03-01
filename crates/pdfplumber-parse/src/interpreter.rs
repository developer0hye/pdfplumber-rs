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
use crate::tokenizer::{Operand, Operator, tokenize};
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
    #[allow(dead_code)]
    writing_mode: u8,
    /// Font encoding from the /Encoding entry (for simple fonts).
    encoding: Option<FontEncoding>,
    /// CJK encoding for predefined CMap encodings (e.g., GBK-EUC-H).
    /// When present, used for variable-length byte decoding and Unicode conversion.
    cjk_encoding: Option<&'static encoding_rs::Encoding>,
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

    let operators = tokenize(stream_bytes)?;
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
                    handle_do(
                        doc, resources, handler, options, depth, gstate, tstate, name,
                    )?;
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

            // Other operators — silently ignore
            _ => {}
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

// --- Font loading ---

#[allow(clippy::too_many_arguments)]
fn load_font_if_needed(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    font_name: &str,
    cache: &mut HashMap<String, CachedFont>,
    handler: &mut dyn ContentHandler,
    options: &ExtractOptions,
    op_index: usize,
) {
    if cache.contains_key(font_name) {
        return;
    }

    // Look up /Resources/Font/<font_name>
    let font_dict = (|| -> Option<&lopdf::Dictionary> {
        let fonts_obj = resources.get(b"Font").ok()?;
        let fonts_obj = resolve_ref(doc, fonts_obj);
        let fonts_dict = fonts_obj.as_dict().ok()?;
        let font_obj = fonts_dict.get(font_name.as_bytes()).ok()?;
        let font_obj = resolve_ref(doc, font_obj);
        font_obj.as_dict().ok()
    })();

    let (metrics, cmap, base_name, cid_metrics, is_cid_font, writing_mode, encoding, cjk_enc) =
        if let Some(fd) = font_dict {
            if is_type0_font(fd) {
                // Type0 (composite/CID) font
                let (cid_met, wm) = load_cid_font(doc, fd);

                // Detect CJK encoding from predefined CMap name
                let cjk_enc = get_type0_encoding(fd)
                    .and_then(|enc_name| cjk_encoding::encoding_for_cmap(&enc_name));
                let metrics = if let Some(ref cm) = cid_met {
                    // Create a FontMetrics from CID font data for backward compat
                    FontMetrics::new(
                        Vec::new(),
                        0,
                        0,
                        cm.default_width(),
                        cm.ascent(),
                        cm.descent(),
                        cm.font_bbox(),
                    )
                } else {
                    if options.collect_warnings {
                        handler.on_warning(
                            ExtractWarning::with_operator_context(
                                "CID font metrics not available, using defaults",
                                op_index,
                                font_name,
                            )
                            .set_code(ExtractWarningCode::MissingFont),
                        );
                    }
                    FontMetrics::default_metrics()
                };

                // Extract ToUnicode CMap if present
                let cmap = extract_tounicode_cmap(doc, fd);

                let raw_base_name_owned;
                let raw_base_name =
                    if let Some(n) = fd.get(b"BaseFont").ok().and_then(|o| o.as_name().ok()) {
                        raw_base_name_owned = String::from_utf8_lossy(n).into_owned();
                        raw_base_name_owned.as_str()
                    } else {
                        font_name
                    };
                let base_name = strip_subset_prefix(raw_base_name).to_string();

                (metrics, cmap, base_name, cid_met, true, wm, None, cjk_enc)
            } else {
                // Simple font
                let metrics = match extract_font_metrics(doc, fd) {
                    Ok(m) => m,
                    Err(_) => {
                        if options.collect_warnings {
                            handler.on_warning(
                                ExtractWarning::with_operator_context(
                                    "failed to extract font metrics, using defaults",
                                    op_index,
                                    font_name,
                                )
                                .set_code(ExtractWarningCode::MissingFont),
                            );
                        }
                        FontMetrics::default_metrics()
                    }
                };
                let cmap = extract_tounicode_cmap(doc, fd);
                let encoding = extract_font_encoding(doc, fd);
                let raw_base_name_owned;
                let raw_base_name =
                    if let Some(n) = fd.get(b"BaseFont").ok().and_then(|o| o.as_name().ok()) {
                        raw_base_name_owned = String::from_utf8_lossy(n).into_owned();
                        raw_base_name_owned.as_str()
                    } else {
                        font_name
                    };
                let base_name = strip_subset_prefix(raw_base_name).to_string();

                (metrics, cmap, base_name, None, false, 0, encoding, None)
            }
        } else {
            // Font not found in page resources — use defaults
            if options.collect_warnings {
                handler.on_warning(
                    ExtractWarning::with_operator_context(
                        "font not found in page resources, using defaults",
                        op_index,
                        font_name,
                    )
                    .set_code(ExtractWarningCode::MissingFont),
                );
            }
            (
                FontMetrics::default_metrics(),
                None,
                font_name.to_string(),
                None,
                false,
                0,
                None,
                None,
            )
        };

    cache.insert(
        font_name.to_string(),
        CachedFont {
            metrics,
            cmap,
            base_name,
            cid_metrics,
            is_cid_font,
            writing_mode,
            encoding,
            cjk_encoding: cjk_enc,
        },
    );
}

/// Extract ToUnicode CMap from a font dictionary.
fn extract_tounicode_cmap(doc: &lopdf::Document, fd: &lopdf::Dictionary) -> Option<CMap> {
    let tounicode_obj = fd.get(b"ToUnicode").ok()?;
    let tounicode_obj = resolve_ref(doc, tounicode_obj);
    let stream = tounicode_obj.as_stream().ok()?;
    let data = decode_stream(stream).ok()?;
    CMap::parse(&data).ok()
}

/// Extract font encoding from a simple font dictionary's /Encoding entry.
fn extract_font_encoding(doc: &lopdf::Document, fd: &lopdf::Dictionary) -> Option<FontEncoding> {
    let encoding_obj = fd.get(b"Encoding").ok()?;
    let encoding_obj = resolve_ref(doc, encoding_obj);

    // Case 1: /Encoding is a name (e.g., /WinAnsiEncoding)
    if let Ok(name) = encoding_obj.as_name() {
        let std_enc = match name {
            b"WinAnsiEncoding" => Some(StandardEncoding::WinAnsi),
            b"MacRomanEncoding" => Some(StandardEncoding::MacRoman),
            b"MacExpertEncoding" => Some(StandardEncoding::MacExpert),
            b"StandardEncoding" => Some(StandardEncoding::Standard),
            _ => None,
        };
        return std_enc.map(FontEncoding::from_standard);
    }

    // Case 2: /Encoding is a dictionary with /BaseEncoding and/or /Differences
    if let Ok(enc_dict) = encoding_obj.as_dict() {
        let base = enc_dict
            .get(b"BaseEncoding")
            .ok()
            .and_then(|o| o.as_name().ok())
            .and_then(|name| match name {
                b"WinAnsiEncoding" => Some(StandardEncoding::WinAnsi),
                b"MacRomanEncoding" => Some(StandardEncoding::MacRoman),
                b"MacExpertEncoding" => Some(StandardEncoding::MacExpert),
                b"StandardEncoding" => Some(StandardEncoding::Standard),
                _ => None,
            })
            .unwrap_or(StandardEncoding::Standard);

        let mut enc = FontEncoding::from_standard(base);

        // Apply /Differences array
        if let Ok(diff_obj) = enc_dict.get(b"Differences") {
            let diff_obj = resolve_ref(doc, diff_obj);
            if let Ok(diff_arr) = diff_obj.as_array() {
                let differences = parse_differences_array(diff_arr);
                enc.apply_differences(&differences);
            }
        }

        return Some(enc);
    }

    None
}

/// Parse a PDF /Differences array into (code, char) pairs.
///
/// Format: `[code1 /name1 /name2 ... codeN /nameN ...]`
/// Each integer starts a run; subsequent names are assigned consecutive codes.
fn parse_differences_array(arr: &[lopdf::Object]) -> Vec<(u8, char)> {
    let mut result = Vec::new();
    let mut current_code: Option<u8> = None;

    for obj in arr {
        match obj {
            lopdf::Object::Integer(i) => {
                current_code = Some(*i as u8);
            }
            lopdf::Object::Name(name_bytes) => {
                if let Some(code) = current_code {
                    let name = String::from_utf8_lossy(name_bytes);
                    if let Some(ch) = glyph_name_to_char(&name) {
                        result.push((code, ch));
                    }
                    current_code = Some(code.wrapping_add(1));
                }
            }
            _ => {}
        }
    }

    result
}

/// Load CID font information from a Type0 font dictionary.
fn load_cid_font(
    doc: &lopdf::Document,
    type0_dict: &lopdf::Dictionary,
) -> (Option<CidFontMetrics>, u8) {
    // Determine writing mode from encoding name
    let writing_mode = get_type0_encoding(type0_dict)
        .and_then(|enc| parse_predefined_cmap_name(&enc))
        .map(|info| info.writing_mode)
        .unwrap_or(0);

    // Get descendant CIDFont dictionary
    let cid_metrics = get_descendant_font(doc, type0_dict)
        .and_then(|desc| extract_cid_font_metrics(doc, desc).ok());

    (cid_metrics, writing_mode)
}

// --- Text rendering ---

/// Build a width lookup function for a cached font.
/// For CID fonts, uses CidFontMetrics; for simple fonts, uses FontMetrics.
fn get_width_fn(cached: Option<&CachedFont>) -> Box<dyn Fn(u32) -> f64 + '_> {
    match cached {
        Some(cf) if cf.is_cid_font => {
            if let Some(ref cid_met) = cf.cid_metrics {
                Box::new(move |code: u32| cid_met.get_width(code))
            } else {
                Box::new(move |code: u32| cf.metrics.get_width(code))
            }
        }
        Some(cf) => Box::new(move |code: u32| cf.metrics.get_width(code)),
        None => {
            let default_metrics = FontMetrics::default_metrics();
            Box::new(move |code: u32| default_metrics.get_width(code))
        }
    }
}

/// Show a string using CJK variable-length byte decoding.
///
/// Unlike `show_string_cid` which always reads 2-byte pairs, this function
/// uses the CJK encoding to determine byte boundaries (1 or 2 bytes per char).
fn show_string_cjk(
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

/// TJ operator with CJK-aware byte decoding.
///
/// Like `show_string_with_positioning_mode` but uses CJK variable-length byte
/// decoding when a CJK encoding is provided. Falls back to 2-byte CID mode when
/// encoding is `None`.
fn show_string_with_positioning_cjk(
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

fn handle_tj(
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
    let raw_chars = if let Some(enc) = cached.and_then(|c| c.cjk_encoding) {
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

fn handle_tj_array(
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
    let cjk_enc = cached.and_then(|c| c.cjk_encoding);
    let is_cid = cached.is_some_and(|c| c.is_cid_font);
    let raw_chars = if cjk_enc.is_some() || is_cid {
        show_string_with_positioning_cjk(tstate, &elements, &*width_fn, cjk_enc)
    } else {
        show_string_with_positioning_mode(tstate, &elements, &*width_fn, false)
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

fn emit_char_events(
    raw_chars: Vec<crate::text_renderer::RawChar>,
    tstate: &TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    cached: Option<&CachedFont>,
    marked_content_stack: &[MarkedContentEntry],
) {
    let ctm = gstate.ctm_array();
    let font_name = cached.map_or_else(|| tstate.font_name.clone(), |c| c.base_name.clone());

    for rc in raw_chars {
        // Unicode resolution chain: CMap → FontEncoding → CJK encoding → char::from_u32
        let unicode = cached
            .and_then(|c| {
                // 1. Try ToUnicode CMap (highest priority)
                c.cmap
                    .as_ref()
                    .and_then(|cm| cm.lookup(rc.char_code).map(|s| s.to_string()))
            })
            .or_else(|| {
                // 2. Try font encoding (for simple fonts)
                cached.and_then(|c| {
                    c.encoding.as_ref().and_then(|enc| {
                        if rc.char_code <= 255 {
                            enc.decode(rc.char_code as u8).map(|ch| ch.to_string())
                        } else {
                            None
                        }
                    })
                })
            })
            .or_else(|| {
                // 3. Try CJK encoding (for CID fonts with predefined CMaps like GBK-EUC-H)
                cached.and_then(|c| {
                    c.cjk_encoding.map(|enc| {
                        let bytes = if rc.char_code > 0xFF {
                            vec![(rc.char_code >> 8) as u8, (rc.char_code & 0xFF) as u8]
                        } else {
                            vec![rc.char_code as u8]
                        };
                        let (decoded, _, _) = enc.decode(&bytes);
                        decoded.into_owned()
                    })
                })
            })
            .or_else(|| {
                // 4. Fallback: char::from_u32 for ASCII-range codes
                char::from_u32(rc.char_code).map(|ch| ch.to_string())
            });

        // Use CID font metrics for displacement if available
        let displacement = match cached {
            Some(cf) if cf.is_cid_font => cf
                .cid_metrics
                .as_ref()
                .map_or(600.0, |cm| cm.get_width(rc.char_code)),
            Some(cf) => cf.metrics.get_width(rc.char_code),
            None => 600.0,
        };

        // Ascent/descent for bounding box calculation.
        // Match pdfminer behavior: use the font descriptor's Descent value and
        // derive ascent as (1000 + descent). This keeps height = font_size while
        // anchoring the bottom to the font's actual descent below the baseline.
        // When both Ascent=0 AND Descent=0 (signals "unknown"), use 1000/0 so
        // bbox spans baseline to baseline+fontsize.
        let (ascent, descent) = match cached {
            Some(cf) if cf.metrics.ascent() == 0.0 && cf.metrics.descent() == 0.0 => (1000.0, 0.0),
            Some(cf) => {
                let desc = cf.metrics.descent();
                (1000.0 + desc, desc)
            }
            _ => (750.0, -250.0),
        };

        handler.on_char(CharEvent {
            char_code: rc.char_code,
            unicode,
            font_name: font_name.clone(),
            font_size: tstate.font_size,
            text_matrix: rc.text_matrix,
            ctm,
            displacement,
            char_spacing: tstate.char_spacing,
            word_spacing: tstate.word_spacing,
            h_scaling: tstate.h_scaling_normalized(),
            rise: tstate.rise,
            ascent,
            descent,
            mcid: marked_content_stack.iter().rev().find_map(|mc| mc.mcid),
            tag: marked_content_stack.last().map(|mc| mc.tag.clone()),
        });
    }
}

// --- Path painting ---

/// Emit a PathEvent from a PaintedPath produced by the PathBuilder.
fn emit_path_event(
    handler: &mut dyn ContentHandler,
    gstate: &InterpreterState,
    painted: &pdfplumber_core::PaintedPath,
    paint_op: PaintOp,
    fill_rule: Option<FillRule>,
) {
    if painted.path.segments.is_empty() {
        return;
    }
    handler.on_path_painted(PathEvent {
        segments: painted.path.segments.clone(),
        paint_op,
        line_width: painted.line_width,
        stroking_color: Some(painted.stroke_color.clone()),
        non_stroking_color: Some(painted.fill_color.clone()),
        ctm: gstate.ctm_array(),
        dash_pattern: if painted.dash_pattern.is_solid() {
            None
        } else {
            Some(painted.dash_pattern.clone())
        },
        fill_rule,
    });
}

// --- gs operator: Extended Graphics State ---

/// Look up the named ExtGState from page resources and apply it to the current
/// graphics state. Unknown keys are silently ignored. If the name is not found
/// in resources, this is a no-op (graceful degradation).
fn apply_ext_gstate(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    gstate: &mut InterpreterState,
    name: &str,
) {
    let ext_dict = (|| -> Option<&lopdf::Dictionary> {
        let egs_obj = resources.get(b"ExtGState").ok()?;
        let egs_obj = resolve_ref(doc, egs_obj);
        let egs_dict = egs_obj.as_dict().ok()?;
        let entry = egs_dict.get(name.as_bytes()).ok()?;
        let entry = resolve_ref(doc, entry);
        entry.as_dict().ok()
    })();

    let Some(ext_dict) = ext_dict else {
        return;
    };

    let mut ext = pdfplumber_core::ExtGState::default();

    // /LW — Line width
    if let Ok(obj) = ext_dict.get(b"LW") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.line_width = Some(v);
        }
    }

    // /D — Dash pattern: [dash_array phase]
    if let Ok(obj) = ext_dict.get(b"D") {
        let obj = resolve_ref(doc, obj);
        if let Ok(arr) = obj.as_array() {
            if arr.len() >= 2 {
                if let Ok(dash_arr) = arr[0].as_array() {
                    let dash_array: Vec<f64> = dash_arr
                        .iter()
                        .filter_map(|o| match o {
                            lopdf::Object::Integer(i) => Some(*i as f64),
                            lopdf::Object::Real(f) => Some(*f as f64),
                            _ => None,
                        })
                        .collect();
                    let phase = match &arr[1] {
                        lopdf::Object::Integer(i) => *i as f64,
                        lopdf::Object::Real(f) => *f as f64,
                        _ => 0.0,
                    };
                    ext.dash_pattern = Some(pdfplumber_core::DashPattern::new(dash_array, phase));
                }
            }
        }
    }

    // /CA — Stroking alpha
    if let Ok(obj) = ext_dict.get(b"CA") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.stroke_alpha = Some(v);
        }
    }

    // /ca — Non-stroking alpha
    if let Ok(obj) = ext_dict.get(b"ca") {
        let obj = resolve_ref(doc, obj);
        if let Ok(v) = object_to_f64(obj) {
            ext.fill_alpha = Some(v);
        }
    }

    // /Font — Font array [fontRef size]
    if let Ok(obj) = ext_dict.get(b"Font") {
        let obj = resolve_ref(doc, obj);
        if let Ok(arr) = obj.as_array() {
            if arr.len() >= 2 {
                let font_name = match &arr[0] {
                    lopdf::Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
                    _ => None,
                };
                let font_size = match &arr[1] {
                    lopdf::Object::Integer(i) => Some(*i as f64),
                    lopdf::Object::Real(f) => Some(*f as f64),
                    _ => None,
                };
                if let (Some(name), Some(size)) = (font_name, font_size) {
                    ext.font = Some((name, size));
                }
            }
        }
    }

    gstate.graphics_state_mut().apply_ext_gstate(&ext);
}

// --- Do operator: XObject handling ---

#[allow(clippy::too_many_arguments)]
fn handle_do(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    handler: &mut dyn ContentHandler,
    options: &ExtractOptions,
    depth: usize,
    gstate: &mut InterpreterState,
    tstate: &mut TextState,
    name: &str,
) -> Result<(), BackendError> {
    // Look up /Resources/XObject/<name>
    let xobj_dict = resources.get(b"XObject").map_err(|_| {
        BackendError::Interpreter(format!(
            "no /XObject dictionary in resources for Do /{name}"
        ))
    })?;
    let xobj_dict = resolve_ref(doc, xobj_dict);
    let xobj_dict = xobj_dict.as_dict().map_err(|_| {
        BackendError::Interpreter("/XObject resource is not a dictionary".to_string())
    })?;

    let xobj_entry = xobj_dict.get(name.as_bytes()).map_err(|_| {
        BackendError::Interpreter(format!("XObject /{name} not found in resources"))
    })?;

    let xobj_id = xobj_entry.as_reference().map_err(|_| {
        BackendError::Interpreter(format!("XObject /{name} is not an indirect reference"))
    })?;

    let xobj = doc.get_object(xobj_id).map_err(|e| {
        BackendError::Interpreter(format!("failed to resolve XObject /{name}: {e}"))
    })?;

    let stream = xobj
        .as_stream()
        .map_err(|e| BackendError::Interpreter(format!("XObject /{name} is not a stream: {e}")))?;

    let subtype = stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .unwrap_or(b"");

    match subtype {
        b"Form" => handle_form_xobject(
            doc, stream, name, resources, handler, options, depth, gstate, tstate,
        ),
        b"Image" => {
            handle_image_xobject(stream, name, gstate, handler);
            Ok(())
        }
        _ => {
            // Unknown XObject subtype — ignore
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_form_xobject(
    doc: &lopdf::Document,
    stream: &lopdf::Stream,
    name: &str,
    parent_resources: &lopdf::Dictionary,
    handler: &mut dyn ContentHandler,
    options: &ExtractOptions,
    depth: usize,
    gstate: &mut InterpreterState,
    tstate: &mut TextState,
) -> Result<(), BackendError> {
    // Save graphics state (including text state per PDF spec Table 52)
    gstate.save_state_with_text(tstate.save_snapshot());

    // Apply /Matrix if present (transforms Form XObject space to parent space)
    if let Ok(matrix_obj) = stream.dict.get(b"Matrix") {
        if let Ok(arr) = matrix_obj.as_array() {
            if arr.len() == 6 {
                let vals: Result<Vec<f64>, _> = arr.iter().map(object_to_f64).collect();
                if let Ok(vals) = vals {
                    gstate.concat_matrix(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]);
                }
            }
        }
    }

    // Get Form XObject's resources (fall back to parent resources)
    let form_resources_dict;
    let form_resources = if let Ok(res_obj) = stream.dict.get(b"Resources") {
        let res_obj = resolve_ref(doc, res_obj);
        match res_obj.as_dict() {
            Ok(d) => d,
            Err(_) => parent_resources,
        }
    } else {
        // Check if /Resources is an inline dictionary (common for Form XObjects)
        // The dict.get already handles this, so use parent as fallback
        // But also check if it's an indirect reference in the dict
        if let Ok(res_ref) = stream.dict.get(b"Resources") {
            if let Ok(id) = res_ref.as_reference() {
                if let Ok(obj) = doc.get_object(id) {
                    if let Ok(d) = obj.as_dict() {
                        form_resources_dict = d.clone();
                        &form_resources_dict
                    } else {
                        parent_resources
                    }
                } else {
                    parent_resources
                }
            } else {
                parent_resources
            }
        } else {
            parent_resources
        }
    };

    // Decode stream content
    let content_bytes = decode_stream(stream).map_err(|e| {
        BackendError::Interpreter(format!("failed to decode Form XObject /{name} stream: {e}"))
    })?;

    // Recursively interpret the Form XObject content stream
    interpret_content_stream(
        doc,
        &content_bytes,
        form_resources,
        handler,
        options,
        depth + 1,
        gstate,
        tstate,
    )?;

    // Restore graphics state (including text state per PDF spec Table 52)
    if let Some(Some(snapshot)) = gstate.restore_state_with_text() {
        tstate.restore_snapshot(snapshot);
    }

    Ok(())
}

fn handle_image_xobject(
    stream: &lopdf::Stream,
    name: &str,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
) {
    let width = stream
        .dict
        .get(b"Width")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0) as u32;

    let height = stream
        .dict
        .get(b"Height")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0) as u32;

    let colorspace = stream
        .dict
        .get(b"ColorSpace")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|s| String::from_utf8_lossy(s).into_owned());

    let bits_per_component = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .map(|v| v as u32);

    // Extract the primary filter name from the stream dictionary
    let filter = stream.dict.get(b"Filter").ok().and_then(|o| {
        if let Ok(name) = o.as_name() {
            Some(String::from_utf8_lossy(name).into_owned())
        } else if let Ok(arr) = o.as_array() {
            // For filter arrays, use the last filter (the one that determines the format)
            arr.last()
                .and_then(|item| item.as_name().ok())
                .map(|s| String::from_utf8_lossy(s).into_owned())
        } else {
            None
        }
    });

    handler.on_image(ImageEvent {
        name: name.to_string(),
        ctm: gstate.ctm_array(),
        width,
        height,
        colorspace,
        bits_per_component,
        filter,
    });
}

/// Handle an inline image (BI/ID/EI) operator.
///
/// The tokenizer packs inline image data into a BI operator with two operands:
/// - operands[0]: Array of flattened key-value pairs from the image dictionary
/// - operands[1]: LiteralString containing the raw image data bytes
///
/// Abbreviated keys and values are expanded to their full PDF names.
fn handle_inline_image(
    op: &Operator,
    op_index: usize,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
) {
    if op.operands.len() < 2 {
        return;
    }

    let dict_entries = match &op.operands[0] {
        Operand::Array(arr) => arr,
        _ => return,
    };

    // Parse key-value pairs from the flattened array
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    let mut colorspace: Option<String> = None;
    let mut bits_per_component: Option<u32> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i + 1 < dict_entries.len() {
        let key = match &dict_entries[i] {
            Operand::Name(k) => expand_inline_image_key(k),
            _ => {
                i += 2;
                continue;
            }
        };
        let value = &dict_entries[i + 1];

        match key.as_str() {
            "Width" => {
                if let Some(v) = operand_to_u32(value) {
                    width = v;
                }
            }
            "Height" => {
                if let Some(v) = operand_to_u32(value) {
                    height = v;
                }
            }
            "ColorSpace" => {
                if let Operand::Name(cs) = value {
                    colorspace = Some(expand_inline_image_colorspace(cs));
                }
            }
            "BitsPerComponent" => {
                if let Some(v) = operand_to_u32(value) {
                    bits_per_component = Some(v);
                }
            }
            "Filter" => {
                if let Operand::Name(f) = value {
                    filter = Some(expand_inline_image_filter(f));
                }
            }
            _ => {}
        }

        i += 2;
    }

    handler.on_image(ImageEvent {
        name: format!("inline-{op_index}"),
        ctm: gstate.ctm_array(),
        width,
        height,
        colorspace,
        bits_per_component,
        filter,
    });
}

/// Expand abbreviated inline image dictionary keys to full PDF names.
fn expand_inline_image_key(key: &str) -> String {
    match key {
        "W" => "Width".to_string(),
        "H" => "Height".to_string(),
        "BPC" => "BitsPerComponent".to_string(),
        "CS" => "ColorSpace".to_string(),
        "F" => "Filter".to_string(),
        "DP" => "DecodeParms".to_string(),
        "D" => "Decode".to_string(),
        "I" => "Interpolate".to_string(),
        "IM" => "ImageMask".to_string(),
        other => other.to_string(),
    }
}

/// Expand abbreviated inline image color space names to full PDF names.
fn expand_inline_image_colorspace(cs: &str) -> String {
    match cs {
        "G" => "DeviceGray".to_string(),
        "RGB" => "DeviceRGB".to_string(),
        "CMYK" => "DeviceCMYK".to_string(),
        "I" => "Indexed".to_string(),
        other => other.to_string(),
    }
}

/// Expand abbreviated inline image filter names to full PDF names.
fn expand_inline_image_filter(filter: &str) -> String {
    match filter {
        "AHx" => "ASCIIHexDecode".to_string(),
        "A85" => "ASCII85Decode".to_string(),
        "LZW" => "LZWDecode".to_string(),
        "Fl" => "FlateDecode".to_string(),
        "RL" => "RunLengthDecode".to_string(),
        "CCF" => "CCITTFaxDecode".to_string(),
        "DCT" => "DCTDecode".to_string(),
        other => other.to_string(),
    }
}

/// Convert an operand to u32, supporting Integer and Real types.
fn operand_to_u32(op: &Operand) -> Option<u32> {
    match op {
        Operand::Integer(i) => Some(*i as u32),
        Operand::Real(f) => Some(*f as u32),
        _ => None,
    }
}

// --- Helpers ---

/// Resolve an indirect reference, returning the referenced object.
/// If the object is not a reference, returns it as-is.
fn resolve_ref<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Decode a PDF stream, decompressing if necessary.
fn decode_stream(stream: &lopdf::Stream) -> Result<Vec<u8>, BackendError> {
    // Check if stream has filters
    if stream.dict.get(b"Filter").is_ok() {
        stream
            .decompressed_content()
            .map_err(|e| BackendError::Interpreter(format!("stream decompression failed: {e}")))
    } else {
        Ok(stream.content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{CharEvent, ContentHandler, ImageEvent};
    use lopdf::Object;

    // --- Collecting handler ---

    struct CollectingHandler {
        chars: Vec<CharEvent>,
        images: Vec<ImageEvent>,
        warnings: Vec<ExtractWarning>,
    }

    impl CollectingHandler {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                images: Vec::new(),
                warnings: Vec::new(),
            }
        }
    }

    impl ContentHandler for CollectingHandler {
        fn on_char(&mut self, event: CharEvent) {
            self.chars.push(event);
        }
        fn on_image(&mut self, event: ImageEvent) {
            self.images.push(event);
        }
        fn on_warning(&mut self, warning: ExtractWarning) {
            self.warnings.push(warning);
        }
    }

    // --- Helper to create a minimal lopdf document for testing ---

    fn empty_resources() -> lopdf::Dictionary {
        lopdf::Dictionary::new()
    }

    fn default_options() -> ExtractOptions {
        ExtractOptions::default()
    }

    // --- Basic text interpretation tests ---

    #[test]
    fn interpret_simple_text() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // "Hello" = 5 characters
        assert_eq!(handler.chars.len(), 5);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'e' as u32);
        assert_eq!(handler.chars[4].char_code, b'o' as u32);
        assert_eq!(handler.chars[0].font_size, 12.0);
    }

    #[test]
    fn interpret_tj_array() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT /F1 12 Tf [(H) -20 (i)] TJ ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'i' as u32);
    }

    #[test]
    fn interpret_ctm_passed_to_char_events() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"1 0 0 1 10 20 cm BT /F1 12 Tf (A) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].ctm, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    }

    // --- Recursion limit tests ---

    #[test]
    fn recursion_depth_zero_allowed() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        let result = interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn recursion_depth_exceeds_limit() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        let mut opts = ExtractOptions::default();
        opts.max_recursion_depth = 3;

        let result = interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &opts,
            4, // depth > max
            &mut gstate,
            &mut tstate,
        );
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("recursion depth"));
    }

    // --- Graphics state tests ---

    #[test]
    fn interpret_q_q_state_save_restore() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        // Set color, save, change color, restore
        let stream = b"0.5 g q 1 0 0 rg Q";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // After Q, fill color should be restored to gray 0.5
        assert_eq!(
            gstate.graphics_state().fill_color,
            pdfplumber_core::Color::Gray(0.5)
        );
    }

    // --- CID font / Identity-H tests ---

    /// Build a resources dictionary containing a Type0 font with Identity-H encoding.
    fn make_cid_font_resources(doc: &mut lopdf::Document) -> lopdf::Dictionary {
        use lopdf::{Object, Stream, dictionary};

        // ToUnicode CMap: map 0x4E2D → U+4E2D (中), 0x6587 → U+6587 (文)
        let tounicode_data = b"\
            /CIDInit /ProcSet findresource begin\n\
            12 dict begin\n\
            begincmap\n\
            /CMapName /Adobe-Identity-UCS def\n\
            /CMapType 2 def\n\
            1 begincodespacerange\n\
            <0000> <FFFF>\n\
            endcodespacerange\n\
            2 beginbfchar\n\
            <4E2D> <4E2D>\n\
            <6587> <6587>\n\
            endbfchar\n\
            endcmap\n";
        let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
        let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

        // CIDFont dictionary
        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
            "DW" => Object::Integer(1000),
            "CIDToGIDMap" => "Identity",
            "CIDSystemInfo" => Object::Dictionary(dictionary! {
                "Registry" => Object::String("Adobe".as_bytes().to_vec(), lopdf::StringFormat::Literal),
                "Ordering" => Object::String("Identity".as_bytes().to_vec(), lopdf::StringFormat::Literal),
                "Supplement" => Object::Integer(0),
            }),
        };
        let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

        // Type0 font dictionary with Identity-H encoding
        let type0_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MSGothic",
            "Encoding" => "Identity-H",
            "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
            "ToUnicode" => Object::Reference(tounicode_id),
        };
        let type0_id = doc.add_object(Object::Dictionary(type0_dict));

        // Resources with Font entry
        dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => Object::Reference(type0_id),
            }),
        }
    }

    #[test]
    fn interpret_cid_font_identity_h_two_byte_codes() {
        let mut doc = lopdf::Document::with_version("1.5");
        let resources = make_cid_font_resources(&mut doc);

        // Content stream: use CID font F1 and show 2-byte character codes
        // 0x4E2D = 中, 0x6587 = 文
        let stream = b"BT /F1 12 Tf <4E2D6587> Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // Should produce 2 characters (2-byte codes), not 4 (1-byte)
        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, 0x4E2D);
        assert_eq!(handler.chars[1].char_code, 0x6587);
        // Unicode should be resolved via ToUnicode CMap
        assert_eq!(handler.chars[0].unicode, Some("中".to_string()));
        assert_eq!(handler.chars[1].unicode, Some("文".to_string()));
        assert_eq!(handler.chars[0].font_name, "MSGothic");
    }

    #[test]
    fn interpret_cid_font_tj_array_two_byte_codes() {
        let mut doc = lopdf::Document::with_version("1.5");
        let resources = make_cid_font_resources(&mut doc);

        // TJ array with 2-byte CID strings and adjustments
        let stream = b"BT /F1 12 Tf [<4E2D> -100 <6587>] TJ ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, 0x4E2D);
        assert_eq!(handler.chars[1].char_code, 0x6587);
    }

    #[test]
    fn interpret_subset_font_name_stripped() {
        let mut doc = lopdf::Document::with_version("1.5");

        use lopdf::{Object, Stream, dictionary};

        // Create a ToUnicode CMap
        let tounicode_data = b"\
            beginbfchar\n\
            <4E2D> <4E2D>\n\
            endbfchar\n";
        let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
        let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

        // CIDFont with subset prefix
        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "ABCDEF+MSGothic",
            "DW" => Object::Integer(1000),
            "CIDToGIDMap" => "Identity",
        };
        let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

        // Type0 font with subset prefix in BaseFont
        let type0_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "ABCDEF+MSGothic",
            "Encoding" => "Identity-H",
            "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
            "ToUnicode" => Object::Reference(tounicode_id),
        };
        let type0_id = doc.add_object(Object::Dictionary(type0_dict));

        let resources = dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => Object::Reference(type0_id),
            }),
        };

        let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 1);
        // Subset prefix should be stripped
        assert_eq!(handler.chars[0].font_name, "MSGothic");
    }

    /// Build resources for Identity-V (vertical writing mode).
    fn make_cid_font_resources_identity_v(doc: &mut lopdf::Document) -> lopdf::Dictionary {
        use lopdf::{Object, Stream, dictionary};

        let tounicode_data = b"\
            beginbfchar\n\
            <4E2D> <4E2D>\n\
            endbfchar\n";
        let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
        let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

        let cid_font_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "CIDFontType2",
            "BaseFont" => "MSGothic",
            "DW" => Object::Integer(1000),
            "CIDToGIDMap" => "Identity",
        };
        let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

        let type0_dict = dictionary! {
            "Type" => "Font",
            "Subtype" => "Type0",
            "BaseFont" => "MSGothic",
            "Encoding" => "Identity-V",
            "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
            "ToUnicode" => Object::Reference(tounicode_id),
        };
        let type0_id = doc.add_object(Object::Dictionary(type0_dict));

        dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => Object::Reference(type0_id),
            }),
        }
    }

    #[test]
    fn interpret_cid_font_identity_v_detected() {
        let mut doc = lopdf::Document::with_version("1.5");
        let resources = make_cid_font_resources_identity_v(&mut doc);

        // Show a CID character with Identity-V encoding
        let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // Should still produce characters (Identity-V uses same CID=charcode mapping)
        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].char_code, 0x4E2D);
        assert_eq!(handler.chars[0].unicode, Some("中".to_string()));
    }

    // --- Warning emission tests ---

    #[test]
    fn interpret_missing_font_emits_warning() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources(); // No fonts defined
        // Use font F1 which is not in resources
        let stream = b"BT /F1 12 Tf (Hi) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // Should emit a warning about missing font
        assert!(!handler.warnings.is_empty());
        assert!(
            handler.warnings[0]
                .description
                .contains("font not found in page resources"),
            "expected 'font not found' warning, got: {}",
            handler.warnings[0].description
        );
        assert_eq!(
            handler.warnings[0].font_name,
            Some("F1".to_string()),
            "warning should include font name"
        );
        assert!(
            handler.warnings[0].operator_index.is_some(),
            "warning should include operator index"
        );

        // Characters should still be extracted (using default metrics)
        assert_eq!(handler.chars.len(), 2);
    }

    #[test]
    fn interpret_no_warnings_when_collection_disabled() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT /F1 12 Tf (Hi) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        let opts = ExtractOptions {
            collect_warnings: false,
            ..ExtractOptions::default()
        };

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &opts,
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // No warnings should be collected
        assert!(handler.warnings.is_empty());

        // Characters should still be extracted normally
        assert_eq!(handler.chars.len(), 2);
    }

    #[test]
    fn interpret_warnings_do_not_affect_output() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        let stream = b"BT /F1 12 Tf (AB) Tj ET";

        // With warnings enabled
        let mut handler_on = CollectingHandler::new();
        let mut gstate_on = InterpreterState::new();
        let mut tstate_on = TextState::new();
        let opts_on = ExtractOptions {
            collect_warnings: true,
            ..ExtractOptions::default()
        };
        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler_on,
            &opts_on,
            0,
            &mut gstate_on,
            &mut tstate_on,
        )
        .unwrap();

        // With warnings disabled
        let mut handler_off = CollectingHandler::new();
        let mut gstate_off = InterpreterState::new();
        let mut tstate_off = TextState::new();
        let opts_off = ExtractOptions {
            collect_warnings: false,
            ..ExtractOptions::default()
        };
        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler_off,
            &opts_off,
            0,
            &mut gstate_off,
            &mut tstate_off,
        )
        .unwrap();

        // Same output regardless of warning collection
        assert_eq!(handler_on.chars.len(), handler_off.chars.len());
        for (a, b) in handler_on.chars.iter().zip(handler_off.chars.iter()) {
            assert_eq!(a.char_code, b.char_code);
        }
    }

    #[test]
    fn interpret_valid_font_no_warnings() {
        let mut doc = lopdf::Document::with_version("1.5");
        let resources = make_cid_font_resources(&mut doc);
        let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // Valid font should not produce warnings
        assert!(
            handler.warnings.is_empty(),
            "expected no warnings for valid font, got: {:?}",
            handler.warnings
        );
        assert_eq!(handler.chars.len(), 1);
    }

    // --- ExtGState (gs operator) tests ---

    /// Helper to create resources with an ExtGState dictionary.
    fn resources_with_ext_gstate(
        name: &str,
        ext_gstate_dict: lopdf::Dictionary,
    ) -> lopdf::Dictionary {
        use lopdf::dictionary;
        dictionary! {
            "ExtGState" => Object::Dictionary(dictionary! {
                name => Object::Dictionary(ext_gstate_dict),
            }),
        }
    }

    #[test]
    fn gs_applies_line_width() {
        use lopdf::dictionary;
        let doc = lopdf::Document::with_version("1.5");
        let resources = resources_with_ext_gstate(
            "GS1",
            dictionary! {
                "LW" => Object::Real(2.5),
            },
        );
        let stream = b"/GS1 gs";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert!(
            (gstate.graphics_state().line_width - 2.5).abs() < f64::EPSILON,
            "expected line_width 2.5, got {}",
            gstate.graphics_state().line_width
        );
    }

    #[test]
    fn gs_applies_dash_pattern() {
        use lopdf::dictionary;
        let doc = lopdf::Document::with_version("1.5");
        // /D [[3 5] 6] — dash array [3, 5] with phase 6
        let resources = resources_with_ext_gstate(
            "GS1",
            dictionary! {
                "D" => Object::Array(vec![
                    Object::Array(vec![Object::Integer(3), Object::Integer(5)]),
                    Object::Integer(6),
                ]),
            },
        );
        let stream = b"/GS1 gs";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        let dp = &gstate.graphics_state().dash_pattern;
        assert_eq!(dp.dash_array, vec![3.0, 5.0]);
        assert!((dp.dash_phase - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gs_applies_alpha() {
        use lopdf::dictionary;
        let doc = lopdf::Document::with_version("1.5");
        let resources = resources_with_ext_gstate(
            "GS1",
            dictionary! {
                "CA" => Object::Real(0.7),
                "ca" => Object::Real(0.3),
            },
        );
        let stream = b"/GS1 gs";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert!(
            (gstate.graphics_state().stroke_alpha - 0.7).abs() < 1e-6,
            "expected stroke_alpha ~0.7, got {}",
            gstate.graphics_state().stroke_alpha
        );
        assert!(
            (gstate.graphics_state().fill_alpha - 0.3).abs() < 1e-6,
            "expected fill_alpha ~0.3, got {}",
            gstate.graphics_state().fill_alpha
        );
    }

    #[test]
    fn gs_missing_name_produces_no_error() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        // gs with a name that doesn't exist in resources — should not error
        let stream = b"/GS_nonexistent gs";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        // Should not return an error
        let result = interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        );
        assert!(result.is_ok(), "missing ExtGState should not produce error");
    }

    #[test]
    fn gs_unknown_keys_silently_ignored() {
        use lopdf::dictionary;
        let doc = lopdf::Document::with_version("1.5");
        let resources = resources_with_ext_gstate(
            "GS1",
            dictionary! {
                "LW" => Object::Real(3.0),
                "BM" => "Normal",  // blend mode — not handled
                "SM" => Object::Real(0.01),  // smoothness — not handled
            },
        );
        let stream = b"/GS1 gs";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        // LW should be applied, unknown keys silently ignored
        assert!(
            (gstate.graphics_state().line_width - 3.0).abs() < f64::EPSILON,
            "expected line_width 3.0, got {}",
            gstate.graphics_state().line_width
        );
    }

    // --- Inline image (BI/ID/EI) tests ---

    #[test]
    fn interpret_inline_image_emits_image_event() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        // Inline image: 2x2 DeviceRGB, 8 bpc, raw data (12 bytes)
        // BI /W 2 /H 2 /CS /RGB /BPC 8 ID <12 bytes of pixel data> EI
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(b"q 100 0 0 50 72 700 cm BI /W 2 /H 2 /CS /RGB /BPC 8 ID ");
        // 2x2 RGB = 12 bytes of image data
        stream.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]);
        stream.extend_from_slice(b" EI Q");

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            &stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.images.len(), 1);
        let img = &handler.images[0];
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.colorspace, Some("DeviceRGB".to_string()));
        assert_eq!(img.bits_per_component, Some(8));
        // CTM should reflect the transformation: 100 0 0 50 72 700
        assert_eq!(img.ctm, [100.0, 0.0, 0.0, 50.0, 72.0, 700.0]);
        // Name should indicate inline image
        assert!(img.name.starts_with("inline-"));
    }

    #[test]
    fn interpret_inline_image_abbreviated_keys() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        // Use abbreviated keys: /W, /H, /BPC, /CS with abbreviated color space /G
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(b"q 50 0 0 50 10 10 cm BI /W 1 /H 1 /CS /G /BPC 8 ID ");
        stream.push(128); // 1x1 grayscale = 1 byte
        stream.extend_from_slice(b" EI Q");

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            &stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.images.len(), 1);
        let img = &handler.images[0];
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.colorspace, Some("DeviceGray".to_string()));
        assert_eq!(img.bits_per_component, Some(8));
    }

    #[test]
    fn interpret_inline_image_with_filter() {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();
        // BI with abbreviated filter /F /DCT
        let mut stream: Vec<u8> = Vec::new();
        stream
            .extend_from_slice(b"q 200 0 0 100 0 0 cm BI /W 10 /H 10 /CS /RGB /BPC 8 /F /DCT ID ");
        // Fake JPEG data (just a few bytes for testing)
        stream.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]);
        stream.extend_from_slice(b" EI Q");

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            &stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.images.len(), 1);
        let img = &handler.images[0];
        assert_eq!(img.width, 10);
        assert_eq!(img.height, 10);
        assert_eq!(img.filter, Some("DCTDecode".to_string()));
    }

    #[test]
    fn interpret_inline_image_abbreviated_filter_names() {
        // Test all abbreviated filter name mappings
        let abbreviated_to_full = [
            ("AHx", "ASCIIHexDecode"),
            ("A85", "ASCII85Decode"),
            ("LZW", "LZWDecode"),
            ("Fl", "FlateDecode"),
            ("RL", "RunLengthDecode"),
            ("CCF", "CCITTFaxDecode"),
            ("DCT", "DCTDecode"),
        ];

        for (abbrev, full_name) in &abbreviated_to_full {
            let doc = lopdf::Document::with_version("1.5");
            let resources = empty_resources();

            let mut stream: Vec<u8> = Vec::new();
            stream.extend_from_slice(
                format!("q 10 0 0 10 0 0 cm BI /W 1 /H 1 /CS /G /BPC 8 /F /{abbrev} ID ")
                    .as_bytes(),
            );
            stream.push(0); // 1 byte image data
            stream.extend_from_slice(b" EI Q");

            let mut handler = CollectingHandler::new();
            let mut gstate = InterpreterState::new();
            let mut tstate = TextState::new();

            interpret_content_stream(
                &doc,
                &stream,
                &resources,
                &mut handler,
                &default_options(),
                0,
                &mut gstate,
                &mut tstate,
            )
            .unwrap();

            assert_eq!(
                handler.images.len(),
                1,
                "no image emitted for filter abbreviation /{abbrev}"
            );
            assert_eq!(
                handler.images[0].filter,
                Some(full_name.to_string()),
                "filter mismatch for /{abbrev}: expected {full_name}"
            );
        }
    }

    // --- Marked content (BMC/BDC/EMC) tests ---

    #[test]
    fn bdc_with_mcid_sets_char_mcid() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"/P <</MCID 5>> BDC BT /F1 12 Tf (Hi) Tj ET EMC";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].mcid, Some(5));
        assert_eq!(handler.chars[0].tag.as_deref(), Some("P"));
        assert_eq!(handler.chars[1].mcid, Some(5));
        assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
    }

    #[test]
    fn bmc_sets_tag_without_mcid() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"/Artifact BMC BT /F1 12 Tf (X) Tj ET EMC";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].tag.as_deref(), Some("Artifact"));
        assert_eq!(handler.chars[0].mcid, None);
    }

    #[test]
    fn nested_bdc_maintains_correct_stack() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"/P <</MCID 1>> BDC BT /F1 12 Tf (A) Tj /Span <</MCID 2>> BDC (B) Tj EMC (C) Tj ET EMC";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 3);
        assert_eq!(handler.chars[0].mcid, Some(1));
        assert_eq!(handler.chars[0].tag.as_deref(), Some("P"));
        assert_eq!(handler.chars[1].mcid, Some(2));
        assert_eq!(handler.chars[1].tag.as_deref(), Some("Span"));
        assert_eq!(handler.chars[2].mcid, Some(1));
        assert_eq!(handler.chars[2].tag.as_deref(), Some("P"));
    }

    #[test]
    fn emc_without_matching_bmc_handled_gracefully() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"EMC BT /F1 12 Tf (A) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        let result = interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        );

        assert!(result.is_ok());
        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].mcid, None);
        assert_eq!(handler.chars[0].tag, None);
    }

    #[test]
    fn chars_outside_marked_content_have_none() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"BT /F1 12 Tf (A) Tj /P <</MCID 3>> BDC (B) Tj EMC (C) Tj ET";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 3);
        assert_eq!(handler.chars[0].mcid, None);
        assert_eq!(handler.chars[0].tag, None);
        assert_eq!(handler.chars[1].mcid, Some(3));
        assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
        assert_eq!(handler.chars[2].mcid, None);
        assert_eq!(handler.chars[2].tag, None);
    }

    #[test]
    fn nested_bmc_inside_bdc_inherits_mcid() {
        let doc = lopdf::Document::with_version("1.7");
        let resources = empty_resources();
        let stream = b"/P <</MCID 7>> BDC BT /F1 12 Tf /Artifact BMC (A) Tj EMC (B) Tj ET EMC";

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].tag.as_deref(), Some("Artifact"));
        assert_eq!(handler.chars[0].mcid, Some(7));
        assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
        assert_eq!(handler.chars[1].mcid, Some(7));
    }
}
