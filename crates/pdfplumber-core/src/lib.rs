//! Backend-independent data types and algorithms for pdfplumber-rs.
//!
//! This crate provides the foundational types ([`BBox`], [`Char`], [`Word`],
//! [`Line`], [`Rect`], [`Table`], etc.) and algorithms (text grouping, table
//! detection) used by pdfplumber-rs. It has no required external dependencies —
//! all functionality is pure Rust.
//!
//! # Modules
//!
//! - [`geometry`] — Geometric primitives: [`Point`], [`BBox`], [`Ctm`], [`Orientation`]
//! - [`text`] — Character data: [`Char`], [`TextDirection`], CJK detection
//! - [`words`] — Word extraction: [`Word`], [`WordExtractor`], [`WordOptions`]
//! - [`layout`] — Text layout: [`TextLine`], [`TextBlock`], [`TextOptions`]
//! - [`shapes`] — Shapes from painted paths: [`Line`], [`Rect`], [`Curve`]
//! - [`edges`] — Edge derivation for table detection: [`Edge`], [`EdgeSource`]
//! - [`table`] — Table detection: [`Table`], [`TableFinder`], [`TableSettings`]
//! - [`images`] — Image extraction: [`Image`], [`ImageMetadata`]
//! - [`painting`] — Graphics state: [`Color`], [`GraphicsState`], [`PaintedPath`]
//! - [`path`] — Path construction: [`Path`], [`PathBuilder`], [`PathSegment`]
//! - [`encoding`] — Font encoding: [`FontEncoding`], [`EncodingResolver`]
//! - [`error`] — Errors and warnings: [`PdfError`], [`ExtractWarning`], [`ExtractOptions`]

#![deny(missing_docs)]

/// PDF annotation types.
pub mod annotation;
/// Edge derivation from geometric primitives for table detection.
pub mod edges;
/// Font encoding mapping (Standard, Windows, Mac, Custom).
pub mod encoding;
/// Error and warning types for PDF processing.
pub mod error;
/// Geometric primitives: Point, BBox, CTM, Orientation.
pub mod geometry;
/// PDF hyperlink types.
pub mod hyperlink;
/// Image extraction and metadata.
pub mod images;
/// Text layout: words → lines → blocks, reading order, text output.
pub mod layout;
/// Document-level metadata types.
pub mod metadata;
/// Graphics state, colors, dash patterns, and painted paths.
pub mod painting;
/// PDF path construction (MoveTo, LineTo, CurveTo, ClosePath).
pub mod path;
/// Shape extraction: Lines, Rects, Curves from painted paths.
pub mod shapes;
/// Table detection: lattice, stream, and explicit strategies.
pub mod table;
/// Character data types and CJK detection.
pub mod text;
/// Word extraction from characters based on spatial proximity.
pub mod words;

pub use annotation::{Annotation, AnnotationType};
pub use edges::{Edge, EdgeSource, derive_edges, edge_from_curve, edge_from_line, edges_from_rect};
pub use encoding::{EncodingResolver, FontEncoding, StandardEncoding};
pub use error::{ExtractOptions, ExtractResult, ExtractWarning, PdfError};
pub use geometry::{BBox, Ctm, Orientation, Point};
pub use hyperlink::Hyperlink;
pub use images::{Image, ImageMetadata, image_from_ctm};
pub use layout::{
    TextBlock, TextLine, TextOptions, blocks_to_text, cluster_lines_into_blocks,
    cluster_words_into_lines, sort_blocks_reading_order, split_lines_at_columns, words_to_text,
};
pub use metadata::DocumentMetadata;
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
