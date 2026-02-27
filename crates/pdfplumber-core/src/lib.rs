//! pdfplumber-core: Backend-independent data types and algorithms.
//!
//! This crate provides the foundational types (BBox, Char, Word, Line, Rect, etc.)
//! and algorithms (text grouping, table detection) used by pdfplumber-rs.
//! It has no external dependencies — all functionality is pure Rust.

pub mod edges;
pub mod geometry;
pub mod images;
pub mod painting;
pub mod path;
pub mod shapes;
pub mod text;
pub mod words;

pub use edges::{Edge, EdgeSource, derive_edges, edge_from_curve, edge_from_line, edges_from_rect};
pub use geometry::{BBox, Ctm, Point};
pub use images::{Image, ImageMetadata, image_from_ctm};
pub use painting::{Color, FillRule, GraphicsState, PaintedPath};
pub use path::{Path, PathBuilder, PathSegment};
pub use shapes::{Curve, Line, LineOrientation, Rect, extract_shapes};
pub use text::{Char, TextDirection, is_cjk, is_cjk_text};
pub use words::{Word, WordExtractor, WordOptions};
