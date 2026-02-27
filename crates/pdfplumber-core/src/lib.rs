//! pdfplumber-core: Backend-independent data types and algorithms.
//!
//! This crate provides the foundational types (BBox, Char, Word, Line, Rect, etc.)
//! and algorithms (text grouping, table detection) used by pdfplumber-rs.
//! It has no external dependencies — all functionality is pure Rust.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
