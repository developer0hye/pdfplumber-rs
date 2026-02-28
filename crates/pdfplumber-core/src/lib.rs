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
//! - [`search`] — Text search: [`SearchMatch`], [`SearchOptions`], [`search_chars`]
//! - [`unicode_norm`] — Unicode normalization: [`UnicodeNorm`], [`normalize_chars`]

#![deny(missing_docs)]

/// PDF annotation types.
pub mod annotation;
/// PDF bookmark / outline / table of contents types.
pub mod bookmark;
/// Duplicate character deduplication.
pub mod dedupe;
/// Edge derivation from geometric primitives for table detection.
pub mod edges;
/// Font encoding mapping (Standard, Windows, Mac, Custom).
pub mod encoding;
/// Error and warning types for PDF processing.
pub mod error;
/// PDF form field types for AcroForm extraction.
pub mod form_field;
/// Geometric primitives: Point, BBox, CTM, Orientation.
pub mod geometry;
/// HTML rendering for PDF page content.
pub mod html;
/// PDF hyperlink types.
pub mod hyperlink;
/// Image extraction and metadata.
pub mod images;
/// Text layout: words → lines → blocks, reading order, text output.
pub mod layout;
/// Markdown rendering for PDF page content.
pub mod markdown;
/// Document-level metadata types.
pub mod metadata;
/// PageObject enum for custom object filtering.
pub mod page_object;
/// Graphics state, colors, dash patterns, and painted paths.
pub mod painting;
/// PDF path construction (MoveTo, LineTo, CurveTo, ClosePath).
pub mod path;
/// PDF repair types for best-effort fixing of common PDF issues.
pub mod repair;
/// Text search with position — find text patterns and return matches with bounding boxes.
pub mod search;
/// Shape extraction: Lines, Rects, Curves from painted paths.
pub mod shapes;
/// PDF digital signature information types.
pub mod signature;
/// PDF structure tree types for tagged PDF access.
pub mod struct_tree;
/// SVG rendering for visual debugging of PDF pages.
pub mod svg;
/// Table detection: lattice, stream, and explicit strategies.
pub mod table;
/// Character data types and CJK detection.
pub mod text;
/// Unicode normalization for extracted text.
pub mod unicode_norm;
/// PDF validation types for detecting specification violations.
pub mod validation;
/// Word extraction from characters based on spatial proximity.
pub mod words;

pub use annotation::{Annotation, AnnotationType};
pub use bookmark::Bookmark;
pub use dedupe::{DedupeOptions, dedupe_chars};
pub use edges::{Edge, EdgeSource, derive_edges, edge_from_curve, edge_from_line, edges_from_rect};
pub use encoding::{EncodingResolver, FontEncoding, StandardEncoding, glyph_name_to_char};
pub use error::{ExtractOptions, ExtractResult, ExtractWarning, PdfError};
pub use form_field::{FieldType, FormField};
pub use geometry::{BBox, Ctm, Orientation, Point};
pub use html::{HtmlOptions, HtmlRenderer};
pub use hyperlink::Hyperlink;
pub use images::{Image, ImageContent, ImageFormat, ImageMetadata, image_from_ctm};
pub use layout::{
    TextBlock, TextLine, TextOptions, blocks_to_text, cluster_lines_into_blocks,
    cluster_words_into_lines, sort_blocks_reading_order, split_lines_at_columns, words_to_text,
};
pub use markdown::{MarkdownOptions, MarkdownRenderer};
pub use metadata::DocumentMetadata;
pub use page_object::PageObject;
pub use painting::{Color, DashPattern, ExtGState, FillRule, GraphicsState, PaintedPath};
pub use path::{Path, PathBuilder, PathSegment};
pub use repair::{RepairOptions, RepairResult};
pub use search::{SearchMatch, SearchOptions, search_chars};
pub use shapes::{Curve, Line, LineOrientation, Rect, extract_shapes};
pub use signature::SignatureInfo;
pub use struct_tree::StructElement;
pub use svg::{DrawStyle, SvgDebugOptions, SvgOptions, SvgRenderer};
pub use table::{
    Cell, ExplicitLines, Intersection, Strategy, Table, TableFinder, TableFinderDebug,
    TableQuality, TableSettings, cells_to_tables, edges_to_intersections, explicit_lines_to_edges,
    extract_text_for_cells, intersections_to_cells, join_edge_group, snap_edges,
    words_to_edges_stream,
};
pub use text::{Char, TextDirection, is_cjk, is_cjk_text};
pub use unicode_norm::{UnicodeNorm, normalize_chars};
pub use validation::{Severity, ValidationIssue};
pub use words::{Word, WordExtractor, WordOptions};
