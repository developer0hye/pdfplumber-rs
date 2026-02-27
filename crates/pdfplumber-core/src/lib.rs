//! pdfplumber-core: Backend-independent data types and algorithms.
//!
//! This crate provides the foundational types (BBox, Char, Word, Line, Rect, etc.)
//! and algorithms (text grouping, table detection) used by pdfplumber-rs.
//! It has no external dependencies — all functionality is pure Rust.

pub mod geometry;
pub mod text;
pub mod words;

pub use geometry::BBox;
pub use text::Char;
pub use words::{Word, WordExtractor, WordOptions};
