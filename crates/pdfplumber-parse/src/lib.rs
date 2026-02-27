//! pdfplumber-parse: PDF parsing backend and content stream interpreter.
//!
//! This crate implements Layer 1 (PDF parsing via pluggable backends) and
//! Layer 2 (content stream interpretation) of the pdfplumber-rs architecture.
//! It depends on pdfplumber-core for shared data types.

pub mod backend;
pub mod error;
pub mod handler;

pub use backend::PdfBackend;
pub use error::BackendError;
pub use handler::{CharEvent, ContentHandler, ImageEvent, PaintOp, PathEvent};
pub use pdfplumber_core;
