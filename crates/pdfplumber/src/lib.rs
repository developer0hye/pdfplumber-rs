//! pdfplumber: Extract chars, words, lines, rects, and tables from PDF documents.
//!
//! This is the public API facade crate for pdfplumber-rs. It re-exports types from
//! pdfplumber-core and uses pdfplumber-parse for PDF reading and interpretation.
//!
//! # Architecture
//!
//! - **pdfplumber-core**: Backend-independent data types and algorithms
//! - **pdfplumber-parse**: PDF parsing (Layer 1) and content stream interpreter (Layer 2)
//! - **pdfplumber** (this crate): Public API that ties everything together
//!
//! # Feature Flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `std` | Yes | Enables file-path APIs (`Pdf::open_file`). Disable for WASM. |
//! | `serde` | No | Adds Serialize/Deserialize to all public data types. |
//! | `parallel` | No | Enables `Pdf::pages_parallel()` via rayon. Not WASM-compatible. |
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

mod cropped_page;
mod page;
mod pdf;

pub use cropped_page::CroppedPage;
pub use page::Page;
pub use pdf::{PagesIter, Pdf};
pub use pdfplumber_core::{
    BBox, Cell, Char, Color, Ctm, Curve, DashPattern, Edge, EdgeSource, EncodingResolver,
    ExplicitLines, ExtGState, ExtractOptions, ExtractResult, ExtractWarning, FillRule,
    FontEncoding, GraphicsState, Image, ImageMetadata, Intersection, Line, LineOrientation,
    Orientation, PaintedPath, Path, PathBuilder, PathSegment, PdfError, Point, Rect,
    StandardEncoding, Strategy, Table, TableFinder, TableSettings, TextBlock, TextDirection,
    TextLine, TextOptions, Word, WordExtractor, WordOptions, blocks_to_text, cells_to_tables,
    cluster_lines_into_blocks, cluster_words_into_lines, derive_edges, edge_from_curve,
    edge_from_line, edges_from_rect, edges_to_intersections, explicit_lines_to_edges,
    extract_shapes, extract_text_for_cells, image_from_ctm, intersections_to_cells, is_cjk,
    is_cjk_text, join_edge_group, snap_edges, sort_blocks_reading_order, split_lines_at_columns,
    words_to_edges_stream, words_to_text,
};
pub use pdfplumber_parse::{
    self, CharEvent, ContentHandler, ImageEvent, LopdfBackend, LopdfDocument, LopdfPage,
    PageGeometry, PaintOp, PathEvent, PdfBackend,
};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
