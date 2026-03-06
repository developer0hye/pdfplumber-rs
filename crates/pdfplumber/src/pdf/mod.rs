//! Top-level PDF document type for opening and extracting content.

use std::sync::atomic::{AtomicUsize, Ordering};

use pdfplumber_core::{
    BBox, Bookmark, Char, Color, Ctm, Curve, DashPattern, DocumentMetadata, ExtractOptions,
    ExtractWarning, FormField, Image, ImageContent, ImageFilter, ImageMetadata, Line, Orientation,
    PageRegionOptions, PageRegions, PaintedPath, Path, PdfError, Rect, RepairOptions, RepairResult,
    SearchMatch, SearchOptions, SignatureInfo, StructElement, TextDirection, TextOptions,
    UnicodeNorm, ValidationIssue, apply_bidi_directions, dedupe_chars, detect_page_regions,
    extract_shapes, image_from_ctm, normalize_chars,
};
use pdfplumber_parse::{
    CharEvent, ContentHandler, ImageEvent, LopdfBackend, LopdfDocument, PageGeometry, PaintOp,
    PathEvent, PdfBackend, char_from_event,
};

use crate::Page;

/// Iterator over pages of a PDF document, yielding each page on demand.
///
/// Created by [`Pdf::pages_iter()`]. Each call to [`next()`](Iterator::next)
/// processes one page from the PDF content stream. Pages are not retained
/// after being yielded — the caller owns the `Page` value.
pub struct PagesIter<'a> {
    pdf: &'a Pdf,
    current: usize,
    count: usize,
}

impl Iterator for PagesIter<'_> {
    type Item = Result<Page, PdfError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.count {
            return None;
        }
        let result = self.pdf.page(self.current);
        self.current += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.current;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PagesIter<'_> {}

/// A PDF document opened for extraction.
///
/// Wraps a parsed PDF and provides methods to access pages and extract content.
///
/// # Example
///
/// ```ignore
/// let pdf = Pdf::open(bytes, None)?;
/// let page = pdf.page(0)?;
/// let text = page.extract_text(&TextOptions::default());
/// ```
pub struct Pdf {
    doc: LopdfDocument,
    options: ExtractOptions,
    /// Cached display heights for each page (for doctop calculation).
    page_heights: Vec<f64>,
    /// Cached raw PDF (MediaBox) heights for y-flip in char extraction.
    raw_page_heights: Vec<f64>,
    /// Cached document metadata from the /Info dictionary.
    metadata: DocumentMetadata,
    /// Cached document bookmarks (outline / table of contents).
    bookmarks: Vec<Bookmark>,
    /// Accumulated total objects extracted across all pages (for max_total_objects budget).
    total_objects: AtomicUsize,
    /// Accumulated total image bytes extracted across all pages (for max_total_image_bytes budget).
    total_image_bytes: AtomicUsize,
}

impl Pdf {
    /// Open a PDF document from a file path.
    ///
    /// This is a convenience wrapper around [`Pdf::open`] that reads the file
    /// into memory first. For WASM or no-filesystem environments, use
    /// [`Pdf::open`] with a byte slice instead.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the PDF file.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the file cannot be read or is not a valid PDF.
    #[cfg(feature = "std")]
    pub fn open_file(
        path: impl AsRef<std::path::Path>,
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let bytes = std::fs::read(path.as_ref()).map_err(|e| PdfError::IoError(e.to_string()))?;
        Self::open(&bytes, options)
    }

    /// Open a PDF document from bytes.
    ///
    /// This is the primary API for opening PDFs and works in all environments,
    /// including WASM. For file-path convenience, see [`Pdf::open_file`] (requires
    /// the `std` feature, enabled by default).
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError::PasswordRequired`] if the PDF is encrypted with a
    /// non-empty password. PDFs encrypted with an empty user password are
    /// auto-decrypted.
    /// Returns [`PdfError`] if the bytes are not a valid PDF document.
    pub fn open(bytes: &[u8], options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        // Check max_input_bytes before parsing
        if let Some(ref opts) = options {
            if let Some(max_bytes) = opts.max_input_bytes {
                if bytes.len() > max_bytes {
                    return Err(PdfError::ResourceLimitExceeded {
                        limit_name: "max_input_bytes".to_string(),
                        limit_value: max_bytes,
                        actual_value: bytes.len(),
                    });
                }
            }
        }
        let doc = LopdfBackend::open(bytes).map_err(PdfError::from)?;
        Self::from_doc(doc, options)
    }

    /// Open an encrypted PDF document from bytes with a password.
    ///
    /// Supports both user and owner passwords. If the PDF is not encrypted,
    /// the password is ignored and the document opens normally.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `password` - The password to decrypt the PDF.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError::InvalidPassword`] if the password is incorrect.
    /// Returns [`PdfError`] if the bytes are not a valid PDF document.
    pub fn open_with_password(
        bytes: &[u8],
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let doc = LopdfBackend::open_with_password(bytes, password).map_err(PdfError::from)?;
        Self::from_doc(doc, options)
    }

    /// Open an encrypted PDF document from a file path with a password.
    ///
    /// Convenience wrapper around [`Pdf::open_with_password`] that reads the file
    /// into memory first.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the PDF file.
    /// * `password` - The password to decrypt the PDF.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the file cannot be read, is not a valid PDF,
    /// or the password is incorrect.
    #[cfg(feature = "std")]
    pub fn open_file_with_password(
        path: impl AsRef<std::path::Path>,
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let bytes = std::fs::read(path.as_ref()).map_err(|e| PdfError::IoError(e.to_string()))?;
        Self::open_with_password(&bytes, password, options)
    }

    /// Open a PDF document with best-effort repair of common issues.
    ///
    /// Attempts to fix common PDF issues (broken xref, wrong stream lengths,
    /// broken references) before opening the document. Returns the opened
    /// PDF along with a [`RepairResult`] describing what was fixed.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    /// * `repair_opts` - Repair options controlling which fixes to attempt. Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the PDF is too corrupted to repair or open.
    pub fn open_with_repair(
        bytes: &[u8],
        options: Option<ExtractOptions>,
        repair_opts: Option<RepairOptions>,
    ) -> Result<(Self, RepairResult), PdfError> {
        let repair_opts = repair_opts.unwrap_or_default();
        let (repaired_bytes, result) =
            LopdfBackend::repair(bytes, &repair_opts).map_err(PdfError::from)?;
        let pdf = Self::open(&repaired_bytes, options)?;
        Ok((pdf, result))
    }

    /// Internal helper to construct a `Pdf` from a loaded `LopdfDocument`.
    fn from_doc(doc: LopdfDocument, options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        let options = options.unwrap_or_default();

        // Cache page heights for doctop calculation
        let page_count = LopdfBackend::page_count(&doc);

        // Check max_pages before processing
        if let Some(max_pages) = options.max_pages {
            if page_count > max_pages {
                return Err(PdfError::ResourceLimitExceeded {
                    limit_name: "max_pages".to_string(),
                    limit_value: max_pages,
                    actual_value: page_count,
                });
            }
        }

        let mut page_heights = Vec::with_capacity(page_count);
        let mut raw_page_heights = Vec::with_capacity(page_count);

        for i in 0..page_count {
            let page = LopdfBackend::get_page(&doc, i).map_err(PdfError::from)?;
            let media_box = LopdfBackend::page_media_box(&doc, &page).map_err(PdfError::from)?;
            let rotation = LopdfBackend::page_rotate(&doc, &page).map_err(PdfError::from)?;
            // Use MediaBox (not CropBox) for page dimensions to match Python pdfplumber.
            // CropBox is stored as page metadata but does not affect coordinate transforms.
            let geometry = PageGeometry::new(media_box, None, rotation);
            page_heights.push(geometry.height());
            // Compute the effective page height for the y-flip transform.
            //
            // Python pdfplumber computes: top = (height - char.y1) + mb_top
            // where mb_top accounts for non-zero MediaBox origins after
            // pdfminer's initial CTM translate(-x0, -y0). Since Rust does NOT
            // apply that initial CTM, we fold the offset into raw_page_height:
            //
            //   raw_page_height = |height| + top - min(top, bottom)
            //
            // - Normal [0 0 612 792]:      |792| + 0 - 0       = 792
            // - Non-zero origin [0 200 420 585]: |385| + 200 - 200 = 385
            // - Inverted [0 842 631 0]:    |842| + 842 - 0     = 1684
            let y_min = media_box.top.min(media_box.bottom);
            raw_page_heights.push(media_box.height().abs() + media_box.top - y_min);
        }

        // Extract document metadata
        let metadata = LopdfBackend::document_metadata(&doc).map_err(PdfError::from)?;

        // Extract document bookmarks (outline / table of contents)
        let bookmarks = LopdfBackend::document_bookmarks(&doc).map_err(PdfError::from)?;

        Ok(Self {
            doc,
            options,
            page_heights,
            raw_page_heights,
            metadata,
            bookmarks,
            total_objects: AtomicUsize::new(0),
            total_image_bytes: AtomicUsize::new(0),
        })
    }

    /// Return the number of pages in the document.
    pub fn page_count(&self) -> usize {
        LopdfBackend::page_count(&self.doc)
    }

    /// Return the document metadata from the PDF /Info dictionary.
    ///
    /// Returns a reference to the cached [`DocumentMetadata`] containing
    /// title, author, subject, keywords, creator, producer, and dates.
    /// Fields not present in the PDF are `None`.
    pub fn metadata(&self) -> &DocumentMetadata {
        &self.metadata
    }

    /// Return the document bookmarks (outline / table of contents).
    ///
    /// Returns a slice of [`Bookmark`]s representing the flattened outline
    /// tree, with each bookmark's `level` indicating nesting depth.
    /// Returns an empty slice if the document has no outlines.
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Extract all form fields from the document's AcroForm dictionary.
    ///
    /// Returns a list of [`FormField`]s from the `/AcroForm` dictionary.
    /// Returns an empty Vec if the document has no AcroForm.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the AcroForm exists but is malformed.
    pub fn form_fields(&self) -> Result<Vec<FormField>, PdfError> {
        LopdfBackend::document_form_fields(&self.doc).map_err(PdfError::from)
    }

    /// Search all pages for a text pattern and return matches with bounding boxes.
    ///
    /// Iterates through every page in the document, searches each page's
    /// characters for the given pattern, and collects all matches. Each match
    /// includes the page number, matched text, and a bounding box computed as
    /// the union of the matched characters' bounding boxes.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The search pattern (regex or literal, depending on options).
    /// * `options` - Controls regex vs. literal mode and case sensitivity.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if any page fails to load.
    pub fn search_all(
        &self,
        pattern: &str,
        options: &SearchOptions,
    ) -> Result<Vec<SearchMatch>, PdfError> {
        let mut all_matches = Vec::new();
        for i in 0..self.page_count() {
            let page = self.page(i)?;
            let matches = page.search(pattern, options);
            all_matches.extend(matches);
        }
        Ok(all_matches)
    }

    /// Extract image content (raw bytes) for a named image XObject on a page.
    ///
    /// Locates the image by its XObject name (e.g., "Im0") in the page's
    /// resources and returns the decoded image bytes along with format and
    /// dimension information.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the page index is out of range, the image
    /// is not found, or stream decoding fails.
    pub fn extract_image_content(
        &self,
        page_index: usize,
        image_name: &str,
    ) -> Result<ImageContent, PdfError> {
        let lopdf_page = LopdfBackend::get_page(&self.doc, page_index).map_err(PdfError::from)?;
        LopdfBackend::extract_image_content(&self.doc, &lopdf_page, image_name)
            .map_err(PdfError::from)
    }

    /// Extract all images with their content from a page.
    ///
    /// First extracts the page to get image metadata, then extracts the
    /// raw content for each image. Returns pairs of (Image, ImageContent).
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if page extraction or any image content
    /// extraction fails.
    pub fn extract_images_with_content(
        &self,
        page_index: usize,
    ) -> Result<Vec<(Image, ImageContent)>, PdfError> {
        let page = self.page(page_index)?;
        let mut results = Vec::new();
        for image in page.images() {
            match self.extract_image_content(page_index, &image.name) {
                Ok(content) => results.push((image.clone(), content)),
                Err(_) => {
                    // Skip images that can't be extracted (e.g., inline images)
                    continue;
                }
            }
        }
        Ok(results)
    }

    /// Return a streaming iterator over all pages in the document.
    ///
    /// Each page is processed on demand when [`Iterator::next()`] is called.
    /// Previously yielded pages are not retained by the iterator, so memory
    /// usage stays bounded regardless of document size.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pdf = Pdf::open(bytes, None)?;
    /// for result in pdf.pages_iter() {
    ///     let page = result?;
    ///     println!("Page {}: {}", page.page_number(), page.extract_text(&TextOptions::default()));
    ///     // page is dropped at end of loop body
    /// }
    /// ```
    pub fn pages_iter(&self) -> PagesIter<'_> {
        PagesIter {
            pdf: self,
            current: 0,
            count: self.page_count(),
        }
    }

    /// Process all pages in parallel using rayon, returning a Vec of Results.
    ///
    /// Each page is extracted concurrently. The returned Vec is ordered by page
    /// index (0-based). Page data (doctop offsets, etc.) is computed correctly
    /// regardless of processing order.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pdf = Pdf::open(bytes, None)?;
    /// let pages: Vec<Page> = pdf.pages_parallel()
    ///     .into_iter()
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// ```
    #[cfg(feature = "parallel")]
    pub fn pages_parallel(&self) -> Vec<Result<Page, PdfError>> {
        use rayon::prelude::*;

        (0..self.page_count())
            .into_par_iter()
            .map(|i| self.page(i))
            .collect()
    }

    /// Access a page by 0-based index, extracting all content.
    ///
    /// Returns a [`Page`] with characters, images, and metadata extracted
    /// from the PDF content stream.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the index is out of range or content
    /// interpretation fails.
    pub fn page(&self, index: usize) -> Result<Page, PdfError> {
        let lopdf_page = LopdfBackend::get_page(&self.doc, index).map_err(PdfError::from)?;

        // Page geometry
        let media_box =
            LopdfBackend::page_media_box(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let crop_box =
            LopdfBackend::page_crop_box(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let trim_box =
            LopdfBackend::page_trim_box(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let bleed_box =
            LopdfBackend::page_bleed_box(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let art_box = LopdfBackend::page_art_box(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let rotation = LopdfBackend::page_rotate(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        // Use MediaBox (not CropBox) for coordinate transforms to match Python pdfplumber.
        let geometry = PageGeometry::new(media_box, None, rotation);

        // Interpret page content
        let mut handler = CollectingHandler::new(index, self.options.collect_warnings);
        LopdfBackend::interpret_page(&self.doc, &lopdf_page, &mut handler, &self.options)
            .map_err(PdfError::from)?;

        // Convert CharEvents to Chars
        let page_height = self.raw_page_heights[index];
        let doctop_offset: f64 = self.page_heights[..index].iter().sum();
        let needs_rotation = geometry.rotation() != 0;

        let mut chars: Vec<Char> = handler
            .chars
            .iter()
            .map(|event| {
                let mut ch = char_from_event(event, page_height, None, None);
                if needs_rotation {
                    // char_from_event applied a simple y-flip using the raw page height.
                    // Undo it to recover PDF native coordinates, then apply the full
                    // rotation + y-flip transform via PageGeometry.
                    let native_min_y = page_height - ch.bbox.bottom;
                    let native_max_y = page_height - ch.bbox.top;
                    ch.bbox =
                        geometry.normalize_bbox(ch.bbox.x0, native_min_y, ch.bbox.x1, native_max_y);
                    ch.doctop = ch.bbox.top;
                    ch.direction = rotate_direction(ch.direction, rotation);
                    // 90°/270° rotation turns upright text non-upright and vice versa
                    if rotation == 90 || rotation == 270 {
                        ch.upright = !ch.upright;
                    }
                }
                ch.doctop += doctop_offset;
                ch
            })
            .collect();

        // Apply Unicode BiDi direction analysis for Arabic/Hebrew/mixed text
        chars = apply_bidi_directions(&chars, 3.0);

        // Apply Unicode normalization if configured
        if self.options.unicode_norm != UnicodeNorm::None {
            chars = normalize_chars(&chars, &self.options.unicode_norm);
        }

        // Apply character deduplication if configured
        if let Some(ref dedupe_opts) = self.options.dedupe {
            chars = dedupe_chars(&chars, dedupe_opts);
        }

        // Convert PathEvents to Lines/Rects/Curves via PaintedPath + extract_shapes
        let mut all_lines: Vec<Line> = Vec::new();
        let mut all_rects: Vec<Rect> = Vec::new();
        let mut all_curves: Vec<Curve> = Vec::new();

        for path_event in &handler.paths {
            let painted = path_event_to_painted_path(path_event);
            let (mut lines, mut rects, mut curves) = extract_shapes(&painted, page_height);
            if needs_rotation {
                for line in &mut lines {
                    let bbox = rotate_bbox(
                        line.x0,
                        line.top,
                        line.x1,
                        line.bottom,
                        page_height,
                        &geometry,
                    );
                    line.x0 = bbox.x0;
                    line.top = bbox.top;
                    line.x1 = bbox.x1;
                    line.bottom = bbox.bottom;
                    line.orientation = classify_orientation(line);
                }
                for rect in &mut rects {
                    let bbox = rotate_bbox(
                        rect.x0,
                        rect.top,
                        rect.x1,
                        rect.bottom,
                        page_height,
                        &geometry,
                    );
                    rect.x0 = bbox.x0;
                    rect.top = bbox.top;
                    rect.x1 = bbox.x1;
                    rect.bottom = bbox.bottom;
                }
                for curve in &mut curves {
                    let bbox = rotate_bbox(
                        curve.x0,
                        curve.top,
                        curve.x1,
                        curve.bottom,
                        page_height,
                        &geometry,
                    );
                    curve.x0 = bbox.x0;
                    curve.top = bbox.top;
                    curve.x1 = bbox.x1;
                    curve.bottom = bbox.bottom;
                    curve.pts = curve
                        .pts
                        .iter()
                        .map(|&(x, y)| {
                            let native_y = page_height - y;
                            geometry.normalize_point(x, native_y)
                        })
                        .collect();
                }
            }
            all_lines.extend(lines);
            all_rects.extend(rects);
            all_curves.extend(curves);
        }

        // Convert ImageEvents to Images
        let images: Vec<Image> = handler
            .images
            .iter()
            .map(|event| {
                let ctm = Ctm::new(
                    event.ctm[0],
                    event.ctm[1],
                    event.ctm[2],
                    event.ctm[3],
                    event.ctm[4],
                    event.ctm[5],
                );
                let meta = ImageMetadata {
                    src_width: Some(event.width),
                    src_height: Some(event.height),
                    bits_per_component: event.bits_per_component,
                    color_space: event.colorspace.clone(),
                };
                let mut img = image_from_ctm(&ctm, &event.name, page_height, &meta);

                // Set filter and mime_type from the event
                if let Some(ref filter_name) = event.filter {
                    let filter = ImageFilter::from_pdf_name(filter_name);
                    img.mime_type = Some(filter.mime_type().to_string());
                    img.filter = Some(filter);
                }

                // Optionally extract image data
                if self.options.extract_image_data {
                    if let Ok(content) =
                        LopdfBackend::extract_image_content(&self.doc, &lopdf_page, &event.name)
                    {
                        img.data = Some(content.data);
                    }
                }

                if needs_rotation {
                    let bbox =
                        rotate_bbox(img.x0, img.top, img.x1, img.bottom, page_height, &geometry);
                    img.x0 = bbox.x0;
                    img.top = bbox.top;
                    img.x1 = bbox.x1;
                    img.bottom = bbox.bottom;
                    img.width = bbox.width();
                    img.height = bbox.height();
                }

                img
            })
            .collect();

        // Extract annotations from the page
        let annotations =
            LopdfBackend::page_annotations(&self.doc, &lopdf_page).map_err(PdfError::from)?;

        // Extract hyperlinks from the page
        let hyperlinks =
            LopdfBackend::page_hyperlinks(&self.doc, &lopdf_page).map_err(PdfError::from)?;

        // Extract form fields for this page (filtered from document AcroForm)
        let all_form_fields =
            LopdfBackend::document_form_fields(&self.doc).map_err(PdfError::from)?;
        let form_fields: Vec<FormField> = all_form_fields
            .into_iter()
            .filter(|f| f.page_index == Some(index))
            .collect();

        // Extract structure tree for this page (filtered from document StructTreeRoot)
        let all_struct_elements =
            LopdfBackend::document_structure_tree(&self.doc).map_err(PdfError::from)?;
        let structure_tree = if all_struct_elements.is_empty() {
            None
        } else {
            let page_elements: Vec<StructElement> =
                filter_struct_elements_for_page(&all_struct_elements, index);
            if page_elements.is_empty() {
                None
            } else {
                Some(page_elements)
            }
        };

        // Check document-level resource budgets
        let page_object_count =
            chars.len() + all_lines.len() + all_rects.len() + all_curves.len() + images.len();
        if let Some(max_total) = self.options.max_total_objects {
            let new_total = self
                .total_objects
                .fetch_add(page_object_count, Ordering::Relaxed)
                + page_object_count;
            if new_total > max_total {
                return Err(PdfError::ResourceLimitExceeded {
                    limit_name: "max_total_objects".to_string(),
                    limit_value: max_total,
                    actual_value: new_total,
                });
            }
        }

        let page_image_bytes: usize = images
            .iter()
            .filter_map(|img| img.data.as_ref().map(|d| d.len()))
            .sum();
        if let Some(max_img_bytes) = self.options.max_total_image_bytes {
            let new_total = self
                .total_image_bytes
                .fetch_add(page_image_bytes, Ordering::Relaxed)
                + page_image_bytes;
            if new_total > max_img_bytes {
                return Err(PdfError::ResourceLimitExceeded {
                    limit_name: "max_total_image_bytes".to_string(),
                    limit_value: max_img_bytes,
                    actual_value: new_total,
                });
            }
        }

        Ok(Page::from_extraction(
            index,
            geometry.width(),
            geometry.height(),
            rotation,
            media_box,
            crop_box,
            trim_box,
            bleed_box,
            art_box,
            chars,
            all_lines,
            all_rects,
            all_curves,
            images,
            annotations,
            hyperlinks,
            form_fields,
            structure_tree,
            handler.warnings,
        ))
    }

    /// Validate the PDF document and report specification violations.
    ///
    /// Checks for common PDF issues such as missing required keys,
    /// broken object references, invalid page tree structure, and
    /// missing fonts referenced in content streams.
    ///
    /// Returns a list of [`ValidationIssue`]s describing any problems
    /// found. An empty list indicates no issues were detected.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the document structure is too corrupted
    /// to perform validation.
    pub fn validate(&self) -> Result<Vec<ValidationIssue>, PdfError> {
        LopdfBackend::validate(&self.doc).map_err(PdfError::from)
    }

    /// Extract digital signature information from the document.
    ///
    /// Returns a list of [`SignatureInfo`]s for each signature field found
    /// in the document's `/AcroForm` dictionary. Both signed and unsigned
    /// signature fields are included.
    ///
    /// Returns an empty Vec if the document has no signature fields.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the AcroForm exists but is malformed.
    pub fn signatures(&self) -> Result<Vec<SignatureInfo>, PdfError> {
        LopdfBackend::document_signatures(&self.doc).map_err(PdfError::from)
    }

    /// Detect repeating headers and footers across all pages.
    ///
    /// Extracts text from the top and bottom margins of each page, compares
    /// across pages with fuzzy matching (masking digits for page numbers),
    /// and returns [`PageRegions`] for each page indicating detected
    /// header/footer regions and the body area.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if any page fails to extract.
    pub fn detect_page_regions(
        &self,
        options: &PageRegionOptions,
    ) -> Result<Vec<PageRegions>, PdfError> {
        let text_options = TextOptions::default();
        let mut page_data: Vec<(String, String, f64, f64)> = Vec::new();

        for page_result in self.pages_iter() {
            let page = page_result?;
            let width = page.width();
            let height = page.height();

            let header_height = height * options.header_margin;
            let header_bbox = BBox::new(0.0, 0.0, width, header_height);
            let header_page = page.crop(header_bbox);
            let header_text = header_page.extract_text(&text_options);

            let footer_height = height * options.footer_margin;
            let footer_top = height - footer_height;
            let footer_bbox = BBox::new(0.0, footer_top, width, height);
            let footer_page = page.crop(footer_bbox);
            let footer_text = footer_page.extract_text(&text_options);

            page_data.push((header_text, footer_text, width, height));
        }

        Ok(detect_page_regions(&page_data, options))
    }
}

mod helpers;

use helpers::{
    classify_orientation, filter_struct_elements_for_page, path_event_to_painted_path,
    rotate_bbox, rotate_direction, CollectingHandler,
};

#[cfg(test)]
mod tests;
