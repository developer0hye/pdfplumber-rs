//! pdfplumber-core: Backend-independent data types and algorithms.
//!
//! This crate provides the foundational types (BBox, Char, Word, Line, Rect, etc.)
//! and algorithms (text grouping, table detection) used by pdfplumber-rs.
//! It has no external dependencies — all functionality is pure Rust.

pub mod geometry;
pub mod path;
pub mod text;
pub mod words;

pub use geometry::{BBox, Ctm, Point};
pub use path::{Path, PathBuilder, PathSegment};
pub use text::{Char, TextDirection, is_cjk, is_cjk_text};
pub use words::{Word, WordExtractor, WordOptions};
