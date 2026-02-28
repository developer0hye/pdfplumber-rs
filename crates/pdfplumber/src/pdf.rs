//! Top-level PDF document type for opening and extracting content.

use pdfplumber_core::{Char, Ctm, ExtractOptions, Image, ImageMetadata, PdfError, image_from_ctm};
use pdfplumber_parse::{
    CharEvent, ContentHandler, FontMetrics, ImageEvent, LopdfBackend, LopdfDocument, PageGeometry,
    PathEvent, PdfBackend, char_from_event,
};

use crate::Page;

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
}

/// Internal handler that collects content stream events during interpretation.
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

    fn on_path_painted(&mut self, _event: PathEvent) {
        // Path painting operators are not yet implemented in the interpreter,
        // so this is a no-op for now.
    }

    fn on_image(&mut self, event: ImageEvent) {
        self.images.push(event);
    }
}

impl Pdf {
    /// Open a PDF document from bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Raw PDF file bytes.
    /// * `options` - Extraction options (resource limits, etc.). Uses defaults if `None`.
    ///
    /// # Errors
    ///
    /// Returns [`PdfError`] if the bytes are not a valid PDF document.
    pub fn open(bytes: &[u8], options: Option<ExtractOptions>) -> Result<Self, PdfError> {
        let doc = LopdfBackend::open(bytes).map_err(PdfError::from)?;
        let options = options.unwrap_or_default();

        // Cache page heights for doctop calculation
        let page_count = LopdfBackend::page_count(&doc);
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

        Ok(Self {
            doc,
            options,
            page_heights,
            raw_page_heights,
        })
    }

    /// Return the number of pages in the document.
    pub fn page_count(&self) -> usize {
        LopdfBackend::page_count(&self.doc)
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
        let rotation = LopdfBackend::page_rotate(&self.doc, &lopdf_page).map_err(PdfError::from)?;
        let geometry = PageGeometry::new(media_box, crop_box, rotation);

        // Interpret page content
        let mut handler = CollectingHandler::new();
        LopdfBackend::interpret_page(&self.doc, &lopdf_page, &mut handler, &self.options)
            .map_err(PdfError::from)?;

        // Convert CharEvents to Chars
        let page_height = self.raw_page_heights[index];
        let default_metrics = FontMetrics::default_metrics();
        let doctop_offset: f64 = self.page_heights[..index].iter().sum();

        let chars: Vec<Char> = handler
            .chars
            .iter()
            .map(|event| {
                let mut ch = char_from_event(event, &default_metrics, page_height, None, None);
                ch.doctop += doctop_offset;
                ch
            })
            .collect();

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
                image_from_ctm(&ctm, &event.name, page_height, &meta)
            })
            .collect();

        Ok(Page::from_extraction(
            index,
            geometry.width(),
            geometry.height(),
            rotation,
            chars,
            images,
        ))
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
}
