//! Parity tests for grouping words into output lines.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_text()
//! ```
//!
//! Words belong to the same line when their **tops** are within the vertical
//! tolerance of each other. A heading and a smaller caption printed beside it
//! can share a vertical centre while starting at very different heights, so
//! measuring from the centre would run them together.

use std::path::PathBuf;

use pdfplumber::{Pdf, TextOptions};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn first_lines(name: &str, count: usize) -> Vec<String> {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_text(&TextOptions::default())
        .lines()
        .take(count)
        .map(str::to_string)
        .collect()
}

#[test]
fn differently_sized_headings_stay_on_their_own_lines() {
    // federal-register-2020-17221.pdf sets "Proposed Rules" in 18pt at top
    // 54.726 and "Federal Register" in 8pt at top 59.016. The tops are 4.3pt
    // apart — beyond the tolerance — while the centres are 0.7pt apart.
    assert_eq!(
        first_lines("federal-register-2020-17221.pdf", 4),
        [
            "47698",
            "Proposed Rules",
            "Federal Register",
            "Vol. 85, No. 152",
        ]
    );
}

#[test]
fn words_sharing_a_top_stay_on_one_line() {
    // The counterpart: a page of uniform text must not gain line breaks.
    let lines = first_lines("issue-33-lorem-ipsum.pdf", 2);

    assert!(
        lines[0].split_whitespace().count() > 3,
        "expected a full line of text, got {:?}",
        lines[0]
    );
}
