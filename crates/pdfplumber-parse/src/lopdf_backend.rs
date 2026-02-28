//! lopdf-based PDF parsing backend.
//!
//! Implements [`PdfBackend`] using the [lopdf](https://crates.io/crates/lopdf)
//! crate for PDF document parsing. This is the default backend for pdfplumber-rs.

use crate::backend::PdfBackend;
use crate::error::BackendError;
use crate::handler::ContentHandler;
use pdfplumber_core::{
    Annotation, AnnotationType, BBox, Bookmark, DocumentMetadata, ExtractOptions, Hyperlink,
};

/// A parsed PDF document backed by lopdf.
pub struct LopdfDocument {
    /// The underlying lopdf document.
    inner: lopdf::Document,
    /// Cached ordered list of page ObjectIds (indexed by 0-based page number).
    page_ids: Vec<lopdf::ObjectId>,
}

impl LopdfDocument {
    /// Access the underlying lopdf document.
    pub fn inner(&self) -> &lopdf::Document {
        &self.inner
    }
}

impl std::fmt::Debug for LopdfDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LopdfDocument")
            .field("page_count", &self.page_ids.len())
            .finish_non_exhaustive()
    }
}

/// A reference to a single page within a [`LopdfDocument`].
#[derive(Debug, Clone, Copy)]
pub struct LopdfPage {
    /// The lopdf object ID for this page.
    pub object_id: lopdf::ObjectId,
    /// The 0-based page index.
    pub index: usize,
}

/// The lopdf-based PDF backend.
///
/// Provides PDF parsing via [`lopdf::Document`]. This is the default
/// backend used by pdfplumber-rs.
///
/// # Example
///
/// ```ignore
/// use pdfplumber_parse::lopdf_backend::LopdfBackend;
/// use pdfplumber_parse::PdfBackend;
///
/// let doc = LopdfBackend::open(pdf_bytes)?;
/// let count = LopdfBackend::page_count(&doc);
/// let page = LopdfBackend::get_page(&doc, 0)?;
/// ```
pub struct LopdfBackend;

/// Extract a [`BBox`] from a lopdf array of 4 numbers `[x0, y0, x1, y1]`.
fn extract_bbox_from_array(array: &[lopdf::Object]) -> Result<BBox, BackendError> {
    if array.len() != 4 {
        return Err(BackendError::Parse(format!(
            "expected 4-element array for box, got {}",
            array.len()
        )));
    }
    let x0 = object_to_f64(&array[0])?;
    let y0 = object_to_f64(&array[1])?;
    let x1 = object_to_f64(&array[2])?;
    let y1 = object_to_f64(&array[3])?;
    Ok(BBox::new(x0, y0, x1, y1))
}

/// Convert a lopdf numeric object (Integer or Real) to f64.
pub(crate) fn object_to_f64(obj: &lopdf::Object) -> Result<f64, BackendError> {
    match obj {
        lopdf::Object::Integer(i) => Ok(*i as f64),
        lopdf::Object::Real(f) => Ok(*f as f64),
        _ => Err(BackendError::Parse(format!("expected number, got {obj:?}"))),
    }
}

/// Look up a key in the page dictionary, walking up the page tree
/// (via /Parent) if the key is not found on the page itself.
///
/// Returns `None` if the key is not found anywhere in the tree.
fn resolve_inherited<'a>(
    doc: &'a lopdf::Document,
    page_id: lopdf::ObjectId,
    key: &[u8],
) -> Result<Option<&'a lopdf::Object>, BackendError> {
    let mut current_id = page_id;
    loop {
        let dict = doc
            .get_object(current_id)
            .and_then(|o| o.as_dict())
            .map_err(|e| BackendError::Parse(format!("failed to get page dictionary: {e}")))?;

        if let Ok(value) = dict.get(key) {
            return Ok(Some(value));
        }

        // Try to follow /Parent link
        match dict.get(b"Parent") {
            Ok(parent_obj) => {
                current_id = parent_obj
                    .as_reference()
                    .map_err(|e| BackendError::Parse(format!("invalid /Parent reference: {e}")))?;
            }
            Err(_) => return Ok(None),
        }
    }
}

impl PdfBackend for LopdfBackend {
    type Document = LopdfDocument;
    type Page = LopdfPage;
    type Error = BackendError;

    fn open(bytes: &[u8]) -> Result<Self::Document, Self::Error> {
        let inner = lopdf::Document::load_mem(bytes)
            .map_err(|e| BackendError::Parse(format!("failed to parse PDF: {e}")))?;

        // Cache page IDs in order (get_pages returns BTreeMap<u32, ObjectId> with 1-based keys)
        let pages_map = inner.get_pages();
        let page_ids: Vec<lopdf::ObjectId> = pages_map.values().copied().collect();

        Ok(LopdfDocument { inner, page_ids })
    }

    fn page_count(doc: &Self::Document) -> usize {
        doc.page_ids.len()
    }

    fn get_page(doc: &Self::Document, index: usize) -> Result<Self::Page, Self::Error> {
        if index >= doc.page_ids.len() {
            return Err(BackendError::Parse(format!(
                "page index {index} out of range (0..{})",
                doc.page_ids.len()
            )));
        }
        Ok(LopdfPage {
            object_id: doc.page_ids[index],
            index,
        })
    }

    fn page_media_box(doc: &Self::Document, page: &Self::Page) -> Result<BBox, Self::Error> {
        let obj = resolve_inherited(&doc.inner, page.object_id, b"MediaBox")?
            .ok_or_else(|| BackendError::Parse("MediaBox not found on page or ancestors".into()))?;
        let array = obj
            .as_array()
            .map_err(|e| BackendError::Parse(format!("MediaBox is not an array: {e}")))?;
        extract_bbox_from_array(array)
    }

    fn page_crop_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error> {
        // CropBox is optional — only look at the page itself, not inherited
        let dict = doc
            .inner
            .get_object(page.object_id)
            .and_then(|o| o.as_dict())
            .map_err(|e| BackendError::Parse(format!("failed to get page dictionary: {e}")))?;

        match dict.get(b"CropBox") {
            Ok(obj) => {
                let array = obj
                    .as_array()
                    .map_err(|e| BackendError::Parse(format!("CropBox is not an array: {e}")))?;
                Ok(Some(extract_bbox_from_array(array)?))
            }
            Err(_) => Ok(None),
        }
    }

    fn page_trim_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error> {
        match resolve_inherited(&doc.inner, page.object_id, b"TrimBox")? {
            Some(obj) => {
                let array = obj
                    .as_array()
                    .map_err(|e| BackendError::Parse(format!("TrimBox is not an array: {e}")))?;
                Ok(Some(extract_bbox_from_array(array)?))
            }
            None => Ok(None),
        }
    }

    fn page_bleed_box(
        doc: &Self::Document,
        page: &Self::Page,
    ) -> Result<Option<BBox>, Self::Error> {
        match resolve_inherited(&doc.inner, page.object_id, b"BleedBox")? {
            Some(obj) => {
                let array = obj
                    .as_array()
                    .map_err(|e| BackendError::Parse(format!("BleedBox is not an array: {e}")))?;
                Ok(Some(extract_bbox_from_array(array)?))
            }
            None => Ok(None),
        }
    }

    fn page_art_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error> {
        match resolve_inherited(&doc.inner, page.object_id, b"ArtBox")? {
            Some(obj) => {
                let array = obj
                    .as_array()
                    .map_err(|e| BackendError::Parse(format!("ArtBox is not an array: {e}")))?;
                Ok(Some(extract_bbox_from_array(array)?))
            }
            None => Ok(None),
        }
    }

    fn page_rotate(doc: &Self::Document, page: &Self::Page) -> Result<i32, Self::Error> {
        match resolve_inherited(&doc.inner, page.object_id, b"Rotate")? {
            Some(obj) => {
                let rotation = obj
                    .as_i64()
                    .map_err(|e| BackendError::Parse(format!("Rotate is not an integer: {e}")))?;
                Ok(rotation as i32)
            }
            None => Ok(0), // Default rotation is 0
        }
    }

    fn document_metadata(doc: &Self::Document) -> Result<DocumentMetadata, Self::Error> {
        extract_document_metadata(&doc.inner)
    }

    fn document_bookmarks(doc: &Self::Document) -> Result<Vec<Bookmark>, Self::Error> {
        extract_document_bookmarks(&doc.inner)
    }

    fn page_annotations(
        doc: &Self::Document,
        page: &Self::Page,
    ) -> Result<Vec<Annotation>, Self::Error> {
        extract_page_annotations(&doc.inner, page.object_id)
    }

    fn page_hyperlinks(
        doc: &Self::Document,
        page: &Self::Page,
    ) -> Result<Vec<Hyperlink>, Self::Error> {
        extract_page_hyperlinks(&doc.inner, page.object_id)
    }

    fn interpret_page(
        doc: &Self::Document,
        page: &Self::Page,
        handler: &mut dyn ContentHandler,
        options: &ExtractOptions,
    ) -> Result<(), Self::Error> {
        let inner = &doc.inner;

        // Get the page dictionary
        let page_dict = inner
            .get_object(page.object_id)
            .and_then(|o| o.as_dict())
            .map_err(|e| BackendError::Parse(format!("failed to get page dictionary: {e}")))?;

        // Get page content stream bytes
        let content_bytes = get_page_content_bytes(inner, page_dict)?;

        // Get page resources (may be inherited)
        let resources = get_page_resources(inner, page.object_id)?;

        // Initialize state machines
        let mut gstate = crate::interpreter_state::InterpreterState::new();
        let mut tstate = crate::text_state::TextState::new();

        // Interpret the content stream
        crate::interpreter::interpret_content_stream(
            inner,
            &content_bytes,
            resources,
            handler,
            options,
            0, // page-level depth
            &mut gstate,
            &mut tstate,
        )
    }
}

/// Get the content stream bytes from a page dictionary.
///
/// Handles both single stream references and arrays of stream references.
fn get_page_content_bytes(
    doc: &lopdf::Document,
    page_dict: &lopdf::Dictionary,
) -> Result<Vec<u8>, BackendError> {
    let contents_obj = match page_dict.get(b"Contents") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // Page with no content
    };

    match contents_obj {
        lopdf::Object::Reference(id) => {
            let obj = doc
                .get_object(*id)
                .map_err(|e| BackendError::Parse(format!("failed to resolve /Contents: {e}")))?;
            let stream = obj
                .as_stream()
                .map_err(|e| BackendError::Parse(format!("/Contents is not a stream: {e}")))?;
            decode_content_stream(stream)
        }
        lopdf::Object::Array(arr) => {
            let mut content = Vec::new();
            for item in arr {
                let id = item.as_reference().map_err(|e| {
                    BackendError::Parse(format!("/Contents array item is not a reference: {e}"))
                })?;
                let obj = doc.get_object(id).map_err(|e| {
                    BackendError::Parse(format!("failed to resolve /Contents stream: {e}"))
                })?;
                let stream = obj.as_stream().map_err(|e| {
                    BackendError::Parse(format!("/Contents array item is not a stream: {e}"))
                })?;
                let bytes = decode_content_stream(stream)?;
                if !content.is_empty() {
                    content.push(b' ');
                }
                content.extend_from_slice(&bytes);
            }
            Ok(content)
        }
        _ => Err(BackendError::Parse(
            "/Contents is not a reference or array".to_string(),
        )),
    }
}

/// Decode a content stream, decompressing if needed.
fn decode_content_stream(stream: &lopdf::Stream) -> Result<Vec<u8>, BackendError> {
    if stream.dict.get(b"Filter").is_ok() {
        stream
            .decompressed_content()
            .map_err(|e| BackendError::Parse(format!("failed to decompress content stream: {e}")))
    } else {
        Ok(stream.content.clone())
    }
}

/// Get the resources dictionary for a page, handling inheritance.
fn get_page_resources(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<&lopdf::Dictionary, BackendError> {
    match resolve_inherited(doc, page_id, b"Resources")? {
        Some(obj) => {
            // Resolve indirect reference if needed
            let obj = match obj {
                lopdf::Object::Reference(id) => doc.get_object(*id).map_err(|e| {
                    BackendError::Parse(format!("failed to resolve /Resources reference: {e}"))
                })?,
                other => other,
            };
            obj.as_dict()
                .map_err(|_| BackendError::Parse("/Resources is not a dictionary".to_string()))
        }
        None => {
            // No resources at all — use empty dictionary
            // This is unusual but we handle it gracefully
            static EMPTY_DICT: std::sync::LazyLock<lopdf::Dictionary> =
                std::sync::LazyLock::new(lopdf::Dictionary::new);
            Ok(&EMPTY_DICT)
        }
    }
}

/// Extract a string value from a lopdf dictionary, handling both String and Name types.
fn extract_string_from_dict(
    doc: &lopdf::Document,
    dict: &lopdf::Dictionary,
    key: &[u8],
) -> Option<String> {
    let obj = dict.get(key).ok()?;
    // Resolve indirect reference if needed
    let obj = match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).ok()?,
        other => other,
    };
    match obj {
        lopdf::Object::String(bytes, _) => {
            // Try UTF-16 BE (BOM: 0xFE 0xFF) first, then Latin-1/UTF-8
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
                // Try UTF-8 first, fall back to Latin-1
                match std::str::from_utf8(bytes) {
                    Ok(s) => Some(s.to_string()),
                    Err(_) => Some(bytes.iter().map(|&b| b as char).collect()),
                }
            }
        }
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        _ => None,
    }
}

/// Extract document-level metadata from the PDF /Info dictionary.
fn extract_document_metadata(doc: &lopdf::Document) -> Result<DocumentMetadata, BackendError> {
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
fn extract_document_bookmarks(doc: &lopdf::Document) -> Result<Vec<Bookmark>, BackendError> {
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

/// Extract annotations from a page's /Annots array.
fn extract_page_annotations(
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
fn extract_page_hyperlinks(
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

/// Create a minimal valid PDF document with the given number of pages.
///
/// Each page is US Letter size (612 x 792 points) with no content.
/// Used for testing purposes.
#[cfg(test)]
fn create_test_pdf(page_count: usize) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let mut page_ids: Vec<Object> = Vec::new();
    for _ in 0..page_count {
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        page_ids.push(page_id.into());
    }

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => page_count as i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF where pages inherit MediaBox from the Pages parent node.
#[cfg(test)]
fn create_test_pdf_inherited_media_box() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Page WITHOUT its own MediaBox — should inherit from parent
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has an explicit CropBox.
#[cfg(test)]
fn create_test_pdf_with_crop_box() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "CropBox" => vec![
            Object::Real(36.0),
            Object::Real(36.0),
            Object::Real(576.0),
            Object::Real(756.0),
        ],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has a /Rotate value.
#[cfg(test)]
fn create_test_pdf_with_rotate(rotation: i64) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Rotate" => rotation,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF where Rotate is inherited from the Pages parent node.
#[cfg(test)]
fn create_test_pdf_inherited_rotate(rotation: i64) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Page WITHOUT Rotate — should inherit from parent
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
            "Rotate" => rotation,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that references a Form XObject containing text.
///
/// Page content: `q /FM1 Do Q`
/// Form XObject FM1 content: `BT /F1 12 Tf 72 700 Td (Hello) Tj ET`
#[cfg(test)]
fn create_test_pdf_with_form_xobject() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Minimal Type1 font dictionary
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Form XObject stream: contains text
    let form_content = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    let form_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        form_content.to_vec(),
    );
    let form_id = doc.add_object(Object::Stream(form_stream));

    // Page content: invoke the form XObject
    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => form_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with nested Form XObjects (2 levels).
///
/// Page content: `q /FM1 Do Q`
/// FM1 content: `q /FM2 Do Q` (references FM2)
/// FM2 content: `BT /F1 10 Tf (Deep) Tj ET` (actual text)
#[cfg(test)]
fn create_test_pdf_with_nested_form_xobjects() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Inner Form XObject (FM2): contains actual text
    let fm2_content = b"BT /F1 10 Tf (Deep) Tj ET";
    let fm2_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        fm2_content.to_vec(),
    );
    let fm2_id = doc.add_object(Object::Stream(fm2_stream));

    // Outer Form XObject (FM1): references FM2
    let fm1_content = b"q /FM2 Do Q";
    let fm1_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => Object::Dictionary(dictionary! {
                "XObject" => Object::Dictionary(dictionary! {
                    "FM2" => fm2_id,
                }),
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        fm1_content.to_vec(),
    );
    let fm1_id = doc.add_object(Object::Stream(fm1_stream));

    // Page content: invoke FM1
    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => fm1_id,
            }),
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a Form XObject that has a /Matrix transform.
///
/// The Form XObject has /Matrix [2 0 0 2 10 20] (scale 2x + translate).
#[cfg(test)]
fn create_test_pdf_form_xobject_with_matrix() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let form_content = b"BT /F1 12 Tf (A) Tj ET";
    let form_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Matrix" => vec![
                Object::Real(2.0), Object::Real(0.0),
                Object::Real(0.0), Object::Real(2.0),
                Object::Real(10.0), Object::Real(20.0),
            ],
            "Resources" => Object::Dictionary(dictionary! {
                "Font" => Object::Dictionary(dictionary! {
                    "F1" => font_id,
                }),
            }),
        },
        form_content.to_vec(),
    );
    let form_id = doc.add_object(Object::Stream(form_stream));

    let page_content = b"q /FM1 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "FM1" => form_id,
            }),
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with an Image XObject (not Form).
#[cfg(test)]
fn create_test_pdf_with_image_xobject() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // 2x2 RGB image (12 bytes of pixel data)
    let image_data = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2i64,
            "Height" => 2i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8i64,
        },
        image_data,
    );
    let image_id = doc.add_object(Object::Stream(image_stream));

    // Page content: scale then place image
    let page_content = b"q 200 0 0 150 100 300 cm /Im0 Do Q";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "XObject" => Object::Dictionary(dictionary! {
                "Im0" => image_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a PDF with a page that has direct text content (no XObjects).
#[cfg(test)]
fn create_test_pdf_with_text_content() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let page_content = b"BT /F1 12 Tf 72 700 Td (Hi) Tj ET";
    let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
    let content_id = doc.add_object(Object::Stream(page_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => Object::Dictionary(dictionary! {
            "Font" => Object::Dictionary(dictionary! {
                "F1" => font_id,
            }),
        }),
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a test PDF with an /Info metadata dictionary.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn create_test_pdf_with_metadata(
    title: Option<&str>,
    author: Option<&str>,
    subject: Option<&str>,
    keywords: Option<&str>,
    creator: Option<&str>,
    producer: Option<&str>,
    creation_date: Option<&str>,
    mod_date: Option<&str>,
) -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    // Build /Info dictionary
    let mut info_dict = lopdf::Dictionary::new();
    if let Some(v) = title {
        info_dict.set("Title", Object::string_literal(v));
    }
    if let Some(v) = author {
        info_dict.set("Author", Object::string_literal(v));
    }
    if let Some(v) = subject {
        info_dict.set("Subject", Object::string_literal(v));
    }
    if let Some(v) = keywords {
        info_dict.set("Keywords", Object::string_literal(v));
    }
    if let Some(v) = creator {
        info_dict.set("Creator", Object::string_literal(v));
    }
    if let Some(v) = producer {
        info_dict.set("Producer", Object::string_literal(v));
    }
    if let Some(v) = creation_date {
        info_dict.set("CreationDate", Object::string_literal(v));
    }
    if let Some(v) = mod_date {
        info_dict.set("ModDate", Object::string_literal(v));
    }

    let info_id = doc.add_object(Object::Dictionary(info_dict));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{CharEvent, ContentHandler, ImageEvent};
    use pdfplumber_core::PdfError;

    // --- CollectingHandler for interpret_page tests ---

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

    // --- open() tests ---

    #[test]
    fn open_valid_single_page_pdf() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_valid_multi_page_pdf() {
        let pdf_bytes = create_test_pdf(5);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 5);
    }

    #[test]
    fn open_invalid_bytes_returns_error() {
        let result = LopdfBackend::open(b"not a pdf");
        assert!(result.is_err());
    }

    #[test]
    fn open_empty_bytes_returns_error() {
        let result = LopdfBackend::open(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn open_error_converts_to_pdf_error() {
        let err = LopdfBackend::open(b"garbage").unwrap_err();
        let pdf_err: PdfError = err.into();
        assert!(matches!(pdf_err, PdfError::ParseError(_)));
    }

    // --- page_count() tests ---

    #[test]
    fn page_count_zero_pages() {
        let pdf_bytes = create_test_pdf(0);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 0);
    }

    #[test]
    fn page_count_three_pages() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 3);
    }

    // --- get_page() tests ---

    #[test]
    fn get_page_first_page() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        assert_eq!(page.index, 0);
    }

    #[test]
    fn get_page_last_page() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 2).unwrap();
        assert_eq!(page.index, 2);
    }

    #[test]
    fn get_page_out_of_bounds() {
        let pdf_bytes = create_test_pdf(2);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let result = LopdfBackend::get_page(&doc, 2);
        assert!(result.is_err());
    }

    #[test]
    fn get_page_out_of_bounds_error_converts_to_pdf_error() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let err = LopdfBackend::get_page(&doc, 5).unwrap_err();
        let pdf_err: PdfError = err.into();
        assert!(matches!(pdf_err, PdfError::ParseError(_)));
        assert!(pdf_err.to_string().contains("out of range"));
    }

    #[test]
    fn get_page_on_empty_document() {
        let pdf_bytes = create_test_pdf(0);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let result = LopdfBackend::get_page(&doc, 0);
        assert!(result.is_err());
    }

    // --- Page object IDs are distinct ---

    #[test]
    fn pages_have_distinct_object_ids() {
        let pdf_bytes = create_test_pdf(3);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page0 = LopdfBackend::get_page(&doc, 0).unwrap();
        let page1 = LopdfBackend::get_page(&doc, 1).unwrap();
        let page2 = LopdfBackend::get_page(&doc, 2).unwrap();
        assert_ne!(page0.object_id, page1.object_id);
        assert_ne!(page1.object_id, page2.object_id);
        assert_ne!(page0.object_id, page2.object_id);
    }

    // --- Integration: open + page_count + get_page round-trip ---

    #[test]
    fn round_trip_open_count_access() {
        let pdf_bytes = create_test_pdf(4);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let count = LopdfBackend::page_count(&doc);
        assert_eq!(count, 4);

        for i in 0..count {
            let page = LopdfBackend::get_page(&doc, i).unwrap();
            assert_eq!(page.index, i);
        }

        // One past the end should fail
        assert!(LopdfBackend::get_page(&doc, count).is_err());
    }

    // --- page_media_box() tests ---

    #[test]
    fn media_box_explicit_us_letter() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box, BBox::new(0.0, 0.0, 612.0, 792.0));
    }

    #[test]
    fn media_box_inherited_from_parent() {
        let pdf_bytes = create_test_pdf_inherited_media_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        // Inherited A4 size from parent Pages node
        assert_eq!(media_box, BBox::new(0.0, 0.0, 595.0, 842.0));
    }

    #[test]
    fn media_box_width_height() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box.width(), 612.0);
        assert_eq!(media_box.height(), 792.0);
    }

    // --- page_crop_box() tests ---

    #[test]
    fn crop_box_present() {
        let pdf_bytes = create_test_pdf_with_crop_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        assert_eq!(crop_box, Some(BBox::new(36.0, 36.0, 576.0, 756.0)));
    }

    #[test]
    fn crop_box_absent() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        assert_eq!(crop_box, None);
    }

    // --- page_rotate() tests ---

    #[test]
    fn rotate_default_zero() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 0);
    }

    #[test]
    fn rotate_90() {
        let pdf_bytes = create_test_pdf_with_rotate(90);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 90);
    }

    #[test]
    fn rotate_180() {
        let pdf_bytes = create_test_pdf_with_rotate(180);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 180);
    }

    #[test]
    fn rotate_270() {
        let pdf_bytes = create_test_pdf_with_rotate(270);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 270);
    }

    #[test]
    fn rotate_inherited_from_parent() {
        let pdf_bytes = create_test_pdf_inherited_rotate(90);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotation, 90);
    }

    // --- Integration: all page properties together ---

    #[test]
    fn page_properties_round_trip() {
        let pdf_bytes = create_test_pdf_with_crop_box();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let media_box = LopdfBackend::page_media_box(&doc, &page).unwrap();
        let crop_box = LopdfBackend::page_crop_box(&doc, &page).unwrap();
        let rotation = LopdfBackend::page_rotate(&doc, &page).unwrap();

        assert_eq!(media_box, BBox::new(0.0, 0.0, 612.0, 792.0));
        assert!(crop_box.is_some());
        assert_eq!(rotation, 0);
    }

    // --- interpret_page: basic text extraction ---

    #[test]
    fn interpret_page_simple_text() {
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // "Hi" = 2 characters
        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'i' as u32);
        assert_eq!(handler.chars[0].font_size, 12.0);
        assert_eq!(handler.chars[0].font_name, "Helvetica");
    }

    #[test]
    fn interpret_page_no_content() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        // Page with no /Contents should not fail
        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();
        assert_eq!(handler.chars.len(), 0);
    }

    // --- interpret_page: Form XObject tests (US-016) ---

    #[test]
    fn interpret_page_form_xobject_text() {
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Form XObject contains "Hello" = 5 chars
        assert_eq!(handler.chars.len(), 5);
        assert_eq!(handler.chars[0].char_code, b'H' as u32);
        assert_eq!(handler.chars[1].char_code, b'e' as u32);
        assert_eq!(handler.chars[2].char_code, b'l' as u32);
        assert_eq!(handler.chars[3].char_code, b'l' as u32);
        assert_eq!(handler.chars[4].char_code, b'o' as u32);
        assert_eq!(handler.chars[0].font_name, "Helvetica");
        assert_eq!(handler.chars[0].font_size, 12.0);
    }

    #[test]
    fn interpret_page_nested_form_xobjects() {
        let pdf_bytes = create_test_pdf_with_nested_form_xobjects();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Nested form XObject FM1→FM2 contains "Deep" = 4 chars
        assert_eq!(handler.chars.len(), 4);
        assert_eq!(handler.chars[0].char_code, b'D' as u32);
        assert_eq!(handler.chars[1].char_code, b'e' as u32);
        assert_eq!(handler.chars[2].char_code, b'e' as u32);
        assert_eq!(handler.chars[3].char_code, b'p' as u32);
    }

    #[test]
    fn interpret_page_form_xobject_matrix_applied() {
        let pdf_bytes = create_test_pdf_form_xobject_with_matrix();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Form XObject has /Matrix [2 0 0 2 10 20], character "A"
        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].char_code, b'A' as u32);
        // CTM should include the form's matrix transform
        let ctm = handler.chars[0].ctm;
        // Form matrix [2 0 0 2 10 20] applied on top of identity
        assert!((ctm[0] - 2.0).abs() < 0.01);
        assert!((ctm[3] - 2.0).abs() < 0.01);
        assert!((ctm[4] - 10.0).abs() < 0.01);
        assert!((ctm[5] - 20.0).abs() < 0.01);
    }

    #[test]
    fn interpret_page_form_xobject_state_restored() {
        // After processing a Form XObject, the graphics state should be restored.
        // The Form XObject is wrapped in q/Q on the page, and the interpreter
        // also saves/restores state around the Form XObject.
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        // This should complete without errors (state properly saved/restored)
        let result = LopdfBackend::interpret_page(&doc, &page, &mut handler, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn interpret_page_image_xobject() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        LopdfBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        // Should have 1 image event, no chars
        assert_eq!(handler.chars.len(), 0);
        assert_eq!(handler.images.len(), 1);
        assert_eq!(handler.images[0].name, "Im0");
        assert_eq!(handler.images[0].width, 2);
        assert_eq!(handler.images[0].height, 2);
        assert_eq!(handler.images[0].colorspace.as_deref(), Some("DeviceRGB"));
        assert_eq!(handler.images[0].bits_per_component, Some(8));
        // CTM should be [200 0 0 150 100 300] from the cm operator
        let ctm = handler.images[0].ctm;
        assert!((ctm[0] - 200.0).abs() < 0.01);
        assert!((ctm[3] - 150.0).abs() < 0.01);
        assert!((ctm[4] - 100.0).abs() < 0.01);
        assert!((ctm[5] - 300.0).abs() < 0.01);
    }

    #[test]
    fn interpret_page_recursion_limit() {
        // Use the nested form XObject PDF but with max_recursion_depth = 0
        let pdf_bytes = create_test_pdf_with_form_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();
        let mut options = ExtractOptions::default();
        options.max_recursion_depth = 0; // Page level = 0, Form XObject = 1 > limit
        let mut handler = CollectingHandler::new();

        let result = LopdfBackend::interpret_page(&doc, &page, &mut handler, &options);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("recursion depth"));
    }

    // --- document_metadata() tests ---

    #[test]
    fn metadata_full_info_dictionary() {
        let pdf_bytes = create_test_pdf_with_metadata(
            Some("Test Document"),
            Some("John Doe"),
            Some("Testing metadata"),
            Some("test, pdf, rust"),
            Some("LibreOffice"),
            Some("pdfplumber-rs"),
            Some("D:20240101120000+00'00'"),
            Some("D:20240615153000+00'00'"),
        );
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert_eq!(meta.title.as_deref(), Some("Test Document"));
        assert_eq!(meta.author.as_deref(), Some("John Doe"));
        assert_eq!(meta.subject.as_deref(), Some("Testing metadata"));
        assert_eq!(meta.keywords.as_deref(), Some("test, pdf, rust"));
        assert_eq!(meta.creator.as_deref(), Some("LibreOffice"));
        assert_eq!(meta.producer.as_deref(), Some("pdfplumber-rs"));
        assert_eq!(
            meta.creation_date.as_deref(),
            Some("D:20240101120000+00'00'")
        );
        assert_eq!(meta.mod_date.as_deref(), Some("D:20240615153000+00'00'"));
        assert!(!meta.is_empty());
    }

    #[test]
    fn metadata_partial_info_dictionary() {
        let pdf_bytes = create_test_pdf_with_metadata(
            Some("Only Title"),
            None,
            None,
            None,
            None,
            Some("A Producer"),
            None,
            None,
        );
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert_eq!(meta.title.as_deref(), Some("Only Title"));
        assert_eq!(meta.author, None);
        assert_eq!(meta.subject, None);
        assert_eq!(meta.keywords, None);
        assert_eq!(meta.creator, None);
        assert_eq!(meta.producer.as_deref(), Some("A Producer"));
        assert_eq!(meta.creation_date, None);
        assert_eq!(meta.mod_date, None);
        assert!(!meta.is_empty());
    }

    #[test]
    fn metadata_no_info_dictionary() {
        // create_test_pdf doesn't add an /Info dictionary
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let meta = LopdfBackend::document_metadata(&doc).unwrap();

        assert!(meta.is_empty());
        assert_eq!(meta.title, None);
        assert_eq!(meta.author, None);
    }
}
