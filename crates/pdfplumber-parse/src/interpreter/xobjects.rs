//! XObject handling for the content stream interpreter.
//!
//! Processes `Do` operator references to Form XObjects and Image XObjects,
//! plus inline image (`BI`/`ID`/`EI`) parsing and inline image key expansion.

use super::interpret_content_stream;
use crate::error::BackendError;
use crate::handler::{ContentHandler, ImageEvent};
use crate::interpreter_state::InterpreterState;
use crate::lopdf_backend::object_to_f64;
use crate::text_state::TextState;
use crate::tokenizer::{Operand, Operator};
use pdfplumber_core::ExtractOptions;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_do(
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
pub(super) fn handle_form_xobject(
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

pub(super) fn handle_image_xobject(
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
pub(super) fn handle_inline_image(
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
pub(super) fn decode_stream(stream: &lopdf::Stream) -> Result<Vec<u8>, BackendError> {
    // Check if stream has filters
    if stream.dict.get(b"Filter").is_ok() {
        stream
            .decompressed_content()
            .map_err(|e| BackendError::Interpreter(format!("stream decompression failed: {e}")))
    } else {
        Ok(stream.content.clone())
    }
}
