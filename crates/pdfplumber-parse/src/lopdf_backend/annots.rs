//! PDF annotation and hyperlink extraction.
//!
//! Extracts annotation metadata and hyperlinks from PDF page /Annots arrays.
//! Called from the main backend module.

use super::{extract_bbox_from_array, extract_string_from_dict};
use crate::error::BackendError;
use pdfplumber_core::{Annotation, AnnotationType, Hyperlink};

pub(super) fn extract_page_annotations(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Vec<Annotation>, BackendError> {
    let page_dict = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map_err(|e| BackendError::Parse(format!("failed to get page dictionary: {e}")))?;

    // Get /Annots array (may be a direct array or indirect reference)
    let annots_obj = match page_dict.get(b"Annots") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // No annotations on this page
    };

    // Resolve indirect reference to the array
    let annots_obj = match annots_obj {
        lopdf::Object::Reference(id) => doc
            .get_object(*id)
            .map_err(|e| BackendError::Parse(format!("failed to resolve /Annots ref: {e}")))?,
        other => other,
    };

    let annots_array = annots_obj
        .as_array()
        .map_err(|e| BackendError::Parse(format!("/Annots is not an array: {e}")))?;

    let mut annotations = Vec::new();

    for annot_entry in annots_array {
        // Each entry may be a direct dictionary or an indirect reference
        let annot_obj = match annot_entry {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => continue, // Skip unresolvable references
            },
            other => other,
        };

        let annot_dict = match annot_obj.as_dict() {
            Ok(dict) => dict,
            Err(_) => continue, // Skip non-dictionary entries
        };

        // Extract /Subtype (required for annotations)
        let raw_subtype = match annot_dict.get(b"Subtype") {
            Ok(obj) => match obj {
                lopdf::Object::Name(name) => String::from_utf8_lossy(name).into_owned(),
                _ => continue, // Skip if /Subtype is not a name
            },
            Err(_) => continue, // Skip annotations without /Subtype
        };

        let annot_type = AnnotationType::from_subtype(&raw_subtype);

        // Extract /Rect (bounding box)
        let bbox = match annot_dict.get(b"Rect") {
            Ok(obj) => {
                let obj = match obj {
                    lopdf::Object::Reference(id) => match doc.get_object(*id) {
                        Ok(resolved) => resolved,
                        Err(_) => continue,
                    },
                    other => other,
                };
                match obj.as_array() {
                    Ok(arr) => match extract_bbox_from_array(arr) {
                        Ok(b) => b,
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                }
            }
            Err(_) => continue, // Skip annotations without /Rect
        };

        // Extract optional fields
        let contents = extract_string_from_dict(doc, annot_dict, b"Contents");
        let author = extract_string_from_dict(doc, annot_dict, b"T");
        let date = extract_string_from_dict(doc, annot_dict, b"M");

        annotations.push(Annotation {
            annot_type,
            bbox,
            contents,
            author,
            date,
            raw_subtype,
        });
    }

    Ok(annotations)
}

/// Extract hyperlinks from a page's Link annotations.
///
/// Filters annotations for `/Subtype /Link` and resolves URI targets from
/// `/A` (action) or `/Dest` entries.
pub(super) fn extract_page_hyperlinks(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Vec<Hyperlink>, BackendError> {
    let page_dict = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map_err(|e| BackendError::Parse(format!("failed to get page dictionary: {e}")))?;

    // Get /Annots array
    let annots_obj = match page_dict.get(b"Annots") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    // Resolve indirect reference to the array
    let annots_obj = match annots_obj {
        lopdf::Object::Reference(id) => doc
            .get_object(*id)
            .map_err(|e| BackendError::Parse(format!("failed to resolve /Annots ref: {e}")))?,
        other => other,
    };

    let annots_array = annots_obj
        .as_array()
        .map_err(|e| BackendError::Parse(format!("/Annots is not an array: {e}")))?;

    let mut hyperlinks = Vec::new();

    for annot_entry in annots_array {
        // Each entry may be a direct dictionary or an indirect reference
        let annot_obj = match annot_entry {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => continue,
            },
            other => other,
        };

        let annot_dict = match annot_obj.as_dict() {
            Ok(dict) => dict,
            Err(_) => continue,
        };

        // Only process Link annotations
        let subtype = match annot_dict.get(b"Subtype") {
            Ok(lopdf::Object::Name(name)) => String::from_utf8_lossy(name).into_owned(),
            _ => continue,
        };
        if subtype != "Link" {
            continue;
        }

        // Extract /Rect (bounding box)
        let bbox = match annot_dict.get(b"Rect") {
            Ok(obj) => {
                let obj = match obj {
                    lopdf::Object::Reference(id) => match doc.get_object(*id) {
                        Ok(resolved) => resolved,
                        Err(_) => continue,
                    },
                    other => other,
                };
                match obj.as_array() {
                    Ok(arr) => match extract_bbox_from_array(arr) {
                        Ok(b) => b,
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                }
            }
            Err(_) => continue,
        };

        // Try to resolve URI from /A (action) dictionary
        let uri = resolve_link_uri(doc, annot_dict);

        // Skip links without a resolvable URI
        if let Some(uri) = uri {
            if !uri.is_empty() {
                hyperlinks.push(Hyperlink { bbox, uri });
            }
        }
    }

    Ok(hyperlinks)
}

/// Resolve the URI target of a Link annotation.
///
/// Checks the /A (action) dictionary first, then /Dest.
fn resolve_link_uri(doc: &lopdf::Document, annot_dict: &lopdf::Dictionary) -> Option<String> {
    // Try /A (Action) dictionary
    if let Ok(action_obj) = annot_dict.get(b"A") {
        let action_obj = match action_obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
            other => other,
        };
        if let Ok(action_dict) = action_obj.as_dict() {
            // Get action type /S
            if let Ok(lopdf::Object::Name(action_type)) = action_dict.get(b"S") {
                let action_type_str = String::from_utf8_lossy(action_type);
                match action_type_str.as_ref() {
                    "URI" => {
                        // Extract /URI string
                        return extract_string_from_dict(doc, action_dict, b"URI");
                    }
                    "GoTo" => {
                        // Extract /D destination
                        return resolve_goto_dest(doc, action_dict);
                    }
                    "GoToR" => {
                        // Remote GoTo — extract /F (file) and /D (dest)
                        let file = extract_string_from_dict(doc, action_dict, b"F");
                        if let Some(f) = file {
                            return Some(f);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Try /Dest (direct destination, no action)
    if let Ok(dest_obj) = annot_dict.get(b"Dest") {
        return resolve_dest_object(doc, dest_obj);
    }

    None
}

/// Resolve a GoTo action's /D destination to a string.
fn resolve_goto_dest(doc: &lopdf::Document, action_dict: &lopdf::Dictionary) -> Option<String> {
    let dest_obj = action_dict.get(b"D").ok()?;
    resolve_dest_object(doc, dest_obj)
}

/// Resolve a destination object to a string representation.
///
/// Destinations can be:
/// - A name string (named destination)
/// - An array [page_ref, /type, ...] (explicit destination)
fn resolve_dest_object(doc: &lopdf::Document, dest_obj: &lopdf::Object) -> Option<String> {
    let dest_obj = match dest_obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };

    match dest_obj {
        // Named destination (string)
        lopdf::Object::String(bytes, _) => {
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let chars: Vec<u16> = bytes[2..]
                    .chunks(2)
                    .filter_map(|c| {
                        if c.len() == 2 {
                            Some(u16::from_be_bytes([c[0], c[1]]))
                        } else {
                            None
                        }
                    })
                    .collect();
                String::from_utf16(&chars).ok()
            } else {
                match std::str::from_utf8(bytes) {
                    Ok(s) => Some(s.to_string()),
                    Err(_) => Some(bytes.iter().map(|&b| b as char).collect()),
                }
            }
        }
        // Named destination (name)
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        // Explicit destination array [page_ref, /type, ...]
        lopdf::Object::Array(arr) => {
            if arr.is_empty() {
                return None;
            }
            // First element is a page reference — try to resolve page number
            if let lopdf::Object::Reference(page_ref) = &arr[0] {
                // Find the page number by matching against document pages
                let pages_map = doc.get_pages();
                for (&page_num, &page_id) in &pages_map {
                    if page_id == *page_ref {
                        return Some(format!("#page={page_num}"));
                    }
                }
                // Couldn't resolve page number, use reference
                return Some(format!("#ref={},{}", page_ref.0, page_ref.1));
            }
            None
        }
        _ => None,
    }
}
