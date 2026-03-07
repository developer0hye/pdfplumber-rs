//! lopdf-based PDF parsing backend.
//!
//! Implements [`PdfBackend`] using the [lopdf](https://crates.io/crates/lopdf)
//! crate for PDF document parsing. This is the default backend for pdfplumber-rs.

use crate::backend::PdfBackend;
use crate::error::BackendError;
use crate::handler::ContentHandler;
use pdfplumber_core::{
    Annotation, BBox, Bookmark, DocumentMetadata, ExtractOptions, FormField, Hyperlink,
    ImageContent, RepairOptions, RepairResult, SignatureInfo, StructElement, ValidationIssue,
};

mod annots;
mod forms;
mod metadata;
mod structure;
mod validate;

use annots::{extract_page_annotations, extract_page_hyperlinks};
use forms::{extract_document_form_fields, extract_document_signatures};
use metadata::{extract_document_bookmarks, extract_document_metadata};
use structure::extract_document_structure_tree;
use validate::{repair_document, try_fix_startxref, try_strip_preamble, validate_document};

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
pub(super) fn extract_bbox_from_array(array: &[lopdf::Object]) -> Result<BBox, BackendError> {
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
/// If the value is an indirect reference, it is automatically dereferenced.
/// Returns `None` if the key is not found anywhere in the tree.
pub(super) fn resolve_inherited<'a>(
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
            // Dereference indirect references (e.g. `/MediaBox 174 0 R`)
            let resolved = match value {
                lopdf::Object::Reference(id) => doc.get_object(*id).map_err(|e| {
                    BackendError::Parse(format!(
                        "failed to resolve indirect reference for /{}: {e}",
                        String::from_utf8_lossy(key)
                    ))
                })?,
                other => other,
            };
            return Ok(Some(resolved));
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
        // If the file has a preamble before %PDF- (or Ghostscript page markers),
        // clean those up first so lopdf can parse the file correctly.
        let effective_bytes = try_strip_preamble(bytes);
        let bytes = effective_bytes.as_deref().unwrap_or(bytes);

        let mut inner = match lopdf::Document::load_mem(bytes) {
            Ok(doc) => doc,
            Err(original_err) => {
                // Attempt startxref recovery: scan for the `xref` keyword
                // and fix the startxref offset if it's wrong. This handles
                // malformed PDFs like issue-297-example.pdf where the
                // startxref offset is incorrect.
                if let Some(repaired) = try_fix_startxref(bytes) {
                    lopdf::Document::load_mem(&repaired).map_err(|_| {
                        BackendError::Parse(format!("failed to parse PDF: {original_err}"))
                    })?
                } else {
                    return Err(BackendError::Parse(format!(
                        "failed to parse PDF: {original_err}"
                    )));
                }
            }
        };

        // For encrypted PDFs, try decrypting with an empty password first.
        // Many PDFs use owner-only encryption (restricting print/copy) with an
        // empty user password, which still allows reading. This matches Python
        // pdfplumber behavior.
        if inner.is_encrypted() && inner.decrypt("").is_err() {
            return Err(BackendError::Core(
                pdfplumber_core::PdfError::PasswordRequired,
            ));
        }

        // Cache page IDs in order (get_pages returns BTreeMap<u32, ObjectId> with 1-based keys)
        let pages_map = inner.get_pages();
        let page_ids: Vec<lopdf::ObjectId> = pages_map.values().copied().collect();

        Ok(LopdfDocument { inner, page_ids })
    }

    fn open_with_password(bytes: &[u8], password: &[u8]) -> Result<Self::Document, Self::Error> {
        let mut inner = lopdf::Document::load_mem(bytes)
            .map_err(|e| BackendError::Parse(format!("failed to parse PDF: {e}")))?;

        // Decrypt if encrypted; ignore password if not encrypted
        if inner.is_encrypted() {
            inner.decrypt_raw(password).map_err(|e| {
                let msg = e.to_string();
                if msg.contains("incorrect") || msg.contains("password") {
                    BackendError::Core(pdfplumber_core::PdfError::InvalidPassword)
                } else {
                    BackendError::Parse(format!("decryption failed: {e}"))
                }
            })?;
        }

        // Cache page IDs in order
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
                // Dereference indirect references
                let obj = match obj {
                    lopdf::Object::Reference(id) => doc.inner.get_object(*id).map_err(|e| {
                        BackendError::Parse(format!(
                            "failed to resolve indirect reference for /CropBox: {e}"
                        ))
                    })?,
                    other => other,
                };
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

    fn document_form_fields(doc: &Self::Document) -> Result<Vec<FormField>, Self::Error> {
        extract_document_form_fields(&doc.inner)
    }

    fn document_signatures(doc: &Self::Document) -> Result<Vec<SignatureInfo>, Self::Error> {
        extract_document_signatures(&doc.inner)
    }

    fn document_structure_tree(doc: &Self::Document) -> Result<Vec<StructElement>, Self::Error> {
        extract_document_structure_tree(&doc.inner)
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

    fn extract_image_content(
        doc: &Self::Document,
        page: &Self::Page,
        image_name: &str,
    ) -> Result<ImageContent, Self::Error> {
        use pdfplumber_core::ImageFormat;

        let inner = &doc.inner;

        // Get page resources
        let resources = get_page_resources(inner, page.object_id)?;

        // Look up /Resources/XObject/<image_name>
        let xobj_dict = resources.get(b"XObject").map_err(|_| {
            BackendError::Parse(format!(
                "no /XObject dictionary in page resources for image /{image_name}"
            ))
        })?;
        let xobj_dict = resolve_ref(inner, xobj_dict);
        let xobj_dict = xobj_dict.as_dict().map_err(|_| {
            BackendError::Parse("/XObject resource is not a dictionary".to_string())
        })?;

        let xobj_entry = xobj_dict.get(image_name.as_bytes()).map_err(|_| {
            BackendError::Parse(format!(
                "image XObject /{image_name} not found in resources"
            ))
        })?;

        let xobj_id = xobj_entry.as_reference().map_err(|_| {
            BackendError::Parse(format!(
                "image XObject /{image_name} is not an indirect reference"
            ))
        })?;

        let xobj = inner.get_object(xobj_id).map_err(|e| {
            BackendError::Parse(format!(
                "failed to resolve image XObject /{image_name}: {e}"
            ))
        })?;

        let stream = xobj.as_stream().map_err(|e| {
            BackendError::Parse(format!("image XObject /{image_name} is not a stream: {e}"))
        })?;

        // Verify it's an Image subtype
        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .unwrap_or(b"");
        if subtype != b"Image" {
            let subtype_str = String::from_utf8_lossy(subtype);
            return Err(BackendError::Parse(format!(
                "XObject /{image_name} is not an Image (subtype: {subtype_str})"
            )));
        }

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

        // Determine the filter to decide image format
        let filter = stream
            .dict
            .get(b"Filter")
            .ok()
            .and_then(|o| {
                // Filter can be a single name or an array of names
                if let Ok(name) = o.as_name() {
                    Some(vec![String::from_utf8_lossy(name).into_owned()])
                } else if let Ok(arr) = o.as_array() {
                    Some(
                        arr.iter()
                            .filter_map(|item| {
                                let resolved = resolve_ref(inner, item);
                                resolved
                                    .as_name()
                                    .ok()
                                    .map(|s| String::from_utf8_lossy(s).into_owned())
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Determine format from the last filter in the chain
        let format = if filter.is_empty() {
            ImageFormat::Raw
        } else {
            match filter.last().map(|s| s.as_str()) {
                Some("DCTDecode") => ImageFormat::Jpeg,
                Some("JBIG2Decode") => ImageFormat::Jbig2,
                Some("CCITTFaxDecode") => ImageFormat::CcittFax,
                _ => ImageFormat::Raw,
            }
        };

        // Extract the image data
        let data = match format {
            ImageFormat::Jpeg => {
                // For JPEG, return the raw stream content (the JPEG bytes)
                // If there are filters before DCTDecode, we need partial decompression
                if filter.len() == 1 {
                    // Only DCTDecode — raw content is the JPEG
                    stream.content.clone()
                } else {
                    // Chained filters: decompress everything (lopdf handles this)
                    stream.decompressed_content().map_err(|e| {
                        BackendError::Parse(format!(
                            "failed to decompress image /{image_name}: {e}"
                        ))
                    })?
                }
            }
            ImageFormat::Jbig2 | ImageFormat::CcittFax => {
                // Return raw stream content for these specialized formats
                stream.content.clone()
            }
            // Raw, Png, and any future non_exhaustive variants: decompress if filtered
            _ => {
                if filter.is_empty() {
                    stream.content.clone()
                } else {
                    stream.decompressed_content().map_err(|e| {
                        BackendError::Parse(format!(
                            "failed to decompress image /{image_name}: {e}"
                        ))
                    })?
                }
            }
        };

        Ok(ImageContent {
            data,
            format,
            width,
            height,
        })
    }

    fn validate(doc: &Self::Document) -> Result<Vec<ValidationIssue>, Self::Error> {
        validate_document(doc)
    }

    fn repair(
        bytes: &[u8],
        options: &RepairOptions,
    ) -> Result<(Vec<u8>, RepairResult), Self::Error> {
        repair_document(bytes, options)
    }
}

/// Validate a PDF document for specification violations.
fn get_page_content_bytes(
    doc: &lopdf::Document,
    page_dict: &lopdf::Dictionary,
) -> Result<Vec<u8>, BackendError> {
    let contents_obj = match page_dict.get(b"Contents") {
        Ok(obj) => obj,
        Err(_) => return Ok(Vec::new()), // Page with no content
    };

    // Resolve reference if needed
    let resolved = match contents_obj {
        lopdf::Object::Reference(id) => doc
            .get_object(*id)
            .map_err(|e| BackendError::Parse(format!("failed to resolve /Contents: {e}")))?,
        other => other,
    };

    match resolved {
        lopdf::Object::Stream(stream) => decode_content_stream(stream),
        lopdf::Object::Array(arr) => decode_contents_array(doc, arr),
        _ => Err(BackendError::Parse(
            "/Contents is not a stream or array".to_string(),
        )),
    }
}

/// Decode an array of content stream references, concatenating their bytes.
fn decode_contents_array(
    doc: &lopdf::Document,
    arr: &[lopdf::Object],
) -> Result<Vec<u8>, BackendError> {
    let mut content = Vec::new();
    for item in arr {
        let id = item.as_reference().map_err(|e| {
            BackendError::Parse(format!("/Contents array item is not a reference: {e}"))
        })?;
        let obj = doc
            .get_object(id)
            .map_err(|e| BackendError::Parse(format!("failed to resolve /Contents stream: {e}")))?;
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
///
/// Resolves indirect references, decodes UTF-16 BE (BOM `0xFE 0xFF`), falls back
/// to UTF-8, then Latin-1 for non-Unicode encoded strings.
pub(super) fn extract_string_from_dict(
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
        lopdf::Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        _ => None,
    }
}

/// Decode a PDF string, handling UTF-16 BE BOM and falling back to Latin-1.
pub(super) fn decode_pdf_string(bytes: &[u8]) -> String {
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
        String::from_utf16_lossy(&chars)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Resolve a potentially indirect object reference.
///
/// If `obj` is a [`lopdf::Object::Reference`], follows the reference in `doc`
/// and returns the resolved object. Otherwise returns `obj` unchanged.
pub(super) fn resolve_ref<'a>(
    doc: &'a lopdf::Document,
    obj: &'a lopdf::Object,
) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

#[cfg(test)]
mod tests;
