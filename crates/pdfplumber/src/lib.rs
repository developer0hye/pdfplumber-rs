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

pub use pdfplumber_core;
pub use pdfplumber_parse;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
