//! PDF form field and digital signature extraction.
//!
//! Extracts AcroForm field trees and digital signature metadata.
//! Called from the main backend module.

use crate::error::BackendError;
use pdfplumber_core::{BBox, FieldType, FormField, SignatureInfo};
use super::{decode_pdf_string, extract_bbox_from_array, extract_string_from_dict};

pub(super) fn extract_document_form_fields(doc: &lopdf::Document) -> Result<Vec<FormField>, BackendError> {
    // Get the catalog dictionary
    let catalog_ref = match doc.trailer.get(b"Root") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let catalog = match catalog_ref {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => match obj.as_dict() {
                Ok(dict) => dict,
                Err(_) => return Ok(Vec::new()),
            },
            Err(_) => return Ok(Vec::new()),
        },
        lopdf::Object::Dictionary(dict) => dict,
        _ => return Ok(Vec::new()),
    };

    // Get /AcroForm dictionary
    let acroform_obj = match catalog.get(b"AcroForm") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // No AcroForm in this document
    };

    let acroform_obj = match acroform_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()),
        },
        other => other,
    };

    let acroform_dict = match acroform_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return Ok(Vec::new()),
    };

    // Get /Fields array
    let fields_obj = match acroform_dict.get(b"Fields") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let fields_obj = match fields_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()),
        },
        other => other,
    };

    let fields_array = match fields_obj.as_array() {
        Ok(arr) => arr,
        Err(_) => return Ok(Vec::new()),
    };

    // Build page map for resolving page references
    let pages_map = doc.get_pages();

    let mut form_fields = Vec::new();
    let max_depth = 64; // Prevent circular references

    for field_entry in fields_array {
        let field_ref = match field_entry {
            lopdf::Object::Reference(id) => *id,
            _ => continue,
        };
        walk_field_tree(
            doc,
            field_ref,
            None, // No parent name prefix
            None, // No inherited field type
            0,
            max_depth,
            &pages_map,
            &mut form_fields,
        );
    }

    Ok(form_fields)
}

/// Recursively walk the form field tree, collecting terminal form fields.
///
/// Handles hierarchical fields where intermediate nodes carry partial
/// names (joined with `.`) and field type may be inherited from parents.
#[allow(clippy::too_many_arguments)]
fn walk_field_tree(
    doc: &lopdf::Document,
    field_id: lopdf::ObjectId,
    parent_name: Option<&str>,
    inherited_ft: Option<&FieldType>,
    depth: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
    fields: &mut Vec<FormField>,
) {
    if depth >= max_depth {
        return;
    }

    let field_obj = match doc.get_object(field_id) {
        Ok(obj) => obj,
        Err(_) => return,
    };

    let field_dict = match field_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return,
    };

    // Extract partial name /T
    let partial_name = extract_string_from_dict(doc, field_dict, b"T");

    // Build full qualified name
    let full_name = match (&parent_name, &partial_name) {
        (Some(parent), Some(name)) => format!("{parent}.{name}"),
        (Some(parent), None) => parent.to_string(),
        (None, Some(name)) => name.clone(),
        (None, None) => String::new(),
    };

    // Extract /FT (field type) — may be inherited from parent
    let field_type = match field_dict.get(b"FT") {
        Ok(lopdf::Object::Name(name)) => FieldType::from_pdf_name(&String::from_utf8_lossy(name)),
        _ => inherited_ft.cloned(),
    };

    // Check for /Kids — if present, this is an intermediate node
    if let Ok(kids_obj) = field_dict.get(b"Kids") {
        let kids_obj = match kids_obj {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => return,
            },
            other => other,
        };

        if let Ok(kids_array) = kids_obj.as_array() {
            // Check if /Kids contains widget annotations or child fields.
            // If a kid has /T, it's a child field; otherwise it's a widget annotation.
            let has_child_fields = kids_array.iter().any(|kid| {
                let kid_obj = match kid {
                    lopdf::Object::Reference(id) => doc.get_object(*id).ok(),
                    _ => Some(kid),
                };
                kid_obj
                    .and_then(|o| o.as_dict().ok())
                    .is_some_and(|d| d.get(b"T").is_ok())
            });

            if has_child_fields {
                // Recurse into child fields
                for kid in kids_array {
                    if let lopdf::Object::Reference(kid_id) = kid {
                        walk_field_tree(
                            doc,
                            *kid_id,
                            Some(&full_name),
                            field_type.as_ref(),
                            depth + 1,
                            max_depth,
                            pages_map,
                            fields,
                        );
                    }
                }
                return;
            }
            // If kids are only widgets (no /T), fall through to extract this as a terminal field.
        }
    }

    // Terminal field — extract all properties
    let Some(field_type) = field_type else {
        return; // Skip fields without a type
    };

    // Extract /V (value)
    let value = extract_field_value(doc, field_dict, b"V");

    // Extract /DV (default value)
    let default_value = extract_field_value(doc, field_dict, b"DV");

    // Extract /Rect (bounding box)
    let bbox = extract_field_bbox(doc, field_dict).unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));

    // Extract /Opt (options for choice fields)
    let options = extract_field_options(doc, field_dict);

    // Extract /Ff (field flags)
    let flags = match field_dict.get(b"Ff") {
        Ok(lopdf::Object::Integer(n)) => *n as u32,
        _ => 0,
    };

    // Try to determine page index from /P reference or widget annotations
    let page_index = resolve_field_page(doc, field_dict, pages_map);

    fields.push(FormField {
        name: full_name,
        field_type,
        value,
        default_value,
        bbox,
        options,
        flags,
        page_index,
    });
}

/// Extract a field value from /V or /DV entry.
///
/// Handles strings, names, and arrays of strings.
fn extract_field_value(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    key: &[u8],
) -> Option<String> {
    let obj = dict.get(key).ok()?;
    let obj = match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    match obj {
        lopdf::Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        lopdf::Object::Array(arr) => {
            // Multi-select: join values
            let vals: Vec<String> = arr
                .iter()
                .filter_map(|item| match item {
                    lopdf::Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
                    lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
                    _ => None,
                })
                .collect();
            if vals.is_empty() {
                None
            } else {
                Some(vals.join(", "))
            }
        }
        _ => None,
    }
}

// decode_pdf_string is provided by super::decode_pdf_string

/// Extract bounding box from a field's /Rect entry.
fn extract_field_bbox(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> Option<BBox> {
    let rect_obj = dict.get(b"Rect").ok()?;
    let rect_obj = match rect_obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    let arr = rect_obj.as_array().ok()?;
    extract_bbox_from_array(arr).ok()
}

/// Extract options from a choice field's /Opt entry.
fn extract_field_options(doc: &lopdf::Document, dict: &lopdf::Dictionary) -> Vec<String> {
    let opt_obj = match dict.get(b"Opt") {
        Ok(obj) => obj,
        Err(_) => return Vec::new(),
    };
    let opt_obj = match opt_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Vec::new(),
        },
        other => other,
    };
    let opt_array = match opt_obj.as_array() {
        Ok(arr) => arr,
        Err(_) => return Vec::new(),
    };

    opt_array
        .iter()
        .filter_map(|item| {
            let item = match item {
                lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
                other => other,
            };
            match item {
                lopdf::Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
                lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
                // Option can be [export_value, display_value] pair
                lopdf::Object::Array(pair) => {
                    if pair.len() >= 2 {
                        // Use display value (second element)
                        match &pair[1] {
                            lopdf::Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
                            lopdf::Object::Name(name) => {
                                Some(String::from_utf8_lossy(name).into_owned())
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
        .collect()
}

/// Resolve a form field's page index from /P reference.
fn resolve_field_page(
    _doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<usize> {
    // Try /P (page reference)
    let page_ref = match dict.get(b"P") {
        Ok(lopdf::Object::Reference(id)) => *id,
        _ => return None,
    };

    // Resolve page reference to 0-based index
    pages_map.iter().find_map(|(&page_num, &page_id)| {
        if page_id == page_ref {
            Some((page_num - 1) as usize) // lopdf pages are 1-indexed
        } else {
            None
        }
    })
}

/// Extract digital signature information from the document's `/AcroForm`.
///
/// Walks the field tree and collects signature fields (`/FT /Sig`).
/// For signed fields (those with `/V`), extracts signer name, date,
/// reason, location, and contact info from the signature value dictionary.
pub(super) fn extract_document_signatures(doc: &lopdf::Document) -> Result<Vec<SignatureInfo>, BackendError> {
    // Get the catalog dictionary
    let catalog_ref = match doc.trailer.get(b"Root") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let catalog = match catalog_ref {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => match obj.as_dict() {
                Ok(dict) => dict,
                Err(_) => return Ok(Vec::new()),
            },
            Err(_) => return Ok(Vec::new()),
        },
        lopdf::Object::Dictionary(dict) => dict,
        _ => return Ok(Vec::new()),
    };

    // Get /AcroForm dictionary
    let acroform_obj = match catalog.get(b"AcroForm") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let acroform_obj = match acroform_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()),
        },
        other => other,
    };

    let acroform_dict = match acroform_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return Ok(Vec::new()),
    };

    // Get /Fields array
    let fields_obj = match acroform_dict.get(b"Fields") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let fields_obj = match fields_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()),
        },
        other => other,
    };

    let fields_array = match fields_obj.as_array() {
        Ok(arr) => arr,
        Err(_) => return Ok(Vec::new()),
    };

    let mut signatures = Vec::new();
    let max_depth = 64;

    for field_entry in fields_array {
        let field_ref = match field_entry {
            lopdf::Object::Reference(id) => *id,
            _ => continue,
        };
        walk_signature_tree(doc, field_ref, None, 0, max_depth, &mut signatures);
    }

    Ok(signatures)
}

/// Recursively walk the form field tree, collecting signature fields.
///
/// Similar to `walk_field_tree` but only collects `/FT /Sig` fields
/// and extracts signature-specific metadata from `/V`.
fn walk_signature_tree(
    doc: &lopdf::Document,
    field_id: lopdf::ObjectId,
    inherited_ft: Option<&[u8]>,
    depth: usize,
    max_depth: usize,
    signatures: &mut Vec<SignatureInfo>,
) {
    if depth >= max_depth {
        return;
    }

    let field_obj = match doc.get_object(field_id) {
        Ok(obj) => obj,
        Err(_) => return,
    };

    let field_dict = match field_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return,
    };

    // Extract /FT — may be inherited from parent
    let field_type = match field_dict.get(b"FT") {
        Ok(lopdf::Object::Name(name)) => Some(name.as_slice()),
        _ => inherited_ft,
    };

    // Check for /Kids — if present, this may be an intermediate node
    if let Ok(kids_obj) = field_dict.get(b"Kids") {
        let kids_obj = match kids_obj {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => return,
            },
            other => other,
        };

        if let Ok(kids_array) = kids_obj.as_array() {
            // Check if /Kids contains child fields (with /T) or widget annotations
            let has_child_fields = kids_array.iter().any(|kid| {
                let kid_obj = match kid {
                    lopdf::Object::Reference(id) => doc.get_object(*id).ok(),
                    _ => Some(kid),
                };
                kid_obj
                    .and_then(|o| o.as_dict().ok())
                    .is_some_and(|d| d.get(b"T").is_ok())
            });

            if has_child_fields {
                for kid in kids_array {
                    if let lopdf::Object::Reference(kid_id) = kid {
                        walk_signature_tree(
                            doc,
                            *kid_id,
                            field_type,
                            depth + 1,
                            max_depth,
                            signatures,
                        );
                    }
                }
                return;
            }
        }
    }

    // Terminal field — check if it's a signature field
    let is_sig = field_type.is_some_and(|ft| ft == b"Sig");
    if !is_sig {
        return;
    }

    // Check for /V (signature value dictionary)
    let sig_dict = field_dict
        .get(b"V")
        .ok()
        .and_then(|obj| match obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok(),
            other => Some(other),
        })
        .and_then(|obj| obj.as_dict().ok());

    let info = match sig_dict {
        Some(v_dict) => SignatureInfo {
            signer_name: extract_string_from_dict(doc, v_dict, b"Name"),
            sign_date: extract_string_from_dict(doc, v_dict, b"M"),
            reason: extract_string_from_dict(doc, v_dict, b"Reason"),
            location: extract_string_from_dict(doc, v_dict, b"Location"),
            contact_info: extract_string_from_dict(doc, v_dict, b"ContactInfo"),
            is_signed: true,
        },
        None => SignatureInfo {
            signer_name: None,
            sign_date: None,
            reason: None,
            location: None,
            contact_info: None,
            is_signed: false,
        },
    };

    signatures.push(info);
}
