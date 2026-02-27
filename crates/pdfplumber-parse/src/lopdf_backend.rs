//! lopdf-based PDF parsing backend.
//!
//! Implements [`PdfBackend`] using the [lopdf](https://crates.io/crates/lopdf)
//! crate for PDF document parsing. This is the default backend for pdfplumber-rs.

use crate::backend::PdfBackend;
use crate::error::BackendError;
use crate::handler::ContentHandler;
use pdfplumber_core::{BBox, ExtractOptions};

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

    fn page_media_box(_doc: &Self::Document, _page: &Self::Page) -> Result<BBox, Self::Error> {
        // Stub: will be implemented in US-007
        todo!("page_media_box not yet implemented")
    }

    fn page_crop_box(
        _doc: &Self::Document,
        _page: &Self::Page,
    ) -> Result<Option<BBox>, Self::Error> {
        // Stub: will be implemented in US-007
        todo!("page_crop_box not yet implemented")
    }

    fn page_rotate(_doc: &Self::Document, _page: &Self::Page) -> Result<i32, Self::Error> {
        // Stub: will be implemented in US-007
        todo!("page_rotate not yet implemented")
    }

    fn interpret_page(
        _doc: &Self::Document,
        _page: &Self::Page,
        _handler: &mut dyn ContentHandler,
        _options: &ExtractOptions,
    ) -> Result<(), Self::Error> {
        // Stub: will be implemented in later stories
        todo!("interpret_page not yet implemented")
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

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::PdfError;

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
}
