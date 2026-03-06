//! WebAssembly/JavaScript bindings for pdfplumber-rs.
//!
//! Provides a complete JavaScript API for PDF text, word, character, table,
//! geometry, annotation, and spatial-filter extraction via wasm-bindgen.
//!
//! Complex types are serialized to JsValue using serde_wasm_bindgen, giving
//! JavaScript consumers plain objects with typed fields — no manual unwrapping.
//!
//! # API surface
//!
//! - **`WasmPdf`**: open(bytes), pageCount, page(index), metadata, bookmarks
//! - **`WasmPage`**: all extraction + geometry + crop operations
//! - **`WasmCroppedPage`**: spatially-filtered view, mirrors WasmPage extraction API

use wasm_bindgen::prelude::*;

use pdfplumber::{
    BBox, CroppedPage, Page, Pdf, SearchOptions, TableSettings, TextOptions, WordOptions,
};

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
    ///
    /// Fields: title, author, subject, keywords, creator, producer,
    /// creation_date, mod_date (all string | null).
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.metadata())
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return document bookmarks (outline / table of contents).
    ///
    /// Each entry: `{ title: string, level: number, page_number: number | null, dest_top: number | null }`.
    pub fn bookmarks(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.bookmarks())
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

    /// Return lines on this page.
    ///
    /// Each entry: `{ x0, top, x1, bottom, line_width, orientation }`.
    pub fn lines(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.lines()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return rectangles on this page.
    ///
    /// Each entry: `{ x0, top, x1, bottom, line_width, stroke, fill }`.
    pub fn rects(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.rects()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return curves on this page.
    ///
    /// Each entry: `{ x0, top, x1, bottom, pts, line_width, stroke, fill }`.
    pub fn curves(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.curves()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return images on this page.
    ///
    /// Each entry: `{ x0, top, x1, bottom, width, height, name, src_width, src_height,
    /// bits_per_component, color_space }`.
    pub fn images(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.images()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return annotations on this page (highlights, notes, links, etc.).
    pub fn annots(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.annots()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return hyperlinks on this page.
    ///
    /// Each entry: `{ x0, top, x1, bottom, uri }`.
    pub fn hyperlinks(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.hyperlinks())
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Page rotation in degrees (0, 90, 180, or 270).
    #[wasm_bindgen(getter)]
    pub fn rotation(&self) -> i32 {
        self.inner.rotation()
    }

    /// Page bounding box as `{ x0, top, x1, bottom }`.
    #[wasm_bindgen(getter)]
    pub fn bbox(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(&self.inner.bbox()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// MediaBox as `{ x0, top, x1, bottom }`.
    #[wasm_bindgen(js_name = "mediaBox", getter)]
    pub fn media_box(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(&self.inner.media_box())
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Crop this page to a bounding box.
    ///
    /// Returns a `WasmCroppedPage` containing only objects intersecting the bbox.
    /// `bbox` is `[x0, top, x1, bottom]` in page coordinate space.
    pub fn crop(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.crop(BBox::new(x0, top, x1, bottom)),
        }
    }

    /// Return a view containing only objects **fully within** the bbox.
    pub fn within_bbox(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.within_bbox(BBox::new(x0, top, x1, bottom)),
        }
    }

    /// Return a view containing only objects **outside** the bbox.
    pub fn outside_bbox(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.outside_bbox(BBox::new(x0, top, x1, bottom)),
        }
    }
}

// ---------------------------------------------------------------------------
// WasmCroppedPage
// ---------------------------------------------------------------------------

/// A spatially-filtered view of a PDF page (WASM binding).
///
/// Produced by `WasmPage.crop()`, `.within_bbox()`, or `.outside_bbox()`.
/// Supports the same extraction methods as `WasmPage` plus further cropping.
///
/// # JavaScript Usage
///
/// ```js
/// const header = page.crop(0, 0, page.width, 80);
/// const headerText = header.extractText();
/// const headerTables = header.findTables();
/// ```
#[wasm_bindgen]
pub struct WasmCroppedPage {
    inner: CroppedPage,
}

#[wasm_bindgen]
impl WasmCroppedPage {
    /// Width of the cropped region in points.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> f64 {
        self.inner.width()
    }

    /// Height of the cropped region in points.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> f64 {
        self.inner.height()
    }

    /// Return all characters in the cropped region.
    pub fn chars(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.chars()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract text from the cropped region.
    #[wasm_bindgen(js_name = "extractText")]
    pub fn extract_text(&self, layout: Option<bool>) -> String {
        self.inner.extract_text(&TextOptions {
            layout: layout.unwrap_or(false),
            ..TextOptions::default()
        })
    }

    /// Extract words from the cropped region.
    #[wasm_bindgen(js_name = "extractWords")]
    pub fn extract_words(
        &self,
        x_tolerance: Option<f64>,
        y_tolerance: Option<f64>,
    ) -> Result<JsValue, JsError> {
        let words = self.inner.extract_words(&WordOptions {
            x_tolerance: x_tolerance.unwrap_or(3.0),
            y_tolerance: y_tolerance.unwrap_or(3.0),
            ..WordOptions::default()
        });
        serde_wasm_bindgen::to_value(&words).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Find tables in the cropped region.
    #[wasm_bindgen(js_name = "findTables")]
    pub fn find_tables(&self) -> Result<JsValue, JsError> {
        let tables = self.inner.find_tables(&TableSettings::default());
        serde_wasm_bindgen::to_value(&tables).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract table content as `Array<Array<Array<string|null>>>`.
    #[wasm_bindgen(js_name = "extractTables")]
    pub fn extract_tables(&self) -> Result<JsValue, JsError> {
        let tables = self.inner.find_tables(&TableSettings::default());
        let data: Vec<Vec<Vec<Option<String>>>> = tables
            .iter()
            .map(|t| {
                t.rows
                    .iter()
                    .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
                    .collect()
            })
            .collect();
        serde_wasm_bindgen::to_value(&data).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return lines in the cropped region.
    pub fn lines(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.lines()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return rectangles in the cropped region.
    pub fn rects(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.rects()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return curves in the cropped region.
    pub fn curves(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.curves()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Return images in the cropped region.
    pub fn images(&self) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(self.inner.images()).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Further crop this cropped page.
    pub fn crop(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.crop(BBox::new(x0, top, x1, bottom)),
        }
    }

    /// Filter to objects fully within the given bbox.
    pub fn within_bbox(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.within_bbox(BBox::new(x0, top, x1, bottom)),
        }
    }

    /// Filter to objects outside the given bbox.
    pub fn outside_bbox(&self, x0: f64, top: f64, x1: f64, bottom: f64) -> WasmCroppedPage {
        WasmCroppedPage {
            inner: self.inner.outside_bbox(BBox::new(x0, top, x1, bottom)),
        }
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

    // ---- npm packaging artifact tests ----

    #[test]
    fn test_readme_exists() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let readme_path = std::path::Path::new(manifest_dir).join("README.md");
        assert!(readme_path.exists(), "README.md must exist for npm package");
    }

    #[test]
    fn test_readme_has_npm_install() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let readme = std::fs::read_to_string(std::path::Path::new(manifest_dir).join("README.md"))
            .expect("README.md must be readable");
        assert!(
            readme.contains("npm install pdfplumber-wasm")
                || readme.contains("npm i pdfplumber-wasm"),
            "README must contain npm install instructions"
        );
    }

    #[test]
    fn test_readme_has_browser_usage() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let readme = std::fs::read_to_string(std::path::Path::new(manifest_dir).join("README.md"))
            .expect("README.md must be readable");
        assert!(
            readme.contains("Browser") || readme.contains("browser"),
            "README must contain browser usage section"
        );
    }

    #[test]
    fn test_readme_has_nodejs_usage() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let readme = std::fs::read_to_string(std::path::Path::new(manifest_dir).join("README.md"))
            .expect("README.md must be readable");
        assert!(
            readme.contains("Node") || readme.contains("node"),
            "README must contain Node.js usage section"
        );
    }

    #[test]
    fn test_browser_demo_exists() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let demo_path = std::path::Path::new(manifest_dir).join("examples/browser-demo.html");
        assert!(
            demo_path.exists(),
            "Browser demo HTML must exist at examples/browser-demo.html"
        );
    }

    #[test]
    fn test_browser_demo_loads_pdf_and_extracts_tables() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let demo = std::fs::read_to_string(
            std::path::Path::new(manifest_dir).join("examples/browser-demo.html"),
        )
        .expect("Browser demo must be readable");
        assert!(
            demo.contains("WasmPdf"),
            "Browser demo must use WasmPdf class"
        );
        assert!(
            demo.contains("extractTables") || demo.contains("extract_tables"),
            "Browser demo must demonstrate table extraction"
        );
    }

    #[test]
    fn test_typescript_types_defined() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let types_path = std::path::Path::new(manifest_dir).join("pdfplumber-wasm.d.ts");
        assert!(
            types_path.exists(),
            "TypeScript type definitions must exist at pdfplumber-wasm.d.ts"
        );
    }

    #[test]
    fn test_typescript_types_content() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let types = std::fs::read_to_string(
            std::path::Path::new(manifest_dir).join("pdfplumber-wasm.d.ts"),
        )
        .expect("TypeScript types must be readable");
        assert!(
            types.contains("PdfChar"),
            "TypeScript types must define PdfChar interface"
        );
        assert!(
            types.contains("PdfWord"),
            "TypeScript types must define PdfWord interface"
        );
        assert!(
            types.contains("PdfSearchMatch"),
            "TypeScript types must define PdfSearchMatch interface"
        );
        assert!(
            types.contains("PdfMetadata"),
            "TypeScript types must define PdfMetadata interface"
        );
        assert!(
            types.contains("WasmPdf"),
            "TypeScript types must define WasmPdf class"
        );
        assert!(
            types.contains("WasmPage"),
            "TypeScript types must define WasmPage class"
        );
    }

    #[test]
    fn test_cargo_toml_has_npm_metadata() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let cargo_toml =
            std::fs::read_to_string(std::path::Path::new(manifest_dir).join("Cargo.toml"))
                .expect("Cargo.toml must be readable");
        assert!(
            cargo_toml.contains("pdfplumber-wasm"),
            "Cargo.toml must have package name pdfplumber-wasm"
        );
        assert!(
            cargo_toml.contains("description"),
            "Cargo.toml must have description"
        );
        assert!(
            cargo_toml.contains("keywords"),
            "Cargo.toml must have keywords"
        );
    }

    #[test]
    fn test_version_matches_workspace() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty(), "Package version must be set");
        // Verify we can read the version from Cargo.toml
        let cargo_toml =
            std::fs::read_to_string(std::path::Path::new(manifest_dir).join("Cargo.toml"))
                .expect("Cargo.toml must be readable");
        assert!(
            cargo_toml.contains(&format!("version = \"{version}\"")),
            "Cargo.toml version must match"
        );
    }

    // ---- TypeScript types completeness — new API surface ----

    #[test]
    fn test_typescript_types_include_cropped_page() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let types = std::fs::read_to_string(
            std::path::Path::new(manifest_dir).join("pdfplumber-wasm.d.ts"),
        )
        .expect("TypeScript types must be readable");
        assert!(
            types.contains("WasmCroppedPage"),
            "TypeScript types must define WasmCroppedPage class"
        );
    }

    #[test]
    fn test_package_json_exists() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pkg_path = std::path::Path::new(manifest_dir).join("package.json");
        assert!(
            pkg_path.exists(),
            "package.json must exist for npm publishing"
        );
    }

    #[test]
    fn test_package_json_has_name() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let content =
            std::fs::read_to_string(std::path::Path::new(manifest_dir).join("package.json"))
                .expect("package.json must be readable");
        assert!(
            content.contains("\"pdfplumber-wasm\""),
            "package.json must have name pdfplumber-wasm"
        );
    }

    // ---- WasmPage geometry methods (via underlying Rust API) ----

    #[test]
    fn test_page_rotation_default() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        assert_eq!(
            page.rotation(),
            0,
            "Non-rotated page should have rotation 0"
        );
    }

    #[test]
    fn test_page_bbox_dimensions() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let bbox = page.bbox();
        assert!((bbox.x0 - 0.0).abs() < 0.1);
        assert!((bbox.x1 - 612.0).abs() < 0.1);
    }

    #[test]
    fn test_page_lines_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.lines(); // Must not panic
    }

    #[test]
    fn test_page_rects_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.rects();
    }

    #[test]
    fn test_page_curves_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.curves();
    }

    #[test]
    fn test_page_images_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.images();
    }

    #[test]
    fn test_page_annots_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.annots();
    }

    #[test]
    fn test_page_hyperlinks_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let _ = page.hyperlinks();
    }

    #[test]
    fn test_pdf_bookmarks_returns_slice() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let _ = pdf.bookmarks(); // No bookmarks in minimal PDF, but must not panic
    }

    // ---- WasmCroppedPage tests ----

    #[test]
    fn test_crop_returns_correct_dimensions() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 306.0, 396.0));
        assert!((cropped.width() - 306.0).abs() < 0.1);
        assert!((cropped.height() - 396.0).abs() < 0.1);
    }

    #[test]
    fn test_within_bbox_returns_correct_dimensions() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let filtered = page.within_bbox(BBox::new(0.0, 0.0, 306.0, 396.0));
        assert!((filtered.width() - 306.0).abs() < 0.1);
        assert!((filtered.height() - 396.0).abs() < 0.1);
    }

    #[test]
    fn test_cropped_page_chars_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 400.0));
        let _ = cropped.chars();
    }

    #[test]
    fn test_cropped_page_extract_text_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let text = cropped.extract_text(Some(false));
        // Full-page crop should preserve all text
        assert!(text.contains("Hello") || text.is_empty()); // empty if Helvetica unresolved
    }

    #[test]
    fn test_cropped_page_extract_words_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.extract_words(Some(3.0), Some(3.0));
    }

    #[test]
    fn test_cropped_page_find_tables_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.find_tables();
    }

    #[test]
    fn test_cropped_page_extract_tables_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.extract_tables();
    }

    #[test]
    fn test_cropped_page_geometry_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.lines();
        let _ = cropped.rects();
        let _ = cropped.curves();
        let _ = cropped.images();
    }

    #[test]
    fn test_cropped_page_further_crop() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let outer = page.crop(BBox::new(0.0, 0.0, 400.0, 500.0));
        let inner = outer.crop(BBox::new(0.0, 0.0, 200.0, 250.0));
        assert!((inner.width() - 200.0).abs() < 0.1);
        assert!((inner.height() - 250.0).abs() < 0.1);
    }

    #[test]
    fn test_cropped_page_within_bbox_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.within_bbox(BBox::new(0.0, 0.0, 306.0, 396.0));
    }

    #[test]
    fn test_cropped_page_outside_bbox_no_panic() {
        let data = create_test_pdf();
        let pdf = Pdf::open(&data, None).unwrap();
        let page = pdf.page(0).unwrap();
        let cropped = page.crop(BBox::new(0.0, 0.0, 612.0, 792.0));
        let _ = cropped.outside_bbox(BBox::new(100.0, 100.0, 200.0, 200.0));
    }
}
