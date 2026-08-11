//! Parity tests for fonts that carry their encoding inside the font program.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_text()
//! ```
//!
//! TeX ships Computer Modern as embedded Type 1 fonts with neither an
//! `/Encoding` entry nor a `/ToUnicode` map: the code-to-glyph mapping lives in
//! the font program's own header. Without reading it, a mathematical paper's
//! symbols come out as unrelated ASCII.

use std::path::PathBuf;

use pdfplumber::{Pdf, TextOptions};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn page_text(name: &str) -> String {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    pdf.page(0).unwrap().extract_text(&TextOptions::default())
}

#[test]
fn computer_modern_symbols_are_decoded() {
    // issue-982-example.pdf: CMR10's own encoding maps code 6 to "Sigma" and
    // CMMI10's maps code 39 to "phi1", neither of which is what those codes
    // mean in StandardEncoding. pdfplumber reads "Martin Thoma x 2 2 Σ ϕ".
    let text = page_text("issue-982-example.pdf");
    let line = text
        .lines()
        .nth(8)
        .expect("the page has at least nine lines");

    assert_eq!(line, "Martin Thoma x 2 2 Σ ϕ");
}

#[test]
fn text_in_ordinary_fonts_is_unaffected() {
    let text = page_text("issue-982-example.pdf");

    assert!(
        text.contains("Martin Thoma"),
        "ordinary text should still read normally"
    );
}
