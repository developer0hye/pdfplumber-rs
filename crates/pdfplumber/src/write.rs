//! PDF writing and incremental update API.
//!
//! This module is only compiled when the `write` feature is enabled.
//!
//! # Overview
//!
//! PDF incremental updates (PDF spec §7.5.6) allow adding new content to a PDF
//! by *appending* a new cross-reference table and updated/new objects to the end
//! of the original file. The original bytes are untouched — forensically clean,
//! and compatible with existing digital signatures.
//!
//! This module provides:
//!
//! - [`PdfWriter`] — collects mutations and serialises them as an incremental update
//! - Annotation constructors: [`HighlightAnnotation`], [`TextAnnotation`], [`LinkAnnotation`]
//! - `Pdf::write_incremental_bytes(&self, mutations) -> Result<Vec<u8>>`
//! - `Pdf::write_bytes(&self) -> Result<Vec<u8>>` — full rewrite (for complex changes)
//!
//! # Incremental update anatomy
//!
//! ```text
//! <original PDF bytes>
//! <new/modified objects in standard format>
//! xref
//! <new xref section pointing at new objects>
//! trailer
//! << /Size N /Prev <offset_of_original_xref> /Root <catalog_ref> >>
//! startxref
//! <offset_of_new_xref>
//! %%EOF
//! ```
//!
//! # Example
//!
//! ```no_run
//! use pdfplumber::{Pdf, BBox};
//! use pdfplumber::write::{PdfWriter, HighlightAnnotation, AnnotationColor};
//!
//! let file_bytes = std::fs::read("document.pdf").unwrap();
//! let pdf = Pdf::open(file_bytes.clone().into(), None).unwrap();
//!
//! let mut writer = PdfWriter::new(&pdf, &file_bytes);
//! writer.add_highlight(0, BBox { x0: 72.0, y0: 700.0, x1: 300.0, y1: 720.0 }, AnnotationColor::Yellow)?;
//! writer.add_text_annotation(0, BBox { x0: 72.0, y0: 650.0, x1: 200.0, y1: 670.0 }, "See also §4.2")?;
//!
//! let updated = writer.write_incremental()?;
//! std::fs::write("document_annotated.pdf", &updated).unwrap();
//! # Ok::<(), pdfplumber::PdfError>(())
//! ```

use std::collections::HashMap;
use std::io::Write;

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pdfplumber_core::{BBox, PdfError};

// ── public types ──────────────────────────────────────────────────────────────

/// Annotation highlight color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationColor {
    /// Standard yellow highlight
    Yellow,
    /// Cyan/blue highlight
    Cyan,
    /// Green highlight
    Green,
    /// Pink/red highlight
    Pink,
    /// Custom RGB (0.0–1.0 each)
    Custom(f32, f32, f32),
}

impl AnnotationColor {
    fn to_rgb(self) -> [f32; 3] {
        match self {
            Self::Yellow => [1.0, 1.0, 0.0],
            Self::Cyan => [0.0, 1.0, 1.0],
            Self::Green => [0.0, 1.0, 0.0],
            Self::Pink => [1.0, 0.41, 0.71],
            Self::Custom(r, g, b) => [r, g, b],
        }
    }
}

/// A highlight annotation to add to a page.
#[derive(Debug, Clone)]
pub struct HighlightAnnotation {
    /// 0-based page index.
    pub page: usize,
    /// Bounding box in PDF user space (origin = bottom-left).
    pub bbox: BBox,
    /// Highlight color.
    pub color: AnnotationColor,
    /// Optional comment text.
    pub contents: Option<String>,
    /// Optional author name.
    pub author: Option<String>,
}

/// A text (sticky note) annotation.
#[derive(Debug, Clone)]
pub struct TextAnnotation {
    /// 0-based page index.
    pub page: usize,
    /// Bounding box for the note icon.
    pub bbox: BBox,
    /// Note text.
    pub contents: String,
    /// Optional author.
    pub author: Option<String>,
    /// Whether the note appears open (expanded) by default.
    pub open: bool,
}

/// A link annotation (URI or internal destination).
#[derive(Debug, Clone)]
pub struct LinkAnnotation {
    /// 0-based page index.
    pub page: usize,
    /// Clickable rectangle.
    pub bbox: BBox,
    /// Target URI.
    pub uri: String,
}

/// PDF metadata field update.
#[derive(Debug, Clone)]
pub struct MetadataUpdate {
    /// `/Info` dictionary key (e.g. `"Title"`, `"Author"`, `"Keywords"`).
    pub key: String,
    /// New value.
    pub value: String,
}

/// Collects mutations to apply to a PDF as an incremental update.
///
/// Build up mutations then call [`PdfWriter::write_incremental`] to produce
/// the updated bytes.
pub struct PdfWriter<'a> {
    pdf: &'a crate::Pdf,
    original_bytes: &'a [u8],
    highlights: Vec<HighlightAnnotation>,
    text_annotations: Vec<TextAnnotation>,
    link_annotations: Vec<LinkAnnotation>,
    metadata_updates: Vec<MetadataUpdate>,
}

impl<'a> PdfWriter<'a> {
    /// Create a new writer for `pdf` backed by `original_bytes`.
    ///
    /// `original_bytes` must be the exact bytes that were passed to
    /// [`Pdf::open`] or read from the file passed to [`Pdf::open_file`].
    /// They are used to construct the incremental update.
    pub fn new(pdf: &'a crate::Pdf, original_bytes: &'a [u8]) -> Self {
        Self {
            pdf,
            original_bytes,
            highlights: Vec::new(),
            text_annotations: Vec::new(),
            link_annotations: Vec::new(),
            metadata_updates: Vec::new(),
        }
    }

    /// Add a highlight annotation to `page` (0-based).
    pub fn add_highlight(
        &mut self,
        page: usize,
        bbox: BBox,
        color: AnnotationColor,
    ) -> Result<&mut Self, PdfError> {
        self.highlights.push(HighlightAnnotation {
            page,
            bbox,
            color,
            contents: None,
            author: None,
        });
        Ok(self)
    }

    /// Add a highlight with optional comment text and author.
    pub fn add_highlight_with_comment(
        &mut self,
        page: usize,
        bbox: BBox,
        color: AnnotationColor,
        contents: impl Into<String>,
        author: impl Into<String>,
    ) -> Result<&mut Self, PdfError> {
        self.highlights.push(HighlightAnnotation {
            page,
            bbox,
            color,
            contents: Some(contents.into()),
            author: Some(author.into()),
        });
        Ok(self)
    }

    /// Add a text (sticky note) annotation.
    pub fn add_text_annotation(
        &mut self,
        page: usize,
        bbox: BBox,
        contents: impl Into<String>,
    ) -> Result<&mut Self, PdfError> {
        self.text_annotations.push(TextAnnotation {
            page,
            bbox,
            contents: contents.into(),
            author: None,
            open: false,
        });
        Ok(self)
    }

    /// Add a URI link annotation.
    pub fn add_link_annotation(
        &mut self,
        page: usize,
        bbox: BBox,
        uri: impl Into<String>,
    ) -> Result<&mut Self, PdfError> {
        self.link_annotations.push(LinkAnnotation {
            page,
            bbox,
            uri: uri.into(),
        });
        Ok(self)
    }

    /// Update a PDF metadata field (written into the `/Info` dictionary).
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.metadata_updates.push(MetadataUpdate {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    /// Produce an incremental-update PDF byte string.
    ///
    /// The returned bytes are: original bytes + appended update section.
    /// Write them to a file to get the annotated PDF.
    pub fn write_incremental(&self) -> Result<Vec<u8>, PdfError> {
        build_incremental_update(
            self.original_bytes,
            &self.highlights,
            &self.text_annotations,
            &self.link_annotations,
            &self.metadata_updates,
        )
    }

    /// Produce a fully-rewritten PDF byte string using lopdf's built-in save.
    ///
    /// Use this when structural changes are needed (e.g. removing pages).
    /// For annotation-only changes, prefer [`write_incremental`] — it
    /// preserves existing digital signatures.
    pub fn write_full_rewrite(&self) -> Result<Vec<u8>, PdfError> {
        let doc = parse_lopdf(self.original_bytes)?;
        let mut buf = Vec::new();
        doc.save_to(&mut buf)
            .map_err(|e| PdfError::write(format!("lopdf save failed: {e}")))?;
        Ok(buf)
    }
}

// ── incremental update construction ──────────────────────────────────────────

fn build_incremental_update(
    original: &[u8],
    highlights: &[HighlightAnnotation],
    texts: &[TextAnnotation],
    links: &[LinkAnnotation],
    metadata: &[MetadataUpdate],
) -> Result<Vec<u8>, PdfError> {
    // Parse the original to get the object structure (for page refs, etc.)
    let mut doc = parse_lopdf(original)?;

    // Determine the next available object number
    let mut next_id = doc.max_id + 1;

    // Map: page_index → Vec<ObjectId> of new annotation objects
    let mut page_annot_map: HashMap<usize, Vec<ObjectId>> = HashMap::new();

    // Collect all new objects to write
    let mut new_objects: Vec<(ObjectId, Object)> = Vec::new();

    // Build highlight annotation objects
    for hl in highlights {
        let id = (next_id, 0u16);
        next_id += 1;
        let obj = build_highlight_object(id, hl);
        new_objects.push((id, obj));
        page_annot_map.entry(hl.page).or_default().push(id);
    }

    // Build text annotation objects
    for ta in texts {
        let id = (next_id, 0u16);
        next_id += 1;
        let obj = build_text_annotation_object(id, ta);
        new_objects.push((id, obj));
        page_annot_map.entry(ta.page).or_default().push(id);
    }

    // Build link annotation objects
    for la in links {
        let id = (next_id, 0u16);
        next_id += 1;
        let obj = build_link_annotation_object(id, la);
        new_objects.push((id, obj));
        page_annot_map.entry(la.page).or_default().push(id);
    }

    // For each modified page, produce an updated page object with the new
    // annotation references appended to /Annots.
    let page_ids = collect_page_ids(&doc);
    let mut modified_page_objects: Vec<(ObjectId, Object)> = Vec::new();

    for (page_idx, new_annot_ids) in &page_annot_map {
        let Some(&page_id) = page_ids.get(*page_idx) else {
            return Err(PdfError::write(format!(
                "page index {page_idx} out of range (doc has {} pages)",
                page_ids.len()
            )));
        };
        let page_obj = doc
            .get_object(page_id)
            .map_err(|e| PdfError::write(format!("get page object: {e}")))?;

        let mut page_dict = page_obj
            .as_dict()
            .map_err(|e| PdfError::write(format!("page is not a dict: {e}")))?
            .clone();

        // Get existing /Annots (may be absent, array, or indirect ref)
        let mut existing_annots: Vec<Object> = get_existing_annots(&doc, &page_dict);

        // Append new annotation refs
        for &annot_id in new_annot_ids {
            existing_annots.push(Object::Reference(annot_id));
        }

        page_dict.set("Annots", Object::Array(existing_annots));
        modified_page_objects.push((page_id, Object::Dictionary(page_dict)));
    }

    // Apply metadata updates to /Info
    let mut modified_info: Option<(ObjectId, Object)> = None;
    if !metadata.is_empty() {
        if let Some(info_id) = get_info_id(&doc) {
            if let Ok(info_obj) = doc.get_object(info_id) {
                if let Ok(mut info_dict) = info_obj.as_dict().cloned() {
                    for update in metadata {
                        info_dict.set(
                            update.key.as_bytes(),
                            Object::String(
                                update.value.as_bytes().to_vec(),
                                lopdf::StringFormat::Literal,
                            ),
                        );
                    }
                    modified_info = Some((info_id, Object::Dictionary(info_dict)));
                }
            }
        }
    }

    // Gather all objects to write in the incremental section
    let all_new: Vec<(ObjectId, Object)> = new_objects
        .into_iter()
        .chain(modified_page_objects)
        .chain(modified_info)
        .collect();

    if all_new.is_empty() {
        // Nothing to do — return original unchanged
        return Ok(original.to_vec());
    }

    // Serialise the incremental update
    serialize_incremental_update(original, &all_new, &doc, next_id)
}

/// Serialise new/modified objects + a new xref + trailer as an append to
/// `original_bytes`.
fn serialize_incremental_update(
    original: &[u8],
    objects: &[(ObjectId, Object)],
    doc: &Document,
    next_id: u32,
) -> Result<Vec<u8>, PdfError> {
    let mut buf: Vec<u8> = original.to_vec();

    // Track byte offsets for the new xref
    let mut offsets: Vec<(ObjectId, u64)> = Vec::with_capacity(objects.len());

    // Write each new object
    for (id, obj) in objects {
        let offset = buf.len() as u64;
        offsets.push((*id, offset));

        // Object header: "N G obj\n"
        write!(buf, "{} {} obj\n", id.0, id.1).map_err(|e| PdfError::write(e.to_string()))?;

        write_object(&mut buf, obj).map_err(|e| PdfError::write(e.to_string()))?;

        write!(buf, "\nendobj\n").map_err(|e| PdfError::write(e.to_string()))?;
    }

    // Write the new xref section
    let xref_offset = buf.len() as u64;
    write!(buf, "xref\n").map_err(|e| PdfError::write(e.to_string()))?;

    // Sort by object number for deterministic output
    let mut sorted_offsets = offsets.clone();
    sorted_offsets.sort_by_key(|(id, _)| id.0);

    // Group into contiguous runs (required by PDF xref format)
    write_xref_sections(&mut buf, &sorted_offsets).map_err(|e| PdfError::write(e.to_string()))?;

    // Write trailer
    let original_xref_offset = find_startxref(original)
        .ok_or_else(|| PdfError::write("could not find startxref in original PDF"))?;

    let catalog_id =
        get_catalog_id(doc).ok_or_else(|| PdfError::write("could not find catalog object id"))?;

    write!(
        buf,
        "trailer\n<< /Size {} /Prev {} /Root {} {} R >>\nstartxref\n{}\n%%EOF\n",
        next_id, original_xref_offset, catalog_id.0, catalog_id.1, xref_offset,
    )
    .map_err(|e| PdfError::write(e.to_string()))?;

    Ok(buf)
}

fn write_xref_sections(
    buf: &mut Vec<u8>,
    sorted_offsets: &[(ObjectId, u64)],
) -> std::io::Result<()> {
    if sorted_offsets.is_empty() {
        return Ok(());
    }

    // Group into contiguous runs
    let mut runs: Vec<(u32, Vec<u64>)> = Vec::new();
    let mut current_start = sorted_offsets[0].0.0;
    let mut current_offsets = vec![sorted_offsets[0].1];

    for &(id, offset) in &sorted_offsets[1..] {
        if id.0 == current_start + current_offsets.len() as u32 {
            current_offsets.push(offset);
        } else {
            runs.push((current_start, std::mem::take(&mut current_offsets)));
            current_start = id.0;
            current_offsets.push(offset);
        }
    }
    runs.push((current_start, current_offsets));

    for (start, offsets) in runs {
        write!(buf, "{} {}\n", start, offsets.len())?;
        for off in offsets {
            // xref entry: 20 bytes: "OOOOOOOOOO GGGGG n \r\n"
            write!(buf, "{:010} {:05} n \r\n", off, 0u16)?;
        }
    }
    Ok(())
}

// ── lopdf object construction ─────────────────────────────────────────────────

fn build_highlight_object(_id: ObjectId, hl: &HighlightAnnotation) -> Object {
    let [r, g, b] = hl.color.to_rgb();
    // PDF coordinate system: (0,0) = bottom-left
    let rect = bbox_to_rect_array(&hl.bbox);

    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"Annot".to_vec()));
    dict.set("Subtype", Object::Name(b"Highlight".to_vec()));
    dict.set("Rect", rect.clone());
    // /QuadPoints: 8 values defining the highlight quadrilateral (matches Rect for simple case)
    dict.set(
        "QuadPoints",
        Object::Array(vec![
            Object::Real(hl.bbox.x0 as f32),
            Object::Real(hl.bbox.y0 as f32),
            Object::Real(hl.bbox.x1 as f32),
            Object::Real(hl.bbox.y0 as f32),
            Object::Real(hl.bbox.x0 as f32),
            Object::Real(hl.bbox.y1 as f32),
            Object::Real(hl.bbox.x1 as f32),
            Object::Real(hl.bbox.y1 as f32),
        ]),
    );
    dict.set(
        "C",
        Object::Array(vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
    );
    dict.set("F", Object::Integer(4)); // Print flag
    if let Some(ref contents) = hl.contents {
        dict.set(
            "Contents",
            Object::String(contents.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }
    if let Some(ref author) = hl.author {
        dict.set(
            "T",
            Object::String(author.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }
    Object::Dictionary(dict)
}

fn build_text_annotation_object(_id: ObjectId, ta: &TextAnnotation) -> Object {
    let rect = bbox_to_rect_array(&ta.bbox);
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"Annot".to_vec()));
    dict.set("Subtype", Object::Name(b"Text".to_vec()));
    dict.set("Rect", rect);
    dict.set(
        "Contents",
        Object::String(
            ta.contents.as_bytes().to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );
    dict.set("F", Object::Integer(4));
    dict.set("Open", Object::Boolean(ta.open));
    if let Some(ref author) = ta.author {
        dict.set(
            "T",
            Object::String(author.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }
    Object::Dictionary(dict)
}

fn build_link_annotation_object(_id: ObjectId, la: &LinkAnnotation) -> Object {
    let rect = bbox_to_rect_array(&la.bbox);
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"Annot".to_vec()));
    dict.set("Subtype", Object::Name(b"Link".to_vec()));
    dict.set("Rect", rect);
    dict.set("F", Object::Integer(4));

    // /A << /Type /Action /S /URI /URI (url) >>
    let mut action = Dictionary::new();
    action.set("Type", Object::Name(b"Action".to_vec()));
    action.set("S", Object::Name(b"URI".to_vec()));
    action.set(
        "URI",
        Object::String(la.uri.as_bytes().to_vec(), lopdf::StringFormat::Literal),
    );
    dict.set("A", Object::Dictionary(action));

    Object::Dictionary(dict)
}

fn bbox_to_rect_array(bbox: &BBox) -> Object {
    Object::Array(vec![
        Object::Real(bbox.x0 as f32),
        Object::Real(bbox.y0 as f32),
        Object::Real(bbox.x1 as f32),
        Object::Real(bbox.y1 as f32),
    ])
}

// ── lopdf document helpers ────────────────────────────────────────────────────

fn parse_lopdf(bytes: &[u8]) -> Result<Document, PdfError> {
    Document::load_mem(bytes).map_err(|e| PdfError::write(format!("lopdf parse error: {e}")))
}

fn collect_page_ids(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages().into_iter().map(|(_, id)| id).collect()
}

fn get_existing_annots(doc: &Document, page_dict: &Dictionary) -> Vec<Object> {
    let annots_obj = match page_dict.get(b"Annots") {
        Ok(obj) => obj,
        Err(_) => return Vec::new(),
    };
    // Resolve indirect ref if needed
    let annots_obj = match annots_obj {
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(obj) => obj,
            Err(_) => return Vec::new(),
        },
        other => other,
    };
    match annots_obj.as_array() {
        Ok(arr) => arr.clone(),
        Err(_) => Vec::new(),
    }
}

fn get_info_id(doc: &Document) -> Option<ObjectId> {
    match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    }
}

fn get_catalog_id(doc: &Document) -> Option<ObjectId> {
    match doc.trailer.get(b"Root") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    }
}

/// Find the byte offset of the final `startxref` value in a PDF.
fn find_startxref(bytes: &[u8]) -> Option<u64> {
    // Scan backwards for "startxref"
    let marker = b"startxref";
    let scan_window = bytes.len().min(1024);
    let start = bytes.len().saturating_sub(scan_window);
    let tail = &bytes[start..];

    let pos = tail.windows(marker.len()).rposition(|w| w == marker)?;
    let after = &tail[pos + marker.len()..];

    // Skip whitespace, read digits
    let trimmed = after
        .iter()
        .skip_while(|&&b| b == b'\r' || b == b'\n' || b == b' ')
        .copied();
    let digits: Vec<u8> = trimmed.take_while(|b| b.is_ascii_digit()).collect();
    let s = std::str::from_utf8(&digits).ok()?;
    s.parse().ok()
}

// ── object serialisation ──────────────────────────────────────────────────────

/// Write a lopdf `Object` to a byte buffer in standard PDF syntax.
fn write_object(buf: &mut Vec<u8>, obj: &Object) -> std::io::Result<()> {
    match obj {
        Object::Null => write!(buf, "null")?,
        Object::Boolean(b) => write!(buf, "{}", if *b { "true" } else { "false" })?,
        Object::Integer(i) => write!(buf, "{i}")?,
        Object::Real(f) => write!(buf, "{f}")?,
        Object::Name(n) => {
            buf.push(b'/');
            buf.extend_from_slice(n);
        }
        Object::String(s, lopdf::StringFormat::Literal) => {
            buf.push(b'(');
            // Escape special chars
            for &byte in s {
                match byte {
                    b'(' | b')' | b'\\' => {
                        buf.push(b'\\');
                        buf.push(byte);
                    }
                    b'\r' => buf.extend_from_slice(b"\\r"),
                    b'\n' => buf.extend_from_slice(b"\\n"),
                    _ => buf.push(byte),
                }
            }
            buf.push(b')');
        }
        Object::String(s, _) => {
            // Hex string
            write!(buf, "<")?;
            for byte in s {
                write!(buf, "{byte:02x}")?;
            }
            write!(buf, ">")?;
        }
        Object::Array(arr) => {
            buf.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(b' ');
                }
                write_object(buf, item)?;
            }
            buf.push(b']');
        }
        Object::Dictionary(dict) => {
            buf.extend_from_slice(b"<<\n");
            for (key, val) in dict.iter() {
                buf.push(b'/');
                buf.extend_from_slice(key);
                buf.push(b' ');
                write_object(buf, val)?;
                buf.push(b'\n');
            }
            buf.extend_from_slice(b">>");
        }
        Object::Stream(stream) => {
            write_object(buf, &Object::Dictionary(stream.dict.clone()))?;
            buf.extend_from_slice(b"\nstream\n");
            buf.extend_from_slice(&stream.content);
            buf.extend_from_slice(b"\nendstream");
        }
        Object::Reference(id) => {
            write!(buf, "{} {} R", id.0, id.1)?;
        }
    }
    Ok(())
}

trait Rposition {
    fn rposition<P: Fn(&Self::Item) -> bool>(&self, predicate: P) -> Option<usize>
    where
        Self: std::ops::Index<usize>;
    type Item;
}

impl<T> Rposition for [T] {
    type Item = T;
    fn rposition<P: Fn(&T) -> bool>(&self, predicate: P) -> Option<usize> {
        self.iter()
            .enumerate()
            .rev()
            .find_map(|(i, x)| if predicate(x) { Some(i) } else { None })
    }
}

// ── PdfError extension ────────────────────────────────────────────────────────

impl PdfError {
    fn write(msg: impl Into<String>) -> Self {
        PdfError::Other(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid PDF bytes for testing — a 1-page empty PDF
    fn minimal_pdf() -> Vec<u8> {
        // This is a tiny but valid PDF produced by iText/Acrobat-compatible tools.
        // Page is empty (no content stream), just structure.
        b"%PDF-1.4\n\
          1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
          2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
          3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
          xref\n\
          0 4\n\
          0000000000 65535 f \r\n\
          0000000009 00000 n \r\n\
          0000000058 00000 n \r\n\
          0000000115 00000 n \r\n\
          trailer\n<< /Size 4 /Root 1 0 R >>\n\
          startxref\n\
          190\n\
          %%EOF\n"
            .to_vec()
    }

    #[test]
    fn find_startxref_minimal() {
        let pdf = minimal_pdf();
        let offset = find_startxref(&pdf);
        assert!(offset.is_some(), "should find startxref");
        // The value 190 is in the minimal PDF
        assert_eq!(offset.unwrap(), 190);
    }

    #[test]
    fn find_startxref_not_present() {
        assert!(find_startxref(b"this is not a pdf").is_none());
    }

    #[test]
    fn annotation_color_rgb_values() {
        let [r, g, b] = AnnotationColor::Yellow.to_rgb();
        assert!((r - 1.0).abs() < f32::EPSILON);
        assert!((g - 1.0).abs() < f32::EPSILON);
        assert!((b - 0.0).abs() < f32::EPSILON);

        let [r, g, b] = AnnotationColor::Cyan.to_rgb();
        assert!((r - 0.0).abs() < f32::EPSILON);
        assert!((g - 1.0).abs() < f32::EPSILON);
        assert!((b - 1.0).abs() < f32::EPSILON);

        let [r, g, b] = AnnotationColor::Custom(0.5, 0.3, 0.8).to_rgb();
        assert!((r - 0.5).abs() < f32::EPSILON);
        assert!((g - 0.3).abs() < f32::EPSILON);
        assert!((b - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn bbox_to_rect_array_values() {
        let bbox = BBox {
            x0: 10.0,
            y0: 20.0,
            x1: 200.0,
            y1: 50.0,
        };
        let arr = bbox_to_rect_array(&bbox);
        if let Object::Array(items) = arr {
            assert_eq!(items.len(), 4);
        } else {
            panic!("expected Array");
        }
    }

    #[test]
    fn write_object_null() {
        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Null).unwrap();
        assert_eq!(buf, b"null");
    }

    #[test]
    fn write_object_boolean() {
        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Boolean(true)).unwrap();
        assert_eq!(buf, b"true");

        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Boolean(false)).unwrap();
        assert_eq!(buf, b"false");
    }

    #[test]
    fn write_object_integer() {
        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Integer(42)).unwrap();
        assert_eq!(buf, b"42");
    }

    #[test]
    fn write_object_name() {
        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Name(b"Annot".to_vec())).unwrap();
        assert_eq!(buf, b"/Annot");
    }

    #[test]
    fn write_object_literal_string_escaping() {
        let mut buf = Vec::new();
        let s = b"hello (world) \\test".to_vec();
        write_object(&mut buf, &Object::String(s, lopdf::StringFormat::Literal)).unwrap();
        assert_eq!(buf, b"(hello \\(world\\) \\\\test)");
    }

    #[test]
    fn write_object_reference() {
        let mut buf = Vec::new();
        write_object(&mut buf, &Object::Reference((5, 0))).unwrap();
        assert_eq!(buf, b"5 0 R");
    }

    #[test]
    fn write_object_array() {
        let mut buf = Vec::new();
        write_object(
            &mut buf,
            &Object::Array(vec![
                Object::Integer(1),
                Object::Integer(2),
                Object::Integer(3),
            ]),
        )
        .unwrap();
        assert_eq!(buf, b"[1 2 3]");
    }

    #[test]
    fn write_incremental_empty_mutations_returns_original() {
        let original = minimal_pdf();
        let pdf = crate::Pdf::open(original.clone().into(), None).unwrap();
        let writer = PdfWriter::new(&pdf, &original);
        let result = writer.write_incremental().unwrap();
        assert_eq!(
            result, original,
            "no mutations should return original unchanged"
        );
    }

    #[test]
    fn pdf_writer_add_highlight_chainable() {
        let original = minimal_pdf();
        let pdf = crate::Pdf::open(original.clone().into(), None).unwrap();
        let mut writer = PdfWriter::new(&pdf, &original);
        let result = writer.add_highlight(
            0,
            BBox {
                x0: 72.0,
                y0: 700.0,
                x1: 300.0,
                y1: 720.0,
            },
            AnnotationColor::Yellow,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn pdf_writer_add_text_annotation_chainable() {
        let original = minimal_pdf();
        let pdf = crate::Pdf::open(original.clone().into(), None).unwrap();
        let mut writer = PdfWriter::new(&pdf, &original);
        let result = writer.add_text_annotation(
            0,
            BBox {
                x0: 72.0,
                y0: 650.0,
                x1: 200.0,
                y1: 670.0,
            },
            "Test note",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn pdf_writer_add_link_annotation() {
        let original = minimal_pdf();
        let pdf = crate::Pdf::open(original.clone().into(), None).unwrap();
        let mut writer = PdfWriter::new(&pdf, &original);
        let result = writer.add_link_annotation(
            0,
            BBox {
                x0: 100.0,
                y0: 600.0,
                x1: 300.0,
                y1: 620.0,
            },
            "https://example.com",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn pdf_writer_set_metadata_chainable() {
        let original = minimal_pdf();
        let pdf = crate::Pdf::open(original.clone().into(), None).unwrap();
        let mut writer = PdfWriter::new(&pdf, &original);
        writer.set_metadata("Title", "Test Document");
        // No panic = success
    }

    #[test]
    fn build_highlight_object_has_required_keys() {
        let hl = HighlightAnnotation {
            page: 0,
            bbox: BBox {
                x0: 50.0,
                y0: 700.0,
                x1: 200.0,
                y1: 720.0,
            },
            color: AnnotationColor::Yellow,
            contents: Some("test".to_string()),
            author: Some("Tester".to_string()),
        };
        let id = (10u32, 0u16);
        let obj = build_highlight_object(id, &hl);
        if let Object::Dictionary(dict) = obj {
            assert!(dict.get(b"Type").is_ok());
            assert!(dict.get(b"Subtype").is_ok());
            assert!(dict.get(b"Rect").is_ok());
            assert!(dict.get(b"QuadPoints").is_ok());
            assert!(dict.get(b"C").is_ok());
            assert!(dict.get(b"Contents").is_ok());
        } else {
            panic!("expected Dictionary");
        }
    }

    #[test]
    fn build_link_annotation_has_action_dict() {
        let la = LinkAnnotation {
            page: 0,
            bbox: BBox {
                x0: 100.0,
                y0: 600.0,
                x1: 300.0,
                y1: 620.0,
            },
            uri: "https://example.com".to_string(),
        };
        let obj = build_link_annotation_object((5, 0), &la);
        if let Object::Dictionary(dict) = obj {
            assert!(dict.get(b"A").is_ok(), "should have /A action dict");
        } else {
            panic!("expected Dictionary");
        }
    }

    #[test]
    fn write_xref_sections_single_object() {
        let mut buf = Vec::new();
        let offsets = vec![((5u32, 0u16), 1234u64)];
        write_xref_sections(&mut buf, &offsets).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("5 1\n"), "xref header for object 5, count 1");
        assert!(
            s.contains("0000001234 00000 n"),
            "xref entry with offset 1234"
        );
    }

    #[test]
    fn write_xref_sections_contiguous_run() {
        let mut buf = Vec::new();
        let offsets = vec![
            ((5u32, 0u16), 1000u64),
            ((6u32, 0u16), 2000u64),
            ((7u32, 0u16), 3000u64),
        ];
        write_xref_sections(&mut buf, &offsets).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        // One run: "5 3\n"
        assert!(s.contains("5 3\n"), "contiguous run of 3 starting at 5");
    }
}
