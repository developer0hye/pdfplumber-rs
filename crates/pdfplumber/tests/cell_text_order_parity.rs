//! Parity tests for the text inside a table cell.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! A cell's text is read exactly as page text is: words in extraction order,
//! broken into lines by the same vertical banding. Sorting a cell's words by
//! height instead reorders a heading whose parts sit a fraction apart.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn tables(name: &str) -> Vec<Vec<Vec<Option<String>>>> {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())
}

#[test]
fn cell_words_keep_their_reading_order() {
    // issue-90-example.pdf heads a column with "# Executed Elements", the "#"
    // sitting a fraction above "Executed". pdfplumber reads it in that order.
    let tables = tables("issue-90-example.pdf");

    assert_eq!(
        tables[1][0][0].as_deref(),
        Some("# Executed ofnI\nElements")
    );
    assert_eq!(
        tables[3][0][1].as_deref(),
        Some("Base GOE The Judges Panel Ref\nValue (in random order)")
    );
}

#[test]
fn a_multi_line_cell_keeps_its_line_breaks() {
    let tables = tables("issue-90-example.pdf");

    assert_eq!(
        tables[2][0][1].as_deref(),
        Some(
            "Total Total Total Total\nSegment Element Program Component Deductions\nScore Score Score (factored)"
        )
    );
}
