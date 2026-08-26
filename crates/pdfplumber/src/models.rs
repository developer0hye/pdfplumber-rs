//! Curated, stable data models for ordinary extraction workflows.
//!
//! This module is the documented model boundary for the `0.3.x` Rust API. It
//! groups the character, word, geometry, table, metadata, warning, and option
//! types returned by or passed to the high-level [`crate::Pdf`] and
//! [`crate::Page`] APIs. See the
//! [data-model contract](https://github.com/developer0hye/pdfplumber-rs/blob/main/docs/rust-data-models.md)
//! for units, coordinates, ordering, optional-field semantics, and
//! compatibility scope.
//!
//! The same items remain available from the crate root for source
//! compatibility. Other root exports are not part of this curated model
//! commitment. Serialized representations are a separate concern tracked by
//! DX-006 and are not stabilized by this module.

pub use pdfplumber_core::{
    BBox, Cell, Char, Color, ColumnMode, Curve, DedupeOptions, DocumentMetadata, ExplicitLines,
    ExtractOptions, ExtractWarning, ExtractWarningCode, Line, MetadataEntry, MetadataReference,
    MetadataValue, Orientation, RawDocumentMetadata, Rect, Strategy, Table, TableQuality,
    TableSettings, TextBlock, TextDirection, TextLine, TextOptions, UnicodeNorm, Word, WordOptions,
};
