//! Parity tests for where rotated text lands in a page's extracted text.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_text().split("\n")
//! ```
//!
//! Words are read in extraction order — upright text first, then anything set
//! at an angle — and a line ends when the next word sits beyond the vertical
//! tolerance of the one before it. Re-sorting everything by height instead
//! would drop a sideways stamp into the middle of the paragraph it sits beside.

use std::path::PathBuf;

use pdfplumber::{Pdf, TextOptions};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn text_lines(name: &str) -> Vec<String> {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_text(&TextOptions::default())
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_sideways_stamp_reads_after_the_upright_text() {
    // senate-expenditures.pdf carries "B-1191" rotated 90° at the page edge,
    // vertically level with a table row. pdfplumber reports it as its own,
    // final line rather than appending it to that row.
    let lines = text_lines("senate-expenditures.pdf");

    assert_eq!(lines.last().map(String::as_str), Some("B-1191"));
    assert!(
        lines
            .iter()
            .all(|line| line == "B-1191" || !line.contains("B-1191")),
        "the stamp should not be attached to another line"
    );
}

#[test]
fn upright_lines_are_unaffected() {
    let lines = text_lines("senate-expenditures.pdf");

    assert_eq!(
        lines[27],
        "AIRFAREFORMBERGWASHINGTONDCTOSPRINGFIELDANDRETURN"
    );
}
