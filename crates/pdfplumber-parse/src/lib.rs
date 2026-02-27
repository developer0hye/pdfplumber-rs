//! pdfplumber-parse: PDF parsing backend and content stream interpreter.
//!
//! This crate implements Layer 1 (PDF parsing via pluggable backends) and
//! Layer 2 (content stream interpretation) of the pdfplumber-rs architecture.
//! It depends on pdfplumber-core for shared data types.

pub mod error;

pub use error::BackendError;
pub use pdfplumber_core;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
