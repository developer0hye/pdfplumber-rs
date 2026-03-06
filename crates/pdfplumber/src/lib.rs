//! Extract chars, words, lines, rects, and tables from PDF documents
//! with precise coordinates.
//!
//! **pdfplumber** is a Rust library for extracting structured content from PDF
//! files. It is a Rust port of Python's
//! [pdfplumber](https://github.com/jsvine/pdfplumber), providing the same
//! coordinate-accurate extraction of characters, words, lines, rectangles,
//! curves, images, and tables.
//!
//! # Quick Start
//!
//! ```no_run
//! use pdfplumber::{Pdf, TextOptions};
//!
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! for page_result in pdf.pages_iter() {
//!     let page = page_result.unwrap();
//!     let text = page.extract_text(&TextOptions::default());
//!     println!("Page {}: {}", page.page_number(), text);
//! }
//! ```
//!
//! # Architecture
//!
//! The library is split into three crates:
//!
//! - **pdfplumber-core**: Backend-independent data types and algorithms
//! - **pdfplumber-parse**: PDF parsing (Layer 1) and content stream interpreter (Layer 2)
//! - **pdfplumber** (this crate): Public API facade that ties everything together
//!
//! # Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `std` | Yes | Enables file-path APIs ([`Pdf::open_file`]). Disable for WASM. |
//! | `serde` | No | Adds `Serialize`/`Deserialize` to all public data types. |
//! | `parallel` | No | Enables `Pdf::pages_parallel()` via rayon. Not WASM-compatible. |
//!
//! # Extracting Text
//!
//! ```no_run
//! # use pdfplumber::{Pdf, TextOptions};
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! let page = pdf.page(0).unwrap();
//!
//! // Simple text extraction
//! let text = page.extract_text(&TextOptions::default());
//!
//! // Layout-preserving text extraction
//! let text = page.extract_text(&TextOptions { layout: true, ..Default::default() });
//! ```
//!
//! # Extracting Tables
//!
//! ```no_run
//! # use pdfplumber::{Pdf, TableSettings};
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! let page = pdf.page(0).unwrap();
//! let tables = page.find_tables(&TableSettings::default());
//! for table in &tables {
//!     for row in &table.rows {
//!         let cells: Vec<&str> = row.iter()
//!             .map(|c| c.text.as_deref().unwrap_or(""))
//!             .collect();
//!         println!("{:?}", cells);
//!     }
//! }
//! ```
//!
//! # WASM Support
//!
//! This crate compiles for `wasm32-unknown-unknown`. For WASM builds, disable
//! the default `std` feature and use the bytes-based API:
//!
//! ```toml
//! [dependencies]
//! pdfplumber = { version = "0.1", default-features = false }
//! ```
//!
//! Then use [`Pdf::open`] with a byte slice:
//!
//! ```ignore
//! let pdf = Pdf::open(pdf_bytes, None)?;
//! let page = pdf.page(0)?;
//! let text = page.extract_text(&TextOptions::default());
//! ```
//!
//! The `parallel` feature is not available for WASM targets (rayon requires OS threads).

#![deny(missing_docs)]

mod cropped_page;
#[cfg(feature = "ollama-fallback")]
pub mod ollama;
mod page;
mod pdf;

pub use cropped_page::CroppedPage;
pub use page::Page;
#[cfg(feature = "ollama-fallback")]
pub use pdf::PdfWithFallback;
pub use pdf::{PagesIter, Pdf};

/// A page view produced by [`Page::filter`] or [`CroppedPage::filter`].
///
/// `FilteredPage` is a type alias for [`CroppedPage`] — it supports all the
/// same query methods (`chars()`, `extract_text()`, `find_tables()`, etc.)
/// and can be filtered again for composable filtering chains.
pub type FilteredPage = CroppedPage;
pub use pdfplumber_core::{
    Annotation, AnnotationType, BBox, Bookmark, Cell, Char, Color, ColumnMode, Ctm, Curve,
    DashPattern, DedupeOptions, DocumentMetadata, DrawStyle, Edge, EdgeSource, EncodingResolver,
    ExplicitLines, ExportedImage, ExtGState, ExtractOptions, ExtractResult, ExtractWarning,
    FieldType, FillRule, FontEncoding, ForensicReport, FormField, GraphicsState, HtmlOptions,
    HtmlRenderer, Hyperlink, Image, ImageContent, ImageExportOptions, ImageFilter, ImageFormat,
    ImageMetadata, IncrementalUpdate, Intersection, Line, LineOrientation, MetadataFinding,
    Orientation, PageGeometryAnomaly, PageObject, PageRegionOptions, PageRegions, PaintedPath,
    Path, PathBuilder, PathSegment, PdfError, Point, ProducerKind, Rect, RepairOptions,
    RepairResult, SearchMatch, SearchOptions, Severity, SignatureInfo, StandardEncoding, Strategy,
    StructElement, SvgDebugOptions, SvgOptions, SvgRenderer, Table, TableFinder, TableFinderDebug,
    TableQuality, TableSettings, TextBlock, TextDirection, TextLine, TextOptions, UnicodeNorm,
    ValidationIssue, WatermarkFinding, WatermarkKind, Word, WordExtractor, WordOptions,
    blocks_to_text, cells_to_tables, cluster_lines_into_blocks, cluster_words_into_lines,
    derive_edges, detect_columns, detect_incremental_updates, edge_from_curve, edge_from_line,
    edges_from_rect, edges_to_cells, edges_to_intersections, explicit_lines_to_edges,
    export_image_set, extract_shapes, extract_text_for_cells, extract_text_for_cells_with_options,
    image_from_ctm, intersections_to_cells, is_cjk, is_cjk_text, join_edge_group,
    normalize_table_columns, snap_edges, sort_blocks_column_order, sort_blocks_reading_order,
    split_lines_at_columns, words_to_edges_stream, words_to_text,
};
pub use pdfplumber_parse::{
    self, CharEvent, ContentHandler, ImageEvent, LopdfBackend, LopdfDocument, LopdfPage,
    PageGeometry, PaintOp, PathEvent, PdfBackend,
};

/// PDF writing and incremental update API (requires `write` feature).
///
/// See [`write::PdfWriter`] for the main entry point.
///
/// # Example
///
/// ```no_run
/// use pdfplumber::write::{PdfWriter, AnnotationColor};
/// use pdfplumber::{Pdf, BBox};
///
/// let bytes = std::fs::read("doc.pdf").unwrap();
/// let pdf = Pdf::open(bytes.clone().into(), None).unwrap();
/// let mut writer = PdfWriter::new(&pdf, &bytes);
/// writer.add_highlight(0, BBox { x0: 72.0, y0: 700.0, x1: 300.0, y1: 720.0 }, AnnotationColor::Yellow).unwrap();
/// let updated = writer.write_incremental().unwrap();
/// std::fs::write("annotated.pdf", updated).unwrap();
/// ```
#[cfg(feature = "write")]
pub mod write;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
