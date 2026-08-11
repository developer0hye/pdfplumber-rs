//! Parity tests for repeated glyphs.
//!
//! Some producers fake bold by drawing the same string several times a fraction
//! of a point apart. Python pdfplumber reports every one of those draws and
//! leaves removing them to an explicit `page.dedupe_chars()`:
//!
//! ```python
//! len(page.chars)                    # every draw
//! len(page.dedupe_chars().chars)     # one per position
//! ```

use std::path::PathBuf;

use pdfplumber::{DedupeOptions, Pdf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn char_count(name: &str) -> usize {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    pdf.page(0).unwrap().chars().len()
}

#[test]
fn every_draw_of_a_repeated_glyph_is_reported() {
    // issue-842-example.pdf draws each line four times, offset by 0.24pt.
    // Python pdfplumber: 430 characters on page 1.
    assert_eq!(char_count("issue-842-example.pdf"), 430);
}

#[test]
fn duplicate_char_fixtures_match_python_pdfplumber() {
    // Counts from Python pdfplumber 0.11.10, page 1 of each.
    assert_eq!(char_count("issue-71-duplicate-chars.pdf"), 1226);
    assert_eq!(char_count("issue-1114-dedupe-chars.pdf"), 126);
}

#[test]
fn dedupe_chars_removes_the_repeats_on_request() {
    let pdf = Pdf::open_file(fixture("issue-842-example.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let all = page.chars().len();
    let deduped = page.dedupe_chars(&DedupeOptions::default()).chars().len();

    assert_eq!(all, 430);
    assert!(
        deduped < all,
        "dedupe should drop repeats: {deduped} is not fewer than {all}"
    );
    // TODO(dedupe-parity): pdfplumber keeps 179 here and we keep fewer, so the
    // two dedupe rules still differ; only the default path is pinned for now.
}
