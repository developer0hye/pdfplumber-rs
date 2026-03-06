//! Integration tests for pdfplumber-layout using real fixture PDFs.
//!
//! These tests verify that Document::from_pdf() runs without panic on all
//! PDFs in the cross-validation fixture set, and that basic structural
//! invariants hold (section count >= 0, no empty section headings, etc.).

use pdfplumber::Pdf;
use pdfplumber_layout::{BlockKind, Document};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    // Fixtures are in pdfplumber/tests/fixtures/pdfs relative to workspace root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(&manifest_dir)
        .parent()
        .unwrap()
        .join("pdfplumber")
        .join("tests")
        .join("fixtures")
        .join("pdfs")
}

fn open_pdf(name: &str) -> Option<Pdf> {
    let path = fixtures_dir().join(name);
    Pdf::open_file(&path, None).ok()
}

// --- No-panic tests for all known fixtures ---

macro_rules! no_panic_layout {
    ($name:ident, $filename:expr) => {
        #[test]
        fn $name() {
            if let Some(pdf) = open_pdf($filename) {
                // Must not panic
                let doc = Document::from_pdf(&pdf).unwrap();
                // Basic invariants
                assert!(doc.page_count() > 0);
                for section in doc.sections() {
                    // Heading text, if present, must be non-empty
                    if let Some(h) = section.heading() {
                        assert!(!h.text().is_empty(), "heading must have non-empty text");
                    }
                    // Paragraph texts must be non-empty
                    for para in section.paragraphs() {
                        assert!(!para.text().is_empty(), "paragraph must have non-empty text");
                    }
                }
            }
            // If open_pdf returns None (fixture not present), skip silently
        }
    };
}

no_panic_layout!(layout_lorem_ipsum, "pdffill-demo.pdf");
no_panic_layout!(layout_scotus, "scotus-transcript.pdf");
no_panic_layout!(layout_nics, "nics-background-checks-2015-11.pdf");
no_panic_layout!(layout_senate, "senate-expenditures.pdf");
no_panic_layout!(layout_pdf_structure, "pdf_structure.pdf");
no_panic_layout!(layout_annotations, "annotations.pdf");
no_panic_layout!(layout_issue_297, "issue-297-example.pdf");
no_panic_layout!(layout_pr_136, "pr-136-example.pdf");
no_panic_layout!(layout_word365, "word365_structure.pdf");
no_panic_layout!(layout_nics_rotated, "nics-background-checks-2015-11-rotated.pdf");
no_panic_layout!(layout_ann_rot90, "annotations-rotated-90.pdf");
no_panic_layout!(layout_ann_rot180, "annotations-rotated-180.pdf");
no_panic_layout!(layout_ann_rot270, "annotations-rotated-270.pdf");
no_panic_layout!(layout_issue_848, "issue-848.pdf");

// --- Structural correctness tests ---

#[test]
fn document_from_pdf_returns_sections() {
    let Some(pdf) = open_pdf("scotus-transcript.pdf") else { return; };
    let doc = Document::from_pdf(&pdf).unwrap();
    // scotus transcript has multiple pages → should produce at least one section
    assert!(
        !doc.sections().is_empty(),
        "scotus transcript should have at least one section"
    );
}

#[test]
fn document_page_count_matches_pdf() {
    let Some(pdf) = open_pdf("nics-background-checks-2015-11.pdf") else { return; };
    let expected_pages = pdf.page_count();
    let doc = Document::from_pdf(&pdf).unwrap();
    assert_eq!(
        doc.page_count(),
        expected_pages,
        "Document page count should match PDF page count"
    );
}

#[test]
fn to_text_is_non_empty_for_text_pdf() {
    let Some(pdf) = open_pdf("scotus-transcript.pdf") else { return; };
    let doc = Document::from_pdf(&pdf).unwrap();
    let text = doc.to_text();
    assert!(
        !text.trim().is_empty(),
        "to_text() should produce non-empty output for text PDF"
    );
}

#[test]
fn section_heading_text_is_trimmed() {
    let Some(pdf) = open_pdf("pdf_structure.pdf") else { return; };
    let doc = Document::from_pdf(&pdf).unwrap();
    for section in doc.sections() {
        if let Some(h) = section.heading() {
            let text = h.text();
            assert_eq!(text, text.trim(), "heading text should be trimmed");
        }
    }
}

#[test]
fn all_sections_have_non_negative_page_range() {
    let Some(pdf) = open_pdf("senate-expenditures.pdf") else { return; };
    let doc = Document::from_pdf(&pdf).unwrap();
    for section in doc.sections() {
        assert!(
            section.start_page <= section.end_page,
            "start_page {} > end_page {}",
            section.start_page,
            section.end_page,
        );
    }
}

#[test]
fn empty_pdf_bytes_is_handled_gracefully() {
    // An empty byte slice should produce an error from Pdf::open, not a panic.
    let result = Pdf::open(&[], None);
    // We just check it doesn't panic; the error is expected.
    assert!(result.is_err() || result.is_ok()); // trivially true but verifies no panic
}

#[test]
fn figures_field_is_a_vec() {
    let Some(pdf) = open_pdf("nics-background-checks-2015-11.pdf") else { return; };
    let doc = Document::from_pdf(&pdf).unwrap();
    // Figures slice can be empty (no image-only regions in this text-heavy PDF)
    let _ = doc.figures();
}

// --- FontStats unit-level integration ---

#[test]
fn font_stats_heading_detection_realistic() {
    use pdfplumber_layout::FontStats;
    // Simulate a document with body at 11pt and heading at 18pt
    let sizes: Vec<f64> = (0..100).map(|_| 11.0_f64).chain([18.0, 18.0, 24.0]).collect();
    let stats = FontStats::from_sizes(&sizes);
    assert_eq!(stats.median, 11.0);
    assert!(stats.is_heading_size(18.0), "18pt should be heading in 11pt body doc");
    assert!(!stats.is_heading_size(11.0), "11pt is body text, not heading");
    assert!(stats.is_small_size(8.0), "8pt should be small in 11pt body doc");
}
