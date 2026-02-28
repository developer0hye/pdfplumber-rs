//! Content stream interpreter.
//!
//! Interprets tokenized PDF content stream operators, maintaining graphics and
//! text state, and emitting events to a [`ContentHandler`]. Handles Form XObject
//! recursion via the `Do` operator.

use std::collections::HashMap;

use crate::cid_font::{
    CidFontMetrics, extract_cid_font_metrics, get_descendant_font, get_type0_encoding,
    is_type0_font, parse_predefined_cmap_name,
};
use crate::cmap::CMap;
use crate::error::BackendError;
use crate::font_metrics::{FontMetrics, extract_font_metrics};
use crate::handler::{CharEvent, ContentHandler, ImageEvent};
use crate::interpreter_state::InterpreterState;
use crate::lopdf_backend::object_to_f64;
use crate::text_renderer::{TjElement, show_string, show_string_with_positioning};
use crate::text_state::TextState;
use crate::tokenizer::{Operand, tokenize};
use pdfplumber_core::ExtractOptions;

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

    for op in &operators {
        match op.name.as_str() {
            // --- Graphics state operators ---
            "q" => gstate.save_state(),
            "Q" => {
                gstate.restore_state();
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
                }
            }
            "w" => {
                if let Some(v) = get_f64(&op.operands, 0) {
                    gstate.set_line_width(v);
                }
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
                    load_font_if_needed(doc, resources, &font_name, &mut font_cache);
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
                handle_tj(tstate, gstate, handler, &op.operands, &font_cache);
            }
            "TJ" => {
                handle_tj_array(tstate, gstate, handler, &op.operands, &font_cache);
            }
            "'" => {
                // T* then Tj
                tstate.move_to_next_line();
                handle_tj(tstate, gstate, handler, &op.operands, &font_cache);
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
                    handle_tj(tstate, gstate, handler, &string_operands, &font_cache);
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

            // Other operators (paths, etc.) - not yet handled for this story
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

fn load_font_if_needed(
    doc: &lopdf::Document,
    resources: &lopdf::Dictionary,
    font_name: &str,
    cache: &mut HashMap<String, CachedFont>,
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

    let (metrics, cmap, base_name, cid_metrics, is_cid_font, writing_mode) =
        if let Some(fd) = font_dict {
            if is_type0_font(fd) {
                // Type0 (composite/CID) font
                let (cid_met, wm) = load_cid_font(doc, fd);
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
                    FontMetrics::default_metrics()
                };

                // Extract ToUnicode CMap if present
                let cmap = extract_tounicode_cmap(doc, fd);

                let base_name = fd
                    .get(b"BaseFont")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .unwrap_or(font_name)
                    .to_string();

                (metrics, cmap, base_name, cid_met, true, wm)
            } else {
                // Simple font
                let metrics = extract_font_metrics(doc, fd)
                    .unwrap_or_else(|_| FontMetrics::default_metrics());
                let cmap = extract_tounicode_cmap(doc, fd);
                let base_name = fd
                    .get(b"BaseFont")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .unwrap_or(font_name)
                    .to_string();

                (metrics, cmap, base_name, None, false, 0)
            }
        } else {
            (
                FontMetrics::default_metrics(),
                None,
                font_name.to_string(),
                None,
                false,
                0,
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

fn handle_tj(
    tstate: &mut TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    operands: &[Operand],
    font_cache: &HashMap<String, CachedFont>,
) {
    let string_bytes = match operands.first().and_then(operand_to_string_bytes) {
        Some(bytes) => bytes,
        None => return,
    };

    let cached = font_cache.get(&tstate.font_name);
    let width_fn = get_width_fn(cached);
    let raw_chars = show_string(tstate, string_bytes, &*width_fn);

    emit_char_events(raw_chars, tstate, gstate, handler, cached);
}

fn handle_tj_array(
    tstate: &mut TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    operands: &[Operand],
    font_cache: &HashMap<String, CachedFont>,
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
    let raw_chars = show_string_with_positioning(tstate, &elements, &*width_fn);

    emit_char_events(raw_chars, tstate, gstate, handler, cached);
}

fn emit_char_events(
    raw_chars: Vec<crate::text_renderer::RawChar>,
    tstate: &TextState,
    gstate: &InterpreterState,
    handler: &mut dyn ContentHandler,
    cached: Option<&CachedFont>,
) {
    let ctm = gstate.ctm_array();
    let font_name = cached.map_or_else(|| tstate.font_name.clone(), |c| c.base_name.clone());

    for rc in raw_chars {
        let unicode = cached.and_then(|c| {
            c.cmap
                .as_ref()
                .and_then(|cm| cm.lookup(rc.char_code).map(|s| s.to_string()))
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
        });
    }
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
        .and_then(|o| o.as_name_str().ok())
        .unwrap_or("");

    match subtype {
        "Form" => handle_form_xobject(
            doc, stream, name, resources, handler, options, depth, gstate, tstate,
        ),
        "Image" => {
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
    // Save graphics state
    gstate.save_state();

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

    // Restore graphics state
    gstate.restore_state();

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
        .and_then(|o| o.as_name_str().ok())
        .map(|s| s.to_string());

    let bits_per_component = stream
        .dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .map(|v| v as u32);

    handler.on_image(ImageEvent {
        name: name.to_string(),
        ctm: gstate.ctm_array(),
        width,
        height,
        colorspace,
        bits_per_component,
    });
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

    // --- Collecting handler ---

    struct CollectingHandler {
        chars: Vec<CharEvent>,
        images: Vec<ImageEvent>,
    }

    impl CollectingHandler {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                images: Vec::new(),
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
}
