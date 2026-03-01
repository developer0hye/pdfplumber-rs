//! Top-level PDF document type for opening and extracting content.

use std::sync::atomic::{AtomicUsize, Ordering};

use pdfplumber_core::{
    BBox, Bookmark, Char, Color, Ctm, Curve, DashPattern, DocumentMetadata, ExtractOptions,
    ExtractWarning, FormField, Image, ImageContent, ImageFilter, ImageMetadata, Line,
    PageRegionOptions, PageRegions, PaintedPath, Path, PdfError, Rect, RepairOptions, RepairResult,
    SearchMatch, SearchOptions, SignatureInfo, StructElement, TextOptions, UnicodeNorm,
    ValidationIssue, detect_page_regions, extract_shapes, image_from_ctm, normalize_chars,
};
use pdfplumber_parse::{
    CharEvent, ContentHandler, FontMetrics, ImageEvent, LopdfBackend, LopdfDocument, PageGeometry,
    PaintOp, PathEvent, PdfBackend, char_from_event,
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

impl<'a> Iterator for PagesIter<'a> {
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

/// Internal handler that collects content stream events during interpretation.
struct CollectingHandler {
    chars: Vec<CharEvent>,
    paths: Vec<PathEvent>,
    images: Vec<ImageEvent>,
    warnings: Vec<ExtractWarning>,
    page_index: usize,
    collect_warnings: bool,
}

impl CollectingHandler {
    fn new(page_index: usize, collect_warnings: bool) -> Self {
        Self {
            chars: Vec::new(),
            paths: Vec::new(),
            images: Vec::new(),
            warnings: Vec::new(),
            page_index,
            collect_warnings,
        }
    }
}

impl ContentHandler for CollectingHandler {
    fn on_char(&mut self, event: CharEvent) {
        self.chars.push(event);
    }

    fn on_path_painted(&mut self, event: PathEvent) {
        self.paths.push(event);
    }

    fn on_image(&mut self, event: ImageEvent) {
        self.images.push(event);
    }

    fn on_warning(&mut self, mut warning: ExtractWarning) {
        if self.collect_warnings {
            // Decorate warnings with page context
            if warning.page.is_none() {
                warning.page = Some(self.page_index);
            }
            self.warnings.push(warning);
        }
    }
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
    /// Returns [`PdfError::PasswordRequired`] if the PDF is encrypted.
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
            let crop_box = LopdfBackend::page_crop_box(&doc, &page).map_err(PdfError::from)?;
            let rotation = LopdfBackend::page_rotate(&doc, &page).map_err(PdfError::from)?;
            let geometry = PageGeometry::new(media_box, crop_box, rotation);
            page_heights.push(geometry.height());
            raw_page_heights.push(media_box.height());
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
        let geometry = PageGeometry::new(media_box, crop_box, rotation);

        // Interpret page content
        let mut handler = CollectingHandler::new(index, self.options.collect_warnings);
        LopdfBackend::interpret_page(&self.doc, &lopdf_page, &mut handler, &self.options)
            .map_err(PdfError::from)?;

        // Convert CharEvents to Chars
        let page_height = self.raw_page_heights[index];
        let default_metrics = FontMetrics::default_metrics();
        let doctop_offset: f64 = self.page_heights[..index].iter().sum();

        let mut chars: Vec<Char> = handler
            .chars
            .iter()
            .map(|event| {
                let mut ch = char_from_event(event, &default_metrics, page_height, None, None);
                ch.doctop += doctop_offset;
                ch
            })
            .collect();

        // Apply Unicode normalization if configured
        if self.options.unicode_norm != UnicodeNorm::None {
            chars = normalize_chars(&chars, &self.options.unicode_norm);
        }

        // Convert PathEvents to Lines/Rects/Curves via PaintedPath + extract_shapes
        let mut all_lines: Vec<Line> = Vec::new();
        let mut all_rects: Vec<Rect> = Vec::new();
        let mut all_curves: Vec<Curve> = Vec::new();

        for path_event in &handler.paths {
            let painted = path_event_to_painted_path(path_event);
            let (lines, rects, curves) = extract_shapes(&painted, page_height);
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

/// Filter structure tree elements to only include those belonging to a specific page.
///
/// Convert a `PathEvent` from the interpreter into a `PaintedPath` for shape extraction.
fn path_event_to_painted_path(event: &PathEvent) -> PaintedPath {
    let (stroke, fill) = match event.paint_op {
        PaintOp::Stroke => (true, false),
        PaintOp::Fill => (false, true),
        PaintOp::FillAndStroke => (true, true),
    };

    PaintedPath {
        path: Path {
            segments: event.segments.clone(),
        },
        stroke,
        fill,
        fill_rule: event.fill_rule.unwrap_or_default(),
        line_width: event.line_width,
        stroke_color: event.stroking_color.clone().unwrap_or(Color::black()),
        fill_color: event.non_stroking_color.clone().unwrap_or(Color::black()),
        dash_pattern: event
            .dash_pattern
            .clone()
            .unwrap_or_else(DashPattern::solid),
        stroke_alpha: 1.0,
        fill_alpha: 1.0,
    }
}

/// Recursively walks the structure tree and includes elements whose `page_index`
/// matches the target page. Elements without a page_index are included if any of
/// their children belong to the page.
fn filter_struct_elements_for_page(
    elements: &[StructElement],
    page_index: usize,
) -> Vec<StructElement> {
    elements
        .iter()
        .filter_map(|elem| filter_struct_element(elem, page_index))
        .collect()
}

/// Filter a single structure element and its children for a specific page.
fn filter_struct_element(elem: &StructElement, page_index: usize) -> Option<StructElement> {
    // Recursively filter children
    let filtered_children = filter_struct_elements_for_page(&elem.children, page_index);

    // Include this element if:
    // 1. It explicitly belongs to this page, OR
    // 2. It has no page_index but has children that belong to this page
    let belongs_to_page = elem.page_index == Some(page_index);
    let has_page_children = !filtered_children.is_empty();

    if belongs_to_page || has_page_children {
        Some(StructElement {
            element_type: elem.element_type.clone(),
            mcids: if belongs_to_page {
                elem.mcids.clone()
            } else {
                Vec::new()
            },
            alt_text: elem.alt_text.clone(),
            actual_text: elem.actual_text.clone(),
            lang: elem.lang.clone(),
            bbox: elem.bbox,
            children: filtered_children,
            page_index: elem.page_index,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::TextOptions;

    /// Helper: create a minimal single-page PDF with the given text content stream.
    fn create_pdf_with_content(content: &[u8]) -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // Font
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        // Content stream
        let stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(stream);

        // Resources
        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_id),
            },
        };

        // Page (parent set after pages tree creation)
        let media_box = vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ];
        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box,
            "Contents" => Object::Reference(content_id),
            "Resources" => resources,
        };
        let page_id = doc.add_object(page_dict);

        // Pages tree
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => Object::Integer(1),
        };
        let pages_id = doc.add_object(pages_dict);

        // Set page parent
        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });

        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// Helper: create a two-page PDF for doctop testing.
    fn create_two_page_pdf() -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // Shared font
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        // Page 1 content: "Hello" at (72, 720)
        let content1 = b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
        let stream1 = Stream::new(dictionary! {}, content1.to_vec());
        let content1_id = doc.add_object(stream1);

        // Page 2 content: "World" at (72, 720)
        let content2 = b"BT /F1 12 Tf 72 720 Td (World) Tj ET";
        let stream2 = Stream::new(dictionary! {}, content2.to_vec());
        let content2_id = doc.add_object(stream2);

        // Resources
        let resources1 = dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        };
        let resources2 = dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        };

        let media_box = vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ];

        // Page 1
        let page1_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box.clone(),
            "Contents" => Object::Reference(content1_id),
            "Resources" => resources1,
        };
        let page1_id = doc.add_object(page1_dict);

        // Page 2
        let page2_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box,
            "Contents" => Object::Reference(content2_id),
            "Resources" => resources2,
        };
        let page2_id = doc.add_object(page2_dict);

        // Pages tree
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page1_id), Object::Reference(page2_id)],
            "Count" => Object::Integer(2),
        };
        let pages_id = doc.add_object(pages_dict);

        // Set parent for both pages
        for pid in [page1_id, page2_id] {
            if let Ok(page_obj) = doc.get_object_mut(pid) {
                if let Ok(dict) = page_obj.as_dict_mut() {
                    dict.set("Parent", Object::Reference(pages_id));
                }
            }
        }

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    // --- Pdf::open tests ---

    #[test]
    fn open_valid_pdf() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }

    #[test]
    fn open_invalid_bytes_returns_error() {
        let result = Pdf::open(b"not a pdf", None);
        assert!(result.is_err());
    }

    #[test]
    fn open_with_custom_options() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf (Hi) Tj ET");
        let opts = ExtractOptions {
            max_recursion_depth: 5,
            ..ExtractOptions::default()
        };
        let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }

    // --- page_count tests ---

    #[test]
    fn page_count_single_page() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }

    #[test]
    fn page_count_two_pages() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();
        assert_eq!(pdf.page_count(), 2);
    }

    // --- page() tests ---

    #[test]
    fn page_returns_correct_dimensions() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        assert_eq!(page.width(), 612.0);
        assert_eq!(page.height(), 792.0);
    }

    #[test]
    fn page_returns_correct_page_number() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();
        assert_eq!(pdf.page(0).unwrap().page_number(), 0);
        assert_eq!(pdf.page(1).unwrap().page_number(), 1);
    }

    #[test]
    fn page_out_of_range_returns_error() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        assert!(pdf.page(1).is_err());
        assert!(pdf.page(100).is_err());
    }

    // --- Page metadata tests ---

    #[test]
    fn page_rotation_default_zero() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        assert_eq!(page.rotation(), 0);
    }

    #[test]
    fn page_bbox_matches_dimensions() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        let bbox = page.bbox();
        assert_eq!(bbox.x0, 0.0);
        assert_eq!(bbox.top, 0.0);
        assert_eq!(bbox.x1, 612.0);
        assert_eq!(bbox.bottom, 792.0);
    }

    // --- Character extraction tests ---

    #[test]
    fn page_chars_from_simple_text() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let chars = page.chars();
        assert_eq!(chars.len(), 5);
        // Characters should be in order H, e, l, l, o
        assert_eq!(chars[0].char_code, b'H' as u32);
        assert_eq!(chars[1].char_code, b'e' as u32);
        assert_eq!(chars[2].char_code, b'l' as u32);
        assert_eq!(chars[3].char_code, b'l' as u32);
        assert_eq!(chars[4].char_code, b'o' as u32);
    }

    #[test]
    fn page_chars_have_valid_bboxes() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (A) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let chars = page.chars();
        assert_eq!(chars.len(), 1);

        let ch = &chars[0];
        // x0 should be at text position 72
        assert!((ch.bbox.x0 - 72.0).abs() < 0.01);
        // Character should have positive width and height
        assert!(ch.bbox.width() > 0.0);
        assert!(ch.bbox.height() > 0.0);
        // Top should be near top of page (PDF y=720 → top-left y ≈ 72)
        assert!(ch.bbox.top < 100.0);
    }

    #[test]
    fn page_chars_fontname_and_size() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf (X) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let chars = page.chars();
        assert_eq!(chars.len(), 1);
        // Font name comes from BaseFont in the font dict
        assert_eq!(chars[0].fontname, "Helvetica");
        assert_eq!(chars[0].size, 12.0);
    }

    #[test]
    fn page_empty_content_has_no_chars() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        assert!(page.chars().is_empty());
    }

    // --- Text extraction tests ---

    #[test]
    fn extract_text_simple_string() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let text = page.extract_text(&TextOptions::default());
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn extract_text_multiline() {
        // Two lines: "Line1" at y=720, "Line2" at y=700
        let content = b"BT /F1 12 Tf 72 720 Td (Line1) Tj 0 -20 Td (Line2) Tj ET";
        let bytes = create_pdf_with_content(content);
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let text = page.extract_text(&TextOptions::default());
        assert!(text.contains("Line1"));
        assert!(text.contains("Line2"));
        // Should be on separate lines
        assert!(text.contains('\n'));
    }

    #[test]
    fn extract_text_empty_page() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let text = page.extract_text(&TextOptions::default());
        assert_eq!(text, "");
    }

    // --- doctop tests ---

    #[test]
    fn doctop_first_page_equals_top() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (A) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let chars = page.chars();
        assert_eq!(chars.len(), 1);
        // On first page, doctop should equal bbox.top
        assert!((chars[0].doctop - chars[0].bbox.top).abs() < 0.01);
    }

    #[test]
    fn doctop_second_page_offset_by_first_page_height() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let page0 = pdf.page(0).unwrap();
        let page1 = pdf.page(1).unwrap();

        let chars0 = page0.chars();
        let chars1 = page1.chars();

        assert!(!chars0.is_empty());
        assert!(!chars1.is_empty());

        // Both pages have same content at same position, so bbox.top should match
        let top0 = chars0[0].bbox.top;
        let top1 = chars1[0].bbox.top;
        assert!((top0 - top1).abs() < 0.01);

        // doctop on page 1 should be offset by page 0's height (792)
        let expected_doctop_1 = top1 + page0.height();
        assert!(
            (chars1[0].doctop - expected_doctop_1).abs() < 0.01,
            "doctop on page 1 ({}) should be {} (top {} + page_height {})",
            chars1[0].doctop,
            expected_doctop_1,
            top1,
            page0.height()
        );
    }

    // --- Parallel page processing tests (US-044) ---

    /// Helper: create a multi-page PDF with distinct text on each page.
    #[cfg(feature = "parallel")]
    fn create_multi_page_pdf(page_texts: &[&str]) -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let media_box = vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ];

        let mut page_ids = Vec::new();
        for text in page_texts {
            let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
            let stream = Stream::new(dictionary! {}, content.into_bytes());
            let content_id = doc.add_object(stream);
            let resources = dictionary! {
                "Font" => dictionary! { "F1" => Object::Reference(font_id) },
            };
            let page_dict = dictionary! {
                "Type" => "Page",
                "MediaBox" => media_box.clone(),
                "Contents" => Object::Reference(content_id),
                "Resources" => resources,
            };
            page_ids.push(doc.add_object(page_dict));
        }

        let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => Object::Integer(page_ids.len() as i64),
        };
        let pages_id = doc.add_object(pages_dict);

        for pid in &page_ids {
            if let Ok(page_obj) = doc.get_object_mut(*pid) {
                if let Ok(dict) = page_obj.as_dict_mut() {
                    dict.set("Parent", Object::Reference(pages_id));
                }
            }
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[cfg(feature = "parallel")]
    mod parallel_tests {
        use super::*;

        #[test]
        fn pages_parallel_returns_all_pages() {
            let bytes = create_multi_page_pdf(&["Alpha", "Beta", "Gamma", "Delta"]);
            let pdf = Pdf::open(&bytes, None).unwrap();
            let results = pdf.pages_parallel();

            assert_eq!(results.len(), 4);
            for result in &results {
                assert!(result.is_ok());
            }
        }

        #[test]
        fn pages_parallel_matches_sequential() {
            let texts = &["Hello", "World", "Foo", "Bar"];
            let bytes = create_multi_page_pdf(texts);
            let pdf = Pdf::open(&bytes, None).unwrap();

            // Sequential extraction
            let sequential: Vec<_> = (0..pdf.page_count())
                .map(|i| pdf.page(i).unwrap())
                .collect();

            // Parallel extraction
            let parallel: Vec<_> = pdf
                .pages_parallel()
                .into_iter()
                .map(|r| r.unwrap())
                .collect();

            assert_eq!(sequential.len(), parallel.len());

            for (seq, par) in sequential.iter().zip(parallel.iter()) {
                // Same page number
                assert_eq!(seq.page_number(), par.page_number());
                // Same dimensions
                assert_eq!(seq.width(), par.width());
                assert_eq!(seq.height(), par.height());
                // Same number of chars
                assert_eq!(seq.chars().len(), par.chars().len());
                // Same char text content
                for (sc, pc) in seq.chars().iter().zip(par.chars().iter()) {
                    assert_eq!(sc.text, pc.text);
                    assert_eq!(sc.char_code, pc.char_code);
                    assert!((sc.bbox.x0 - pc.bbox.x0).abs() < 0.01);
                    assert!((sc.bbox.top - pc.bbox.top).abs() < 0.01);
                    assert!((sc.doctop - pc.doctop).abs() < 0.01);
                }
                // Same text extraction
                let seq_text = seq.extract_text(&TextOptions::default());
                let par_text = par.extract_text(&TextOptions::default());
                assert_eq!(seq_text, par_text);
            }
        }

        #[test]
        fn pages_parallel_single_page() {
            let bytes = create_multi_page_pdf(&["Only"]);
            let pdf = Pdf::open(&bytes, None).unwrap();
            let results = pdf.pages_parallel();

            assert_eq!(results.len(), 1);
            let page = results.into_iter().next().unwrap().unwrap();
            assert_eq!(page.page_number(), 0);
            let text = page.extract_text(&TextOptions::default());
            assert!(text.contains("Only"));
        }

        #[test]
        fn pages_parallel_preserves_doctop() {
            let bytes = create_multi_page_pdf(&["Page0", "Page1", "Page2"]);
            let pdf = Pdf::open(&bytes, None).unwrap();
            let pages: Vec<_> = pdf
                .pages_parallel()
                .into_iter()
                .map(|r| r.unwrap())
                .collect();

            // Page 0: doctop == bbox.top (no offset)
            let c0 = &pages[0].chars()[0];
            assert!((c0.doctop - c0.bbox.top).abs() < 0.01);

            // Page 1: doctop == bbox.top + page0.height
            let c1 = &pages[1].chars()[0];
            let expected1 = c1.bbox.top + pages[0].height();
            assert!(
                (c1.doctop - expected1).abs() < 0.01,
                "page 1 doctop {} expected {}",
                c1.doctop,
                expected1
            );

            // Page 2: doctop == bbox.top + page0.height + page1.height
            let c2 = &pages[2].chars()[0];
            let expected2 = c2.bbox.top + pages[0].height() + pages[1].height();
            assert!(
                (c2.doctop - expected2).abs() < 0.01,
                "page 2 doctop {} expected {}",
                c2.doctop,
                expected2
            );
        }

        #[test]
        fn pdf_is_sync() {
            // Compile-time assertion that Pdf can be shared across threads
            fn assert_sync<T: Sync>() {}
            assert_sync::<Pdf>();
        }
    }

    // --- Warning collection tests ---

    #[test]
    fn page_has_empty_warnings_for_valid_pdf() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        // Valid PDF with proper font → no warnings
        assert!(page.warnings().is_empty());
    }

    #[test]
    fn page_collects_warnings_when_font_missing_from_resources() {
        // Create PDF where the font reference F2 is not in resources
        // The content references F2 but the PDF only defines F1
        let bytes = create_pdf_with_content(b"BT /F2 12 Tf (X) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        // Should collect warnings about missing font
        assert!(
            !page.warnings().is_empty(),
            "expected warnings for missing font"
        );
        assert!(page.warnings()[0].description.contains("font not found"));
        assert_eq!(page.warnings()[0].page, Some(0));
        assert_eq!(page.warnings()[0].font_name, Some("F2".to_string()));
    }

    #[test]
    fn page_no_warnings_when_collection_disabled() {
        let bytes = create_pdf_with_content(b"BT /F2 12 Tf (X) Tj ET");
        let opts = ExtractOptions {
            collect_warnings: false,
            ..ExtractOptions::default()
        };
        let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
        let page = pdf.page(0).unwrap();

        // Warnings suppressed → empty
        assert!(page.warnings().is_empty());

        // But characters should still be extracted
        assert_eq!(page.chars().len(), 1);
    }

    #[test]
    fn warnings_do_not_affect_char_extraction() {
        let bytes = create_pdf_with_content(b"BT /F2 12 Tf (AB) Tj ET");

        // With warnings
        let pdf_on = Pdf::open(
            &bytes,
            Some(ExtractOptions {
                collect_warnings: true,
                ..ExtractOptions::default()
            }),
        )
        .unwrap();
        let page_on = pdf_on.page(0).unwrap();

        // Without warnings
        let pdf_off = Pdf::open(
            &bytes,
            Some(ExtractOptions {
                collect_warnings: false,
                ..ExtractOptions::default()
            }),
        )
        .unwrap();
        let page_off = pdf_off.page(0).unwrap();

        // Same number of characters
        assert_eq!(page_on.chars().len(), page_off.chars().len());
        // Same char codes
        for (a, b) in page_on.chars().iter().zip(page_off.chars().iter()) {
            assert_eq!(a.char_code, b.char_code);
            assert_eq!(a.text, b.text);
        }
    }

    #[test]
    fn warning_includes_page_number() {
        let bytes = create_pdf_with_content(b"BT /F2 12 Tf (X) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        // Verify page number is set in warning
        for w in page.warnings() {
            assert_eq!(w.page, Some(0), "warning should have page context");
        }
    }

    // --- US-046: Page-level memory management tests ---

    #[test]
    fn pages_iter_yields_all_pages() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let pages: Vec<_> = pdf.pages_iter().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_number(), 0);
        assert_eq!(pages[1].page_number(), 1);
    }

    #[test]
    fn pages_iter_yields_correct_content() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let pages: Vec<_> = pdf.pages_iter().collect::<Result<Vec<_>, _>>().unwrap();

        // Page 0 has "Hello"
        let text0 = pages[0].extract_text(&TextOptions::default());
        assert!(text0.contains("Hello"));

        // Page 1 has "World"
        let text1 = pages[1].extract_text(&TextOptions::default());
        assert!(text1.contains("World"));
    }

    #[test]
    fn pages_iter_matches_page_method() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        // Iterator results should match individual page() calls
        for (iter_page, idx) in pdf.pages_iter().zip(0usize..) {
            let iter_page = iter_page.unwrap();
            let direct_page = pdf.page(idx).unwrap();

            assert_eq!(iter_page.page_number(), direct_page.page_number());
            assert_eq!(iter_page.width(), direct_page.width());
            assert_eq!(iter_page.height(), direct_page.height());
            assert_eq!(iter_page.chars().len(), direct_page.chars().len());

            for (ic, dc) in iter_page.chars().iter().zip(direct_page.chars().iter()) {
                assert_eq!(ic.text, dc.text);
                assert!((ic.doctop - dc.doctop).abs() < 0.01);
            }
        }
    }

    #[test]
    fn pages_iter_single_page() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Only) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();

        let pages: Vec<_> = pdf.pages_iter().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(pages.len(), 1);
        assert!(
            pages[0]
                .extract_text(&TextOptions::default())
                .contains("Only")
        );
    }

    #[test]
    fn pages_iter_empty_after_exhaustion() {
        let bytes = create_pdf_with_content(b"BT ET");
        let pdf = Pdf::open(&bytes, None).unwrap();

        let mut iter = pdf.pages_iter();
        assert!(iter.next().is_some()); // First page
        assert!(iter.next().is_none()); // Exhausted
        assert!(iter.next().is_none()); // Still exhausted
    }

    #[test]
    fn pages_iter_size_hint() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let mut iter = pdf.pages_iter();
        assert_eq!(iter.size_hint(), (2, Some(2)));

        let _ = iter.next();
        assert_eq!(iter.size_hint(), (1, Some(1)));

        let _ = iter.next();
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }

    #[test]
    fn pages_iter_preserves_doctop() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let pages: Vec<_> = pdf.pages_iter().collect::<Result<Vec<_>, _>>().unwrap();

        // Page 0: doctop == bbox.top
        let c0 = &pages[0].chars()[0];
        assert!((c0.doctop - c0.bbox.top).abs() < 0.01);

        // Page 1: doctop == bbox.top + page0.height
        let c1 = &pages[1].chars()[0];
        let expected = c1.bbox.top + pages[0].height();
        assert!(
            (c1.doctop - expected).abs() < 0.01,
            "page 1 doctop {} expected {}",
            c1.doctop,
            expected
        );
    }

    #[test]
    fn page_independence_no_shared_state() {
        // Processing page 1 should not affect page 0's data
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let page0_first = pdf.page(0).unwrap();
        let chars0_before = page0_first.chars().len();
        let text0_before = page0_first.extract_text(&TextOptions::default());

        // Process page 1
        let _page1 = pdf.page(1).unwrap();

        // Process page 0 again — should get identical results
        let page0_second = pdf.page(0).unwrap();
        assert_eq!(page0_second.chars().len(), chars0_before);
        assert_eq!(
            page0_second.extract_text(&TextOptions::default()),
            text0_before
        );
    }

    #[test]
    fn page_data_released_on_drop() {
        // Verify pages are independent owned values — dropping one doesn't
        // affect subsequent page calls
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        {
            let page0 = pdf.page(0).unwrap();
            assert!(!page0.chars().is_empty());
            // page0 dropped here
        }

        // Can still create a new page after the previous one is dropped
        let page0_again = pdf.page(0).unwrap();
        assert!(!page0_again.chars().is_empty());

        let page1 = pdf.page(1).unwrap();
        assert!(!page1.chars().is_empty());
    }

    #[test]
    fn streaming_iteration_drops_previous_pages() {
        // Simulates streaming: process one page at a time, dropping previous
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let mut last_page_number = None;
        for result in pdf.pages_iter() {
            let page = result.unwrap();
            // Each page is independent — we can extract text
            let _text = page.extract_text(&TextOptions::default());
            last_page_number = Some(page.page_number());
            // page is dropped at end of loop iteration
        }

        assert_eq!(last_page_number, Some(1));
    }

    #[test]
    fn page_count_available_without_processing() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        // page_count should work without calling page() at all
        assert_eq!(pdf.page_count(), 2);
    }

    #[test]
    fn pages_iter_can_be_partially_consumed() {
        let bytes = create_two_page_pdf();
        let pdf = Pdf::open(&bytes, None).unwrap();

        // Only consume first page from iterator
        let mut iter = pdf.pages_iter();
        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.page_number(), 0);

        // Don't consume the rest — iterator is just dropped
        // This should not cause any issues
        drop(iter);

        // Pdf is still usable
        let page1 = pdf.page(1).unwrap();
        assert!(!page1.chars().is_empty());
    }

    // --- US-047: WASM build support tests ---

    #[cfg(feature = "std")]
    mod std_feature_tests {
        use super::*;

        #[test]
        fn open_file_reads_valid_pdf() {
            // Write a PDF to a temp file, then open via open_file
            let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (FileTest) Tj ET");
            let dir = std::env::temp_dir();
            let path = dir.join("pdfplumber_test_open_file.pdf");
            std::fs::write(&path, &bytes).unwrap();

            let pdf = Pdf::open_file(&path, None).unwrap();
            assert_eq!(pdf.page_count(), 1);

            let page = pdf.page(0).unwrap();
            let text = page.extract_text(&TextOptions::default());
            assert!(text.contains("FileTest"));

            // Clean up
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn open_file_nonexistent_returns_error() {
            let result = Pdf::open_file("/nonexistent/path/to/file.pdf", None);
            assert!(result.is_err());
        }

        #[test]
        fn open_file_matches_open_bytes() {
            let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Match) Tj ET");
            let dir = std::env::temp_dir();
            let path = dir.join("pdfplumber_test_match.pdf");
            std::fs::write(&path, &bytes).unwrap();

            let pdf_bytes = Pdf::open(&bytes, None).unwrap();
            let pdf_file = Pdf::open_file(&path, None).unwrap();

            assert_eq!(pdf_bytes.page_count(), pdf_file.page_count());

            let page_bytes = pdf_bytes.page(0).unwrap();
            let page_file = pdf_file.page(0).unwrap();

            assert_eq!(page_bytes.chars().len(), page_file.chars().len());
            for (a, b) in page_bytes.chars().iter().zip(page_file.chars().iter()) {
                assert_eq!(a.text, b.text);
                assert_eq!(a.char_code, b.char_code);
            }

            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn bytes_api_works_without_filesystem() {
        // Verify the bytes-based API works — this is the WASM-compatible path
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (WasmOK) Tj ET");
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        let text = page.extract_text(&TextOptions::default());
        assert!(text.contains("WasmOK"));
    }

    // --- extract_image_content tests ---

    /// Helper: create a PDF with a raw image XObject.
    fn create_pdf_with_image() -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // 2x2 RGB image (12 bytes)
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

        let page_content = b"q 200 0 0 150 100 300 cm /Im0 Do Q";
        let page_stream = Stream::new(lopdf::Dictionary::new(), page_content.to_vec());
        let content_id = doc.add_object(Object::Stream(page_stream));

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => Object::Dictionary(dictionary! {
                "XObject" => Object::Dictionary(dictionary! {
                    "Im0" => image_id,
                }),
            }),
        };
        let page_id = doc.add_object(page_dict);

        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1i64,
        });

        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn extract_image_content_returns_raw_bytes() {
        let bytes = create_pdf_with_image();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let content = pdf.extract_image_content(0, "Im0").unwrap();
        assert_eq!(content.format, pdfplumber_core::ImageFormat::Raw);
        assert_eq!(content.width, 2);
        assert_eq!(content.height, 2);
        assert_eq!(
            content.data,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]
        );
    }

    #[test]
    fn extract_image_content_not_found_error() {
        let bytes = create_pdf_with_image();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let result = pdf.extract_image_content(0, "NonExistent");
        assert!(result.is_err());
    }

    #[test]
    fn extract_images_with_content_returns_pairs() {
        let bytes = create_pdf_with_image();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let pairs = pdf.extract_images_with_content(0).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0.name, "Im0");
        assert_eq!(pairs[0].1.format, pdfplumber_core::ImageFormat::Raw);
        assert_eq!(pairs[0].1.data.len(), 12);
    }

    #[test]
    fn extract_image_content_page_out_of_range() {
        let bytes = create_pdf_with_image();
        let pdf = Pdf::open(&bytes, None).unwrap();

        let result = pdf.extract_image_content(99, "Im0");
        assert!(result.is_err());
    }

    // --- Image data opt-in tests ---

    fn create_pdf_with_jpeg_image() -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // Minimal JPEG-like data (starts with SOI marker)
        let jpeg_data = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
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

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => Object::Dictionary(dictionary! {
                "XObject" => Object::Dictionary(dictionary! {
                    "Im0" => image_id,
                }),
            }),
        };
        let page_id = doc.add_object(page_dict);

        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1i64,
        });

        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn image_data_not_extracted_by_default() {
        let bytes = create_pdf_with_image();
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].data, None);
        // Filter and mime_type should still be set (no filter = no filter info)
        assert_eq!(images[0].filter, None);
        assert_eq!(images[0].mime_type, None);
    }

    #[test]
    fn image_data_extracted_when_opt_in() {
        let bytes = create_pdf_with_image();
        let opts = ExtractOptions {
            extract_image_data: true,
            ..ExtractOptions::default()
        };
        let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);
        assert!(images[0].data.is_some());
        let data = images[0].data.as_ref().unwrap();
        // 2x2 RGB image = 12 bytes
        assert_eq!(data.len(), 12);
        assert_eq!(data, &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
    }

    #[test]
    fn jpeg_image_filter_and_mime_type() {
        let bytes = create_pdf_with_jpeg_image();
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].filter, Some(ImageFilter::DCTDecode));
        assert_eq!(images[0].mime_type, Some("image/jpeg".to_string()));
        // Data not extracted by default
        assert_eq!(images[0].data, None);
    }

    #[test]
    fn jpeg_image_data_extracted_as_is() {
        let bytes = create_pdf_with_jpeg_image();
        let opts = ExtractOptions {
            extract_image_data: true,
            ..ExtractOptions::default()
        };
        let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);
        assert!(images[0].data.is_some());
        let data = images[0].data.as_ref().unwrap();
        // JPEG data starts with SOI marker
        assert!(data.starts_with(&[0xFF, 0xD8]));
        assert_eq!(images[0].filter, Some(ImageFilter::DCTDecode));
        assert_eq!(images[0].mime_type, Some("image/jpeg".to_string()));
    }

    // --- Inline image tests ---

    fn create_pdf_with_inline_image() -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // Content stream with an inline image: 2x2 RGB, 8 bpc
        let mut content = Vec::new();
        content.extend_from_slice(b"q 200 0 0 150 100 300 cm BI /W 2 /H 2 /CS /RGB /BPC 8 ID ");
        // 2x2 RGB = 12 bytes of pixel data
        content.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]);
        content.extend_from_slice(b" EI Q");

        let page_stream = Stream::new(lopdf::Dictionary::new(), content);
        let content_id = doc.add_object(Object::Stream(page_stream));

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => Object::Dictionary(lopdf::Dictionary::new()),
        };
        let page_id = doc.add_object(page_dict);

        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1i64,
        });

        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn inline_image_appears_in_page_images() {
        let bytes = create_pdf_with_inline_image();
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);

        let img = &images[0];
        assert_eq!(img.src_width, Some(2));
        assert_eq!(img.src_height, Some(2));
        assert_eq!(img.color_space, Some("DeviceRGB".to_string()));
        assert_eq!(img.bits_per_component, Some(8));
        // Should have correct position from CTM
        assert!(img.width > 0.0);
        assert!(img.height > 0.0);
    }

    #[test]
    fn inline_image_with_abbreviated_colorspace() {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.5");

        // Use abbreviated key /G for DeviceGray
        let mut content = Vec::new();
        content.extend_from_slice(b"q 100 0 0 100 50 50 cm BI /W 1 /H 1 /CS /G /BPC 8 ID ");
        content.push(200); // 1x1 gray = 1 byte
        content.extend_from_slice(b" EI Q");

        let page_stream = Stream::new(lopdf::Dictionary::new(), content);
        let content_id = doc.add_object(Object::Stream(page_stream));

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => Object::Dictionary(lopdf::Dictionary::new()),
        };
        let page_id = doc.add_object(page_dict);

        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1i64,
        });

        if let Ok(page_obj) = doc.get_object_mut(page_id) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();

        let pdf = Pdf::open(&buf, None).unwrap();
        let page = pdf.page(0).unwrap();

        let images = page.images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].color_space, Some("DeviceGray".to_string()));
    }

    // --- Encrypted PDF facade tests ---

    /// PDF standard padding bytes.
    const PAD_BYTES: [u8; 32] = [
        0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01,
        0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64, 0x53,
        0x69, 0x7A,
    ];

    /// Simple RC4 for test encryption.
    fn rc4_transform(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut s: Vec<u8> = (0..=255).collect();
        let mut j: usize = 0;
        for i in 0..256 {
            j = (j + s[i] as usize + key[i % key.len()] as usize) & 0xFF;
            s.swap(i, j);
        }
        let mut out = Vec::with_capacity(data.len());
        let mut i: usize = 0;
        j = 0;
        for &byte in data {
            i = (i + 1) & 0xFF;
            j = (j + s[i] as usize) & 0xFF;
            s.swap(i, j);
            out.push(byte ^ s[(s[i] as usize + s[j] as usize) & 0xFF]);
        }
        out
    }

    /// Create an encrypted PDF with user password for facade tests.
    fn create_encrypted_pdf(user_password: &[u8]) -> Vec<u8> {
        use lopdf::{Object, Stream, StringFormat, dictionary};

        let file_id = b"testfileid123456";
        let permissions: i32 = -4;

        let mut padded_pw = Vec::with_capacity(32);
        let pw_len = user_password.len().min(32);
        padded_pw.extend_from_slice(&user_password[..pw_len]);
        padded_pw.extend_from_slice(&PAD_BYTES[..32 - pw_len]);

        let o_key_digest = md5::compute(&padded_pw);
        let o_key = &o_key_digest[..5];
        let o_value = rc4_transform(o_key, &padded_pw);

        let mut key_input = Vec::with_capacity(128);
        key_input.extend_from_slice(&padded_pw);
        key_input.extend_from_slice(&o_value);
        key_input.extend_from_slice(&(permissions as u32).to_le_bytes());
        key_input.extend_from_slice(file_id);
        let key_digest = md5::compute(&key_input);
        let enc_key = key_digest[..5].to_vec();

        let u_value = rc4_transform(&enc_key, &PAD_BYTES);

        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id: lopdf::ObjectId = doc.new_object_id();

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

        // Encrypt objects
        for (&obj_id, obj) in doc.objects.iter_mut() {
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
                    *content = rc4_transform(obj_key, content);
                }
                _ => {}
            }
        }

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
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::String(file_id.to_vec(), StringFormat::Literal),
                Object::String(file_id.to_vec(), StringFormat::Literal),
            ]),
        );

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("failed to save encrypted PDF");
        buf
    }

    #[test]
    fn pdf_open_encrypted_without_password_returns_password_required() {
        let bytes = create_encrypted_pdf(b"testpass");
        let result = Pdf::open(&bytes, None);
        match result {
            Err(PdfError::PasswordRequired) => {} // expected
            Err(e) => panic!("expected PasswordRequired, got: {e}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn pdf_open_with_password_correct() {
        let password = b"testpass";
        let bytes = create_encrypted_pdf(password);
        let pdf = Pdf::open_with_password(&bytes, password, None).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }

    #[test]
    fn pdf_open_with_password_wrong_returns_invalid_password() {
        let bytes = create_encrypted_pdf(b"testpass");
        let result = Pdf::open_with_password(&bytes, b"wrongpass", None);
        match result {
            Err(PdfError::InvalidPassword) => {} // expected
            Err(e) => panic!("expected InvalidPassword, got: {e}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn pdf_open_with_password_unencrypted_ignores_password() {
        let bytes = create_pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hi) Tj ET");
        let pdf = Pdf::open_with_password(&bytes, b"anypassword", None).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }
}
