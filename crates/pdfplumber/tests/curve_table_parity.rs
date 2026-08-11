//! Parity tests for tables ruled with curves.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! Some producers draw table rules as filled paths rather than lines or rects.
//! Each straight run between two consecutive points of such a path is a rule,
//! so a path has to contribute one edge per segment rather than a single chord
//! from its first point to its last.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

#[test]
fn a_curve_ruled_table_is_read_as_one_table() {
    let pdf = Pdf::open_file(fixture("table-curves-example.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.extract_tables(&TableSettings::default());

    // Python pdfplumber: a single 33-row table.
    assert_eq!(tables.len(), 1, "expected one table, got {}", tables.len());
    assert_eq!(tables[0].len(), 33);
    assert_eq!(
        tables[0][0][0].as_deref(),
        Some("System organ class"),
        "first header cell"
    );
    assert_eq!(tables[0][0].len(), 4, "the table has four columns");
}
