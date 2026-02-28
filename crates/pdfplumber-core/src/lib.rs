//! pdfplumber-core: Backend-independent data types and algorithms.
//!
//! This crate provides the foundational types (BBox, Char, Word, Line, Rect, etc.)
//! and algorithms (text grouping, table detection) used by pdfplumber-rs.
//! It has no external dependencies — all functionality is pure Rust.

pub mod edges;
pub mod encoding;
pub mod error;
pub mod geometry;
pub mod images;
pub mod layout;
pub mod painting;
pub mod path;
pub mod shapes;
pub mod table;
pub mod text;
pub mod words;

pub use edges::{Edge, EdgeSource, derive_edges, edge_from_curve, edge_from_line, edges_from_rect};
pub use encoding::{EncodingResolver, FontEncoding, StandardEncoding};
pub use error::{ExtractOptions, ExtractResult, ExtractWarning, PdfError};
pub use geometry::{BBox, Ctm, Orientation, Point};
pub use images::{Image, ImageMetadata, image_from_ctm};
pub use layout::{
    TextBlock, TextLine, TextOptions, blocks_to_text, cluster_lines_into_blocks,
    cluster_words_into_lines, sort_blocks_reading_order, split_lines_at_columns, words_to_text,
};
pub use painting::{Color, DashPattern, ExtGState, FillRule, GraphicsState, PaintedPath};
pub use path::{Path, PathBuilder, PathSegment};
pub use shapes::{Curve, Line, LineOrientation, Rect, extract_shapes};
pub use table::{
    Cell, ExplicitLines, Intersection, Strategy, Table, TableFinder, TableSettings,
    cells_to_tables, edges_to_intersections, explicit_lines_to_edges, extract_text_for_cells,
    intersections_to_cells, join_edge_group, snap_edges, words_to_edges_stream,
};
pub use text::{Char, TextDirection, is_cjk, is_cjk_text};
pub use words::{Word, WordExtractor, WordOptions};
