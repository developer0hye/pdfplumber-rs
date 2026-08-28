//! Top-level PDF document type for opening and extracting content.

use std::sync::atomic::{AtomicUsize, Ordering};

use pdfplumber_core::{
    BBox, Bookmark, Char, Color, Ctm, Curve, DashPattern, DocumentMetadata, ExtractOptions,
    ExtractWarning, FormField, Image, ImageContent, ImageFilter, ImageMetadata, Line, Orientation,
    PageRegionOptions, PageRegions, PaintedPath, Path, PdfError, RawDocumentMetadata, Rect,
    RepairOptions, RepairResult, SearchMatch, SearchOptions, ShapeKind, SignatureInfo,
    StructElement, TextDirection, TextOptions, UnicodeNorm, ValidationIssue, apply_bidi_directions,
    dedupe_chars, detect_page_regions, extract_shapes_with_order, image_from_ctm, normalize_chars,
};
use pdfplumber_parse::{
    CharEvent, ContentHandler, ImageEvent, LopdfBackend, LopdfDocument, PageGeometry, PaintOp,
    PathEvent, PdfBackend, char_from_event,
};

use crate::{Page, PageObjectKind};

#[cfg(test)]
use pdfplumber_core::PdfErrorKind;

/// A borrowed collection view over the pages in a [`Pdf`].
///
/// Created by [`Pdf::pages`]. The view borrows the document and does not clone
/// it or extract any page content. Use [`Pages::get`] for direct zero-based
/// selection or iterate over the view to process pages on demand.
/// `Pages` is `Send + Sync` when its borrowed [`Pdf`] is, but cannot outlive
/// that document.
///
/// # Compile-time diagnostics
///
/// The view implements `IntoIterator` rather than `Iterator`. It works directly
/// in a `for` loop, but iterator adapters require an explicit `into_iter()`.
/// Calling an adapter on the view itself does not compile:
///
/// ```compile_fail
/// use pdfplumber::Pdf;
///
/// fn map_pages_directly(pdf: &Pdf) {
///     let _ = pdf.pages().map(|page| page.map(|page| page.page_number()));
/// }
/// ```
///
/// Convert the view before using an adapter:
///
/// ```no_run
/// use pdfplumber::{Pdf, PdfError};
///
/// fn collect_page_numbers(pdf: &Pdf) -> Result<Vec<usize>, PdfError> {
///     pdf.pages()
///         .into_iter()
///         .map(|page| page.map(|page| page.page_number()))
///         .collect()
/// }
/// ```
///
/// A borrowed `Pages<'_>` cannot outlive its source document, so returning a
/// view created from a local `Pdf` does not compile:
///
/// ```compile_fail
/// use pdfplumber::{Pages, Pdf, PdfError};
///
/// fn dangling_pages(bytes: &[u8]) -> Result<Pages<'_>, PdfError> {
///     let pdf = Pdf::open_bytes(bytes, None)?;
///     Ok(pdf.pages())
/// }
/// ```
///
/// Return an owned [`Page`] when the result must outlive the document:
///
/// ```no_run
/// use pdfplumber::{Page, Pdf, PdfError};
///
/// fn first_page(bytes: &[u8]) -> Result<Page, PdfError> {
///     let pdf = Pdf::open_bytes(bytes, None)?;
///     pdf.pages().get(0)
/// }
/// ```
#[derive(Clone, Copy)]
pub struct Pages<'a> {
    pdf: &'a Pdf,
}

impl<'a> Pages<'a> {
    /// Return the number of pages without interpreting their content streams.
    pub fn len(&self) -> usize {
        self.pdf.page_count()
    }

    /// Return `true` when the document contains no pages.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Extract one page by zero-based index without processing earlier pages.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the index is out of range or the selected page's
    /// content cannot be interpreted.
    pub fn get(&self, index: usize) -> Result<Page, PdfError> {
        self.pdf.page(index)
    }

    /// Return a lazy iterator over all pages from front to back.
    ///
    /// The iterator also implements [`DoubleEndedIterator`], so callers can
    /// select from the back without processing earlier pages.
    pub fn iter(&self) -> PagesIter<'a> {
        PagesIter::new(self.pdf)
    }
}

impl<'a> IntoIterator for Pages<'a> {
    type Item = Result<Page, PdfError>;
    type IntoIter = PagesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over pages of a PDF document, yielding each page on demand.
///
/// Created by [`Pages::iter`] or [`Pages::into_iter`]. Each call to
/// [`next()`](Iterator::next) or [`next_back()`](DoubleEndedIterator::next_back)
/// processes one page from the PDF content stream. Pages are not retained after
/// being yielded — the caller owns the [`Page`] value.
pub struct PagesIter<'a> {
    pdf: &'a Pdf,
    front: usize,
    back: usize,
}

impl<'a> PagesIter<'a> {
    fn new(pdf: &'a Pdf) -> Self {
        Self {
            pdf,
            front: 0,
            back: pdf.page_count(),
        }
    }
}

impl<'a> Iterator for PagesIter<'a> {
    type Item = Result<Page, PdfError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let result = self.pdf.page(self.front);
        self.front += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for PagesIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        Some(self.pdf.page(self.back))
    }
}

impl ExactSizeIterator for PagesIter<'_> {}
impl std::iter::FusedIterator for PagesIter<'_> {}

/// A PDF document opened for extraction.
///
/// Wraps a parsed PDF and provides methods to access pages and extract content.
/// `Pdf` is `Send + Sync`; callers may share it through [`std::sync::Arc`] for
/// concurrent immutable extraction. Object and image-byte resource budgets are
/// document-wide and count every page that reaches resource accounting,
/// including repeated pages.
///
/// # Example
///
/// ```ignore
/// let pdf = Pdf::open_bytes(bytes, None)?;
/// let page = pdf.page(0)?;
/// let text = page.extract_text(&TextOptions::default());
/// ```
pub struct Pdf {
    doc: LopdfDocument,
    options: ExtractOptions,
    /// Cached display widths for creating lazy page handles.
    page_widths: Vec<f64>,
    /// Cached display heights for each page (for doctop calculation).
    page_heights: Vec<f64>,
    /// Cached normalized rotations for creating lazy page handles.
    page_rotations: Vec<i32>,
    /// Cached source MediaBoxes for creating lazy page handles.
    page_media_boxes: Vec<BBox>,
    /// Cached source MediaBox integer/real kinds for compatibility adapters.
    page_media_box_integer_flags: Vec<[bool; 4]>,
    /// Cached inherited source CropBoxes for creating lazy page handles.
    page_crop_boxes: Vec<Option<BBox>>,
    /// Cached source CropBox integer/real kinds for compatibility adapters.
    page_crop_box_integer_flags: Vec<Option<[bool; 4]>>,
    /// Cached source TrimBoxes defined directly on each page.
    page_trim_boxes: Vec<Option<BBox>>,
    /// Cached source TrimBox integer/real kinds for compatibility adapters.
    page_trim_box_integer_flags: Vec<Option<[bool; 4]>>,
    /// Cached source BleedBoxes defined directly on each page.
    page_bleed_boxes: Vec<Option<BBox>>,
    /// Cached source BleedBox integer/real kinds for compatibility adapters.
    page_bleed_box_integer_flags: Vec<Option<[bool; 4]>>,
    /// Cached source ArtBoxes defined directly on each page.
    page_art_boxes: Vec<Option<BBox>>,
    /// Cached source ArtBox integer/real kinds for compatibility adapters.
    page_art_box_integer_flags: Vec<Option<[bool; 4]>>,
    /// Cached raw PDF (MediaBox) heights for y-flip in char extraction.
    raw_page_heights: Vec<f64>,
    /// Cached document metadata from the /Info dictionary.
    metadata: DocumentMetadata,
    /// Cached source-ordered document information dictionary.
    raw_metadata: RawDocumentMetadata,
    /// Cached document bookmarks (outline / table of contents).
    bookmarks: Vec<Bookmark>,
    /// Cached document structure tree from /StructTreeRoot.
    structure_tree: Vec<StructElement>,
    /// Accumulated total objects extracted across all pages (for max_total_objects budget).
    total_objects: AtomicUsize,
    /// Accumulated total image bytes extracted across all pages (for max_total_image_bytes budget).
    total_image_bytes: AtomicUsize,
}

/// Internal handler that collects content stream events during interpretation.
enum CollectedObjectEvent {
    Char,
    Path(usize),
    Image,
}

struct CollectingHandler {
    chars: Vec<CharEvent>,
    paths: Vec<PathEvent>,
    images: Vec<ImageEvent>,
    object_events: Vec<CollectedObjectEvent>,
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
            object_events: Vec::new(),
            warnings: Vec::new(),
            page_index,
            collect_warnings,
        }
    }
}

impl ContentHandler for CollectingHandler {
    fn on_char(&mut self, event: CharEvent) {
        self.object_events.push(CollectedObjectEvent::Char);
        self.chars.push(event);
    }

    fn on_path_painted(&mut self, event: PathEvent) {
        self.object_events
            .push(CollectedObjectEvent::Path(self.paths.len()));
        self.paths.push(event);
    }

    fn on_image(&mut self, event: ImageEvent) {
        self.object_events.push(CollectedObjectEvent::Image);
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

fn operation_error(error: impl Into<PdfError>, operation: &'static str) -> PdfError {
    error.into().during(operation)
}

fn page_error(
    error: impl Into<PdfError>,
    operation: &'static str,
    page_index: usize,
    object_id: Option<(u32, u16)>,
) -> PdfError {
    let mut error = error.into().during(operation).at_page(page_index);
    if let Some((number, generation)) = object_id {
        error = error.at_object(number, generation);
    }
    error
}

impl Pdf {
    /// Open a PDF document from a filesystem path.
    ///
    /// The file is read into memory and closed before this method returns. The
    /// returned `Pdf` owns its parsed document and does not retain the path or a
    /// file handle. For inputs already in memory, use [`Pdf::open_bytes`].
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the PDF file.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns an error with [I/O](crate::PdfErrorKind::Io) kind if the file cannot be opened or read,
    /// [parse](crate::PdfErrorKind::Parse) kind if its contents are not a valid PDF, and the
    /// same password and resource-limit errors as [`Pdf::open_bytes`].
    #[cfg(feature = "std")]
    pub fn open_path(
        path: impl AsRef<std::path::Path>,
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let file = std::fs::File::open(path.as_ref())
            .map_err(|error| operation_error(error, "open path"))?;
        Self::open_reader(file, options)
    }

    /// Open a PDF document from a byte buffer.
    ///
    /// The byte slice is borrowed only for the duration of this call. The
    /// returned `Pdf` owns the parsed document and does not retain or borrow
    /// `bytes`. This method works in all supported environments, including
    /// WebAssembly. For filesystem inputs, use [`Pdf::open_path`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns an error with [password-required](crate::PdfErrorKind::PasswordRequired) kind if the PDF is encrypted with a
    /// non-empty password. PDFs encrypted with an empty user password are
    /// auto-decrypted.
    /// Returns [parse](crate::PdfErrorKind::Parse) kind if the bytes are not a valid PDF
    /// document and [resource-limit](crate::PdfErrorKind::ResourceLimit) kind if `max_input_bytes`
    /// is smaller than the supplied buffer.
    pub fn open_bytes(bytes: &[u8], options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        Self::check_input_limit(bytes.len(), options.as_ref())
            .map_err(|error| error.during("open bytes"))?;
        let doc =
            LopdfBackend::open(bytes).map_err(|error| operation_error(error, "open bytes"))?;
        Self::from_doc(doc, options)
    }

    /// Open a PDF document from a synchronous reader.
    ///
    /// The reader needs only [`std::io::Read`], not `Seek`. It is consumed from
    /// its current position through end-of-file and read into memory. The
    /// returned `Pdf` owns its parsed document and does not retain the reader.
    /// Passing `&mut reader` lets the caller keep using that reader after this
    /// method returns.
    ///
    /// # Arguments
    ///
    /// * `reader` - A byte reader positioned at the start of the PDF data to read.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns an error with [I/O](crate::PdfErrorKind::Io) kind if the reader fails,
    /// [parse](crate::PdfErrorKind::Parse) kind if the bytes read are not a valid PDF, and the
    /// same password and resource-limit errors as [`Pdf::open_bytes`].
    pub fn open_reader<R: std::io::Read>(
        reader: R,
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let bytes = Self::read_input(reader, options.as_ref())?;
        Self::open_bytes(&bytes, options)
    }

    /// Open an encrypted PDF document from a filesystem path with a password.
    ///
    /// The file is read into memory and closed before this method returns. The
    /// returned `Pdf` owns its parsed document and does not retain the path or a
    /// file handle.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the PDF file.
    /// * `password` - The password to decrypt the PDF.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [I/O](crate::PdfErrorKind::Io) kind for file failures,
    /// [invalid-password](crate::PdfErrorKind::InvalidPassword) kind for an incorrect password,
    /// [parse](crate::PdfErrorKind::Parse) kind for invalid PDF data, and
    /// [resource-limit](crate::PdfErrorKind::ResourceLimit) kind for an oversized input.
    #[cfg(feature = "std")]
    pub fn open_path_with_password(
        path: impl AsRef<std::path::Path>,
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let file = std::fs::File::open(path.as_ref())
            .map_err(|error| operation_error(error, "open path with password"))?;
        Self::open_reader_with_password(file, password, options)
    }

    /// Open an encrypted PDF document from a byte buffer with a password.
    ///
    /// Supports both user and owner passwords. If the PDF is not encrypted,
    /// the password is ignored and the document opens normally. The byte slice
    /// is borrowed only for this call; the returned `Pdf` does not retain it.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `password` - The password to decrypt the PDF.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [invalid-password](crate::PdfErrorKind::InvalidPassword) kind if the password is incorrect,
    /// [parse](crate::PdfErrorKind::Parse) kind if the bytes are not a valid PDF, and
    /// [resource-limit](crate::PdfErrorKind::ResourceLimit) kind for an oversized input.
    pub fn open_bytes_with_password(
        bytes: &[u8],
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        Self::check_input_limit(bytes.len(), options.as_ref())
            .map_err(|error| error.during("open bytes with password"))?;
        let doc = LopdfBackend::open_with_password(bytes, password)
            .map_err(|error| operation_error(error, "open bytes with password"))?;
        Self::from_doc(doc, options)
    }

    /// Open an encrypted PDF document from a synchronous reader with a password.
    ///
    /// Like [`Pdf::open_reader`], this accepts any [`std::io::Read`] source,
    /// consumes it from the current position through end-of-file, and does not
    /// retain the reader after returning.
    ///
    /// # Arguments
    ///
    /// * `reader` - A byte reader positioned at the start of the PDF data to read.
    /// * `password` - The password to decrypt the PDF.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns an error with [I/O](crate::PdfErrorKind::Io) kind if the reader fails,
    /// [invalid-password](crate::PdfErrorKind::InvalidPassword) kind if the password is incorrect,
    /// [parse](crate::PdfErrorKind::Parse) kind for invalid PDF data, and
    /// [resource-limit](crate::PdfErrorKind::ResourceLimit) kind for an oversized input.
    pub fn open_reader_with_password<R: std::io::Read>(
        reader: R,
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        let bytes = Self::read_input(reader, options.as_ref())?;
        Self::open_bytes_with_password(&bytes, password, options)
    }

    /// Compatibility alias for [`Pdf::open_path`].
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn open_file(
        path: impl AsRef<std::path::Path>,
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        Self::open_path(path, options)
    }

    /// Compatibility alias for [`Pdf::open_bytes`].
    #[doc(hidden)]
    pub fn open(bytes: &[u8], options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        Self::open_bytes(bytes, options)
    }

    /// Compatibility alias for [`Pdf::open_bytes_with_password`].
    #[doc(hidden)]
    pub fn open_with_password(
        bytes: &[u8],
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        Self::open_bytes_with_password(bytes, password, options)
    }

    /// Compatibility alias for [`Pdf::open_path_with_password`].
    #[doc(hidden)]
    #[cfg(feature = "std")]
    pub fn open_file_with_password(
        path: impl AsRef<std::path::Path>,
        password: &[u8],
        options: Option<ExtractOptions>,
    ) -> Result<Self, PdfError> {
        Self::open_path_with_password(path, password, options)
    }

    /// Open PDF bytes with best-effort repair of common issues.
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
    pub fn open_bytes_with_repair(
        bytes: &[u8],
        options: Option<ExtractOptions>,
        repair_opts: Option<RepairOptions>,
    ) -> Result<(Self, RepairResult), PdfError> {
        Self::check_input_limit(bytes.len(), options.as_ref())
            .map_err(|error| error.during("repair PDF bytes"))?;
        let repair_opts = repair_opts.unwrap_or_default();
        let (repaired_bytes, result) = LopdfBackend::repair(bytes, &repair_opts)
            .map_err(|error| operation_error(error, "repair PDF bytes"))?;
        let pdf = Self::open_bytes(&repaired_bytes, options)?;
        Ok((pdf, result))
    }

    /// Compatibility alias for [`Pdf::open_bytes_with_repair`].
    #[doc(hidden)]
    pub fn open_with_repair(
        bytes: &[u8],
        options: Option<ExtractOptions>,
        repair_opts: Option<RepairOptions>,
    ) -> Result<(Self, RepairResult), PdfError> {
        Self::open_bytes_with_repair(bytes, options, repair_opts)
    }

    fn check_input_limit(
        actual_value: usize,
        options: Option<&ExtractOptions>,
    ) -> Result<(), PdfError> {
        if let Some(max_bytes) = options.and_then(|options| options.max_input_bytes)
            && actual_value > max_bytes
        {
            return Err(PdfError::limit_exceeded(
                "max_input_bytes",
                max_bytes,
                actual_value,
            ));
        }
        Ok(())
    }

    fn read_input<R: std::io::Read>(
        reader: R,
        options: Option<&ExtractOptions>,
    ) -> Result<Vec<u8>, PdfError> {
        let mut bytes = Vec::new();
        if let Some(max_bytes) = options.and_then(|options| options.max_input_bytes) {
            let observed_limit = max_bytes.saturating_add(1);
            let read_limit = u64::try_from(observed_limit).unwrap_or(u64::MAX);
            let mut limited = std::io::Read::take(reader, read_limit);
            std::io::Read::read_to_end(&mut limited, &mut bytes)
                .map_err(|error| operation_error(error, "read input"))?;
            Self::check_input_limit(bytes.len(), options)
                .map_err(|error| error.during("read input"))?;
        } else {
            let mut reader = reader;
            std::io::Read::read_to_end(&mut reader, &mut bytes)
                .map_err(|error| operation_error(error, "read input"))?;
        }
        Ok(bytes)
    }

    /// Internal helper to construct a `Pdf` from a loaded `LopdfDocument`.
    fn from_doc(doc: LopdfDocument, options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        let options = options.unwrap_or_default();

        // Cache page heights for doctop calculation
        let page_count = LopdfBackend::page_count(&doc);

        // Check max_pages before processing
        if let Some(max_pages) = options.max_pages
            && page_count > max_pages
        {
            return Err(PdfError::limit_exceeded("max_pages", max_pages, page_count));
        }

        let mut page_widths = Vec::with_capacity(page_count);
        let mut page_heights = Vec::with_capacity(page_count);
        let mut page_rotations = Vec::with_capacity(page_count);
        let mut page_media_boxes = Vec::with_capacity(page_count);
        let mut page_media_box_integer_flags = Vec::with_capacity(page_count);
        let mut page_crop_boxes = Vec::with_capacity(page_count);
        let mut page_crop_box_integer_flags = Vec::with_capacity(page_count);
        let mut page_trim_boxes = Vec::with_capacity(page_count);
        let mut page_trim_box_integer_flags = Vec::with_capacity(page_count);
        let mut page_bleed_boxes = Vec::with_capacity(page_count);
        let mut page_bleed_box_integer_flags = Vec::with_capacity(page_count);
        let mut page_art_boxes = Vec::with_capacity(page_count);
        let mut page_art_box_integer_flags = Vec::with_capacity(page_count);
        let mut raw_page_heights = Vec::with_capacity(page_count);

        for i in 0..page_count {
            let page = LopdfBackend::get_page(&doc, i)
                .map_err(|error| page_error(error, "load page", i, None))?;
            let object_id = Some(page.object_id);
            let media_box = LopdfBackend::page_media_box(&doc, &page)
                .map_err(|error| page_error(error, "read page media box", i, object_id))?;
            let media_box_integer_flags = LopdfBackend::page_media_box_integer_flags(&doc, &page)
                .map_err(|error| {
                page_error(error, "read page media box number types", i, object_id)
            })?;
            let crop_box = LopdfBackend::page_crop_box(&doc, &page)
                .map_err(|error| page_error(error, "read page crop box", i, object_id))?;
            let crop_box_integer_flags = LopdfBackend::page_crop_box_integer_flags(&doc, &page)
                .map_err(|error| {
                    page_error(error, "read page crop box number types", i, object_id)
                })?;
            let trim_box = LopdfBackend::page_explicit_trim_box(&doc, &page)
                .map_err(|error| page_error(error, "read page trim box", i, object_id))?;
            let trim_box_integer_flags = LopdfBackend::page_explicit_trim_box_integer_flags(
                &doc, &page,
            )
            .map_err(|error| page_error(error, "read page trim box number types", i, object_id))?;
            let bleed_box = LopdfBackend::page_explicit_bleed_box(&doc, &page)
                .map_err(|error| page_error(error, "read page bleed box", i, object_id))?;
            let bleed_box_integer_flags = LopdfBackend::page_explicit_bleed_box_integer_flags(
                &doc, &page,
            )
            .map_err(|error| page_error(error, "read page bleed box number types", i, object_id))?;
            let art_box = LopdfBackend::page_explicit_art_box(&doc, &page)
                .map_err(|error| page_error(error, "read page art box", i, object_id))?;
            let art_box_integer_flags = LopdfBackend::page_explicit_art_box_integer_flags(
                &doc, &page,
            )
            .map_err(|error| page_error(error, "read page art box number types", i, object_id))?;
            let rotation = LopdfBackend::page_rotate(&doc, &page)
                .map_err(|error| page_error(error, "read page rotation", i, object_id))?;
            // Use MediaBox (not CropBox) for page dimensions to match Python pdfplumber.
            // CropBox is stored as page metadata but does not affect coordinate transforms.
            let geometry = PageGeometry::new(media_box, None, rotation);
            page_widths.push(geometry.width());
            page_heights.push(geometry.height());
            page_rotations.push(geometry.rotation());
            page_media_boxes.push(media_box);
            page_media_box_integer_flags.push(media_box_integer_flags);
            page_crop_boxes.push(crop_box);
            page_crop_box_integer_flags.push(crop_box_integer_flags);
            page_trim_boxes.push(trim_box);
            page_trim_box_integer_flags.push(trim_box_integer_flags);
            page_bleed_boxes.push(bleed_box);
            page_bleed_box_integer_flags.push(bleed_box_integer_flags);
            page_art_boxes.push(art_box);
            page_art_box_integer_flags.push(art_box_integer_flags);
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
        let metadata = LopdfBackend::document_metadata(&doc)
            .map_err(|error| operation_error(error, "read document metadata"))?;
        let raw_metadata = LopdfBackend::raw_document_metadata(&doc);

        // Extract document bookmarks (outline / table of contents)
        let bookmarks = LopdfBackend::document_bookmarks(&doc)
            .map_err(|error| operation_error(error, "read document bookmarks"))?;

        // Extract the document structure tree once for document and page views.
        let structure_tree = LopdfBackend::document_structure_tree(&doc)
            .map_err(|error| operation_error(error, "read document structure tree"))?;

        Ok(Self {
            doc,
            options,
            page_widths,
            page_heights,
            page_rotations,
            page_media_boxes,
            page_media_box_integer_flags,
            page_crop_boxes,
            page_crop_box_integer_flags,
            page_trim_boxes,
            page_trim_box_integer_flags,
            page_bleed_boxes,
            page_bleed_box_integer_flags,
            page_art_boxes,
            page_art_box_integer_flags,
            raw_page_heights,
            metadata,
            raw_metadata,
            bookmarks,
            structure_tree,
            total_objects: AtomicUsize::new(0),
            total_image_bytes: AtomicUsize::new(0),
        })
    }

    /// Return the number of pages in the document.
    pub fn page_count(&self) -> usize {
        LopdfBackend::page_count(&self.doc)
    }

    /// Return a page's display dimensions without interpreting its content stream.
    ///
    /// `index` is zero-based. The returned `(width, height)` values are PDF
    /// points after page rotation. Returns `None` when the index is out of
    /// range.
    pub fn page_dimensions(&self, index: usize) -> Option<(f64, f64)> {
        self.page_widths
            .get(index)
            .zip(self.page_heights.get(index))
            .map(|(width, height)| (*width, *height))
    }

    /// Return a page's normalized rotation without interpreting its content stream.
    ///
    /// `index` is zero-based. The angle is in clockwise degrees normalized to
    /// `0..360`; `None` means the index is out of range.
    pub fn page_rotation(&self, index: usize) -> Option<i32> {
        self.page_rotations.get(index).copied()
    }

    /// Return a page's source MediaBox without interpreting its content stream.
    ///
    /// `index` is zero-based. Returns `None` when the index is out of range.
    pub fn page_media_box(&self, index: usize) -> Option<BBox> {
        self.page_media_boxes.get(index).copied()
    }

    /// Return which source MediaBox coordinates were PDF integers.
    ///
    /// Flags follow `[x0, top, x1, bottom]` order. `index` is zero-based, and
    /// `None` means it is out of range.
    pub fn page_media_box_integer_flags(&self, index: usize) -> Option<[bool; 4]> {
        self.page_media_box_integer_flags.get(index).copied()
    }

    /// Return a page's inherited source CropBox without interpreting its content stream.
    ///
    /// `index` is zero-based. Returns `None` when the page has no inherited
    /// CropBox or when the index is out of range.
    pub fn page_crop_box(&self, index: usize) -> Option<BBox> {
        self.page_crop_boxes.get(index).copied().flatten()
    }

    /// Return which source CropBox coordinates were PDF integers.
    ///
    /// Flags follow `[x0, top, x1, bottom]` order. Returns `None` when the page
    /// has no inherited CropBox or the zero-based index is out of range.
    pub fn page_crop_box_integer_flags(&self, index: usize) -> Option<[bool; 4]> {
        self.page_crop_box_integer_flags
            .get(index)
            .copied()
            .flatten()
    }

    /// Return a page's explicit source TrimBox without interpreting its content stream.
    ///
    /// Returns `None` when the page does not define one or the zero-based index
    /// is out of range.
    pub fn page_trim_box(&self, index: usize) -> Option<BBox> {
        self.page_trim_boxes.get(index).copied().flatten()
    }

    /// Return which source TrimBox coordinates were PDF integers.
    ///
    /// Flags follow `[x0, top, x1, bottom]` order. Returns `None` when the page
    /// has no explicit TrimBox or the zero-based index is out of range.
    pub fn page_trim_box_integer_flags(&self, index: usize) -> Option<[bool; 4]> {
        self.page_trim_box_integer_flags
            .get(index)
            .copied()
            .flatten()
    }

    /// Return a page's explicit source BleedBox without interpreting its content stream.
    ///
    /// Returns `None` when the page does not define one or the zero-based index
    /// is out of range.
    pub fn page_bleed_box(&self, index: usize) -> Option<BBox> {
        self.page_bleed_boxes.get(index).copied().flatten()
    }

    /// Return which source BleedBox coordinates were PDF integers.
    ///
    /// Flags follow `[x0, top, x1, bottom]` order. Returns `None` when the page
    /// has no explicit BleedBox or the zero-based index is out of range.
    pub fn page_bleed_box_integer_flags(&self, index: usize) -> Option<[bool; 4]> {
        self.page_bleed_box_integer_flags
            .get(index)
            .copied()
            .flatten()
    }

    /// Return a page's explicit source ArtBox without interpreting its content stream.
    ///
    /// Returns `None` when the page does not define one or the zero-based index
    /// is out of range.
    pub fn page_art_box(&self, index: usize) -> Option<BBox> {
        self.page_art_boxes.get(index).copied().flatten()
    }

    /// Return which source ArtBox coordinates were PDF integers.
    ///
    /// Flags follow `[x0, top, x1, bottom]` order. Returns `None` when the page
    /// has no explicit ArtBox or the zero-based index is out of range.
    pub fn page_art_box_integer_flags(&self, index: usize) -> Option<[bool; 4]> {
        self.page_art_box_integer_flags
            .get(index)
            .copied()
            .flatten()
    }

    /// Return the document metadata from the PDF /Info dictionary.
    ///
    /// Returns a reference to the cached [`DocumentMetadata`] containing
    /// title, author, subject, keywords, creator, producer, and dates.
    /// Fields not present in the PDF are `None`.
    pub fn metadata(&self) -> &DocumentMetadata {
        &self.metadata
    }

    /// Return the complete source-ordered document information dictionary.
    ///
    /// Unlike [`Pdf::metadata`], this retains arbitrary keys, recursively
    /// decoded arrays and dictionaries, and per-entry resolution failures.
    pub fn raw_metadata(&self) -> &RawDocumentMetadata {
        &self.raw_metadata
    }

    /// Validate that the raw document information dictionary has no
    /// indirect-reference cycles.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] when the metadata graph is malformed, including
    /// an indirect-reference cycle or an unresolvable referenced object.
    pub fn validate_metadata(&self) -> Result<(), PdfError> {
        LopdfBackend::validate_document_metadata(&self.doc)
            .map_err(|error| operation_error(error, "validate document metadata"))
    }

    /// Return the document bookmarks (outline / table of contents).
    ///
    /// Returns a slice of [`Bookmark`]s representing the flattened outline
    /// tree, with each bookmark's `level` indicating nesting depth.
    /// Returns an empty slice if the document has no outlines.
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Return the cached document structure tree.
    ///
    /// Returns an empty slice when the PDF has no `/StructTreeRoot`.
    pub fn structure_tree(&self) -> &[StructElement] {
        &self.structure_tree
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
        LopdfBackend::document_form_fields(&self.doc)
            .map_err(|error| operation_error(error, "read document form fields"))
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
        let lopdf_page = LopdfBackend::get_page(&self.doc, page_index)
            .map_err(|error| page_error(error, "load page", page_index, None))?;
        let object_id = Some(lopdf_page.object_id);
        LopdfBackend::extract_image_content(&self.doc, &lopdf_page, image_name)
            .map_err(|error| page_error(error, "extract page image content", page_index, object_id))
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

    /// Return a borrowed collection view for page selection and lazy iteration.
    ///
    /// Creating the view does not interpret page content or clone the document.
    /// [`Pages::get`] extracts one selected page directly, while iterating the
    /// view extracts pages on demand and returns independently owned [`Page`]
    /// values.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pdf = Pdf::open_bytes(bytes, None)?;
    /// let first = pdf.pages().get(0)?;
    /// for result in pdf.pages() {
    ///     let page = result?;
    ///     println!("Page {}: {}", page.page_number(), page.extract_text(&TextOptions::default()));
    ///     // page is dropped at end of loop body
    /// }
    /// ```
    pub fn pages(&self) -> Pages<'_> {
        Pages { pdf: self }
    }

    /// Compatibility shortcut for [`Pages::iter`].
    #[doc(hidden)]
    pub fn pages_iter(&self) -> PagesIter<'_> {
        self.pages().iter()
    }

    /// Process all pages in parallel using rayon, returning a Vec of Results.
    ///
    /// Each page is extracted concurrently. The returned Vec is ordered by page
    /// index (0-based). Page data (doctop offsets, etc.) is computed correctly
    /// regardless of processing order. Every page produces one result; an error
    /// does not cancel extraction of the remaining pages.
    ///
    /// This method uses the current Rayon thread pool (normally the global pool)
    /// and does not configure a pool or worker count. Document-wide resource
    /// budgets are shared by all workers.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pdf = Pdf::open_bytes(bytes, None)?;
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
        let lopdf_page = LopdfBackend::get_page(&self.doc, index)
            .map_err(|error| page_error(error, "load page", index, None))?;
        let object_id = Some(lopdf_page.object_id);

        // Page geometry
        let media_box = LopdfBackend::page_media_box(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page media box", index, object_id))?;
        let crop_box = LopdfBackend::page_crop_box(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page crop box", index, object_id))?;
        let trim_box = LopdfBackend::page_trim_box(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page trim box", index, object_id))?;
        let bleed_box = LopdfBackend::page_bleed_box(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page bleed box", index, object_id))?;
        let art_box = LopdfBackend::page_art_box(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page art box", index, object_id))?;
        let rotation = LopdfBackend::page_rotate(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page rotation", index, object_id))?;
        // Use MediaBox (not CropBox) for coordinate transforms to match Python pdfplumber.
        let geometry = PageGeometry::new(media_box, None, rotation);

        // Interpret page content
        let mut handler = CollectingHandler::new(index, self.options.collect_warnings);
        LopdfBackend::interpret_page(&self.doc, &lopdf_page, &mut handler, &self.options)
            .map_err(|error| page_error(error, "interpret page content", index, object_id))?;

        // Convert CharEvents to Chars
        let page_height = self.raw_page_heights[index];
        let doctop_offset: f64 = self.page_heights[..index].iter().sum();
        let needs_rotation = geometry.rotation() != 0;
        let page_origin = rotated_page_origin(media_box, geometry.rotation());
        let page_layout_ctm = pdfminer_page_ctm(media_box, geometry.rotation());

        let mut chars: Vec<Char> = handler
            .chars
            .iter()
            .map(|event| {
                let mut ch = char_from_event(
                    event,
                    page_height,
                    Some(event.stroking_color.clone()),
                    Some(event.non_stroking_color.clone()),
                );
                let character_matrix = pdfminer_character_matrix(event, page_layout_ctm);
                ch.ctm = [
                    character_matrix.a,
                    character_matrix.b,
                    character_matrix.c,
                    character_matrix.d,
                    character_matrix.e,
                    character_matrix.f,
                ];
                if needs_rotation {
                    let unrotated_width = ch.bbox.width();
                    // char_from_event applied a simple y-flip using the raw page height.
                    // Undo it to recover PDF native coordinates, then apply the full
                    // rotation + y-flip transform via PageGeometry.
                    let native_min_y = page_height - ch.bbox.bottom;
                    let native_max_y = page_height - ch.bbox.top;
                    ch.bbox = offset_bbox(
                        geometry.normalize_bbox(ch.bbox.x0, native_min_y, ch.bbox.x1, native_max_y),
                        page_origin,
                    );
                    // Page rotation is folded into pdfminer's layout matrix before
                    // LTChar measures `size`. Our equivalent page transform runs
                    // later, so recover the same dimension here. For 270-degree
                    // horizontal text, keep the pre-transform width: subtracting
                    // the normalized coordinates again is geometrically equal but
                    // changes the last floating-point bits.
                    if rotation == 90 {
                        ch.size = if event.is_vertical {
                            ch.bbox.width()
                        } else {
                            ch.bbox.height()
                        };
                    } else if rotation == 270 {
                        ch.size = if event.is_vertical {
                            ch.bbox.width()
                        } else {
                            unrotated_width
                        };
                    }
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
        let mut path_object_kinds: Vec<Vec<PageObjectKind>> =
            Vec::with_capacity(handler.paths.len());

        for path_event in &handler.paths {
            let painted = path_event_to_painted_path(path_event);
            let (mut lines, mut rects, mut curves, shape_order) =
                extract_shapes_with_order(&painted, page_height);
            if needs_rotation {
                for line in &mut lines {
                    let bbox = rotate_bbox(
                        line.x0,
                        line.top,
                        line.x1,
                        line.bottom,
                        page_height,
                        &geometry,
                        page_origin,
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
                        page_origin,
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
                        page_origin,
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
                            offset_point(geometry.normalize_point(x, native_y), page_origin)
                        })
                        .collect();
                }
            }
            let kinds = shape_order
                .into_iter()
                .map(|kind| match kind {
                    ShapeKind::Line => PageObjectKind::Line,
                    ShapeKind::Rect => PageObjectKind::Rect,
                    ShapeKind::Curve => PageObjectKind::Curve,
                })
                .collect();
            path_object_kinds.push(kinds);
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
                if self.options.extract_image_data
                    && let Ok(content) =
                        LopdfBackend::extract_image_content(&self.doc, &lopdf_page, &event.name)
                {
                    img.data = Some(content.data);
                }

                if needs_rotation {
                    let bbox = rotate_bbox(
                        img.x0,
                        img.top,
                        img.x1,
                        img.bottom,
                        page_height,
                        &geometry,
                        page_origin,
                    );
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

        let mut object_order = Vec::new();
        for event in &handler.object_events {
            let kinds: &[PageObjectKind] = match event {
                CollectedObjectEvent::Char if !chars.is_empty() => &[PageObjectKind::Char],
                CollectedObjectEvent::Path(index) => &path_object_kinds[*index],
                CollectedObjectEvent::Image if !images.is_empty() => &[PageObjectKind::Image],
                CollectedObjectEvent::Char | CollectedObjectEvent::Image => &[],
            };
            for &kind in kinds {
                if !object_order.contains(&kind) {
                    object_order.push(kind);
                }
            }
        }

        // Extract annotations from the page
        let annotations = LopdfBackend::page_annotations(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page annotations", index, object_id))?;

        // Extract hyperlinks from the page
        let hyperlinks = LopdfBackend::page_hyperlinks(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page hyperlinks", index, object_id))?;

        // Preserve action type so callers can distinguish external URI actions
        // from internal and remote destinations.
        let uri_hyperlinks = LopdfBackend::page_uri_hyperlinks(&self.doc, &lopdf_page)
            .map_err(|error| page_error(error, "read page URI hyperlinks", index, object_id))?;

        // Extract form fields for this page (filtered from document AcroForm)
        let all_form_fields = LopdfBackend::document_form_fields(&self.doc)
            .map_err(|error| page_error(error, "read page form fields", index, object_id))?;
        let form_fields: Vec<FormField> = all_form_fields
            .into_iter()
            .filter(|f| f.page_index == Some(index))
            .collect();

        // Filter the cached document structure tree for this page.
        let structure_tree = if self.structure_tree.is_empty() {
            None
        } else {
            let page_elements: Vec<StructElement> =
                filter_struct_elements_for_page(&self.structure_tree, index);
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
                return Err(
                    PdfError::limit_exceeded("max_total_objects", max_total, new_total)
                        .at_page(index)
                        .at_object(lopdf_page.object_id.0, lopdf_page.object_id.1)
                        .during("extract page objects"),
                );
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
                return Err(PdfError::limit_exceeded(
                    "max_total_image_bytes",
                    max_img_bytes,
                    new_total,
                )
                .at_page(index)
                .at_object(lopdf_page.object_id.0, lopdf_page.object_id.1)
                .during("extract page images"));
            }
        }

        Ok(Page::from_extraction(
            index,
            geometry.width(),
            geometry.height(),
            geometry.rotation(),
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
            object_order,
            annotations,
            hyperlinks,
            uri_hyperlinks,
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
        LopdfBackend::validate(&self.doc)
            .map_err(|error| operation_error(error, "validate document"))
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
        LopdfBackend::document_signatures(&self.doc)
            .map_err(|error| operation_error(error, "read document signatures"))
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

/// Rotate a text direction by the page rotation angle (clockwise).
fn rotate_direction(dir: TextDirection, rotation: i32) -> TextDirection {
    match rotation {
        90 => match dir {
            TextDirection::Ltr => TextDirection::Ttb,
            TextDirection::Rtl => TextDirection::Btt,
            TextDirection::Ttb => TextDirection::Rtl,
            TextDirection::Btt => TextDirection::Ltr,
        },
        180 => match dir {
            TextDirection::Ltr => TextDirection::Rtl,
            TextDirection::Rtl => TextDirection::Ltr,
            TextDirection::Ttb => TextDirection::Btt,
            TextDirection::Btt => TextDirection::Ttb,
        },
        270 => match dir {
            TextDirection::Ltr => TextDirection::Btt,
            TextDirection::Rtl => TextDirection::Ttb,
            TextDirection::Ttb => TextDirection::Ltr,
            TextDirection::Btt => TextDirection::Rtl,
        },
        _ => dir,
    }
}

/// Undo a simple y-flip and re-apply through `PageGeometry` to account for rotation.
///
/// `char_from_event` and `extract_shapes` produce coordinates using a simple
/// `y' = page_height - y` flip. This helper undoes that flip to recover PDF native
/// coordinates, then applies the full rotation + y-flip transform via `PageGeometry`.
fn rotate_bbox(
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
    page_height: f64,
    geometry: &PageGeometry,
    page_origin: (f64, f64),
) -> BBox {
    let native_min_y = page_height - bottom;
    let native_max_y = page_height - top;
    offset_bbox(
        geometry.normalize_bbox(x0, native_min_y, x1, native_max_y),
        page_origin,
    )
}

fn rotated_page_origin(media_box: BBox, rotation: i32) -> (f64, f64) {
    // PageGeometry returns viewport-relative coordinates, while pdfplumber keeps
    // the normalized MediaBox's absolute top-left origin on page objects.
    let x0 = media_box.x0.min(media_box.x1);
    let y0 = media_box.top.min(media_box.bottom);
    if matches!(rotation, 90 | 270) {
        (y0, -x0)
    } else {
        (x0, -y0)
    }
}

/// Build the page matrix passed to pdfminer before content interpretation.
///
/// Content-stream matrices are concatenated on top of this value. Keeping the
/// page transform separate lets the parser remain page-agnostic while the
/// final character matrix still matches `PDFPageInterpreter.process_page`.
fn pdfminer_page_ctm(media_box: BBox, rotation: i32) -> Ctm {
    let x0 = media_box.x0;
    let y0 = media_box.top;
    let x1 = media_box.x1;
    let y1 = media_box.bottom;
    match rotation.rem_euclid(360) {
        90 => Ctm::new(0.0, -1.0, 1.0, 0.0, -y0, x1),
        180 => Ctm::new(-1.0, 0.0, 0.0, -1.0, x1, y1),
        270 => Ctm::new(0.0, 1.0, -1.0, 0.0, y1, -x0),
        _ => Ctm::new(1.0, 0.0, 0.0, 1.0, -x0, -y0),
    }
}

/// Reproduce pdfminer's LTChar layout matrix construction order.
///
/// pdfminer first combines the stable text, graphics, and page matrices, then
/// applies the glyph's local text position once. Keeping that order avoids
/// last-bit differences caused by composing an already-translated matrix with
/// the page transform.
fn pdfminer_character_matrix(event: &CharEvent, page_ctm: Ctm) -> Ctm {
    let text_matrix = Ctm::new(
        event.text_matrix_base[0],
        event.text_matrix_base[1],
        event.text_matrix_base[2],
        event.text_matrix_base[3],
        event.text_matrix_base[4],
        event.text_matrix_base[5],
    );
    let graphics_ctm = Ctm::new(
        event.ctm[0],
        event.ctm[1],
        event.ctm[2],
        event.ctm[3],
        event.ctm[4],
        event.ctm[5],
    );
    let base = text_matrix.concat(&graphics_ctm).concat(&page_ctm);
    let (x, y) = event.text_position;
    Ctm::new(
        base.a,
        base.b,
        base.c,
        base.d,
        x * base.a + y * base.c + base.e,
        x * base.b + y * base.d + base.f,
    )
}

fn offset_bbox(bbox: BBox, origin: (f64, f64)) -> BBox {
    BBox::new(
        bbox.x0 + origin.0,
        bbox.top + origin.1,
        bbox.x1 + origin.0,
        bbox.bottom + origin.1,
    )
}

fn offset_point(point: (f64, f64), origin: (f64, f64)) -> (f64, f64) {
    (point.0 + origin.0, point.1 + origin.1)
}

/// Re-classify line orientation after rotation.
fn classify_orientation(line: &Line) -> Orientation {
    let dx = (line.x1 - line.x0).abs();
    let dy = (line.bottom - line.top).abs();
    if dy < 1e-6 {
        Orientation::Horizontal
    } else if dx < 1e-6 {
        Orientation::Vertical
    } else {
        Orientation::Diagonal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::TextOptions;

    #[test]
    fn pdfminer_page_ctm_matches_nonzero_origin_rotation_contract() {
        let media_box = BBox::new(10.0, 20.0, 610.0, 820.0);
        assert_eq!(
            pdfminer_page_ctm(media_box, 0),
            Ctm::new(1.0, 0.0, 0.0, 1.0, -10.0, -20.0)
        );
        assert_eq!(
            pdfminer_page_ctm(media_box, 90),
            Ctm::new(0.0, -1.0, 1.0, 0.0, -20.0, 610.0)
        );
        assert_eq!(
            pdfminer_page_ctm(media_box, 180),
            Ctm::new(-1.0, 0.0, 0.0, -1.0, 610.0, 820.0)
        );
        assert_eq!(
            pdfminer_page_ctm(media_box, 270),
            Ctm::new(0.0, 1.0, -1.0, 0.0, 820.0, -10.0)
        );
        assert_eq!(
            pdfminer_page_ctm(media_box, 450),
            pdfminer_page_ctm(media_box, 90)
        );
    }

    /// Helper: create a minimal single-page PDF with the given text content stream.
    fn create_pdf_with_content(content: &[u8]) -> Vec<u8> {
        create_pdf_with_content_and_page_properties(content, None, None, None, None, None)
    }

    fn create_pdf_with_content_and_inherited_rotation(
        content: &[u8],
        rotation: Option<i64>,
    ) -> Vec<u8> {
        create_pdf_with_content_and_page_properties(content, rotation, None, None, None, None)
    }

    fn create_pdf_with_content_and_page_properties(
        content: &[u8],
        rotation: Option<i64>,
        crop_box: Option<[i64; 4]>,
        trim_box: Option<[i64; 4]>,
        bleed_box: Option<[i64; 4]>,
        art_box: Option<[i64; 4]>,
    ) -> Vec<u8> {
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
        let mut page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box,
            "Contents" => Object::Reference(content_id),
            "Resources" => resources,
        };
        if let Some([x0, y0, x1, y1]) = trim_box {
            page_dict.set("TrimBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        if let Some([x0, y0, x1, y1]) = bleed_box {
            page_dict.set("BleedBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        if let Some([x0, y0, x1, y1]) = art_box {
            page_dict.set("ArtBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
        let page_id = doc.add_object(page_dict);

        // Pages tree
        let mut pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => Object::Integer(1),
        };
        if let Some(rotation) = rotation {
            pages_dict.set("Rotate", rotation);
        }
        if let Some([x0, y0, x1, y1]) = crop_box {
            pages_dict.set("CropBox", vec![x0.into(), y0.into(), x1.into(), y1.into()]);
        }
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

    #[test]
    fn page_dimensions_are_available_without_content_interpretation() {
        let bytes = create_pdf_with_content(b"not a valid content stream operator");
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(pdf.page_dimensions(0), Some((612.0, 792.0)));
        assert_eq!(pdf.page_dimensions(1), None);
    }

    #[test]
    fn page_rotation_is_inherited_normalized_and_available_without_content_interpretation() {
        let bytes = create_pdf_with_content_and_inherited_rotation(
            b"not a valid content stream operator",
            Some(-90),
        );
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(pdf.page_rotation(0), Some(270));
        assert_eq!(pdf.page_rotation(1), None);
    }

    #[test]
    fn page_media_box_is_available_without_content_interpretation() {
        let bytes = create_pdf_with_content(b"not a valid content stream operator");
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(
            pdf.page_media_box(0),
            Some(BBox::new(0.0, 0.0, 612.0, 792.0))
        );
        assert_eq!(pdf.page_media_box(1), None);
    }

    #[test]
    fn page_crop_box_is_inherited_and_available_without_content_interpretation() {
        let bytes = create_pdf_with_content_and_page_properties(
            b"not a valid content stream operator",
            None,
            Some([36, 36, 576, 756]),
            None,
            None,
            None,
        );
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(
            pdf.page_crop_box(0),
            Some(BBox::new(36.0, 36.0, 576.0, 756.0))
        );
        assert_eq!(pdf.page_crop_box(1), None);
    }

    #[test]
    fn page_trim_box_is_explicit_and_available_without_content_interpretation() {
        let bytes = create_pdf_with_content_and_page_properties(
            b"not a valid content stream operator",
            None,
            None,
            Some([40, 50, 560, 740]),
            None,
            None,
        );
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(
            pdf.page_trim_box(0),
            Some(BBox::new(40.0, 50.0, 560.0, 740.0))
        );
        assert_eq!(pdf.page_trim_box(1), None);
    }

    #[test]
    fn page_bleed_box_is_explicit_and_available_without_content_interpretation() {
        let bytes = create_pdf_with_content_and_page_properties(
            b"not a valid content stream operator",
            None,
            None,
            None,
            Some([45, 55, 555, 735]),
            None,
        );
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(
            pdf.page_bleed_box(0),
            Some(BBox::new(45.0, 55.0, 555.0, 735.0))
        );
        assert_eq!(pdf.page_bleed_box(1), None);
    }

    #[test]
    fn page_art_box_is_explicit_and_available_without_content_interpretation() {
        let bytes = create_pdf_with_content_and_page_properties(
            b"not a valid content stream operator",
            None,
            None,
            None,
            None,
            Some([50, 60, 550, 730]),
        );
        let pdf = Pdf::open(&bytes, None).unwrap();

        assert_eq!(
            pdf.page_art_box(0),
            Some(BBox::new(50.0, 60.0, 550.0, 730.0))
        );
        assert_eq!(pdf.page_art_box(1), None);
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
    fn page_rotation_is_normalized() {
        let bytes = create_pdf_with_content_and_inherited_rotation(b"BT ET", Some(450));
        let pdf = Pdf::open(&bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        assert_eq!(page.rotation(), 90);
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
            Err(error) if error.kind() == PdfErrorKind::PasswordRequired => {} // expected
            Err(e) => panic!("expected PasswordRequired, got: {e}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn pdf_open_with_password_correct() {
        // Use the real pr-138-example.pdf which is encrypted with an empty user password
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdfs/pr-138-example.pdf");
        if !fixture_path.exists() {
            eprintln!("skipping: fixture not found at {}", fixture_path.display());
            return;
        }
        let bytes = std::fs::read(&fixture_path).unwrap();
        let pdf = Pdf::open_with_password(&bytes, b"", None).unwrap();
        assert_eq!(pdf.page_count(), 2);
    }

    #[test]
    fn pdf_open_with_nonempty_password_fixture() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdfs/password-example.pdf");
        let bytes = std::fs::read(&fixture_path).unwrap();

        let pdf = Pdf::open_with_password(&bytes, b"test", None).unwrap();

        assert_eq!(pdf.page_count(), 4);
        assert!(
            !pdf.page(0)
                .unwrap()
                .extract_text(&TextOptions::default())
                .is_empty()
        );
    }

    #[test]
    fn pdf_open_with_password_wrong_returns_invalid_password() {
        let bytes = create_encrypted_pdf(b"testpass");
        let result = Pdf::open_with_password(&bytes, b"wrongpass", None);
        match result {
            Err(error) if error.kind() == PdfErrorKind::InvalidPassword => {} // expected
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
