//! WebAssembly/JavaScript bindings for pdfplumber-rs.
//!
//! Provides ergonomic JavaScript API for PDF text, word, and table extraction
//! via wasm-bindgen. Complex types are serialized to JsValue using
//! serde_wasm_bindgen.

use wasm_bindgen::prelude::*;

use pdfplumber::{Page, Pdf, SearchOptions, TableSettings, TextOptions, WordOptions};

/// A PDF document opened for extraction (WASM binding).
///
/// # JavaScript Usage
///
/// ```js
/// const pdf = WasmPdf.open(pdfBytes);
/// console.log(`Pages: ${pdf.pageCount}`);
/// const page = pdf.page(0);
/// console.log(page.extractText());
/// ```
#[wasm_bindgen]
pub struct WasmPdf {
    inner: Pdf,
}

#[wasm_bindgen]
impl WasmPdf {
    /// Open a PDF from raw bytes (Uint8Array in JavaScript).
    pub fn open(data: &[u8]) -> Result<WasmPdf, JsError> {
        let pdf = Pdf::open(data, None).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(WasmPdf { inner: pdf })
    }

    /// Return the number of pages in the document.
    #[wasm_bindgen(getter, js_name = "pageCount")]
    pub fn page_count(&self) -> usize {
        self.inner.page_count()
    }

    /// Get a page by 0-based index.
    pub fn page(&self, index: usize) -> Result<WasmPage, JsError> {
        let page = self
            .inner
            .page(index)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(WasmPage { inner: page })
    }

    /// Return document metadata as a JavaScript object.
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.metadata())
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

/// A single PDF page (WASM binding).
///
/// Provides text, word, character, and table extraction methods.
/// Properties (width, height, pageNumber) are accessed as JS getters.
/// Complex return types (chars, words, tables) are returned as JsValue.
#[wasm_bindgen]
pub struct WasmPage {
    inner: Page,
}

#[wasm_bindgen]
impl WasmPage {
    /// Page index (0-based).
    #[wasm_bindgen(getter, js_name = "pageNumber")]
    pub fn page_number(&self) -> usize {
        self.inner.page_number()
    }

    /// Page width in points.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> f64 {
        self.inner.width()
    }

    /// Page height in points.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> f64 {
        self.inner.height()
    }

    /// Return all characters as an array of objects.
    pub fn chars(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.chars()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract text from the page.
    ///
    /// When `layout` is true, detects multi-column layouts and reading order.
    /// Defaults to false (simple spatial ordering).
    #[wasm_bindgen(js_name = "extractText")]
    pub fn extract_text(&self, layout: Option<bool>) -> String {
        let options = TextOptions {
            layout: layout.unwrap_or(false),
            ..TextOptions::default()
        };
        self.inner.extract_text(&options)
    }

    /// Extract words from the page.
    ///
    /// Returns an array of word objects with text and bounding box.
    #[wasm_bindgen(js_name = "extractWords")]
    pub fn extract_words(
        &self,
        x_tolerance: Option<f64>,
        y_tolerance: Option<f64>,
    ) -> Result<JsValue, JsError> {
        let options = WordOptions {
            x_tolerance: x_tolerance.unwrap_or(3.0),
            y_tolerance: y_tolerance.unwrap_or(3.0),
            ..WordOptions::default()
        };
        let words = self.inner.extract_words(&options);
        serde_wasm_bindgen::to_value(&words).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Find tables on the page.
    ///
    /// Returns an array of table objects with cells, rows, and bounding boxes.
    #[wasm_bindgen(js_name = "findTables")]
    pub fn find_tables(&self) -> Result<JsValue, JsError> {
        let settings = TableSettings::default();
        let tables = self.inner.find_tables(&settings);
        serde_wasm_bindgen::to_value(&tables).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract tables as 2D text arrays.
    ///
    /// Returns `Array<Array<Array<string|null>>>` — one array per table,
    /// each containing rows of cell values.
    #[wasm_bindgen(js_name = "extractTables")]
    pub fn extract_tables(&self) -> Result<JsValue, JsError> {
        let settings = TableSettings::default();
        let tables = self.inner.extract_tables(&settings);
        serde_wasm_bindgen::to_value(&tables).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Search for a text pattern on the page.
    ///
    /// Returns matches with text and bounding box. Supports regex.
    pub fn search(
        &self,
        pattern: &str,
        regex: Option<bool>,
        case: Option<bool>,
    ) -> Result<JsValue, JsError> {
        let options = SearchOptions {
            regex: regex.unwrap_or(true),
            case_sensitive: case.unwrap_or(true),
        };
        let matches = self.inner.search(pattern, &options);
        serde_wasm_bindgen::to_value(&matches).map_err(|e| JsError::new(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal single-page PDF with "Hello World" text for testing.
    fn create_test_pdf() -> Vec<u8> {
        use lopdf::dictionary;
        use lopdf::{Document, Object, Stream};

        let mut doc = Document::with_version("1.7");

        // Font
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        // Content stream: "Hello World" at position (72, 700)
        let content = b"BT /F1 12 Tf 72 700 Td (Hello World) Tj ET";
        let content_stream = Stream::new(dictionary! {}, content.to_vec());
        let content_id = doc.add_object(content_stream);

        // Resources
        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        };

        // Page
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => resources,
        });

        // Pages
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        });

        // Set parent on page
        if let Ok(page) = doc.get_object_mut(page_id) {
            if let Object::Dictionary(dict) = page {
                dict.set("Parent", pages_id);
            }
        }

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    // ---- WasmPdf tests ----

    #[test]
    fn test_open_valid_pdf() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data);
        assert!(pdf.is_ok());
    }

    // Error path tests use the underlying Rust API because JsError::new()
    // cannot be called on non-wasm targets.

    #[test]
    fn test_open_invalid_data() {
        let result = Pdf::open(b"not a valid pdf", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_open_empty_data() {
        let result = Pdf::open(b"", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_page_count() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        assert_eq!(pdf.page_count(), 1);
    }

    #[test]
    fn test_page_valid_index() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0);
        assert!(page.is_ok());
    }

    #[test]
    fn test_page_invalid_index() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let result = pdf.page(100);
        assert!(result.is_err());
    }

    // ---- WasmPage property tests ----

    #[test]
    fn test_page_number() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        assert_eq!(page.page_number(), 0);
    }

    #[test]
    fn test_page_width() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        assert!((page.width() - 612.0).abs() < 0.1);
    }

    #[test]
    fn test_page_height() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        assert!((page.height() - 792.0).abs() < 0.1);
    }

    // ---- Text extraction tests ----

    #[test]
    fn test_extract_text_default() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        let text = page.extract_text(None);
        assert!(text.contains("Hello"), "Expected 'Hello' in text: {text}");
    }

    #[test]
    fn test_extract_text_no_layout() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        let text = page.extract_text(Some(false));
        assert!(
            text.contains("Hello World"),
            "Expected 'Hello World' in text: {text}"
        );
    }

    #[test]
    fn test_extract_text_with_layout() {
        let data = create_test_pdf();
        let pdf = WasmPdf::open(&data).unwrap();
        let page = pdf.page(0).unwrap();
        let text = page.extract_text(Some(true));
        assert!(!text.is_empty());
    }

    // ---- Tests via underlying Rust API (for complex return types) ----
    // These verify the logic that chars/words/search/tables would serialize
    // without actually going through serde_wasm_bindgen (which requires WASM
    // runtime for full JS interop).

    #[test]
    fn test_underlying_chars() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let chars = page.chars();
        assert!(!chars.is_empty(), "Expected chars from test PDF");
        // Verify char content matches "Hello World"
        let text: String = chars.iter().map(|c| c.text.as_str()).collect();
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_underlying_words() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let words = page.extract_words(&WordOptions::default());
        assert!(!words.is_empty(), "Expected words from test PDF");
        let has_hello = words.iter().any(|w| w.text == "Hello");
        assert!(has_hello, "Expected 'Hello' word");
    }

    #[test]
    fn test_underlying_search() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let matches = page.search(
            "Hello",
            &SearchOptions {
                regex: false,
                case_sensitive: true,
            },
        );
        assert!(!matches.is_empty(), "Expected search match for 'Hello'");
        assert_eq!(matches[0].text, "Hello");
    }

    #[test]
    fn test_underlying_search_regex() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let matches = page.search(
            "H.llo",
            &SearchOptions {
                regex: true,
                case_sensitive: true,
            },
        );
        assert!(!matches.is_empty(), "Expected regex match for 'H.llo'");
    }

    #[test]
    fn test_underlying_tables_empty() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let tables = page.find_tables(&TableSettings::default());
        // Simple text PDF should not have any tables
        assert!(tables.is_empty(), "Expected no tables in simple text PDF");
    }

    #[test]
    fn test_underlying_extract_tables_empty() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let tables = page.extract_tables(&TableSettings::default());
        assert!(
            tables.is_empty(),
            "Expected no extracted tables in simple text PDF"
        );
    }
}
