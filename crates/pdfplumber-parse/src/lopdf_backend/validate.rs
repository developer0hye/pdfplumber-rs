//! PDF document validation and repair.
//!
//! Validates structural integrity of PDF documents and attempts repairs
//! for common malformations. Called from the main backend module.

use crate::error::BackendError;
use pdfplumber_core::{RepairOptions, RepairResult, ValidationIssue};
use super::{LopdfDocument, resolve_inherited, resolve_ref};

pub(super) fn validate_document(doc: &LopdfDocument) -> Result<Vec<ValidationIssue>, BackendError> {
    use pdfplumber_core::{Severity, ValidationIssue};

    let inner = &doc.inner;
    let mut issues = Vec::new();

    // 1. Check catalog for required /Type key
    let catalog_location = get_catalog_location(inner);
    let catalog_dict = get_catalog_dict(inner);

    if let Some(dict) = catalog_dict {
        match dict.get(b"Type") {
            Ok(type_obj) => {
                if let Ok(name) = type_obj.as_name() {
                    if name != b"Catalog" {
                        let name_str = String::from_utf8_lossy(name);
                        issues.push(ValidationIssue::with_location(
                            Severity::Warning,
                            "WRONG_CATALOG_TYPE",
                            format!("catalog /Type is '{name_str}' instead of 'Catalog'"),
                            &catalog_location,
                        ));
                    }
                }
            }
            Err(_) => {
                issues.push(ValidationIssue::with_location(
                    Severity::Warning,
                    "MISSING_TYPE",
                    "catalog dictionary missing /Type key",
                    &catalog_location,
                ));
            }
        }

        // Check /Pages exists
        if dict.get(b"Pages").is_err() {
            issues.push(ValidationIssue::with_location(
                Severity::Error,
                "MISSING_PAGES",
                "catalog dictionary missing /Pages key",
                &catalog_location,
            ));
        }
    }

    // 2. Check page tree structure
    for (page_idx, &page_id) in doc.page_ids.iter().enumerate() {
        let page_num = page_idx + 1;
        let location = format!("page {page_num} (object {} {})", page_id.0, page_id.1);

        match inner.get_object(page_id) {
            Ok(obj) => {
                if let Ok(dict) = obj.as_dict() {
                    // Check page /Type key
                    match dict.get(b"Type") {
                        Ok(type_obj) => {
                            if let Ok(name) = type_obj.as_name() {
                                if name != b"Page" {
                                    let name_str = String::from_utf8_lossy(name);
                                    issues.push(ValidationIssue::with_location(
                                        Severity::Warning,
                                        "WRONG_PAGE_TYPE",
                                        format!("page /Type is '{name_str}' instead of 'Page'"),
                                        &location,
                                    ));
                                }
                            }
                        }
                        Err(_) => {
                            issues.push(ValidationIssue::with_location(
                                Severity::Warning,
                                "MISSING_TYPE",
                                "page dictionary missing /Type key",
                                &location,
                            ));
                        }
                    }

                    // Check MediaBox (required, can be inherited)
                    if resolve_inherited(inner, page_id, b"MediaBox")
                        .ok()
                        .flatten()
                        .is_none()
                    {
                        issues.push(ValidationIssue::with_location(
                            Severity::Error,
                            "MISSING_MEDIABOX",
                            "page has no /MediaBox (not on page or ancestors)",
                            &location,
                        ));
                    }

                    // Check for missing fonts referenced in content streams
                    check_page_fonts(inner, page_id, dict, &location, &mut issues);
                } else {
                    issues.push(ValidationIssue::with_location(
                        Severity::Error,
                        "INVALID_PAGE",
                        "page object is not a dictionary",
                        &location,
                    ));
                }
            }
            Err(_) => {
                issues.push(ValidationIssue::with_location(
                    Severity::Error,
                    "BROKEN_REF",
                    format!("page object {} {} not found", page_id.0, page_id.1),
                    &location,
                ));
            }
        }
    }

    // 3. Check for broken object references in the xref table
    check_broken_references(inner, &mut issues);

    Ok(issues)
}

/// Get the catalog dictionary from the document.
fn get_catalog_dict(doc: &lopdf::Document) -> Option<&lopdf::Dictionary> {
    let root_obj = doc.trailer.get(b"Root").ok()?;
    match root_obj {
        lopdf::Object::Reference(id) => {
            let obj = doc.get_object(*id).ok()?;
            obj.as_dict().ok()
        }
        lopdf::Object::Dictionary(dict) => Some(dict),
        _ => None,
    }
}

/// Get a human-readable location string for the catalog object.
fn get_catalog_location(doc: &lopdf::Document) -> String {
    if let Ok(lopdf::Object::Reference(id)) = doc.trailer.get(b"Root") {
        return format!("object {} {}", id.0, id.1);
    }
    "catalog".to_string()
}

/// Check that fonts referenced in content streams are defined in page resources.
fn check_page_fonts(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    page_dict: &lopdf::Dictionary,
    location: &str,
    issues: &mut Vec<pdfplumber_core::ValidationIssue>,
) {
    use pdfplumber_core::{Severity, ValidationIssue};

    // Get fonts from resources
    let font_names = get_resource_font_names(doc, page_id, page_dict);

    // Get content stream to find font references
    let content_fonts = get_content_stream_font_refs(doc, page_dict);

    // Check each font referenced in the content stream
    for font_ref in &content_fonts {
        if !font_names.contains(font_ref) {
            issues.push(ValidationIssue::with_location(
                Severity::Warning,
                "MISSING_FONT",
                format!("font /{font_ref} referenced in content stream but not in resources"),
                location,
            ));
        }
    }
}

/// Get the names of fonts defined in the page's resources.
fn get_resource_font_names(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    page_dict: &lopdf::Dictionary,
) -> Vec<String> {
    let mut names = Vec::new();

    // Try to get Resources from the page or inherited
    let resources = if let Ok(res_obj) = page_dict.get(b"Resources") {
        let resolved = resolve_ref(doc, res_obj);
        resolved.as_dict().ok()
    } else {
        // Try inherited resources
        resolve_inherited(doc, page_id, b"Resources")
            .ok()
            .flatten()
            .and_then(|obj| obj.as_dict().ok())
    };

    if let Some(resources_dict) = resources {
        if let Ok(font_obj) = resources_dict.get(b"Font") {
            let font_obj = resolve_ref(doc, font_obj);
            if let Ok(font_dict) = font_obj.as_dict() {
                for (key, _) in font_dict.iter() {
                    if let Ok(name) = std::str::from_utf8(key) {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }

    names
}

/// Parse content stream operators to find font name references (Tf operator).
fn get_content_stream_font_refs(
    doc: &lopdf::Document,
    page_dict: &lopdf::Dictionary,
) -> Vec<String> {
    let mut font_refs = Vec::new();

    let content_bytes = match get_content_stream_bytes(doc, page_dict) {
        Some(bytes) => bytes,
        None => return font_refs,
    };

    // Simple parser: look for "/FontName <number> Tf" patterns
    let content = String::from_utf8_lossy(&content_bytes);
    let tokens: Vec<&str> = content.split_whitespace().collect();

    for (i, token) in tokens.iter().enumerate() {
        if *token == "Tf" && i >= 2 {
            let font_name_token = tokens[i - 2];
            if let Some(name) = font_name_token.strip_prefix('/') {
                if !font_refs.contains(&name.to_string()) {
                    font_refs.push(name.to_string());
                }
            }
        }
    }

    font_refs
}

/// Try to get decompressed content from a stream, falling back to raw content.
fn stream_bytes(stream: &lopdf::Stream) -> Option<Vec<u8>> {
    stream
        .decompressed_content()
        .ok()
        .or_else(|| Some(stream.content.clone()))
        .filter(|b| !b.is_empty())
}

/// Get the raw bytes of a page's content stream(s).
fn get_content_stream_bytes(
    doc: &lopdf::Document,
    page_dict: &lopdf::Dictionary,
) -> Option<Vec<u8>> {
    let contents_obj = page_dict.get(b"Contents").ok()?;

    // Resolve reference if needed
    let resolved = match contents_obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };

    match resolved {
        lopdf::Object::Stream(stream) => stream_bytes(stream),
        lopdf::Object::Array(arr) => {
            let mut all_bytes = Vec::new();
            for item in arr {
                let resolved = resolve_ref(doc, item);
                if let Ok(stream) = resolved.as_stream() {
                    if let Some(bytes) = stream_bytes(stream) {
                        all_bytes.extend_from_slice(&bytes);
                        all_bytes.push(b' ');
                    }
                }
            }
            if all_bytes.is_empty() {
                None
            } else {
                Some(all_bytes)
            }
        }
        _ => None,
    }
}

/// Check for broken object references across the document.
fn check_broken_references(
    doc: &lopdf::Document,
    issues: &mut Vec<pdfplumber_core::ValidationIssue>,
) {
    use pdfplumber_core::{Severity, ValidationIssue};

    // Iterate through all objects and check references
    for (&obj_id, obj) in &doc.objects {
        check_references_in_object(doc, obj, obj_id, issues);
    }

    fn check_references_in_object(
        doc: &lopdf::Document,
        obj: &lopdf::Object,
        source_id: lopdf::ObjectId,
        issues: &mut Vec<ValidationIssue>,
    ) {
        match obj {
            lopdf::Object::Reference(ref_id) => {
                if doc.get_object(*ref_id).is_err() {
                    issues.push(ValidationIssue::with_location(
                        Severity::Warning,
                        "BROKEN_REF",
                        format!(
                            "reference to object {} {} which does not exist",
                            ref_id.0, ref_id.1
                        ),
                        format!("object {} {}", source_id.0, source_id.1),
                    ));
                }
            }
            lopdf::Object::Array(arr) => {
                for item in arr {
                    check_references_in_object(doc, item, source_id, issues);
                }
            }
            lopdf::Object::Dictionary(dict) => {
                for (_, value) in dict.iter() {
                    check_references_in_object(doc, value, source_id, issues);
                }
            }
            lopdf::Object::Stream(stream) => {
                for (_, value) in stream.dict.iter() {
                    check_references_in_object(doc, value, source_id, issues);
                }
            }
            _ => {}
        }
    }
}

/// Resolve an indirect reference, returning the referenced object.
///
/// If the object is a `Reference`, resolves it via the document.
// resolve_ref is provided by super::resolve_ref

/// Attempt best-effort repair of common PDF issues.
pub(super) fn repair_document(
    bytes: &[u8],
    options: &RepairOptions,
) -> Result<(Vec<u8>, RepairResult), BackendError> {
    let mut doc = lopdf::Document::load_mem(bytes)
        .map_err(|e| BackendError::Parse(format!("failed to parse PDF for repair: {e}")))?;

    let mut result = RepairResult::new();

    if options.fix_stream_lengths {
        repair_stream_lengths(&mut doc, &mut result);
    }

    if options.remove_broken_objects {
        repair_broken_references(&mut doc, &mut result);
    }

    // rebuild_xref: lopdf rebuilds xref automatically when saving,
    // so just saving the document effectively rebuilds the xref table.
    if options.rebuild_xref {
        // Force xref rebuild by saving (lopdf always writes a fresh xref on save).
        // Only log if we explicitly opted in and haven't already logged anything.
    }

    let mut buf = Vec::new();
    doc.save_to(&mut buf)
        .map_err(|e| BackendError::Parse(format!("failed to save repaired PDF: {e}")))?;

    Ok((buf, result))
}

/// Fix stream `/Length` entries to match actual stream content size.
fn repair_stream_lengths(doc: &mut lopdf::Document, result: &mut RepairResult) {
    let obj_ids: Vec<lopdf::ObjectId> = doc.objects.keys().copied().collect();

    for obj_id in obj_ids {
        let needs_fix = if let Some(lopdf::Object::Stream(stream)) = doc.objects.get(&obj_id) {
            let actual_len = stream.content.len() as i64;
            match stream.dict.get(b"Length") {
                Ok(lopdf::Object::Integer(stored_len)) => *stored_len != actual_len,
                Ok(lopdf::Object::Reference(_)) => {
                    // Length stored as indirect reference — skip, too complex to fix
                    false
                }
                _ => true, // Missing Length key
            }
        } else {
            false
        };

        if needs_fix {
            if let Some(lopdf::Object::Stream(stream)) = doc.objects.get_mut(&obj_id) {
                let actual_len = stream.content.len() as i64;
                let old_len = stream.dict.get(b"Length").ok().and_then(|o| {
                    if let lopdf::Object::Integer(v) = o {
                        Some(*v)
                    } else {
                        None
                    }
                });
                stream
                    .dict
                    .set("Length", lopdf::Object::Integer(actual_len));
                match old_len {
                    Some(old) => {
                        result.log.push(format!(
                            "fixed stream length for object {} {}: {} -> {}",
                            obj_id.0, obj_id.1, old, actual_len
                        ));
                    }
                    None => {
                        result.log.push(format!(
                            "added missing stream length for object {} {}: {}",
                            obj_id.0, obj_id.1, actual_len
                        ));
                    }
                }
            }
        }
    }
}

/// Remove broken object references, replacing them with Null.
fn repair_broken_references(doc: &mut lopdf::Document, result: &mut RepairResult) {
    let obj_ids: Vec<lopdf::ObjectId> = doc.objects.keys().copied().collect();
    let existing_ids: std::collections::BTreeSet<lopdf::ObjectId> =
        doc.objects.keys().copied().collect();

    for obj_id in obj_ids {
        if let Some(obj) = doc.objects.remove(&obj_id) {
            let fixed = fix_references_in_object(obj, &existing_ids, obj_id, result);
            doc.objects.insert(obj_id, fixed);
        }
    }
}

/// Recursively replace broken references with Null in an object tree.
fn fix_references_in_object(
    obj: lopdf::Object,
    existing_ids: &std::collections::BTreeSet<lopdf::ObjectId>,
    source_id: lopdf::ObjectId,
    result: &mut RepairResult,
) -> lopdf::Object {
    match obj {
        lopdf::Object::Reference(ref_id) => {
            if existing_ids.contains(&ref_id) {
                obj
            } else {
                result.log.push(format!(
                    "removed broken reference to object {} {} (in object {} {})",
                    ref_id.0, ref_id.1, source_id.0, source_id.1
                ));
                lopdf::Object::Null
            }
        }
        lopdf::Object::Array(arr) => {
            let fixed: Vec<lopdf::Object> = arr
                .into_iter()
                .map(|item| fix_references_in_object(item, existing_ids, source_id, result))
                .collect();
            lopdf::Object::Array(fixed)
        }
        lopdf::Object::Dictionary(dict) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in dict.into_iter() {
                let fixed = fix_references_in_object(value, existing_ids, source_id, result);
                new_dict.set(key, fixed);
            }
            lopdf::Object::Dictionary(new_dict)
        }
        lopdf::Object::Stream(mut stream) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in stream.dict.into_iter() {
                let fixed = fix_references_in_object(value, existing_ids, source_id, result);
                new_dict.set(key, fixed);
            }
            stream.dict = new_dict;
            lopdf::Object::Stream(stream)
        }
        other => other,
    }
}
