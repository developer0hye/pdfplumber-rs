//! Integration tests for US-186-3: issue-1181.pdf and issue-848.pdf.
//!
//! issue-1181.pdf has pages with non-zero MediaBox origins (e.g. [0 200 420 585]).
//! Coordinates must account for the MediaBox y-offset to match Python pdfplumber.
//!
//! issue-848.pdf has a Ghostscript preamble before the %PDF- header.
//! The parser must strip the preamble to successfully open the file.

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

/// issue-848.pdf opens without parse error.
#[test]
fn issue_848_opens_without_error() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping: issue-848.pdf not found");
        return;
    }

    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None);
    assert!(
        pdf.is_ok(),
        "issue-848.pdf should open without error, got: {:?}",
        pdf.err()
    );

    let pdf = pdf.unwrap();
    assert_eq!(pdf.page_count(), 8, "issue-848.pdf should have 8 pages");
}

/// issue-848.pdf extracts chars from all pages.
#[test]
fn issue_848_extracts_chars() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    let golden_path = fixtures_dir().join("golden/issue-848.json");
    if !pdf_path.exists() {
        eprintln!("Skipping: issue-848.pdf not found");
        return;
    }

    let golden: GoldenData =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();

    for golden_page in &golden.pages {
        let page = pdf.page(golden_page.page_number).unwrap();
        let chars = page.chars();
        assert!(
            !chars.is_empty(),
            "Page {} should extract chars",
            golden_page.page_number
        );
    }
}

/// issue-1181.pdf opens without parse error.
#[test]
fn issue_1181_opens_without_error() {
    let pdf_path = fixtures_dir().join("pdfs/issue-1181.pdf");
    if !pdf_path.exists() {
        eprintln!("Skipping: issue-1181.pdf not found");
        return;
    }

    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None);
    assert!(
        pdf.is_ok(),
        "issue-1181.pdf should open without error, got: {:?}",
        pdf.err()
    );

    let pdf = pdf.unwrap();
    assert_eq!(pdf.page_count(), 2, "issue-1181.pdf should have 2 pages");
}

/// issue-1181.pdf page 0 has non-zero MediaBox y-origin [0 200 420.9449 585.2756].
/// Char coordinates must account for this offset to match Python pdfplumber.
#[test]
fn issue_1181_page0_coordinates_match_golden() {
    let pdf_path = fixtures_dir().join("pdfs/issue-1181.pdf");
    let golden_path = fixtures_dir().join("golden/issue-1181.json");
    if !pdf_path.exists() {
        eprintln!("Skipping: issue-1181.pdf not found");
        return;
    }

    let golden: GoldenData =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let opts = pdfplumber::ExtractOptions {
        unicode_norm: pdfplumber::UnicodeNorm::None,
        ..pdfplumber::ExtractOptions::default()
    };
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, Some(opts)).unwrap();

    // Test page 0 (MediaBox [0 200 420.9449 585.2756] — non-zero y-origin)
    let golden_page0 = &golden.pages[0];
    let page0 = pdf.page(0).unwrap();
    let chars0 = page0.chars();

    // Page dimensions should match golden data
    assert!(
        (page0.width() - golden_page0.width).abs() < 1.0,
        "Page 0 width: got {}, expected {}",
        page0.width(),
        golden_page0.width
    );
    assert!(
        (page0.height() - golden_page0.height).abs() < 1.0,
        "Page 0 height: got {}, expected {}",
        page0.height(),
        golden_page0.height
    );

    // Match chars between Rust and golden data
    let total = golden_page0.chars.len();
    assert!(total > 0, "Golden page 0 should have chars");

    let mut used = vec![false; chars0.len()];
    let mut matched = 0;
    for gc in &golden_page0.chars {
        for (i, rc) in chars0.iter().enumerate() {
            if !used[i] && char_matches(rc, gc) {
                used[i] = true;
                matched += 1;
                break;
            }
        }
    }

    let rate = matched as f64 / total as f64;
    eprintln!(
        "issue-1181.pdf page 0: chars={}/{} ({:.1}%)",
        matched,
        total,
        rate * 100.0
    );

    // Page 0 should achieve >80% char match rate
    // (the non-zero MediaBox y-origin must be handled correctly)
    assert!(
        rate > 0.80,
        "Page 0 char match rate should be >80%, got {:.1}% ({}/{})",
        rate * 100.0,
        matched,
        total
    );
}

/// Both pages of issue-1181.pdf should have matching coordinates.
#[test]
fn issue_1181_both_pages_match_golden() {
    let pdf_path = fixtures_dir().join("pdfs/issue-1181.pdf");
    let golden_path = fixtures_dir().join("golden/issue-1181.json");
    if !pdf_path.exists() {
        eprintln!("Skipping: issue-1181.pdf not found");
        return;
    }

    let golden: GoldenData =
        serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    let opts = pdfplumber::ExtractOptions {
        unicode_norm: pdfplumber::UnicodeNorm::None,
        ..pdfplumber::ExtractOptions::default()
    };
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, Some(opts)).unwrap();

    for golden_page in &golden.pages {
        let page = pdf.page(golden_page.page_number).unwrap();
        let chars = page.chars();
        let total = golden_page.chars.len();

        let mut used = vec![false; chars.len()];
        let mut matched = 0;
        for gc in &golden_page.chars {
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
            "issue-1181.pdf page {}: chars={}/{} ({:.1}%)",
            golden_page.page_number,
            matched,
            total,
            rate * 100.0
        );

        assert!(
            rate > 0.80,
            "Page {} char match rate should be >80%, got {:.1}%",
            golden_page.page_number,
            rate * 100.0
        );
    }
}
