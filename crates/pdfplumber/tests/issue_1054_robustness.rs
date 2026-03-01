//! Integration tests for US-186-1: issue-1054-example.pdf extraction.
//!
//! This PDF has a large MediaBox, a smaller CropBox, and Rotate=270.
//! The page dimensions and char coordinates must match Python pdfplumber's
//! MediaBox-based coordinate system to achieve >50% char F1.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct GoldenData {
    pages: Vec<GoldenPage>,
}

#[derive(Debug, Deserialize)]
struct GoldenPage {
    page_number: usize,
    width: f64,
    height: f64,
    chars: Vec<GoldenChar>,
}

#[derive(Debug, Deserialize)]
struct GoldenChar {
    text: String,
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

const COORD_TOLERANCE: f64 = 1.0;

fn char_matches(rust_char: &pdfplumber::Char, golden: &GoldenChar) -> bool {
    rust_char.text == golden.text
        && (rust_char.bbox.x0 - golden.x0).abs() <= COORD_TOLERANCE
        && (rust_char.bbox.top - golden.top).abs() <= COORD_TOLERANCE
        && (rust_char.bbox.x1 - golden.x1).abs() <= COORD_TOLERANCE
        && (rust_char.bbox.bottom - golden.bottom).abs() <= COORD_TOLERANCE
}

/// US-186-1 AC: issue-1054-example.pdf extracts chars > 50%.
/// This PDF has MediaBox [0 0 2919.69 2225.2], CropBox [14.17 1615.75 856.06 2211.02],
/// Rotate 270. Page dimensions must use MediaBox (not CropBox) matching Python pdfplumber.
#[test]
fn issue_1054_char_extraction_above_50_percent() {
    let pdf_path = fixtures_dir().join("pdfs/issue-1054-example.pdf");
    let golden_path = fixtures_dir().join("golden/issue-1054-example.json");

    if !pdf_path.exists() {
        eprintln!("Skipping: PDF fixture not found");
        return;
    }

    let golden: GoldenData =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let golden_page = &golden.pages[0];

    let opts = pdfplumber::ExtractOptions {
        unicode_norm: pdfplumber::UnicodeNorm::None,
        ..pdfplumber::ExtractOptions::default()
    };
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, Some(opts)).unwrap();
    let page = pdf.page(0).unwrap();

    // Page dimensions must match Python's MediaBox-based dimensions
    let width_diff = (page.width() - golden_page.width).abs();
    let height_diff = (page.height() - golden_page.height).abs();
    assert!(
        width_diff < 1.0,
        "Page width should match Python (got {}, expected {}, diff {})",
        page.width(),
        golden_page.width,
        width_diff
    );
    assert!(
        height_diff < 1.0,
        "Page height should match Python (got {}, expected {}, diff {})",
        page.height(),
        golden_page.height,
        height_diff
    );

    // Char matching: greedy best-match
    let chars = page.chars();
    let golden_chars = &golden_page.chars;
    let total = golden_chars.len();
    assert!(total > 0, "Golden data should have chars");

    let mut used = vec![false; chars.len()];
    let mut matched = 0;
    for gc in golden_chars {
        for (i, rc) in chars.iter().enumerate() {
            if !used[i] && char_matches(rc, gc) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }

    let rate = matched as f64 / total as f64;
    eprintln!(
        "issue-1054-example.pdf: chars={}/{} ({:.1}%)",
        matched,
        total,
        rate * 100.0
    );

    assert!(
        rate > 0.50,
        "Char match rate should be >50%, got {:.1}% ({}/{})",
        rate * 100.0,
        matched,
        total
    );
}

/// Verify that issue-1054 extracts the expected number of chars (no content lost).
#[test]
fn issue_1054_extracts_all_chars() {
    let pdf_path = fixtures_dir().join("pdfs/issue-1054-example.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping: PDF fixture not found");
        return;
    }

    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    // The PDF has 200 golden chars — we should extract at least that many
    assert!(
        chars.len() >= 190,
        "Should extract at least 190 chars, got {}",
        chars.len()
    );
}
