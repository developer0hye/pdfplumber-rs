//! PDF document metadata and bookmark extraction.
//!
//! Extracts document-level information from the PDF /Info dictionary
//! and /Outlines bookmark tree. Called from the main backend module.

use super::extract_string_from_dict;
use crate::error::BackendError;
use pdfplumber_core::{Bookmark, DocumentMetadata};

pub(super) fn extract_document_metadata(
    doc: &lopdf::Document,
) -> Result<DocumentMetadata, BackendError> {
    // The /Info dictionary is referenced from the trailer
    let info_ref = match doc.trailer.get(b"Info") {
        Ok(obj) => obj,
        Err(_) => return Ok(DocumentMetadata::default()),
    };

    let info_dict = match info_ref {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => match obj.as_dict() {
                Ok(dict) => dict,
                Err(_) => return Ok(DocumentMetadata::default()),
            },
            Err(_) => return Ok(DocumentMetadata::default()),
        },
        lopdf::Object::Dictionary(dict) => dict,
        _ => return Ok(DocumentMetadata::default()),
    };

    Ok(DocumentMetadata {
        title: extract_string_from_dict(doc, info_dict, b"Title"),
        author: extract_string_from_dict(doc, info_dict, b"Author"),
        subject: extract_string_from_dict(doc, info_dict, b"Subject"),
        keywords: extract_string_from_dict(doc, info_dict, b"Keywords"),
        creator: extract_string_from_dict(doc, info_dict, b"Creator"),
        producer: extract_string_from_dict(doc, info_dict, b"Producer"),
        creation_date: extract_string_from_dict(doc, info_dict, b"CreationDate"),
        mod_date: extract_string_from_dict(doc, info_dict, b"ModDate"),
    })
}

/// Extract the document outline (bookmarks / table of contents) from the PDF catalog.
///
/// Walks the `/Outlines` tree using `/First`, `/Next` sibling links,
/// resolving destinations to page numbers and y-coordinates.
pub(super) fn extract_document_bookmarks(
    doc: &lopdf::Document,
) -> Result<Vec<Bookmark>, BackendError> {
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

    // Get /Outlines dictionary
    let outlines_obj = match catalog.get(b"Outlines") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()),
    };

    let outlines_obj = match outlines_obj {
        lopdf::Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()),
        },
        other => other,
    };

    let outlines_dict = match outlines_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return Ok(Vec::new()),
    };

    // Get /First child of the outlines root
    let first_ref = match outlines_dict.get(b"First") {
        Ok(lopdf::Object::Reference(id)) => *id,
        _ => return Ok(Vec::new()),
    };

    // Build page map for resolving destinations
    let pages_map = doc.get_pages();

    let mut bookmarks = Vec::new();
    let max_depth = 64; // Prevent circular references
    walk_outline_tree(doc, first_ref, 0, max_depth, &pages_map, &mut bookmarks);

    Ok(bookmarks)
}

/// Recursively walk the outline tree, collecting bookmarks.
fn walk_outline_tree(
    doc: &lopdf::Document,
    item_id: lopdf::ObjectId,
    level: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
    bookmarks: &mut Vec<Bookmark>,
) {
    if level >= max_depth {
        return;
    }

    let mut current_id = Some(item_id);
    let mut visited = std::collections::HashSet::new();
    let max_siblings = 10_000; // Safety limit on siblings at one level
    let mut sibling_count = 0;

    while let Some(node_id) = current_id {
        // Circular reference protection
        if !visited.insert(node_id) || sibling_count >= max_siblings {
            break;
        }
        sibling_count += 1;

        let node_obj = match doc.get_object(node_id) {
            Ok(obj) => obj,
            Err(_) => break,
        };

        let node_dict = match node_obj.as_dict() {
            Ok(dict) => dict,
            Err(_) => break,
        };

        // Extract /Title
        let title = extract_string_from_dict(doc, node_dict, b"Title").unwrap_or_default();

        // Resolve destination (page number and y-coordinate)
        let (page_number, dest_top) = resolve_bookmark_dest(doc, node_dict, pages_map);

        bookmarks.push(Bookmark {
            title,
            level,
            page_number,
            dest_top,
        });

        // Recurse into children (/First)
        if let Ok(lopdf::Object::Reference(child_id)) = node_dict.get(b"First") {
            walk_outline_tree(doc, *child_id, level + 1, max_depth, pages_map, bookmarks);
        }

        // Move to next sibling (/Next)
        current_id = match node_dict.get(b"Next") {
            Ok(lopdf::Object::Reference(next_id)) => Some(*next_id),
            _ => None,
        };
    }
}

/// Resolve a bookmark's destination to (page_number, dest_top).
///
/// Checks /Dest first, then /A (GoTo action).
fn resolve_bookmark_dest(
    doc: &lopdf::Document,
    node_dict: &lopdf::Dictionary,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> (Option<usize>, Option<f64>) {
    // Try /Dest first
    if let Ok(dest_obj) = node_dict.get(b"Dest") {
        if let Some(result) = resolve_dest_to_page(doc, dest_obj, pages_map) {
            return result;
        }
    }

    // Try /A (Action) dictionary — only GoTo actions
    if let Ok(action_obj) = node_dict.get(b"A") {
        let action_obj = match action_obj {
            lopdf::Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => return (None, None),
            },
            other => other,
        };
        if let Ok(action_dict) = action_obj.as_dict() {
            if let Ok(lopdf::Object::Name(action_type)) = action_dict.get(b"S") {
                if String::from_utf8_lossy(action_type) == "GoTo" {
                    if let Ok(dest_obj) = action_dict.get(b"D") {
                        if let Some(result) = resolve_dest_to_page(doc, dest_obj, pages_map) {
                            return result;
                        }
                    }
                }
            }
        }
    }

    (None, None)
}

/// Resolve a destination object to (page_number, dest_top).
///
/// Handles explicit destination arrays `[page_ref, /type, ...]` and named destinations.
fn resolve_dest_to_page(
    doc: &lopdf::Document,
    dest_obj: &lopdf::Object,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<(Option<usize>, Option<f64>)> {
    let dest_obj = match dest_obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };

    match dest_obj {
        // Explicit destination array: [page_ref, /type, ...]
        lopdf::Object::Array(arr) => {
            if arr.is_empty() {
                return None;
            }
            // First element is a page reference
            if let lopdf::Object::Reference(page_ref) = &arr[0] {
                // Resolve to 0-indexed page number
                let page_number = pages_map.iter().find_map(|(&page_num, &page_id)| {
                    if page_id == *page_ref {
                        Some((page_num - 1) as usize) // lopdf pages are 1-indexed
                    } else {
                        None
                    }
                });

                // Try to extract dest_top from /XYZ or /FitH or /FitBH destination types
                let dest_top = extract_dest_top(arr);

                return Some((page_number, dest_top));
            }
            None
        }
        // Named destination (string) — look up in /Names or /Dests
        lopdf::Object::String(bytes, _) => {
            let name = if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
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
                String::from_utf16(&chars).ok()?
            } else {
                match std::str::from_utf8(bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => bytes.iter().map(|&b| b as char).collect(),
                }
            };
            resolve_named_dest(doc, &name, pages_map)
        }
        // Named destination (name)
        lopdf::Object::Name(name) => {
            let name_str = String::from_utf8_lossy(name);
            resolve_named_dest(doc, &name_str, pages_map)
        }
        _ => None,
    }
}

/// Extract the dest_top (y-coordinate) from a destination array.
///
/// Supports /XYZ (index 3), /FitH (index 2), /FitBH (index 2).
fn extract_dest_top(arr: &[lopdf::Object]) -> Option<f64> {
    if arr.len() < 2 {
        return None;
    }
    // Second element is the destination type
    if let lopdf::Object::Name(dest_type) = &arr[1] {
        let type_str = String::from_utf8_lossy(dest_type);
        match type_str.as_ref() {
            "XYZ" => {
                // [page, /XYZ, left, top, zoom]
                if arr.len() >= 4 {
                    return obj_to_f64(&arr[3]);
                }
            }
            "FitH" | "FitBH" => {
                // [page, /FitH, top] or [page, /FitBH, top]
                if arr.len() >= 3 {
                    return obj_to_f64(&arr[2]);
                }
            }
            _ => {} // /Fit, /FitV, /FitR, /FitB — no meaningful top
        }
    }
    None
}

/// Convert a lopdf Object to f64 (handles Integer, Real, and Null).
fn obj_to_f64(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some((*f).into()),
        lopdf::Object::Null => None, // null means "unchanged" in PDF spec
        _ => None,
    }
}

/// Resolve a named destination to (page_number, dest_top).
///
/// Looks up the name in the catalog's /Names → /Dests name tree,
/// or in the catalog's /Dests dictionary.
fn resolve_named_dest(
    doc: &lopdf::Document,
    name: &str,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<(Option<usize>, Option<f64>)> {
    // Get catalog
    let catalog_ref = doc.trailer.get(b"Root").ok()?;
    let catalog = match catalog_ref {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok()?,
        lopdf::Object::Dictionary(dict) => dict,
        _ => return None,
    };

    // Try /Names → /Dests name tree first
    if let Ok(names_obj) = catalog.get(b"Names") {
        let names_obj = match names_obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
            other => other,
        };
        if let Ok(names_dict) = names_obj.as_dict() {
            if let Ok(dests_obj) = names_dict.get(b"Dests") {
                let dests_obj = match dests_obj {
                    lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
                    other => other,
                };
                if let Ok(dests_dict) = dests_obj.as_dict() {
                    if let Some(result) = lookup_name_tree(doc, dests_dict, name, pages_map) {
                        return Some(result);
                    }
                }
            }
        }
    }

    // Try /Dests dictionary (older PDF spec)
    if let Ok(dests_obj) = catalog.get(b"Dests") {
        let dests_obj = match dests_obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
            other => other,
        };
        if let Ok(dests_dict) = dests_obj.as_dict() {
            if let Ok(dest_obj) = dests_dict.get(name.as_bytes()) {
                let dest_obj = match dest_obj {
                    lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
                    other => other,
                };
                // Could be an array directly or a dict with /D key
                match dest_obj {
                    lopdf::Object::Array(arr) => {
                        if let Some(result) =
                            resolve_dest_to_page(doc, &lopdf::Object::Array(arr.clone()), pages_map)
                        {
                            return Some(result);
                        }
                    }
                    lopdf::Object::Dictionary(d) => {
                        if let Ok(d_dest) = d.get(b"D") {
                            if let Some(result) = resolve_dest_to_page(doc, d_dest, pages_map) {
                                return Some(result);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    None
}

/// Look up a name in a PDF name tree (/Names array with key-value pairs).
fn lookup_name_tree(
    doc: &lopdf::Document,
    tree_dict: &lopdf::Dictionary,
    name: &str,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<(Option<usize>, Option<f64>)> {
    // Check /Names array (leaf node)
    if let Ok(names_arr_obj) = tree_dict.get(b"Names") {
        let names_arr_obj = match names_arr_obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
            other => other,
        };
        if let Ok(names_arr) = names_arr_obj.as_array() {
            // Names array is [key1, value1, key2, value2, ...]
            let mut i = 0;
            while i + 1 < names_arr.len() {
                let key_obj = match &names_arr[i] {
                    lopdf::Object::Reference(id) => match doc.get_object(*id) {
                        Ok(obj) => obj.clone(),
                        Err(_) => {
                            i += 2;
                            continue;
                        }
                    },
                    other => other.clone(),
                };
                if let lopdf::Object::String(key_bytes, _) = &key_obj {
                    let key_str = String::from_utf8_lossy(key_bytes);
                    if key_str == name {
                        let value = &names_arr[i + 1];
                        let value = match value {
                            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
                            other => other,
                        };
                        // Value can be an array (destination) or dict with /D
                        match value {
                            lopdf::Object::Array(arr) => {
                                return resolve_dest_to_page(
                                    doc,
                                    &lopdf::Object::Array(arr.clone()),
                                    pages_map,
                                );
                            }
                            lopdf::Object::Dictionary(d) => {
                                if let Ok(d_dest) = d.get(b"D") {
                                    return resolve_dest_to_page(doc, d_dest, pages_map);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                i += 2;
            }
        }
    }

    // Check /Kids array (intermediate nodes)
    if let Ok(kids_obj) = tree_dict.get(b"Kids") {
        let kids_obj = match kids_obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
            other => other,
        };
        if let Ok(kids_arr) = kids_obj.as_array() {
            for kid in kids_arr {
                let kid_obj = match kid {
                    lopdf::Object::Reference(id) => match doc.get_object(*id) {
                        Ok(obj) => obj,
                        Err(_) => continue,
                    },
                    other => other,
                };
                if let Ok(kid_dict) = kid_obj.as_dict() {
                    if let Some(result) = lookup_name_tree(doc, kid_dict, name, pages_map) {
                        return Some(result);
                    }
                }
            }
        }
    }

    None
}
