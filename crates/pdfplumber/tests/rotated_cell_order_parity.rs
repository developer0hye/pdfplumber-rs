//! Parity tests for the order words are read in on a rotated page.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! pdfplumber flips its two reading directions for text that is not upright:
//! `line_dir_rotated` defaults to `char_dir` (`ltr`) and `char_dir_rotated` to
//! `line_dir` (`ttb`). So on a rotated page the "lines" are columns taken left
//! to right by `x0`, and the words within a column run top to bottom.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn cell(name: &str, table: usize, row: usize, col: usize) -> String {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())[table][row][col]
        .clone()
        .unwrap_or_default()
}

#[test]
fn a_rotated_header_cell_reads_left_to_right() {
    // The state names stand sideways in one header cell, each its own column.
    // Taken left to right by x0 they read Wyoming first; "West Virginia" wraps,
    // so its second word lands on a line of its own.
    assert_eq!(
        cell("nics-background-checks-2015-11-rotated.pdf", 0, 0, 4),
        "Wyoming Wisconsin West Washington Virginia\nVirginia",
    );
}

#[test]
fn every_rotated_header_cell_matches_python() {
    // Neighbouring cells of the same header row, so the rule is not tuned to
    // one cell's spacing.
    assert_eq!(
        cell("nics-background-checks-2015-11-rotated.pdf", 0, 0, 5),
        "Virgin Vermont Utah Texas Tennessee\nIslands",
    );
    assert_eq!(
        cell("nics-background-checks-2015-11-rotated.pdf", 0, 0, 6),
        "South South Rhode Puerto Pennsylvania\nDakota Carolina Island Rico",
    );
}

#[test]
fn an_upright_page_is_unaffected() {
    // The same report without the rotation: upright text keeps its own
    // directions (lines top to bottom, words left to right).
    let pdf = Pdf::open_file(fixture("nics-background-checks-2015-11.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.extract_tables(&TableSettings::default());
    assert_eq!(
        tables[0][0][0].as_deref(),
        Some("NICS Firearm Background Checks\nNovember - 2015"),
    );
}
