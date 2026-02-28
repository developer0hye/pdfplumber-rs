//! lopdf-based PDF parsing backend.
//!
//! Implements [`PdfBackend`] using the [lopdf](https://crates.io/crates/lopdf)
//! crate for PDF document parsing. This is the default backend for pdfplumber-rs.

use crate::backend::PdfBackend;
use crate::error::BackendError;
use crate::handler::ContentHandler;
use pdfplumber_core::{
    Annotation, AnnotationType, BBox, Bookmark, DocumentMetadata, ExtractOptions, FieldType,
    FormField, Hyperlink, ImageContent, RepairOptions, RepairResult, SignatureInfo, StructElement,
    ValidationIssue,
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

        // Reject encrypted PDFs when no password is provided
        if inner.is_encrypted() {
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
            inner.decrypt(password).map_err(|e| {
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
            .and_then(|o| o.as_name_str().ok())
            .unwrap_or("");
        if subtype != "Image" {
            return Err(BackendError::Parse(format!(
                "XObject /{image_name} is not an Image (subtype: {subtype})"
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
                if let Ok(name) = o.as_name_str() {
                    Some(vec![name.to_string()])
                } else if let Ok(arr) = o.as_array() {
                    Some(
                        arr.iter()
                            .filter_map(|item| {
                                let resolved = resolve_ref(inner, item);
                                resolved.as_name_str().ok().map(|s| s.to_string())
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
            ImageFormat::Raw | ImageFormat::Png => {
                // Decompress if filters present, otherwise return raw
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
fn validate_document(doc: &LopdfDocument) -> Result<Vec<ValidationIssue>, BackendError> {
    use pdfplumber_core::{Severity, ValidationIssue};

    let inner = &doc.inner;
    let mut issues = Vec::new();

    // 1. Check catalog for required /Type key
    let catalog_location = get_catalog_location(inner);
    let catalog_dict = get_catalog_dict(inner);

    if let Some(dict) = catalog_dict {
        match dict.get(b"Type") {
            Ok(type_obj) => {
                if let Ok(name) = type_obj.as_name_str() {
                    if name != "Catalog" {
                        issues.push(ValidationIssue::with_location(
                            Severity::Warning,
                            "WRONG_CATALOG_TYPE",
                            format!("catalog /Type is '{name}' instead of 'Catalog'"),
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
                            if let Ok(name) = type_obj.as_name_str() {
                                if name != "Page" {
                                    issues.push(ValidationIssue::with_location(
                                        Severity::Warning,
                                        "WRONG_PAGE_TYPE",
                                        format!("page /Type is '{name}' instead of 'Page'"),
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

    match contents_obj {
        lopdf::Object::Reference(id) => {
            let obj = doc.get_object(*id).ok()?;
            if let Ok(stream) = obj.as_stream() {
                stream_bytes(stream)
            } else {
                None
            }
        }
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
/// Otherwise, returns the object as-is.
fn resolve_ref<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Attempt best-effort repair of common PDF issues.
fn repair_document(
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

/// Extract form fields from the document catalog's /AcroForm dictionary.
///
/// Walks the `/Fields` array recursively (handling `/Kids` for hierarchical
/// fields) and extracts field name, type, value, default value, options,
/// rect, and flags for each terminal field.
fn extract_document_form_fields(doc: &lopdf::Document) -> Result<Vec<FormField>, BackendError> {
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

/// Decode a PDF string, handling UTF-16 BE BOM and Latin-1.
fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16 BE
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
fn extract_document_signatures(doc: &lopdf::Document) -> Result<Vec<SignatureInfo>, BackendError> {
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

/// Extract the document structure tree from `/StructTreeRoot`.
///
/// Walks the structure tree recursively, extracting element types, MCIDs,
/// alt text, actual text, language, and child elements. Returns an empty
/// Vec for untagged PDFs (no `/StructTreeRoot`).
fn extract_document_structure_tree(
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

/// Create a PDF with a JPEG (DCTDecode) image XObject.
#[cfg(test)]
fn create_test_pdf_with_jpeg_image() -> Vec<u8> {
    use lopdf::{Document, Object, ObjectId, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id: ObjectId = doc.new_object_id();

    // Minimal JPEG data (SOI + APP0 + EOI markers)
    // A real JPEG starts with FF D8 and ends with FF D9
    let jpeg_data = vec![
        0xFF, 0xD8, 0xFF, 0xE0, // SOI + APP0 marker
        0x00, 0x10, // Length of APP0
        0x4A, 0x46, 0x49, 0x46, 0x00, // "JFIF\0"
        0x01, 0x01, // Version
        0x00, // Units
        0x00, 0x01, 0x00, 0x01, // X/Y density
        0x00, 0x00, // No thumbnail
        0xFF, 0xD9, // EOI marker
    ];

    let image_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 2i64,
            "Height" => 2i64,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8i64,
            "Filter" => "DCTDecode",
        },
        jpeg_data,
    );
    let image_id = doc.add_object(Object::Stream(image_stream));

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

    // --- extract_image_content() tests ---

    #[test]
    fn extract_image_content_raw_data() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let content = LopdfBackend::extract_image_content(&doc, &page, "Im0").unwrap();

        assert_eq!(content.format, pdfplumber_core::ImageFormat::Raw);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
        // 2x2 RGB image = 12 bytes
        assert_eq!(content.data.len(), 12);
        assert_eq!(
            content.data,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]
        );
    }

    #[test]
    fn extract_image_content_not_found() {
        let pdf_bytes = create_test_pdf_with_image_xobject();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let result = LopdfBackend::extract_image_content(&doc, &page, "NonExistent");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn extract_image_content_jpeg() {
        // Create a PDF with a JPEG (DCTDecode) image
        let pdf_bytes = create_test_pdf_with_jpeg_image();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let content = LopdfBackend::extract_image_content(&doc, &page, "Im0").unwrap();

        assert_eq!(content.format, pdfplumber_core::ImageFormat::Jpeg);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
        // JPEG data should be returned as-is
        assert!(content.data.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn extract_image_content_no_xobject_resources() {
        // A page without XObject resources
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let page = LopdfBackend::get_page(&doc, 0).unwrap();

        let result = LopdfBackend::extract_image_content(&doc, &page, "Im0");
        assert!(result.is_err());
    }

    // --- Encrypted PDF test helpers ---

    /// PDF standard padding bytes used in encryption key derivation.
    const PAD_BYTES: [u8; 32] = [
        0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
        0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
        0x69, 0x7A,
    ];

    /// Simple RC4 implementation for test encryption.
    fn rc4_transform(key: &[u8], data: &[u8]) -> Vec<u8> {
        // RC4 KSA
        let mut s: Vec<u8> = (0..=255).collect();
        let mut j: usize = 0;
        for i in 0..256 {
            j = (j + s[i] as usize + key[i % key.len()] as usize) & 0xFF;
            s.swap(i, j);
        }
        // RC4 PRGA
        let mut out = Vec::with_capacity(data.len());
        let mut i: usize = 0;
        j = 0;
        for &byte in data {
            i = (i + 1) & 0xFF;
            j = (j + s[i] as usize) & 0xFF;
            s.swap(i, j);
            let k = s[(s[i] as usize + s[j] as usize) & 0xFF];
            out.push(byte ^ k);
        }
        out
    }

    /// Create an encrypted PDF with the given user password (RC4, 40-bit, V=1, R=2).
    fn create_encrypted_test_pdf(user_password: &[u8]) -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, StringFormat, dictionary};

        let file_id = b"testfileid123456"; // 16 bytes
        let permissions: i32 = -4; // all permissions

        // Pad password to 32 bytes
        let mut padded_pw = Vec::with_capacity(32);
        let pw_len = user_password.len().min(32);
        padded_pw.extend_from_slice(&user_password[..pw_len]);
        padded_pw.extend_from_slice(&PAD_BYTES[..32 - pw_len]);

        // Algorithm 3.3: Compute /O value (owner password hash)
        // Using same password for owner and user (simplification for tests)
        let o_key_digest = md5::compute(&padded_pw);
        let o_key = &o_key_digest[..5]; // 40-bit key = 5 bytes
        let o_value = rc4_transform(o_key, &padded_pw);

        // Algorithm 3.2: Compute encryption key
        let mut key_input = Vec::with_capacity(128);
        key_input.extend_from_slice(&padded_pw);
        key_input.extend_from_slice(&o_value);
        key_input.extend_from_slice(&(permissions as u32).to_le_bytes());
        key_input.extend_from_slice(file_id);
        let key_digest = md5::compute(&key_input);
        let enc_key = key_digest[..5].to_vec(); // 40-bit key

        // Algorithm 3.4: Compute /U value (R=2)
        let u_value = rc4_transform(&enc_key, &PAD_BYTES);

        // Build the PDF document
        let mut doc = Document::with_version("1.5");
        let pages_id: ObjectId = doc.new_object_id();

        // Create page with text content (will be encrypted)
        let content_bytes = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
        let stream = Stream::new(dictionary! {}, content_bytes.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => 1_i64,
            }),
        );

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        // Now encrypt all string/stream objects
        for (&obj_id, obj) in doc.objects.iter_mut() {
            // Compute per-object key: MD5(enc_key + obj_num_le + gen_num_le)[:key_len+5]
            let mut obj_key_input = Vec::with_capacity(10);
            obj_key_input.extend_from_slice(&enc_key);
            obj_key_input.extend_from_slice(&obj_id.0.to_le_bytes()[..3]);
            obj_key_input.extend_from_slice(&obj_id.1.to_le_bytes()[..2]);
            let obj_key_digest = md5::compute(&obj_key_input);
            let obj_key_len = (enc_key.len() + 5).min(16);
            let obj_key = &obj_key_digest[..obj_key_len];

            match obj {
                Object::Stream(stream) => {
                    let encrypted = rc4_transform(obj_key, &stream.content);
                    stream.set_content(encrypted);
                }
                Object::String(content, _) => {
                    let encrypted = rc4_transform(obj_key, content);
                    *content = encrypted;
                }
                _ => {}
            }
        }

        // Add /Encrypt dictionary
        let encrypt_id = doc.add_object(dictionary! {
            "Filter" => "Standard",
            "V" => 1_i64,
            "R" => 2_i64,
            "Length" => 40_i64,
            "O" => Object::String(o_value, StringFormat::Literal),
            "U" => Object::String(u_value, StringFormat::Literal),
            "P" => permissions as i64,
        });
        doc.trailer.set("Encrypt", Object::Reference(encrypt_id));

        // Add /ID array
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::String(file_id.to_vec(), StringFormat::Literal),
                Object::String(file_id.to_vec(), StringFormat::Literal),
            ]),
        );

        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .expect("failed to save encrypted test PDF");
        buf
    }

    // --- Encrypted PDF tests ---

    #[test]
    fn open_encrypted_pdf_without_password_returns_password_required() {
        let pdf_bytes = create_encrypted_test_pdf(b"secret123");
        let result = LopdfBackend::open(&pdf_bytes);
        assert!(result.is_err());
        let err: pdfplumber_core::PdfError = result.unwrap_err().into();
        assert_eq!(err, pdfplumber_core::PdfError::PasswordRequired);
    }

    #[test]
    fn open_encrypted_pdf_with_correct_password() {
        let password = b"secret123";
        let pdf_bytes = create_encrypted_test_pdf(password);
        let result = LopdfBackend::open_with_password(&pdf_bytes, password);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_encrypted_pdf_with_wrong_password_returns_invalid_password() {
        let pdf_bytes = create_encrypted_test_pdf(b"secret123");
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"wrongpassword");
        assert!(result.is_err());
        let err: pdfplumber_core::PdfError = result.unwrap_err().into();
        assert_eq!(err, pdfplumber_core::PdfError::InvalidPassword);
    }

    #[test]
    fn open_unencrypted_pdf_with_password_succeeds() {
        // Password is ignored for unencrypted PDFs
        let pdf_bytes = create_test_pdf(1);
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"anypassword");
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    #[test]
    fn open_encrypted_pdf_with_empty_password() {
        // Encrypted with empty password — should be openable with empty password
        let pdf_bytes = create_encrypted_test_pdf(b"");
        let result = LopdfBackend::open_with_password(&pdf_bytes, b"");
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(LopdfBackend::page_count(&doc), 1);
    }

    // --- Form field extraction tests ---

    /// Create a PDF with form fields for testing AcroForm extraction.
    fn create_test_pdf_with_form_fields() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        // Create a page
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Text field
        let text_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("name"),
            "FT" => "Tx",
            "V" => Object::string_literal("John Doe"),
            "DV" => Object::string_literal(""),
            "Rect" => vec![50.into(), 700.into(), 200.into(), 720.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Checkbox field (Button)
        let checkbox_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("agree"),
            "FT" => "Btn",
            "V" => "Yes",
            "DV" => "Off",
            "Rect" => vec![50.into(), 650.into(), 70.into(), 670.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Radio button field (Button with flags)
        let radio_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("gender"),
            "FT" => "Btn",
            "V" => "Male",
            "Rect" => vec![50.into(), 600.into(), 70.into(), 620.into()],
            "Ff" => Object::Integer(49152), // Radio flag (bit 15) + NoToggleToOff (bit 14)
            "P" => Object::Reference(page_id),
        });

        // Dropdown field (Choice)
        let dropdown_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("country"),
            "FT" => "Ch",
            "V" => Object::string_literal("US"),
            "Rect" => vec![50.into(), 550.into(), 200.into(), 570.into()],
            "Opt" => vec![
                Object::string_literal("US"),
                Object::string_literal("UK"),
                Object::string_literal("FR"),
            ],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // Field with no value
        let empty_field_id = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "T" => Object::string_literal("email"),
            "FT" => "Tx",
            "Rect" => vec![50.into(), 500.into(), 200.into(), 520.into()],
            "Ff" => Object::Integer(0),
            "P" => Object::Reference(page_id),
        });

        // AcroForm dictionary
        let acroform_id = doc.add_object(dictionary! {
            "Fields" => vec![
                Object::Reference(text_field_id),
                Object::Reference(checkbox_field_id),
                Object::Reference(radio_field_id),
                Object::Reference(dropdown_field_id),
                Object::Reference(empty_field_id),
            ],
        });

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => Object::Reference(acroform_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");
        buf
    }

    #[test]
    fn form_fields_text_field() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let text_field = fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(text_field.field_type, FieldType::Text);
        assert_eq!(text_field.value.as_deref(), Some("John Doe"));
        assert_eq!(text_field.default_value.as_deref(), Some(""));
    }

    #[test]
    fn form_fields_checkbox() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let checkbox = fields.iter().find(|f| f.name == "agree").unwrap();
        assert_eq!(checkbox.field_type, FieldType::Button);
        assert_eq!(checkbox.value.as_deref(), Some("Yes"));
        assert_eq!(checkbox.default_value.as_deref(), Some("Off"));
    }

    #[test]
    fn form_fields_radio_button() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let radio = fields.iter().find(|f| f.name == "gender").unwrap();
        assert_eq!(radio.field_type, FieldType::Button);
        assert_eq!(radio.value.as_deref(), Some("Male"));
        assert_eq!(radio.flags, 49152); // Radio flags
    }

    #[test]
    fn form_fields_dropdown_with_options() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let dropdown = fields.iter().find(|f| f.name == "country").unwrap();
        assert_eq!(dropdown.field_type, FieldType::Choice);
        assert_eq!(dropdown.value.as_deref(), Some("US"));
        assert_eq!(dropdown.options, vec!["US", "UK", "FR"]);
    }

    #[test]
    fn form_fields_no_value() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let empty = fields.iter().find(|f| f.name == "email").unwrap();
        assert_eq!(empty.field_type, FieldType::Text);
        assert!(empty.value.is_none());
        assert!(empty.default_value.is_none());
    }

    #[test]
    fn form_fields_count() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();
        assert_eq!(fields.len(), 5);
    }

    #[test]
    fn form_fields_no_acroform_returns_empty() {
        let pdf_bytes = create_test_pdf(1);
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();
        assert!(fields.is_empty());
    }

    #[test]
    fn form_fields_have_bbox() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        let text_field = fields.iter().find(|f| f.name == "name").unwrap();
        assert!((text_field.bbox.x0 - 50.0).abs() < 0.1);
        assert!((text_field.bbox.x1 - 200.0).abs() < 0.1);
    }

    #[test]
    fn form_fields_have_page_index() {
        let pdf_bytes = create_test_pdf_with_form_fields();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let fields = LopdfBackend::document_form_fields(&doc).unwrap();

        // All fields reference page 0
        for field in &fields {
            assert_eq!(field.page_index, Some(0));
        }
    }

    // --- Structure tree tests (US-081) ---

    /// Create a test PDF with a structure tree (tagged PDF).
    ///
    /// Structure: Document -> H1 (MCID 0) -> P (MCID 1)
    fn create_test_pdf_with_structure_tree() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        // Content stream with marked content
        let content = b"BT /F1 24 Tf /H1 <</MCID 0>> BDC 72 700 Td (Chapter 1) Tj EMC /P <</MCID 1>> BDC /F1 12 Tf 72 670 Td (This is paragraph text.) Tj EMC ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Structure tree elements
        // H1 element with MCID 0
        let h1_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "H1",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
        });

        // P element with MCID 1
        let p_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "P",
            "K" => Object::Integer(1),
            "Pg" => Object::Reference(page_id),
            "Lang" => Object::string_literal("en-US"),
        });

        // Document root element
        let doc_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Document",
            "K" => vec![
                Object::Reference(h1_elem_id),
                Object::Reference(p_elem_id),
            ],
        });

        // StructTreeRoot
        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(doc_elem_id),
        });

        // Mark document as tagged
        let mark_info_id = doc.add_object(dictionary! {
            "Marked" => Object::Boolean(true),
        });

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
            "MarkInfo" => Object::Reference(mark_info_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .expect("failed to save tagged test PDF");
        buf
    }

    /// Create a test PDF with a structure tree containing a table.
    fn create_test_pdf_with_table_structure() -> Vec<u8> {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (Cell 1) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Table structure: Table -> TR -> TD (MCID 0), TD (MCID 1)
        let td1_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TD",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
        });

        let td2_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TD",
            "K" => Object::Integer(1),
            "Pg" => Object::Reference(page_id),
        });

        let tr_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "TR",
            "K" => vec![Object::Reference(td1_id), Object::Reference(td2_id)],
        });

        let table_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Table",
            "K" => Object::Reference(tr_id),
            "Pg" => Object::Reference(page_id),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(table_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");
        buf
    }

    #[test]
    fn structure_tree_tagged_pdf_has_elements() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert!(!elements.is_empty());
    }

    #[test]
    fn structure_tree_document_root_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        // Root should be "Document" element
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].element_type, "Document");
        assert_eq!(elements[0].children.len(), 2);
    }

    #[test]
    fn structure_tree_heading_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        let doc_elem = &elements[0];
        let h1 = &doc_elem.children[0];
        assert_eq!(h1.element_type, "H1");
        assert_eq!(h1.mcids, vec![0]);
        assert_eq!(h1.page_index, Some(0));
    }

    #[test]
    fn structure_tree_paragraph_element() {
        let pdf_bytes = create_test_pdf_with_structure_tree();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        let doc_elem = &elements[0];
        let p = &doc_elem.children[1];
        assert_eq!(p.element_type, "P");
        assert_eq!(p.mcids, vec![1]);
        assert_eq!(p.page_index, Some(0));
        assert_eq!(p.lang.as_deref(), Some("en-US"));
    }

    #[test]
    fn structure_tree_untagged_pdf_returns_empty() {
        // Use the basic test PDF helper (no structure tree)
        let pdf_bytes = create_test_pdf_with_text_content();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert!(elements.is_empty());
    }

    #[test]
    fn structure_tree_table_nested_structure() {
        let pdf_bytes = create_test_pdf_with_table_structure();
        let doc = LopdfBackend::open(&pdf_bytes).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        // Root is Table element
        assert_eq!(elements.len(), 1);
        let table = &elements[0];
        assert_eq!(table.element_type, "Table");

        // Table -> TR
        assert_eq!(table.children.len(), 1);
        let tr = &table.children[0];
        assert_eq!(tr.element_type, "TR");

        // TR -> TD, TD
        assert_eq!(tr.children.len(), 2);
        assert_eq!(tr.children[0].element_type, "TD");
        assert_eq!(tr.children[0].mcids, vec![0]);
        assert_eq!(tr.children[1].element_type, "TD");
        assert_eq!(tr.children[1].mcids, vec![1]);
    }

    #[test]
    fn structure_tree_mcr_dictionary_handling() {
        // Test with MCR (marked content reference) dictionaries instead of integer MCIDs
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (text) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Structure element with MCR dictionary in /K
        let p_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "P",
            "K" => dictionary! {
                "Type" => "MCR",
                "MCID" => Object::Integer(5),
                "Pg" => Object::Reference(page_id),
            },
            "Pg" => Object::Reference(page_id),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(p_elem_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");

        let doc = LopdfBackend::open(&buf).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert_eq!(elements.len(), 1);
        let p = &elements[0];
        assert_eq!(p.element_type, "P");
        assert_eq!(p.mcids, vec![5]); // MCID from MCR dictionary
    }

    #[test]
    fn structure_tree_alt_text() {
        use lopdf::{Document, Object, ObjectId, Stream, dictionary};

        let mut doc = Document::with_version("1.7");
        let pages_id: ObjectId = doc.new_object_id();

        let content = b"BT /F1 12 Tf 72 700 Td (image) Tj ET";
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(Object::Stream(stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
            },
        });

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );

        // Figure element with /Alt and /ActualText
        let fig_elem_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Figure",
            "K" => Object::Integer(0),
            "Pg" => Object::Reference(page_id),
            "Alt" => Object::string_literal("A photo of a sunset"),
            "ActualText" => Object::string_literal("Sunset photo"),
        });

        let struct_tree_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => Object::Reference(fig_elem_id),
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "StructTreeRoot" => Object::Reference(struct_tree_id),
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save test PDF");

        let doc = LopdfBackend::open(&buf).unwrap();
        let elements = LopdfBackend::document_structure_tree(&doc).unwrap();

        assert_eq!(elements.len(), 1);
        let fig = &elements[0];
        assert_eq!(fig.element_type, "Figure");
        assert_eq!(fig.alt_text.as_deref(), Some("A photo of a sunset"));
        assert_eq!(fig.actual_text.as_deref(), Some("Sunset photo"));
    }
}
