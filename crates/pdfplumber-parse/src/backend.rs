//! PDF parsing backend trait.
//!
//! Defines the [`PdfBackend`] trait that abstracts PDF parsing operations.
//! This enables pluggable backends (e.g., lopdf, pdf-rs) for PDF reading.

use pdfplumber_core::{BBox, DocumentMetadata, ExtractOptions, PdfError};

use crate::handler::ContentHandler;

/// Trait abstracting PDF parsing operations.
///
/// A backend provides methods to open PDF documents, access pages,
/// extract page properties (MediaBox, CropBox, Rotate), and interpret
/// page content streams via a [`ContentHandler`] callback.
///
/// # Associated Types
///
/// - `Document`: The parsed PDF document representation.
/// - `Page`: A reference to a single page within a document.
/// - `Error`: Backend-specific error type, convertible to [`PdfError`].
///
/// # Usage
///
/// ```ignore
/// let doc = MyBackend::open(pdf_bytes)?;
/// let page_count = MyBackend::page_count(&doc);
/// let page = MyBackend::get_page(&doc, 0)?;
/// let media_box = MyBackend::page_media_box(&doc, &page)?;
/// MyBackend::interpret_page(&doc, &page, &mut handler, &options)?;
/// ```
pub trait PdfBackend {
    /// The parsed PDF document type.
    type Document;

    /// A reference to a single page within a document.
    type Page;

    /// Backend-specific error type, convertible to [`PdfError`].
    type Error: std::error::Error + Into<PdfError>;

    /// Parse PDF bytes into a document.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes do not represent a valid PDF document.
    fn open(bytes: &[u8]) -> Result<Self::Document, Self::Error>;

    /// Return the number of pages in the document.
    fn page_count(doc: &Self::Document) -> usize;

    /// Access a page by 0-based index.
    ///
    /// # Errors
    ///
    /// Returns an error if the index is out of range or the page cannot be loaded.
    fn get_page(doc: &Self::Document, index: usize) -> Result<Self::Page, Self::Error>;

    /// Get the MediaBox for a page.
    ///
    /// MediaBox is required by the PDF specification and defines the boundaries
    /// of the physical page. The returned [`BBox`] uses the library's top-left
    /// origin coordinate system.
    ///
    /// # Errors
    ///
    /// Returns an error if the MediaBox cannot be resolved (e.g., missing
    /// from both the page and its parent page tree).
    fn page_media_box(doc: &Self::Document, page: &Self::Page) -> Result<BBox, Self::Error>;

    /// Get the CropBox for a page, if explicitly set.
    ///
    /// CropBox defines the visible region of the page. Returns `None` if
    /// not explicitly set (in which case MediaBox serves as the CropBox).
    ///
    /// # Errors
    ///
    /// Returns an error if the CropBox entry exists but is malformed.
    fn page_crop_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error>;

    /// Get the TrimBox for a page, if explicitly set.
    ///
    /// TrimBox defines the intended dimensions of the finished page after
    /// trimming. Returns `None` if not set. Supports inheritance from
    /// parent page tree nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if the TrimBox entry exists but is malformed.
    fn page_trim_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error>;

    /// Get the BleedBox for a page, if explicitly set.
    ///
    /// BleedBox defines the region to which page contents should be clipped
    /// when output in a production environment. Returns `None` if not set.
    /// Supports inheritance from parent page tree nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if the BleedBox entry exists but is malformed.
    fn page_bleed_box(doc: &Self::Document, page: &Self::Page)
    -> Result<Option<BBox>, Self::Error>;

    /// Get the ArtBox for a page, if explicitly set.
    ///
    /// ArtBox defines the extent of the page's meaningful content as intended
    /// by the page's creator. Returns `None` if not set. Supports inheritance
    /// from parent page tree nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if the ArtBox entry exists but is malformed.
    fn page_art_box(doc: &Self::Document, page: &Self::Page) -> Result<Option<BBox>, Self::Error>;

    /// Get the page rotation angle in degrees.
    ///
    /// Returns one of: 0, 90, 180, or 270. Defaults to 0 if not specified.
    ///
    /// # Errors
    ///
    /// Returns an error if the Rotate entry exists but is malformed.
    fn page_rotate(doc: &Self::Document, page: &Self::Page) -> Result<i32, Self::Error>;

    /// Extract document-level metadata from the PDF /Info dictionary.
    ///
    /// Returns a [`DocumentMetadata`] containing title, author, subject,
    /// keywords, creator, producer, creation date, and modification date.
    /// Fields not present in the PDF are returned as `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if the /Info dictionary exists but is malformed.
    fn document_metadata(doc: &Self::Document) -> Result<DocumentMetadata, Self::Error>;

    /// Interpret the page's content stream, calling back into the handler.
    ///
    /// The interpreter processes PDF content stream operators (text, path,
    /// image) and notifies the `handler` of extracted content via
    /// [`ContentHandler`] callbacks. Resource limits from `options` are
    /// enforced during interpretation.
    ///
    /// # Errors
    ///
    /// Returns an error if content stream parsing fails or a resource limit
    /// is exceeded.
    fn interpret_page(
        doc: &Self::Document,
        page: &Self::Page,
        handler: &mut dyn ContentHandler,
        options: &ExtractOptions,
    ) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{CharEvent, ImageEvent, PaintOp, PathEvent};
    use pdfplumber_core::{Color, PathSegment, Point};

    // --- Mock types ---

    #[derive(Debug)]
    struct MockDocument {
        pages: Vec<MockPageData>,
    }

    #[derive(Debug)]
    struct MockPageData {
        media_box: BBox,
        crop_box: Option<BBox>,
        trim_box: Option<BBox>,
        bleed_box: Option<BBox>,
        art_box: Option<BBox>,
        rotate: i32,
    }

    #[derive(Debug)]
    struct MockPage {
        index: usize,
    }

    // --- CollectingHandler for testing ---

    struct CollectingHandler {
        chars: Vec<CharEvent>,
        paths: Vec<PathEvent>,
        images: Vec<ImageEvent>,
    }

    impl CollectingHandler {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                paths: Vec::new(),
                images: Vec::new(),
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
    }

    // --- MockBackend implementation ---

    struct MockBackend;

    impl PdfBackend for MockBackend {
        type Document = MockDocument;
        type Page = MockPage;
        type Error = PdfError;

        fn open(bytes: &[u8]) -> Result<Self::Document, Self::Error> {
            if bytes.is_empty() {
                return Err(PdfError::ParseError("empty input".to_string()));
            }
            // Mock: first byte encodes page count
            let page_count = bytes[0] as usize;
            let mut pages = Vec::new();
            for _ in 0..page_count {
                pages.push(MockPageData {
                    media_box: BBox::new(0.0, 0.0, 612.0, 792.0), // US Letter
                    crop_box: None,
                    trim_box: None,
                    bleed_box: None,
                    art_box: None,
                    rotate: 0,
                });
            }
            Ok(MockDocument { pages })
        }

        fn page_count(doc: &Self::Document) -> usize {
            doc.pages.len()
        }

        fn get_page(doc: &Self::Document, index: usize) -> Result<Self::Page, Self::Error> {
            if index >= doc.pages.len() {
                return Err(PdfError::ParseError(format!(
                    "page index {index} out of range (0..{})",
                    doc.pages.len()
                )));
            }
            Ok(MockPage { index })
        }

        fn page_media_box(doc: &Self::Document, page: &Self::Page) -> Result<BBox, Self::Error> {
            Ok(doc.pages[page.index].media_box)
        }

        fn page_crop_box(
            doc: &Self::Document,
            page: &Self::Page,
        ) -> Result<Option<BBox>, Self::Error> {
            Ok(doc.pages[page.index].crop_box)
        }

        fn page_trim_box(
            doc: &Self::Document,
            page: &Self::Page,
        ) -> Result<Option<BBox>, Self::Error> {
            Ok(doc.pages[page.index].trim_box)
        }

        fn page_bleed_box(
            doc: &Self::Document,
            page: &Self::Page,
        ) -> Result<Option<BBox>, Self::Error> {
            Ok(doc.pages[page.index].bleed_box)
        }

        fn page_art_box(
            doc: &Self::Document,
            page: &Self::Page,
        ) -> Result<Option<BBox>, Self::Error> {
            Ok(doc.pages[page.index].art_box)
        }

        fn page_rotate(doc: &Self::Document, page: &Self::Page) -> Result<i32, Self::Error> {
            Ok(doc.pages[page.index].rotate)
        }

        fn document_metadata(_doc: &Self::Document) -> Result<DocumentMetadata, Self::Error> {
            Ok(DocumentMetadata::default())
        }

        fn interpret_page(
            _doc: &Self::Document,
            _page: &Self::Page,
            handler: &mut dyn ContentHandler,
            _options: &ExtractOptions,
        ) -> Result<(), Self::Error> {
            // Emit a sample char
            handler.on_char(CharEvent {
                char_code: 72, // 'H'
                unicode: Some("H".to_string()),
                font_name: "Times-Roman".to_string(),
                font_size: 14.0,
                text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 720.0],
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                displacement: 722.0,
                char_spacing: 0.0,
                word_spacing: 0.0,
                h_scaling: 1.0,
                rise: 0.0,
            });

            // Emit a sample path (horizontal line)
            handler.on_path_painted(PathEvent {
                segments: vec![
                    PathSegment::MoveTo(Point::new(72.0, 700.0)),
                    PathSegment::LineTo(Point::new(540.0, 700.0)),
                ],
                paint_op: PaintOp::Stroke,
                line_width: 0.5,
                stroking_color: Some(Color::black()),
                non_stroking_color: None,
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                dash_pattern: None,
                fill_rule: None,
            });

            // Emit a sample image
            handler.on_image(ImageEvent {
                name: "Im1".to_string(),
                ctm: [100.0, 0.0, 0.0, 75.0, 72.0, 600.0],
                width: 400,
                height: 300,
                colorspace: Some("DeviceRGB".to_string()),
                bits_per_component: Some(8),
            });

            Ok(())
        }
    }

    // --- PdfBackend::open tests ---

    #[test]
    fn mock_backend_open_valid_document() {
        let doc = MockBackend::open(&[3]).unwrap();
        assert_eq!(MockBackend::page_count(&doc), 3);
    }

    #[test]
    fn mock_backend_open_single_page() {
        let doc = MockBackend::open(&[1]).unwrap();
        assert_eq!(MockBackend::page_count(&doc), 1);
    }

    #[test]
    fn mock_backend_open_empty_bytes_fails() {
        let result = MockBackend::open(&[]);
        assert!(result.is_err());
    }

    // --- PdfBackend::get_page tests ---

    #[test]
    fn mock_backend_get_page_valid_index() {
        let doc = MockBackend::open(&[3]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        assert_eq!(page.index, 0);

        let page2 = MockBackend::get_page(&doc, 2).unwrap();
        assert_eq!(page2.index, 2);
    }

    #[test]
    fn mock_backend_get_page_out_of_bounds() {
        let doc = MockBackend::open(&[2]).unwrap();
        let result = MockBackend::get_page(&doc, 5);
        assert!(result.is_err());
    }

    // --- PdfBackend::page_media_box tests ---

    #[test]
    fn mock_backend_page_media_box() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let media_box = MockBackend::page_media_box(&doc, &page).unwrap();
        assert_eq!(media_box, BBox::new(0.0, 0.0, 612.0, 792.0));
    }

    // --- PdfBackend::page_crop_box tests ---

    #[test]
    fn mock_backend_page_crop_box_none() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let crop_box = MockBackend::page_crop_box(&doc, &page).unwrap();
        assert_eq!(crop_box, None);
    }

    // --- PdfBackend::page_rotate tests ---

    #[test]
    fn mock_backend_page_rotate_default() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let rotate = MockBackend::page_rotate(&doc, &page).unwrap();
        assert_eq!(rotate, 0);
    }

    // --- PdfBackend::interpret_page tests ---

    #[test]
    fn mock_backend_interpret_page_emits_char() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        MockBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.chars[0].char_code, 72);
        assert_eq!(handler.chars[0].unicode.as_deref(), Some("H"));
        assert_eq!(handler.chars[0].font_name, "Times-Roman");
        assert_eq!(handler.chars[0].font_size, 14.0);
    }

    #[test]
    fn mock_backend_interpret_page_emits_path() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        MockBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        assert_eq!(handler.paths.len(), 1);
        assert_eq!(handler.paths[0].paint_op, PaintOp::Stroke);
        assert_eq!(handler.paths[0].segments.len(), 2);
        assert_eq!(handler.paths[0].line_width, 0.5);
    }

    #[test]
    fn mock_backend_interpret_page_emits_image() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        MockBackend::interpret_page(&doc, &page, &mut handler, &options).unwrap();

        assert_eq!(handler.images.len(), 1);
        assert_eq!(handler.images[0].name, "Im1");
        assert_eq!(handler.images[0].width, 400);
        assert_eq!(handler.images[0].height, 300);
    }

    #[test]
    fn mock_backend_interpret_page_uses_trait_object() {
        let doc = MockBackend::open(&[1]).unwrap();
        let page = MockBackend::get_page(&doc, 0).unwrap();
        let options = ExtractOptions::default();
        let mut handler = CollectingHandler::new();

        // Pass handler as &mut dyn ContentHandler explicitly
        let handler_ref: &mut dyn ContentHandler = &mut handler;
        MockBackend::interpret_page(&doc, &page, handler_ref, &options).unwrap();

        assert_eq!(handler.chars.len(), 1);
        assert_eq!(handler.paths.len(), 1);
        assert_eq!(handler.images.len(), 1);
    }

    // --- Error conversion tests ---

    #[test]
    fn mock_backend_error_converts_to_pdf_error() {
        let result = MockBackend::open(&[]);
        let err = result.unwrap_err();
        // PdfError::into() PdfError is identity
        let pdf_err: PdfError = err.into();
        assert!(matches!(pdf_err, PdfError::ParseError(_)));
    }

    #[test]
    fn mock_backend_error_is_std_error() {
        let result = MockBackend::open(&[]);
        let err = result.unwrap_err();
        let std_err: Box<dyn std::error::Error> = Box::new(err);
        assert!(std_err.to_string().contains("empty input"));
    }

    // --- Custom mock with CropBox and Rotate ---

    #[test]
    fn mock_backend_custom_page_properties() {
        let doc = MockDocument {
            pages: vec![
                MockPageData {
                    media_box: BBox::new(0.0, 0.0, 595.0, 842.0), // A4
                    crop_box: Some(BBox::new(10.0, 10.0, 585.0, 832.0)),
                    trim_box: None,
                    bleed_box: None,
                    art_box: None,
                    rotate: 90,
                },
                MockPageData {
                    media_box: BBox::new(0.0, 0.0, 842.0, 595.0), // A4 landscape
                    crop_box: None,
                    trim_box: None,
                    bleed_box: None,
                    art_box: None,
                    rotate: 0,
                },
            ],
        };

        // Page 0: A4 portrait with CropBox and rotation
        let page0 = MockBackend::get_page(&doc, 0).unwrap();
        let media_box0 = MockBackend::page_media_box(&doc, &page0).unwrap();
        assert_eq!(media_box0, BBox::new(0.0, 0.0, 595.0, 842.0));

        let crop_box0 = MockBackend::page_crop_box(&doc, &page0).unwrap();
        assert_eq!(crop_box0, Some(BBox::new(10.0, 10.0, 585.0, 832.0)));

        let rotate0 = MockBackend::page_rotate(&doc, &page0).unwrap();
        assert_eq!(rotate0, 90);

        // Page 1: A4 landscape, no CropBox, no rotation
        let page1 = MockBackend::get_page(&doc, 1).unwrap();
        let crop_box1 = MockBackend::page_crop_box(&doc, &page1).unwrap();
        assert_eq!(crop_box1, None);

        let rotate1 = MockBackend::page_rotate(&doc, &page1).unwrap();
        assert_eq!(rotate1, 0);
    }
}
