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
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pdf = Pdf::open_path("document.pdf", None)?;
//!     for page in pdf.pages() {
//!         let page = page?;
//!         let text = page.extract_text(&TextOptions::default());
//!         println!("Page {}: {}", page.page_number(), text);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! The high-level boundary is simple: ordinary applications should depend only on this crate.
//! [`Pdf`] is the canonical high-level entry point. Its options, errors, and extracted data are
//! re-exported here. Parser-internal types are intentionally not re-exported. Advanced parser work
//! can depend on the separate `pdfplumber-parse` crate explicitly. The
//! [public rustdoc contract](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-api.md)
//! defines the stable facade and the documentation gates applied to it.
//! Stable facade changes also follow the
//! [Rust deprecation policy](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-deprecation-policy.md).
//! The
//! [workspace and extraction architecture guide](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/architecture.md)
//! traces the parser, core algorithms, binding adapters, caches, extension boundaries, and
//! one complete text request without expanding the stable facade.
//!
//! The library is split into three crates:
//!
//! - **pdfplumber-core**: Backend-independent data types and algorithms
//! - **pdfplumber-parse**: PDF parsing (Layer 1) and content stream interpreter (Layer 2)
//! - **pdfplumber** (this crate): Public API facade that ties everything together
//!
//! # Opening inputs
//!
//! The canonical input family names the source explicitly:
//!
//! - [`Pdf::open_path`] reads a filesystem path (default `std` feature).
//! - [`Pdf::open_bytes`] parses an in-memory byte slice and works in WebAssembly.
//! - [`Pdf::open_reader`] consumes any synchronous [`std::io::Read`] source from
//!   its current position through end-of-file; it does not require `Seek`.
//!
//! All three return an owned [`Pdf`]. The document does not borrow the path,
//! byte slice, or reader after the constructor returns. Path and reader failures
//! have [`PdfErrorKind::Io`]; invalid PDF data has [`PdfErrorKind::Parse`].
//! Password-protected inputs use the matching `open_*_with_password` methods.
//! Best-effort repair is currently byte-only via [`Pdf::open_bytes_with_repair`].
//!
//! # Selecting and iterating pages
//!
//! [`Pdf::pages`] returns a borrowed [`Pages`] collection view. Creating the
//! view does not clone the document or interpret page content. Select one page
//! directly with `pdf.pages().get(0)?`, or process owned [`Page`] values on
//! demand with `for page in pdf.pages()` and propagate each result with `?`.
//! The iterator is double-ended and exact-sized, so selection from either end
//! does not require eagerly extracting every page.
//!
//! # Stable data models
//!
//! [`models`] is the curated data-model boundary for ordinary extraction.
//! Its [contract](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-data-models.md)
//! documents units, coordinate origins, ordering, optional fields, and the
//! compatibility scope for the `0.3.x` line. Root re-exports remain available
//! for source compatibility. With the optional `serde` feature, the curated
//! models follow the separate
//! [Serde JSON compatibility policy](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-serde-schema.md).
//!
//! # Errors and diagnostics
//!
//! [`PdfError`] is opaque and classified by [`PdfErrorKind`]. Its default
//! display and debug output is actionable but excludes source messages and
//! document content. [`PdfError::context`] exposes available zero-based page
//! and PDF indirect-object context, while [`std::error::Error::source`] keeps
//! the underlying cause available for opt-in protected diagnostics. See the
//! [Rust error guide](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-errors.md).
//!
//! The opaque `PdfError` has no public variants. Matching the removed
//! string-payload enum shape does not compile:
//!
//! ```compile_fail
//! use pdfplumber::PdfError;
//!
//! fn match_removed_variant(error: &PdfError) -> bool {
//!     match error {
//!         PdfError::ParseError(_) => true,
//!         _ => false,
//!     }
//! }
//! ```
//!
//! Classify it through [`PdfError::kind`] and include a wildcard because
//! [`PdfErrorKind`] is non-exhaustive:
//!
//! ```no_run
//! use pdfplumber::{PdfError, PdfErrorKind};
//!
//! fn is_parse_error(error: &PdfError) -> bool {
//!     match error.kind() {
//!         PdfErrorKind::Parse => true,
//!         _ => false,
//!     }
//! }
//! ```
//!
//! # Concurrency
//!
//! [`Pdf`], [`Pages`], [`PagesIter`], [`Page`], and [`CroppedPage`] implement
//! `Send` and `Sync`. Share a document as `Arc<Pdf>` for concurrent immutable
//! extraction. The document-wide object and image-byte resource budgets are
//! shared across all attempts. The optional [`Pdf::pages_parallel`] API keeps
//! results in page-index order; Python has a separate Global Interpreter Lock
//! boundary. See the full
//! [Rust concurrency contract](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-concurrency.md).
//!
//! # Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `std` | Yes | Enables file-path APIs ([`Pdf::open_path`]). Disable for WASM. |
//! | `serde` | No | Adds `Serialize`/`Deserialize`; curated-model JSON follows a versioned compatibility policy. |
//! | `parallel` | No | Enables `Pdf::pages_parallel()` via rayon. Not WASM-compatible. |
//!
//! Features are additive and their supported combinations are defined by the
//! [Rust feature policy](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-features.md).
//!
//! # Extracting Text
//!
//! ```no_run
//! # use pdfplumber::{Pdf, TextOptions};
//! let pdf = Pdf::open_path("document.pdf", None).unwrap();
//! let page = pdf.pages().get(0).unwrap();
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
//! let pdf = Pdf::open_path("document.pdf", None).unwrap();
//! let page = pdf.pages().get(0).unwrap();
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
//! pdfplumber = { version = "0.3", default-features = false }
//! ```
//!
//! Then use [`Pdf::open_bytes`] with a byte slice:
//!
//! ```ignore
//! let pdf = Pdf::open_bytes(pdf_bytes, None)?;
//! let page = pdf.pages().get(0)?;
//! let text = page.extract_text(&TextOptions::default());
//! ```
//!
//! The `parallel` feature is not available for WASM targets (rayon requires OS threads).

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod cropped_page;
pub mod models;
mod page;
mod pdf;

pub use cropped_page::CroppedPage;
pub use page::{Page, PageObjectKind};
pub use pdf::{Pages, PagesIter, Pdf};

/// A page view produced by [`Page::filter`] or [`CroppedPage::filter`].
///
/// `FilteredPage` is a type alias for [`CroppedPage`] — it supports all the
/// same query methods (`chars()`, `extract_text()`, `find_tables()`, etc.)
/// and can be filtered again for composable filtering chains.
pub type FilteredPage = CroppedPage;
pub use pdfplumber_core::{
    Annotation, AnnotationType, BBox, Bookmark, Cell, Char, Color, ColumnMode, Ctm, Curve,
    DEFAULT_SPLIT_PUNCTUATION, DashPattern, DedupeOptions, DocumentMetadata, DrawStyle, Edge,
    EdgeSource, EncodingResolver, ExplicitLines, ExportedImage, ExtGState, ExtractOptions,
    ExtractResult, ExtractWarning, ExtractWarningCode, FieldType, FillRule, FontEncoding,
    FormField, GraphicsState, HtmlOptions, HtmlRenderer, Hyperlink, Image, ImageContent,
    ImageExportOptions, ImageFilter, ImageFormat, ImageMetadata, Intersection, Line,
    LineOrientation, MetadataEntry, MetadataReference, MetadataValue, Orientation, PageObject,
    PageRegionOptions, PageRegions, PaintedPath, Path, PathBuilder, PathSegment, PdfError,
    PdfErrorContext, PdfErrorKind, PdfObjectId, PdfResourceLimit, Point, RawDocumentMetadata, Rect,
    RepairOptions, RepairResult, SearchMatch, SearchOptions, Severity, ShapeKind, SignatureInfo,
    StandardEncoding, Strategy, StructElement, SvgDebugOptions, SvgOptions, SvgRenderer, Table,
    TableFinder, TableFinderDebug, TableQuality, TableSettings, TextBlock, TextDirection, TextLine,
    TextOptions, UnicodeNorm, ValidationIssue, Word, WordExtractor, WordOptions, blocks_to_text,
    cells_to_tables, cluster_lines_into_blocks, cluster_words_into_lines, derive_edges,
    detect_columns, edge_from_curve, edge_from_line, edges_from_curve, edges_from_rect,
    edges_to_cells, edges_to_intersections, explicit_lines_to_edges, export_image_set,
    extract_shapes, extract_shapes_with_order, extract_text_for_cells,
    extract_text_for_cells_with_options, image_from_ctm, intersections_to_cells, is_cjk,
    is_cjk_text, join_edge_group, snap_edges, sort_blocks_column_order, sort_blocks_reading_order,
    split_lines_at_columns, words_to_edges_h, words_to_edges_stream, words_to_edges_v,
    words_to_text,
};
#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
