//! Cross-validation tests: compare pdfplumber-rs output against Python pdfplumber golden data.
//!
//! Run with: `cargo test -p pdfplumber --test cross_validation -- --nocapture`
//!
//! # Status
//!
//! All char/word/line/rect metrics at or above PRD targets (95%+).
//! - **scotus-transcript**: 1 char gap (synthetic `\n` from Python layout analysis).
//! - **nics-background-checks tables**: Table cell accuracy ~6.8% (needs investigation).

mod common;

use common::*;

// ─── Test functions ─────────────────────────────────────────────────────────

/// issue-33-lorem-ipsum.pdf: simple text with tables.
/// All metrics at 100%.
#[test]
fn cross_validate_lorem_ipsum() {
    let result = validate_pdf("issue-33-lorem-ipsum.pdf");
    assert!(result.parse_error.is_none(), "parse error");
    assert!(
        result.total_char_rate() >= CHAR_THRESHOLD,
        "char rate {:.1}% < {:.1}%",
        result.total_char_rate() * 100.0,
        CHAR_THRESHOLD * 100.0,
    );
    assert!(
        result.total_word_rate() >= WORD_THRESHOLD,
        "word rate {:.1}% < {:.1}%",
        result.total_word_rate() * 100.0,
        WORD_THRESHOLD * 100.0,
    );
    assert!(
        result.total_line_rate() >= 1.0,
        "line rate {:.1}% < 100%",
        result.total_line_rate() * 100.0,
    );
    assert!(
        result.total_table_rate() >= TABLE_THRESHOLD,
        "table rate {:.1}% < {:.1}%",
        result.total_table_rate() * 100.0,
        TABLE_THRESHOLD * 100.0,
    );
}

/// pdffill-demo.pdf: text + form fields.
/// All metrics at 100%.
#[test]
fn cross_validate_pdffill_demo() {
    let result = validate_pdf("pdffill-demo.pdf");
    assert!(result.parse_error.is_none(), "parse error");
    assert!(
        result.total_char_rate() >= CHAR_THRESHOLD,
        "char rate {:.1}% < {:.1}%",
        result.total_char_rate() * 100.0,
        CHAR_THRESHOLD * 100.0,
    );
    assert!(
        result.total_word_rate() >= WORD_THRESHOLD,
        "word rate {:.1}% < {:.1}%",
        result.total_word_rate() * 100.0,
        WORD_THRESHOLD * 100.0,
    );
    assert!(
        result.total_line_rate() >= 1.0,
        "line rate {:.1}% < 100%",
        result.total_line_rate() * 100.0,
    );
    assert!(
        result.total_rect_rate() >= 1.0,
        "rect rate {:.1}% < 100%",
        result.total_rect_rate() * 100.0,
    );
}

/// scotus-transcript-p1.pdf: dense multi-column text with inline images.
/// chars=99.9%, words=100%.
#[test]
fn cross_validate_scotus_transcript() {
    let result = validate_pdf("scotus-transcript-p1.pdf");
    assert!(result.parse_error.is_none(), "parse error");
    assert!(
        result.total_char_rate() >= CHAR_THRESHOLD,
        "char rate {:.1}% < {:.1}%",
        result.total_char_rate() * 100.0,
        CHAR_THRESHOLD * 100.0,
    );
    assert!(
        result.total_word_rate() >= WORD_THRESHOLD,
        "word rate {:.1}% < {:.1}%",
        result.total_word_rate() * 100.0,
        WORD_THRESHOLD * 100.0,
    );
}

/// nics-background-checks-2015-11.pdf: complex lattice table.
/// Chars/words/lines/rects at 100%. Table accuracy ~6.8% (needs investigation).
#[test]
fn cross_validate_nics_background_checks() {
    let result = validate_pdf("nics-background-checks-2015-11.pdf");
    assert!(result.parse_error.is_none(), "parse error");
    assert!(
        result.total_char_rate() >= CHAR_THRESHOLD,
        "char rate {:.1}% < {:.1}%",
        result.total_char_rate() * 100.0,
        CHAR_THRESHOLD * 100.0,
    );
    assert!(
        result.total_word_rate() >= WORD_THRESHOLD,
        "word rate {:.1}% < {:.1}%",
        result.total_word_rate() * 100.0,
        WORD_THRESHOLD * 100.0,
    );
    assert!(
        result.total_line_rate() >= 1.0,
        "line rate {:.1}% < 100%",
        result.total_line_rate() * 100.0,
    );
    assert!(
        result.total_rect_rate() >= 1.0,
        "rect rate {:.1}% < 100%",
        result.total_rect_rate() * 100.0,
    );
}

/// Combined summary across all test PDFs (informational, never fails).
#[test]
fn cross_validate_all_summary() {
    let pdfs = [
        "issue-33-lorem-ipsum.pdf",
        "pdffill-demo.pdf",
        "scotus-transcript-p1.pdf",
        "nics-background-checks-2015-11.pdf",
    ];

    eprintln!("\n========================================");
    eprintln!("Cross-Validation Summary");
    eprintln!(
        "PRD targets: chars/words >= {:.0}%, tables >= {:.0}%",
        CHAR_THRESHOLD * 100.0,
        TABLE_THRESHOLD * 100.0
    );
    eprintln!("========================================");

    for pdf_name in &pdfs {
        let result = validate_pdf(pdf_name);
        if result.parse_error.is_some() {
            continue;
        }
        let char_ok = result.total_char_rate() >= CHAR_THRESHOLD;
        let word_ok = result.total_word_rate() >= WORD_THRESHOLD;
        let status = if char_ok && word_ok {
            "PASS"
        } else {
            "BELOW TARGET"
        };
        eprintln!("  {} -> {}", pdf_name, status);
    }
    eprintln!("========================================\n");
}
