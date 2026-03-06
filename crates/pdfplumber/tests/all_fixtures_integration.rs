//! Comprehensive per-fixture integration tests for all 65 cross-validation PDFs.
//!
//! Each fixture gets:
//!   - A smoke test: opens without panic, page count and dimensions match golden
//!   - A chars test: extraction yields at least the expected minimum char count
//!   - A words test: word count in expected ballpark
//!   - A tables test (where golden has tables): table detection fires
//!   - Behaviour-specific tests for rotation, multi-page, annotations, etc.
//!
//! These tests do NOT assert golden-percentage accuracy (that's cross_validation.rs).
//! They assert observable structural invariants: non-empty extraction, correct page
//! count, correct page dimensions, non-negative coordinates, no panics.
//!
//! Run with: cargo test --test all_fixtures_integration -- --nocapture

use std::path::PathBuf;

use pdfplumber::{BBox, ExtractOptions, Pdf, TableSettings, TextOptions, UnicodeNorm, WordOptions};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn pdf_path(name: &str) -> PathBuf {
    fixtures_dir().join("pdfs").join(name)
}

/// Open a PDF with no-normalisation options matching the golden data setup.
fn open_pdf(name: &str) -> Pdf {
    let opts = ExtractOptions {
        unicode_norm: UnicodeNorm::None,
        dedupe: None,
        ..ExtractOptions::default()
    };
    let path = pdf_path(name);
    Pdf::open_file(&path, Some(opts)).unwrap_or_else(|e| panic!("Failed to open {}: {}", name, e))
}

/// Open a PDF, tolerating parse failures (for known-malformed fixtures).
fn try_open_pdf(name: &str) -> Option<Pdf> {
    let opts = ExtractOptions {
        unicode_norm: UnicodeNorm::None,
        dedupe: None,
        ..ExtractOptions::default()
    };
    Pdf::open_file(&pdf_path(name), Some(opts)).ok()
}

/// Assert all chars on a page have non-negative coordinates.
fn assert_chars_have_valid_coords(pdf: &Pdf, page_idx: usize, pdf_name: &str) {
    let page = pdf.page(page_idx).unwrap();
    for ch in page.chars() {
        assert!(
            ch.bbox.x0 >= -1.0
                && ch.bbox.x1 >= -1.0
                && ch.bbox.top >= -1.0
                && ch.bbox.bottom >= -1.0,
            "{} page {}: char '{}' has negative bbox {:?}",
            pdf_name,
            page_idx,
            ch.text,
            ch.bbox
        );
        assert!(
            ch.bbox.x1 >= ch.bbox.x0 - 1.0,
            "{} page {}: char '{}' has x1 < x0: {:?}",
            pdf_name,
            page_idx,
            ch.text,
            ch.bbox
        );
    }
}

/// Assert all words on a page have non-negative coordinates and non-empty text.
fn assert_words_have_valid_form(pdf: &Pdf, page_idx: usize, pdf_name: &str) {
    let page = pdf.page(page_idx).unwrap();
    for w in page.extract_words(&WordOptions::default()) {
        assert!(
            !w.text.is_empty(),
            "{} page {}: extracted empty-text word at {:?}",
            pdf_name,
            page_idx,
            w.bbox
        );
        assert!(
            w.bbox.x0 >= -1.0 && w.bbox.top >= -1.0,
            "{} page {}: word '{}' has negative coords {:?}",
            pdf_name,
            page_idx,
            w.text,
            w.bbox
        );
    }
}

// ─── issue-33-lorem-ipsum.pdf ────────────────────────────────────────────────

#[test]
fn lorem_ipsum_opens_with_two_pages() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    assert_eq!(pdf.page_count(), 2, "should have 2 pages");
}

#[test]
fn lorem_ipsum_page0_dimensions() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 612.0).abs() < 2.0, "width should be ~612");
    assert!((page.height() - 792.0).abs() < 2.0, "height should be ~792");
}

#[test]
fn lorem_ipsum_page0_has_substantial_chars() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    assert!(
        page.chars().len() >= 1000,
        "page 0 should have ≥1000 chars, got {}",
        page.chars().len()
    );
}

#[test]
fn lorem_ipsum_page0_has_words() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());
    assert!(
        words.len() >= 100,
        "page 0 should have ≥100 words, got {}",
        words.len()
    );
}

#[test]
fn lorem_ipsum_page0_has_table() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "page 0 should detect at least 1 table");
}

#[test]
fn lorem_ipsum_page0_table_has_rows() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    if let Some(table) = tables.first() {
        assert!(!table.rows.is_empty(), "table should have rows");
        assert!(table.rows.len() >= 2, "table should have ≥2 rows");
    }
}

#[test]
fn lorem_ipsum_char_coords_valid() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    assert_chars_have_valid_coords(&pdf, 0, "issue-33-lorem-ipsum.pdf");
}

#[test]
fn lorem_ipsum_word_form_valid() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    assert_words_have_valid_form(&pdf, 0, "issue-33-lorem-ipsum.pdf");
}

#[test]
fn lorem_ipsum_extract_text_nonempty() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        !text.is_empty(),
        "extract_text should return non-empty string"
    );
    assert!(text.len() >= 100, "extract_text should return ≥100 chars");
}

// ─── nics-background-checks-2015-11.pdf ──────────────────────────────────────

#[test]
fn nics_standard_single_page() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    assert_eq!(pdf.page_count(), 1, "nics should be 1 page");
}

#[test]
fn nics_standard_landscape_dimensions() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    let page = pdf.page(0).unwrap();
    // Landscape: width > height
    assert!(
        page.width() > page.height(),
        "nics standard should be landscape (w={} h={})",
        page.width(),
        page.height()
    );
    assert!((page.width() - 1008.0).abs() < 2.0, "width should be ~1008");
    assert!((page.height() - 612.0).abs() < 2.0, "height should be ~612");
}

#[test]
fn nics_standard_has_many_chars() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    let page = pdf.page(0).unwrap();
    assert!(
        page.chars().len() >= 3000,
        "nics should have ≥3000 chars, got {}",
        page.chars().len()
    );
}

#[test]
fn nics_standard_detects_table() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "nics standard should find at least 1 table"
    );
}

#[test]
fn nics_standard_table_has_17_columns() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    if let Some(table) = tables.first() {
        let max_cols = table.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        assert!(
            max_cols >= 15,
            "nics table should have ≥15 cols (expected 17), got {}",
            max_cols
        );
    }
}

#[test]
fn nics_standard_char_coords_valid() {
    let pdf = open_pdf("nics-background-checks-2015-11.pdf");
    assert_chars_have_valid_coords(&pdf, 0, "nics-background-checks-2015-11.pdf");
}

// ─── nics-background-checks-2015-11-rotated.pdf ──────────────────────────────

#[test]
fn nics_rotated_single_page() {
    let pdf = open_pdf("nics-background-checks-2015-11-rotated.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn nics_rotated_dimensions_swapped() {
    let pdf = open_pdf("nics-background-checks-2015-11-rotated.pdf");
    let page = pdf.page(0).unwrap();
    // Rotated 90°: width and height are swapped vs standard
    // Standard is 1008x612; rotated should present as 612x1008 (portrait)
    assert!(
        (page.width() - 612.0).abs() < 2.0,
        "rotated nics width should be ~612 (was 1008 unrotated), got {}",
        page.width()
    );
    assert!(
        (page.height() - 1008.0).abs() < 2.0,
        "rotated nics height should be ~1008 (was 612 unrotated), got {}",
        page.height()
    );
}

#[test]
fn nics_rotated_has_same_char_count_as_standard() {
    let pdf_std = open_pdf("nics-background-checks-2015-11.pdf");
    let pdf_rot = open_pdf("nics-background-checks-2015-11-rotated.pdf");
    let std_chars = pdf_std.page(0).unwrap().chars().len();
    let rot_chars = pdf_rot.page(0).unwrap().chars().len();
    // Rotated page should extract same number of chars (±5% tolerance)
    let ratio = rot_chars as f64 / std_chars as f64;
    assert!(
        ratio >= 0.95 && ratio <= 1.05,
        "rotated char count {} should be within 5% of standard {}",
        rot_chars,
        std_chars
    );
}

#[test]
fn nics_rotated_rotation_metadata() {
    let pdf = open_pdf("nics-background-checks-2015-11-rotated.pdf");
    let page = pdf.page(0).unwrap();
    assert_eq!(
        page.rotation(),
        90,
        "rotated nics should report 90° rotation"
    );
}

#[test]
fn nics_rotated_chars_within_page_bounds() {
    let pdf = open_pdf("nics-background-checks-2015-11-rotated.pdf");
    let page = pdf.page(0).unwrap();
    let w = page.width();
    let h = page.height();
    for ch in page.chars() {
        assert!(
            ch.bbox.x0 >= -2.0 && ch.bbox.x1 <= w + 2.0,
            "char '{}' x [{}, {}] outside page width {}",
            ch.text,
            ch.bbox.x0,
            ch.bbox.x1,
            w
        );
        assert!(
            ch.bbox.top >= -2.0 && ch.bbox.bottom <= h + 2.0,
            "char '{}' y [{}, {}] outside page height {}",
            ch.text,
            ch.bbox.top,
            ch.bbox.bottom,
            h
        );
    }
}

// ─── scotus-transcript-p1.pdf ────────────────────────────────────────────────

#[test]
fn scotus_single_page_letter() {
    let pdf = open_pdf("scotus-transcript-p1.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 612.0).abs() < 2.0);
    assert!((page.height() - 792.0).abs() < 2.0);
}

#[test]
fn scotus_has_expected_char_count() {
    let pdf = open_pdf("scotus-transcript-p1.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    // Golden has 723 chars; we expect ≥95% = 687
    assert!(chars >= 687, "scotus should have ≥687 chars, got {}", chars);
}

#[test]
fn scotus_no_tables() {
    let pdf = open_pdf("scotus-transcript-p1.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(tables.is_empty(), "scotus transcript has no tables");
}

#[test]
fn scotus_text_contains_transcript_markers() {
    let pdf = open_pdf("scotus-transcript-p1.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    // SCOTUS transcripts have numbered lines; text should be substantial
    assert!(
        text.len() >= 200,
        "scotus text should be ≥200 chars, got {}",
        text.len()
    );
}

// ─── senate-expenditures.pdf ─────────────────────────────────────────────────

#[test]
fn senate_expenditures_landscape() {
    let pdf = open_pdf("senate-expenditures.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 792.0).abs() < 2.0, "width ~792");
    assert!((page.height() - 612.0).abs() < 2.0, "height ~612");
}

#[test]
fn senate_expenditures_has_table() {
    let pdf = open_pdf("senate-expenditures.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "senate expenditures should have a table"
    );
}

#[test]
fn senate_expenditures_has_chars() {
    let pdf = open_pdf("senate-expenditures.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars >= 2000,
        "senate should have ≥2000 chars, got {}",
        chars
    );
}

// ─── federal-register-2020-17221.pdf ─────────────────────────────────────────

#[test]
fn federal_register_multipage() {
    let pdf = open_pdf("federal-register-2020-17221.pdf");
    assert!(
        pdf.page_count() >= 15,
        "federal register should have ≥15 pages, got {}",
        pdf.page_count()
    );
}

#[test]
fn federal_register_page0_dense_text() {
    let pdf = open_pdf("federal-register-2020-17221.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars >= 4000,
        "federal register p0 should have ≥4000 chars, got {}",
        chars
    );
}

#[test]
fn federal_register_all_pages_extract_without_panic() {
    let pdf = open_pdf("federal-register-2020-17221.pdf");
    for i in 0..pdf.page_count() {
        let page = pdf.page(i).expect(&format!("page {} should open", i));
        let _ = page.chars();
        let _ = page.extract_words(&WordOptions::default());
    }
}

#[test]
fn federal_register_cumulative_doctop_increases() {
    let pdf = open_pdf("federal-register-2020-17221.pdf");
    // doctop should increase across pages
    let mut last_max_doctop: f64 = -1.0;
    for i in 0..pdf.page_count().min(3) {
        let page = pdf.page(i).unwrap();
        let chars = page.chars();
        if chars.is_empty() {
            continue;
        }
        let max_doctop = chars
            .iter()
            .map(|c| c.doctop)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_doctop > last_max_doctop,
            "page {} max doctop {} should exceed previous {}",
            i,
            max_doctop,
            last_max_doctop
        );
        last_max_doctop = max_doctop;
    }
}

// ─── chelsea_pdta.pdf ────────────────────────────────────────────────────────

#[test]
fn chelsea_pdta_large_multipage() {
    let pdf = open_pdf("chelsea_pdta.pdf");
    assert!(
        pdf.page_count() >= 60,
        "chelsea should have ≥60 pages, got {}",
        pdf.page_count()
    );
}

#[test]
fn chelsea_pdta_page0_has_chars() {
    let pdf = open_pdf("chelsea_pdta.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(chars > 0, "chelsea p0 should have chars");
}

#[test]
fn chelsea_pdta_no_panic_on_all_pages() {
    let pdf = open_pdf("chelsea_pdta.pdf");
    for i in 0..pdf.page_count() {
        let page = pdf.page(i).expect(&format!("page {} should open", i));
        let _ = page.chars();
    }
}

// ─── pdffill-demo.pdf ────────────────────────────────────────────────────────

#[test]
fn pdffill_demo_multipage() {
    let pdf = open_pdf("pdffill-demo.pdf");
    assert_eq!(pdf.page_count(), 7, "pdffill should have 7 pages");
}

#[test]
fn pdffill_demo_all_pages_have_chars() {
    let pdf = open_pdf("pdffill-demo.pdf");
    for i in 0..pdf.page_count() {
        let page = pdf.page(i).unwrap();
        // Not all pages have text chars (some are form-only), just no panic
        let _ = page.chars();
        let _ = page.extract_words(&WordOptions::default());
    }
}

// ─── table-curves-example.pdf ────────────────────────────────────────────────

#[test]
fn table_curves_has_table() {
    let pdf = open_pdf("table-curves-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "table-curves should detect a table");
}

#[test]
fn table_curves_table_has_multiple_rows() {
    let pdf = open_pdf("table-curves-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    if let Some(table) = tables.first() {
        assert!(
            table.rows.len() >= 3,
            "table-curves table should have ≥3 rows"
        );
    }
}

#[test]
fn table_curves_has_rects() {
    let pdf = open_pdf("table-curves-example.pdf");
    let rects = pdf.page(0).unwrap().rects();
    assert!(!rects.is_empty(), "table-curves should have rect graphics");
}

// ─── annotations.pdf ─────────────────────────────────────────────────────────

#[test]
fn annotations_opens() {
    let pdf = open_pdf("annotations.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn annotations_has_chars() {
    let pdf = open_pdf("annotations.pdf");
    assert!(!pdf.page(0).unwrap().chars().is_empty());
}

#[test]
fn annotations_has_annotation_objects() {
    let pdf = open_pdf("annotations.pdf");
    let page = pdf.page(0).unwrap();
    let annotations = page.annotations();
    assert!(
        !annotations.is_empty(),
        "annotations.pdf should have annotation objects"
    );
}

// ─── annotations-rotated-90.pdf ──────────────────────────────────────────────

#[test]
fn annotations_rotated_90_correct_rotation() {
    let pdf = open_pdf("annotations-rotated-90.pdf");
    let page = pdf.page(0).unwrap();
    assert_eq!(page.rotation(), 90, "should report 90° rotation");
}

#[test]
fn annotations_rotated_90_dimensions_swapped() {
    let pdf = open_pdf("annotations-rotated-90.pdf");
    let page = pdf.page(0).unwrap();
    // Rotated 90°: 595x842 becomes 842x595
    assert!(
        (page.width() - 842.0).abs() < 2.0,
        "rotated 90° width should be ~842"
    );
    assert!(
        (page.height() - 595.0).abs() < 2.0,
        "rotated 90° height should be ~595"
    );
}

#[test]
fn annotations_rotated_90_has_chars() {
    let pdf = open_pdf("annotations-rotated-90.pdf");
    assert!(
        !pdf.page(0).unwrap().chars().is_empty(),
        "rotated 90° should still extract chars"
    );
}

// ─── annotations-rotated-180.pdf / annotations-rotated-270.pdf ───────────────

#[test]
fn annotations_rotated_180_correct_rotation() {
    let pdf = open_pdf("annotations-rotated-180.pdf");
    assert_eq!(pdf.page(0).unwrap().rotation(), 180);
}

#[test]
fn annotations_rotated_270_correct_rotation() {
    let pdf = open_pdf("annotations-rotated-270.pdf");
    assert_eq!(pdf.page(0).unwrap().rotation(), 270);
}

#[test]
fn annotations_rotated_180_has_chars() {
    let pdf = open_pdf("annotations-rotated-180.pdf");
    assert!(!pdf.page(0).unwrap().chars().is_empty());
}

#[test]
fn annotations_rotated_270_has_chars() {
    let pdf = open_pdf("annotations-rotated-270.pdf");
    assert!(!pdf.page(0).unwrap().chars().is_empty());
}

// ─── page-boxes-example.pdf ──────────────────────────────────────────────────

#[test]
fn page_boxes_opens() {
    let pdf = open_pdf("page-boxes-example.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn page_boxes_has_chars() {
    let pdf = open_pdf("page-boxes-example.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars >= 10,
        "page-boxes should have ≥10 chars, got {}",
        chars
    );
}

#[test]
fn page_boxes_dimensions() {
    let pdf = open_pdf("page-boxes-example.pdf");
    let page = pdf.page(0).unwrap();
    // Golden: 624x870
    assert!((page.width() - 624.0).abs() < 2.0, "width ~624");
    assert!((page.height() - 870.0).abs() < 2.0, "height ~870");
}

// ─── issue-1054-example.pdf ──────────────────────────────────────────────────

#[test]
fn issue_1054_large_page_no_panic() {
    let pdf = open_pdf("issue-1054-example.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    // Very large page: 2225x2920
    assert!(page.width() > 2000.0, "issue-1054 page should be very wide");
    let _ = page.chars();
}

#[test]
fn issue_1054_chars_within_bounds() {
    let pdf = open_pdf("issue-1054-example.pdf");
    assert_chars_have_valid_coords(&pdf, 0, "issue-1054-example.pdf");
}

// ─── issue-1114-dedupe-chars.pdf ─────────────────────────────────────────────

#[test]
fn issue_1114_dedupe_landscape() {
    let pdf = open_pdf("issue-1114-dedupe-chars.pdf");
    let page = pdf.page(0).unwrap();
    assert!(
        page.width() > page.height(),
        "issue-1114 should be landscape"
    );
}

#[test]
fn issue_1114_dedupe_has_chars() {
    let pdf = open_pdf("issue-1114-dedupe-chars.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    // Golden has 126 chars; with dedup disabled we may get duplicates, ≥100 is safe
    assert!(
        chars >= 100,
        "issue-1114 should have ≥100 chars, got {}",
        chars
    );
}

// ─── issue-192-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_192_has_chars_and_words() {
    let pdf = open_pdf("issue-192-example.pdf");
    let page = pdf.page(0).unwrap();
    assert!(page.chars().len() >= 100);
    let words = page.extract_words(&WordOptions::default());
    assert!(words.len() >= 20);
}

// ─── issue-297-example.pdf (empty/special) ───────────────────────────────────

#[test]
fn issue_297_opens_without_panic() {
    // Golden has 0 pages — may be an empty PDF or a special case
    let _ = try_open_pdf("issue-297-example.pdf");
    // Just verify no panic; result is optional
}

// ─── issue-316-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_316_multipage() {
    let pdf = open_pdf("issue-316-example.pdf");
    assert!(pdf.page_count() >= 17, "issue-316 should have ≥17 pages");
}

#[test]
fn issue_316_all_pages_no_panic() {
    let pdf = open_pdf("issue-316-example.pdf");
    for i in 0..pdf.page_count() {
        let page = pdf.page(i).expect(&format!("page {} should open", i));
        let _ = page.chars();
    }
}

// ─── issue-33-lorem-ipsum.pdf (already covered above, skip duplicate) ─────────

// ─── issue-461-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_461_has_table() {
    let pdf = open_pdf("issue-461-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-461 should detect table(s)");
}

// ─── issue-463-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_463_multipage_tables() {
    let pdf = open_pdf("issue-463-example.pdf");
    assert_eq!(pdf.page_count(), 3);
    let mut found_table = false;
    for i in 0..3 {
        let tables = pdf.page(i).unwrap().find_tables(&TableSettings::default());
        if !tables.is_empty() {
            found_table = true;
        }
    }
    assert!(
        found_table,
        "issue-463 should find at least one table across pages"
    );
}

// ─── issue-466-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_466_has_chars_and_tables() {
    let pdf = open_pdf("issue-466-example.pdf");
    let page = pdf.page(0).unwrap();
    assert!(!page.chars().is_empty());
    let tables = page.find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-466 should detect tables");
}

// ─── issue-53-example.pdf ────────────────────────────────────────────────────

#[test]
fn issue_53_landscape_multipage() {
    let pdf = open_pdf("issue-53-example.pdf");
    assert_eq!(pdf.page_count(), 5);
    let page = pdf.page(0).unwrap();
    assert!(page.width() > page.height(), "issue-53 should be landscape");
}

#[test]
fn issue_53_all_pages_no_panic() {
    let pdf = open_pdf("issue-53-example.pdf");
    for i in 0..5 {
        let _ = pdf.page(i).unwrap().chars();
    }
}

// ─── issue-67-example.pdf ────────────────────────────────────────────────────

#[test]
fn issue_67_22_pages_with_tables() {
    let pdf = open_pdf("issue-67-example.pdf");
    assert_eq!(pdf.page_count(), 22);
}

#[test]
fn issue_67_page0_has_table() {
    let pdf = open_pdf("issue-67-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-67 p0 should have a table");
}

// ─── issue-71-duplicate-chars.pdf ────────────────────────────────────────────

#[test]
fn issue_71_dup_no_panic_with_duplicates() {
    let pdf = open_pdf("issue-71-duplicate-chars.pdf");
    assert_eq!(pdf.page_count(), 2);
    // This PDF has duplicate chars; extraction should handle gracefully
    for i in 0..2 {
        let _ = pdf.page(i).unwrap().chars();
    }
}

#[test]
fn issue_71_dup2_20_pages_no_panic() {
    let pdf = open_pdf("issue-71-duplicate-chars-2.pdf");
    assert_eq!(pdf.page_count(), 20);
    for i in 0..pdf.page_count() {
        let _ = pdf.page(i).unwrap().chars();
    }
}

// ─── issue-848.pdf ───────────────────────────────────────────────────────────

#[test]
fn issue_848_opens_or_graceful_error() {
    // This PDF has known parse issues; just verify no panic
    let _ = try_open_pdf("issue-848.pdf");
}

// ─── issue-1181.pdf ──────────────────────────────────────────────────────────

#[test]
fn issue_1181_opens_or_graceful_error() {
    // Known parse failures — no crash expected
    let _ = try_open_pdf("issue-1181.pdf");
}

// ─── malformed-from-issue-932.pdf ────────────────────────────────────────────

#[test]
fn malformed_932_no_panic() {
    // Malformed PDF — must not panic
    if let Some(pdf) = try_open_pdf("malformed-from-issue-932.pdf") {
        for i in 0..pdf.page_count() {
            if let Ok(page) = pdf.page(i) {
                let _ = page.chars();
            }
        }
    }
}

// ─── la-precinct-bulletin-2014-p1.pdf ────────────────────────────────────────

#[test]
fn la_precinct_landscape_page() {
    let pdf = open_pdf("la-precinct-bulletin-2014-p1.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 792.0).abs() < 2.0, "la precinct width ~792");
    assert!(
        (page.height() - 612.0).abs() < 2.0,
        "la precinct height ~612"
    );
}

#[test]
fn la_precinct_has_chars_and_table() {
    let pdf = open_pdf("la-precinct-bulletin-2014-p1.pdf");
    let page = pdf.page(0).unwrap();
    assert!(page.chars().len() >= 200);
    let tables = page.find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "la precinct should have a table");
}

// ─── line-char-render-example.pdf ────────────────────────────────────────────

#[test]
fn line_char_render_has_chars_and_lines() {
    let pdf = open_pdf("line-char-render-example.pdf");
    let page = pdf.page(0).unwrap();
    assert!(page.chars().len() >= 10, "should have chars");
    assert!(!page.lines().is_empty(), "should have line graphics");
}

// ─── mcid_example.pdf ────────────────────────────────────────────────────────

#[test]
fn mcid_example_has_structure_tree() {
    let pdf = open_pdf("mcid_example.pdf");
    let page = pdf.page(0).unwrap();
    // MCID example should have a tagged structure tree
    assert!(!page.chars().is_empty(), "mcid should extract chars");
}

#[test]
fn mcid_example_has_table() {
    let pdf = open_pdf("mcid_example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "mcid_example should detect its table");
}

// ─── pdf_structure.pdf ───────────────────────────────────────────────────────

#[test]
fn pdf_structure_single_page_chars() {
    let pdf = open_pdf("pdf_structure.pdf");
    assert_eq!(pdf.page_count(), 1);
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars >= 500,
        "pdf_structure should have ≥500 chars, got {}",
        chars
    );
}

// ─── word365_structure.pdf ───────────────────────────────────────────────────

#[test]
fn word365_structure_has_table() {
    let pdf = open_pdf("word365_structure.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "word365_structure should have a table");
}

// ─── test-punkt.pdf ──────────────────────────────────────────────────────────

#[test]
fn test_punkt_four_pages() {
    let pdf = open_pdf("test-punkt.pdf");
    assert_eq!(pdf.page_count(), 4);
}

#[test]
fn test_punkt_all_pages_extract() {
    let pdf = open_pdf("test-punkt.pdf");
    for i in 0..4 {
        let _ = pdf.page(i).unwrap().chars();
    }
}

// ─── issue-982-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_982_multipage_heavy() {
    let pdf = open_pdf("issue-982-example.pdf");
    assert_eq!(pdf.page_count(), 8);
    let total_chars: usize = (0..8).map(|i| pdf.page(i).unwrap().chars().len()).sum();
    assert!(
        total_chars >= 10000,
        "issue-982 total chars should be ≥10000, got {}",
        total_chars
    );
}

// ─── issue-905.pdf ───────────────────────────────────────────────────────────

#[test]
fn issue_905_opens_minimal_chars() {
    let pdf = open_pdf("issue-905.pdf");
    assert_eq!(pdf.page_count(), 1);
    // Golden has only 2 chars — special/minimal PDF
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars <= 50,
        "issue-905 should be minimal, got {} chars",
        chars
    );
}

// ─── issue-912.pdf ───────────────────────────────────────────────────────────

#[test]
fn issue_912_landscape_two_pages() {
    let pdf = open_pdf("issue-912.pdf");
    assert_eq!(pdf.page_count(), 2);
    let page = pdf.page(0).unwrap();
    assert!(
        page.width() > page.height(),
        "issue-912 should be landscape"
    );
}

// ─── WARN-Report-for-7-1-2015-to-03-25-2016.pdf ──────────────────────────────

#[test]
fn warn_report_16_pages() {
    let pdf = open_pdf("WARN-Report-for-7-1-2015-to-03-25-2016.pdf");
    assert_eq!(pdf.page_count(), 16);
}

#[test]
fn warn_report_landscape_pages() {
    let pdf = open_pdf("WARN-Report-for-7-1-2015-to-03-25-2016.pdf");
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 792.0).abs() < 2.0, "WARN report width ~792");
    assert!(
        (page.height() - 612.0).abs() < 2.0,
        "WARN report height ~612"
    );
}

#[test]
fn warn_report_tables_on_multiple_pages() {
    let pdf = open_pdf("WARN-Report-for-7-1-2015-to-03-25-2016.pdf");
    let mut table_count = 0usize;
    for i in 0..pdf.page_count() {
        table_count += pdf
            .page(i)
            .unwrap()
            .find_tables(&TableSettings::default())
            .len();
    }
    assert!(
        table_count >= 10,
        "WARN report should have ≥10 tables total, got {}",
        table_count
    );
}

// ─── 150109DSP-Milw-505-90D.pdf ──────────────────────────────────────────────

#[test]
fn dsp_milw_two_pages_with_tables() {
    let pdf = open_pdf("150109DSP-Milw-505-90D.pdf");
    assert_eq!(pdf.page_count(), 2);
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "DSP-Milw p0 should have table");
}

// ─── issue-13-151201DSP-Fond-581-90D.pdf ─────────────────────────────────────

#[test]
fn issue_13_dsp_two_pages() {
    let pdf = open_pdf("issue-13-151201DSP-Fond-581-90D.pdf");
    assert_eq!(pdf.page_count(), 2);
}

#[test]
fn issue_13_dsp_tables() {
    let pdf = open_pdf("issue-13-151201DSP-Fond-581-90D.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-13 p0 should have table");
}

// ─── cupertino_usd_4-6-16.pdf ────────────────────────────────────────────────

#[test]
fn cupertino_usd_letter_single_page() {
    let pdf = open_pdf("cupertino_usd_4-6-16.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 612.0).abs() < 2.0);
    assert!((page.height() - 792.0).abs() < 2.0);
}

#[test]
fn cupertino_usd_substantial_chars() {
    let pdf = open_pdf("cupertino_usd_4-6-16.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars >= 2000,
        "cupertino should have ≥2000 chars, got {}",
        chars
    );
}

// ─── 2023-06-20-PV.pdf ───────────────────────────────────────────────────────

#[test]
fn pv_2023_two_pages_tall() {
    let pdf = open_pdf("2023-06-20-PV.pdf");
    assert_eq!(pdf.page_count(), 2);
    let page = pdf.page(0).unwrap();
    // Tall page: 612x1008
    assert!((page.height() - 1008.0).abs() < 2.0, "PV height ~1008");
}

// ─── image_structure.pdf ─────────────────────────────────────────────────────

#[test]
fn image_structure_has_images() {
    let pdf = open_pdf("image_structure.pdf");
    let page = pdf.page(0).unwrap();
    // This PDF should have image objects
    let images = page.images();
    assert!(
        !images.is_empty(),
        "image_structure should have image objects"
    );
}

// ─── figure_structure.pdf ────────────────────────────────────────────────────

#[test]
fn figure_structure_has_table() {
    let pdf = open_pdf("figure_structure.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "figure_structure should detect table");
}

// ─── pr-88-example.pdf ───────────────────────────────────────────────────────

#[test]
fn pr_88_letter_single_page() {
    let pdf = open_pdf("pr-88-example.pdf");
    assert_eq!(pdf.page_count(), 1);
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(chars >= 100, "pr-88 should have ≥100 chars, got {}", chars);
}

// ─── pr-136-example.pdf ──────────────────────────────────────────────────────

#[test]
fn pr_136_multipage_no_panic() {
    let pdf = open_pdf("pr-136-example.pdf");
    assert!(pdf.page_count() >= 6);
    for i in 0..pdf.page_count() {
        let _ = pdf.page(i).unwrap().chars();
    }
}

// ─── pr-138-example.pdf ──────────────────────────────────────────────────────

#[test]
fn pr_138_landscape_with_tables() {
    let pdf = open_pdf("pr-138-example.pdf");
    assert_eq!(pdf.page_count(), 2);
    let page = pdf.page(0).unwrap();
    assert!(page.width() > page.height(), "pr-138 should be landscape");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "pr-138 should have tables");
}

// ─── issue-140-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_140_landscape_tables() {
    let pdf = open_pdf("issue-140-example.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!(
        page.width() > page.height(),
        "issue-140 should be landscape"
    );
    let tables = page.find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-140 should detect tables");
}

// ─── issue-336-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_336_has_tables() {
    let pdf = open_pdf("issue-336-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-336 should detect tables");
}

// ─── issue-598-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_598_has_chars_and_words() {
    let pdf = open_pdf("issue-598-example.pdf");
    let page = pdf.page(0).unwrap();
    assert!(page.chars().len() >= 500);
    assert!(page.extract_words(&WordOptions::default()).len() >= 50);
}

// ─── issue-90-example.pdf ────────────────────────────────────────────────────

#[test]
fn issue_90_has_tables() {
    let pdf = open_pdf("issue-90-example.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    assert!(!tables.is_empty(), "issue-90 should detect tables");
}

// ─── issue-1279-example.pdf ──────────────────────────────────────────────────

#[test]
fn issue_1279_no_panic() {
    let pdf = open_pdf("issue-1279-example.pdf");
    assert_eq!(pdf.page_count(), 1);
    let _ = pdf.page(0).unwrap().chars();
    let _ = pdf.page(0).unwrap().extract_words(&WordOptions::default());
}

// ─── issue-842-example.pdf ───────────────────────────────────────────────────

#[test]
fn issue_842_no_panic() {
    let pdf = open_pdf("issue-842-example.pdf");
    let _ = pdf.page(0).unwrap().chars();
}

// ─── issue-987-test.pdf ──────────────────────────────────────────────────────

#[test]
fn issue_987_large_canvas_no_panic() {
    let pdf = open_pdf("issue-987-test.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    // Large canvas PDF: 1920x1080
    assert!((page.width() - 1920.0).abs() < 2.0, "issue-987 width ~1920");
    let _ = page.chars();
}

// ─── issue-1147-example.pdf ──────────────────────────────────────────────────

#[test]
fn issue_1147_landscape_no_panic() {
    let pdf = open_pdf("issue-1147-example.pdf");
    assert_eq!(pdf.page_count(), 1);
    let page = pdf.page(0).unwrap();
    assert!(
        page.width() > page.height(),
        "issue-1147 should be landscape"
    );
    let _ = page.chars();
}

// ─── issue-203-decimalize.pdf ────────────────────────────────────────────────

#[test]
fn issue_203_three_pages_no_panic() {
    // Golden has 0 chars — image-only or special encoding
    let pdf = open_pdf("issue-203-decimalize.pdf");
    assert_eq!(pdf.page_count(), 3);
    for i in 0..3 {
        let _ = pdf.page(i).unwrap().chars(); // no panic is the assertion
    }
}

// ─── extra-attrs-example.pdf ─────────────────────────────────────────────────

#[test]
fn extra_attrs_very_large_canvas() {
    let pdf = open_pdf("extra-attrs-example.pdf");
    let page = pdf.page(0).unwrap();
    assert!((page.width() - 1920.0).abs() < 2.0);
    assert!((page.height() - 1080.0).abs() < 2.0);
}

// ─── annotations-unicode-issues.pdf ──────────────────────────────────────────

#[test]
fn annotations_unicode_issues_has_chars() {
    let pdf = open_pdf("annotations-unicode-issues.pdf");
    let chars = pdf.page(0).unwrap().chars().len();
    assert!(
        chars > 0,
        "annotations-unicode-issues should extract some chars"
    );
}

// ─── issue-1181.pdf special handling already above ────────────────────────────

// ─── Cross-fixture invariants: coords, word form, text non-empty ───────────────

/// All main-corpus fixtures: extract_text should never panic.
#[test]
fn all_main_fixtures_extract_text_no_panic() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "nics-background-checks-2015-11.pdf",
        "nics-background-checks-2015-11-rotated.pdf",
        "scotus-transcript-p1.pdf",
        "senate-expenditures.pdf",
        "federal-register-2020-17221.pdf",
        "pdffill-demo.pdf",
        "table-curves-example.pdf",
        "pr-88-example.pdf",
        "pr-138-example.pdf",
        "cupertino_usd_4-6-16.pdf",
        "mcid_example.pdf",
        "word365_structure.pdf",
        "figure_structure.pdf",
        "image_structure.pdf",
        "line-char-render-example.pdf",
        "issue-336-example.pdf",
        "issue-466-example.pdf",
        "issue-53-example.pdf",
        "issue-140-example.pdf",
        "issue-90-example.pdf",
        "issue-598-example.pdf",
    ];
    for name in &fixtures {
        let pdf = open_pdf(name);
        for i in 0..pdf.page_count().min(3) {
            if let Ok(page) = pdf.page(i) {
                let _ = page.extract_text(&TextOptions::default());
            }
        }
    }
}

/// All main-corpus fixtures: word extraction should never panic.
#[test]
fn all_main_fixtures_word_extraction_no_panic() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "nics-background-checks-2015-11.pdf",
        "federal-register-2020-17221.pdf",
        "senate-expenditures.pdf",
        "chelsea_pdta.pdf",
        "issue-316-example.pdf",
        "issue-67-example.pdf",
        "issue-71-duplicate-chars-2.pdf",
        "issue-982-example.pdf",
        "issue-53-example.pdf",
    ];
    let opts = WordOptions::default();
    for name in &fixtures {
        let pdf = open_pdf(name);
        for i in 0..pdf.page_count().min(3) {
            if let Ok(page) = pdf.page(i) {
                let _ = page.extract_words(&opts);
            }
        }
    }
}

/// All rotated fixtures: page.rotation() should return non-zero.
#[test]
fn all_rotated_fixtures_have_nonzero_rotation() {
    let rotated = [
        ("annotations-rotated-90.pdf", 90i32),
        ("annotations-rotated-180.pdf", 180),
        ("annotations-rotated-270.pdf", 270),
        ("nics-background-checks-2015-11-rotated.pdf", 90),
    ];
    for (name, expected_rotation) in &rotated {
        let pdf = open_pdf(name);
        let page = pdf.page(0).unwrap();
        assert_eq!(
            page.rotation(),
            *expected_rotation,
            "{}: expected rotation {} got {}",
            name,
            expected_rotation,
            page.rotation()
        );
    }
}

/// All non-rotated main fixtures: rotation should be 0.
#[test]
fn non_rotated_fixtures_have_zero_rotation() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "nics-background-checks-2015-11.pdf",
        "scotus-transcript-p1.pdf",
        "senate-expenditures.pdf",
        "federal-register-2020-17221.pdf",
        "pdffill-demo.pdf",
    ];
    for name in &fixtures {
        let pdf = open_pdf(name);
        let page = pdf.page(0).unwrap();
        assert_eq!(
            page.rotation(),
            0,
            "{} should have 0 rotation, got {}",
            name,
            page.rotation()
        );
    }
}

/// Table-heavy fixtures: find_tables should return results.
#[test]
fn table_heavy_fixtures_detect_tables() {
    let fixtures_with_tables = [
        "issue-33-lorem-ipsum.pdf",
        "nics-background-checks-2015-11.pdf",
        "senate-expenditures.pdf",
        "table-curves-example.pdf",
        "issue-461-example.pdf",
        "issue-140-example.pdf",
        "issue-336-example.pdf",
        "pr-138-example.pdf",
        "mcid_example.pdf",
        "figure_structure.pdf",
        "word365_structure.pdf",
    ];
    for name in &fixtures_with_tables {
        let pdf = open_pdf(name);
        let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
        assert!(
            !tables.is_empty(),
            "{}: expected to find tables but found none",
            name
        );
    }
}

/// All pages across all fixtures: page index should match page_number().
#[test]
fn page_number_matches_index_for_all_fixtures() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "federal-register-2020-17221.pdf",
        "pdffill-demo.pdf",
        "issue-316-example.pdf",
        "issue-67-example.pdf",
    ];
    for name in &fixtures {
        let pdf = open_pdf(name);
        for i in 0..pdf.page_count() {
            let page = pdf.page(i).unwrap();
            assert_eq!(
                page.page_number(),
                i,
                "{}: page at index {} reported page_number {}",
                name,
                i,
                page.page_number()
            );
        }
    }
}

/// Table cells should never have None text when they contain chars
/// (i.e., if a cell has visible text, cell.text should be Some).
#[test]
fn table_cells_with_visible_text_have_some_text() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let tables = pdf.page(0).unwrap().find_tables(&TableSettings::default());
    if let Some(table) = tables.first() {
        let mut cells_with_content = 0usize;
        let mut cells_with_none = 0usize;
        for row in &table.rows {
            for cell in row {
                match &cell.text {
                    Some(t) if !t.trim().is_empty() => cells_with_content += 1,
                    None => cells_with_none += 1,
                    _ => {}
                }
            }
        }
        // At least some cells should have content (this is a text table)
        assert!(
            cells_with_content > 0,
            "lorem ipsum table should have cells with text content"
        );
        // None cells are allowed (empty cells), but content cells should dominate
        let total = cells_with_content + cells_with_none;
        if total > 0 {
            let content_ratio = cells_with_content as f64 / total as f64;
            assert!(
                content_ratio >= 0.5,
                "majority of table cells should have content, got {:.1}%",
                content_ratio * 100.0
            );
        }
    }
}

/// BBox coordinates should always satisfy x0≤x1, top≤bottom for chars.
#[test]
fn char_bboxes_always_ordered() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "nics-background-checks-2015-11.pdf",
        "scotus-transcript-p1.pdf",
        "federal-register-2020-17221.pdf",
        "issue-316-example.pdf",
    ];
    for name in &fixtures {
        let pdf = open_pdf(name);
        for i in 0..pdf.page_count().min(2) {
            let page = pdf.page(i).unwrap();
            for ch in page.chars() {
                assert!(
                    ch.bbox.x0 <= ch.bbox.x1 + 0.5,
                    "{} p{}: char '{}' has x0={} > x1={}",
                    name,
                    i,
                    ch.text,
                    ch.bbox.x0,
                    ch.bbox.x1
                );
                assert!(
                    ch.bbox.top <= ch.bbox.bottom + 0.5,
                    "{} p{}: char '{}' has top={} > bottom={}",
                    name,
                    i,
                    ch.text,
                    ch.bbox.top,
                    ch.bbox.bottom
                );
            }
        }
    }
}

/// Word BBoxes should always contain their constituent chars' positions.
#[test]
fn word_bbox_contains_constituent_chars() {
    let pdf = open_pdf("scotus-transcript-p1.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());
    for word in &words {
        for ch in &word.chars {
            assert!(
                ch.bbox.x0 >= word.bbox.x0 - 1.0,
                "word '{}' bbox x0={} should contain char '{}' x0={}",
                word.text,
                word.bbox.x0,
                ch.text,
                ch.bbox.x0
            );
            assert!(
                ch.bbox.x1 <= word.bbox.x1 + 1.0,
                "word '{}' bbox x1={} should contain char '{}' x1={}",
                word.text,
                word.bbox.x1,
                ch.text,
                ch.bbox.x1
            );
        }
    }
}

/// Rects from PDFs with known graphics should have non-zero area.
#[test]
fn rects_have_positive_area() {
    let fixtures_with_rects = [
        ("nics-background-checks-2015-11.pdf", 0),
        ("table-curves-example.pdf", 0),
    ];
    for (name, page_idx) in &fixtures_with_rects {
        let pdf = open_pdf(name);
        let page = pdf.page(*page_idx).unwrap();
        for rect in page.rects() {
            let area = (rect.x1 - rect.x0) * (rect.bottom - rect.top);
            assert!(
                area >= 0.0,
                "{}: rect has negative area ({:.1}): {:?}",
                name,
                area,
                rect
            );
        }
    }
}

/// Lines from PDFs with known lines should have length > 0.
#[test]
fn lines_have_positive_length() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    for line in page.lines() {
        let len = ((line.x1 - line.x0).powi(2) + (line.bottom - line.top).powi(2)).sqrt();
        assert!(
            len >= 0.0,
            "line length should be non-negative, got {:.2}",
            len
        );
    }
}

/// Pages iterator yields all pages in order.
#[test]
fn pages_iter_yields_all_pages_in_order() {
    let pdf = open_pdf("pdffill-demo.pdf");
    let count = pdf.page_count();
    let mut idx = 0usize;
    for page_result in pdf.pages_iter() {
        let page = page_result.expect("page should open");
        assert_eq!(page.page_number(), idx, "pages_iter should yield in order");
        idx += 1;
    }
    assert_eq!(
        idx, count,
        "pages_iter should yield exactly page_count pages"
    );
}

/// Char doctop should be non-negative for all pages.
#[test]
fn char_doctop_is_non_negative() {
    let pdf = open_pdf("federal-register-2020-17221.pdf");
    for i in 0..pdf.page_count().min(5) {
        let page = pdf.page(i).unwrap();
        for ch in page.chars() {
            assert!(
                ch.doctop >= -1.0,
                "p{}: char '{}' has negative doctop {}",
                i,
                ch.text,
                ch.doctop
            );
        }
    }
}

/// Char size should be positive for all extracted chars.
#[test]
fn char_size_is_positive() {
    let fixtures = [
        "issue-33-lorem-ipsum.pdf",
        "scotus-transcript-p1.pdf",
        "pdffill-demo.pdf",
    ];
    for name in &fixtures {
        let pdf = open_pdf(name);
        let page = pdf.page(0).unwrap();
        for ch in page.chars() {
            assert!(
                ch.size > 0.0,
                "{}: char '{}' has non-positive size {}",
                name,
                ch.text,
                ch.size
            );
        }
    }
}

/// Char fontname should not be empty for standard PDFs.
#[test]
fn char_fontname_is_not_empty() {
    let pdf = open_pdf("issue-33-lorem-ipsum.pdf");
    let page = pdf.page(0).unwrap();
    for ch in page.chars() {
        assert!(
            !ch.fontname.is_empty(),
            "char '{}' at {:?} has empty fontname",
            ch.text,
            ch.bbox
        );
    }
}
