//! Parity tests for the Unicode a PDF actually maps to.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [c["text"] for c in page.chars]
//! ```
//!
//! A font's ToUnicode mapping is what the document says its glyphs mean, and
//! pdfplumber hands it back unchanged. Normalising it — even canonically — can
//! change the code points a caller is searching for.

use std::path::PathBuf;

use pdfplumber::{ExtractOptions, Pdf, UnicodeNorm};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn chars_of(name: &str, options: Option<ExtractOptions>) -> Vec<String> {
    let bytes = std::fs::read(fixture(name)).unwrap();
    let pdf = Pdf::open(&bytes, options).unwrap();
    pdf.page(0)
        .unwrap()
        .chars()
        .iter()
        .map(|ch| ch.text.clone())
        .collect()
}

#[test]
fn a_greek_question_mark_stays_a_greek_question_mark() {
    // issue-905.pdf maps its first glyph to U+037E, which canonically
    // decomposes to an ordinary semicolon. pdfplumber reports U+037E.
    assert_eq!(chars_of("issue-905.pdf", None), ["\u{37e}", ";"]);
}

#[test]
fn normalization_is_available_on_request() {
    let options = ExtractOptions {
        unicode_norm: UnicodeNorm::Nfc,
        ..ExtractOptions::default()
    };

    assert_eq!(chars_of("issue-905.pdf", Some(options)), [";", ";"]);
}

#[test]
fn page_text_keeps_the_mapped_code_points() {
    let bytes = std::fs::read(fixture("issue-905.pdf")).unwrap();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(
        page.extract_text(&pdfplumber::TextOptions::default()),
        "\u{37e};"
    );
}
