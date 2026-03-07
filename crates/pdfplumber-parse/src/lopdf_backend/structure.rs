//! PDF structure tree extraction.
//!
//! Parses the PDF logical structure tree (Tagged PDF) into [`StructElement`]
//! trees. Called from the main backend module.

use super::decode_pdf_string;
use crate::error::BackendError;
use pdfplumber_core::StructElement;

pub(super) fn extract_document_structure_tree(
    doc: &lopdf::Document,
) -> Result<Vec<StructElement>, BackendError> {
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

    // Get /StructTreeRoot dictionary
    let struct_tree_obj = match catalog.get(b"StructTreeRoot") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // Not a tagged PDF
    };

    let struct_tree_obj = resolve_object(doc, struct_tree_obj);
    let struct_tree_dict = match struct_tree_obj.as_dict() {
        Ok(dict) => dict,
        Err(_) => return Ok(Vec::new()),
    };

    // Build page map for resolving page references
    let pages_map = doc.get_pages();

    // Get /K (kids) — the children of the root structure element
    let kids_obj = match struct_tree_dict.get(b"K") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // Empty structure tree
    };

    let max_depth = 64; // Prevent circular references
    let elements = parse_struct_kids(doc, kids_obj, 0, max_depth, &pages_map);
    Ok(elements)
}

/// Parse the /K (kids) entry of a structure element, which can be:
/// - An integer MCID
/// - A reference to a structure element dictionary
/// - A dictionary (MCR or structure element)
/// - An array of the above
fn parse_struct_kids(
    doc: &lopdf::Document,
    kids_obj: &lopdf::Object,
    depth: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Vec<StructElement> {
    if depth >= max_depth {
        return Vec::new();
    }

    let kids_obj = resolve_object(doc, kids_obj);

    match kids_obj {
        lopdf::Object::Array(arr) => {
            let mut elements = Vec::new();
            for item in arr {
                let item = resolve_object(doc, item);
                match item {
                    lopdf::Object::Dictionary(dict) => {
                        if let Some(elem) =
                            parse_struct_element(doc, dict, depth + 1, max_depth, pages_map)
                        {
                            elements.push(elem);
                        }
                    }
                    lopdf::Object::Reference(id) => {
                        if let Ok(obj) = doc.get_object(*id) {
                            if let Ok(dict) = obj.as_dict() {
                                if let Some(elem) =
                                    parse_struct_element(doc, dict, depth + 1, max_depth, pages_map)
                                {
                                    elements.push(elem);
                                }
                            }
                        }
                    }
                    // Integer MCID at root level — create a minimal element
                    lopdf::Object::Integer(_) => {
                        // MCIDs at root level without a structure element are unusual;
                        // typically they appear inside a structure element's /K
                    }
                    _ => {}
                }
            }
            elements
        }
        lopdf::Object::Dictionary(dict) => {
            if let Some(elem) = parse_struct_element(doc, dict, depth + 1, max_depth, pages_map) {
                vec![elem]
            } else {
                Vec::new()
            }
        }
        lopdf::Object::Reference(id) => {
            if let Ok(obj) = doc.get_object(*id) {
                if let Ok(dict) = obj.as_dict() {
                    if let Some(elem) =
                        parse_struct_element(doc, dict, depth + 1, max_depth, pages_map)
                    {
                        return vec![elem];
                    }
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Parse a single structure element dictionary.
///
/// Extracts /S (type), /K (kids/MCIDs), /Alt, /ActualText, /Lang,
/// and recurses into children.
fn parse_struct_element(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    depth: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<StructElement> {
    // Check if this is a marked-content reference (MCR) dictionary
    // MCR dicts have /Type /MCR and /MCID, but no /S
    if dict.get(b"MCID").is_ok() && dict.get(b"S").is_err() {
        return None; // MCR, not a structure element
    }

    // Get /S (structure type) — required for structure elements
    let element_type = match dict.get(b"S") {
        Ok(obj) => {
            let obj = resolve_object(doc, obj);
            match obj {
                lopdf::Object::Name(name) => String::from_utf8_lossy(name).into_owned(),
                _ => return None,
            }
        }
        Err(_) => return None, // Not a structure element without /S
    };

    // Extract MCIDs and children from /K
    let mut mcids = Vec::new();
    let mut children = Vec::new();

    if let Ok(k_obj) = dict.get(b"K") {
        collect_mcids_and_children(
            doc,
            k_obj,
            &mut mcids,
            &mut children,
            depth,
            max_depth,
            pages_map,
        );
    }

    // Extract /Alt (alternative text)
    let alt_text = extract_string_entry(doc, dict, b"Alt");

    // Extract /ActualText
    let actual_text = extract_string_entry(doc, dict, b"ActualText");

    // Extract /Lang
    let lang = extract_string_entry(doc, dict, b"Lang");

    // Extract page index from /Pg (page reference for this element)
    let page_index = resolve_struct_page(doc, dict, pages_map);

    Some(StructElement {
        element_type,
        mcids,
        alt_text,
        actual_text,
        lang,
        bbox: None, // PDF structure elements don't always have explicit bbox
        children,
        page_index,
    })
}

/// Collect MCIDs and child structure elements from a /K entry.
///
/// /K can be:
/// - An integer (MCID)
/// - A dictionary (MCR with /MCID, or a child structure element)
/// - A reference to a dictionary
/// - An array of the above
fn collect_mcids_and_children(
    doc: &lopdf::Document,
    k_obj: &lopdf::Object,
    mcids: &mut Vec<u32>,
    children: &mut Vec<StructElement>,
    depth: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) {
    if depth >= max_depth {
        return;
    }

    let k_obj = resolve_object(doc, k_obj);

    match k_obj {
        lopdf::Object::Integer(n) => {
            // Direct MCID
            if *n >= 0 {
                mcids.push(*n as u32);
            }
        }
        lopdf::Object::Dictionary(dict) => {
            process_k_dict(doc, dict, mcids, children, depth, max_depth, pages_map);
        }
        lopdf::Object::Reference(id) => {
            if let Ok(obj) = doc.get_object(*id) {
                match obj {
                    lopdf::Object::Dictionary(dict) => {
                        process_k_dict(doc, dict, mcids, children, depth, max_depth, pages_map);
                    }
                    lopdf::Object::Integer(n) => {
                        if *n >= 0 {
                            mcids.push(*n as u32);
                        }
                    }
                    _ => {}
                }
            }
        }
        lopdf::Object::Array(arr) => {
            for item in arr {
                collect_mcids_and_children(doc, item, mcids, children, depth, max_depth, pages_map);
            }
        }
        _ => {}
    }
}

/// Process a dictionary found in /K — it can be an MCR (with /MCID) or a child struct element.
fn process_k_dict(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    mcids: &mut Vec<u32>,
    children: &mut Vec<StructElement>,
    depth: usize,
    max_depth: usize,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) {
    // Check if this is a marked-content reference (MCR)
    if let Ok(mcid_obj) = dict.get(b"MCID") {
        let mcid_obj = resolve_object(doc, mcid_obj);
        if let lopdf::Object::Integer(n) = mcid_obj {
            if *n >= 0 {
                mcids.push(*n as u32);
            }
        }
        return;
    }

    // Otherwise, treat as a child structure element
    if let Some(elem) = parse_struct_element(doc, dict, depth + 1, max_depth, pages_map) {
        children.push(elem);
    }
}

/// Resolve a structure element's page index from /Pg reference.
fn resolve_struct_page(
    _doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    pages_map: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
) -> Option<usize> {
    let page_ref = match dict.get(b"Pg") {
        Ok(lopdf::Object::Reference(id)) => *id,
        _ => return None,
    };

    // Find which page index this reference corresponds to
    for (page_num, page_id) in pages_map {
        if *page_id == page_ref {
            return Some((*page_num - 1) as usize); // pages_map uses 1-based
        }
    }

    None
}

/// Extract a string entry from a dictionary (handles both String and Name objects).
fn extract_string_entry(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    key: &[u8],
) -> Option<String> {
    let obj = dict.get(key).ok()?;
    let obj = resolve_object(doc, obj);
    match obj {
        lopdf::Object::String(bytes, _) => Some(decode_pdf_string(bytes)),
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        _ => None,
    }
}

/// Resolve a potentially indirect object reference.
fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}
