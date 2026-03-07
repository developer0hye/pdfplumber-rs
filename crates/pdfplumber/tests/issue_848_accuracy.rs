//! Cross-validation tests for issue-848.pdf against Python pdfplumber golden data.
//!
//! issue-848.pdf: 8-page Ghostscript-preambled PDF with alternating page orientation:
//! - Even pages (0,2,4,6): upright=true LTR text, 1-column tables.
//! - Odd  pages (1,3,5,7): upright=false physically-RTL text, 3-column tables.
//!
//! Thresholds:
//!   WORD_THRESHOLD  = 0.90  (≥90% of golden words matched within COORD_TOLERANCE)
//!   TABLE_THRESHOLD = 0.80  (≥80% of golden table rows, same column count)
//!   COORD_TOLERANCE = 1.5   (pt)

use pdfplumber::{TableSettings, WordOptions};
use serde::Deserialize;
use std::path::PathBuf;

// ─── Golden schema ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GoldenData {
    pages: Vec<GoldenPage>,
}

#[derive(Debug, Deserialize)]
struct GoldenPage {
    page_number: usize,
    chars: Vec<GoldenChar>,
    words: Vec<GoldenWord>,
    tables: Vec<GoldenTable>,
}

#[derive(Debug, Deserialize)]
struct GoldenChar {
    text: String,
    x0: f64,
    top: f64,
    upright: bool,
}

#[derive(Debug, Deserialize)]
struct GoldenWord {
    text: String,
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
}

#[derive(Debug, Deserialize)]
struct GoldenTable {
    bbox: GoldenBBox,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GoldenBBox {
    x0: f64,
    top: f64,
    x1: f64,
    bottom: f64,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

const COORD_TOLERANCE: f64 = 1.5;
const WORD_THRESHOLD: f64 = 0.90;
const TABLE_THRESHOLD: f64 = 0.80;

fn word_matches(rust: &pdfplumber::Word, golden: &GoldenWord) -> bool {
    rust.text == golden.text
        && (rust.bbox.x0 - golden.x0).abs() <= COORD_TOLERANCE
        && (rust.bbox.top - golden.top).abs() <= COORD_TOLERANCE
        && (rust.bbox.x1 - golden.x1).abs() <= COORD_TOLERANCE
        && (rust.bbox.bottom - golden.bottom).abs() <= COORD_TOLERANCE
}

fn load_golden(name: &str) -> GoldenData {
    let path = fixtures_dir().join("golden").join(name);
    let s = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("cannot read golden: {}", path.display()));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("cannot parse golden {}: {}", name, e))
}

fn skip_if_missing(path: &PathBuf) -> bool {
    if !path.exists() {
        eprintln!("SKIP: {} not found", path.display());
        return true;
    }
    false
}

// ─── Smoke ────────────────────────────────────────────────────────────────────

#[test]
fn issue_848_opens_and_has_8_pages() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();
    assert_eq!(pdf.page_count(), 8, "issue-848.pdf should have 8 pages");
}

// ─── Char accuracy ────────────────────────────────────────────────────────────

#[test]
fn issue_848_char_accuracy_all_pages() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }

    let golden = load_golden("issue-848.json");
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();

    for gp in &golden.pages {
        let page = pdf.page(gp.page_number).unwrap();
        let chars = page.chars();
        let total = gp.chars.len();
        if total == 0 {
            continue;
        }

        let mut used = vec![false; chars.len()];
        let mut matched = 0usize;
        for gc in &gp.chars {
            for (i, rc) in chars.iter().enumerate() {
                if !used[i]
                    && rc.text == gc.text
                    && (rc.bbox.x0 - gc.x0).abs() <= COORD_TOLERANCE
                    && (rc.bbox.top - gc.top).abs() <= COORD_TOLERANCE
                {
                    used[i] = true;
                    matched += 1;
                    break;
                }
            }
        }

        let rate = matched as f64 / total as f64;
        let is_rtl = gp.chars.iter().filter(|c| !c.upright).count() > total / 2;
        eprintln!(
            "  page {} chars: {}/{} ({:.1}%) [{}]",
            gp.page_number,
            matched,
            total,
            rate * 100.0,
            if is_rtl { "non-upright" } else { "upright" }
        );
        assert!(
            rate >= 0.95,
            "page {} char match {:.1}% below 95% ({}/{})",
            gp.page_number,
            rate * 100.0,
            matched,
            total
        );
    }
}

// ─── Word accuracy ────────────────────────────────────────────────────────────

#[test]
#[ignore = "rotated-page word ordering not yet implemented; tracked in issue backlog"]
fn issue_848_word_accuracy_all_pages() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }

    let golden = load_golden("issue-848.json");
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();

    let opts = WordOptions::default();
    let mut total_g = 0usize;
    let mut total_m = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for gp in &golden.pages {
        let page = pdf.page(gp.page_number).unwrap();
        let words = page.extract_words(&opts);
        let total = gp.words.len();
        total_g += total;
        if total == 0 {
            continue;
        }

        let mut used = vec![false; words.len()];
        let mut matched = 0usize;
        for gw in &gp.words {
            for (i, rw) in words.iter().enumerate() {
                if !used[i] && word_matches(rw, gw) {
                    used[i] = true;
                    matched += 1;
                    break;
                }
            }
        }
        total_m += matched;

        let rate = matched as f64 / total as f64;
        let is_rtl = gp.page_number % 2 == 1;
        eprintln!(
            "  page {} words: {}/{} ({:.1}%) [{}]",
            gp.page_number,
            matched,
            total,
            rate * 100.0,
            if is_rtl {
                "RTL/non-upright"
            } else {
                "LTR/upright"
            }
        );

        if rate < WORD_THRESHOLD {
            // Collect first 5 unmatched golden words for diagnosis
            let unmatched_g: Vec<_> = gp
                .words
                .iter()
                .filter(|gw| !words.iter().any(|rw| word_matches(rw, gw)))
                .take(5)
                .map(|w| w.text.clone())
                .collect();
            failures.push(format!(
                "page {} {:.1}% ({}/{}) — unmatched: {:?}",
                gp.page_number,
                rate * 100.0,
                matched,
                total,
                unmatched_g
            ));
        }
    }

    let overall = total_m as f64 / total_g.max(1) as f64;
    eprintln!(
        "issue-848 overall words: {}/{} ({:.1}%)",
        total_m,
        total_g,
        overall * 100.0
    );

    assert!(
        failures.is_empty(),
        "Word accuracy below {:.0}% threshold on pages:\n  {}",
        WORD_THRESHOLD * 100.0,
        failures.join("\n  ")
    );
}

// ─── Regression: even pages must not regress ──────────────────────────────────

#[test]
fn issue_848_even_pages_no_regression() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }

    let golden = load_golden("issue-848.json");
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();
    let opts = WordOptions::default();

    for gp in golden.pages.iter().filter(|p| p.page_number % 2 == 0) {
        let page = pdf.page(gp.page_number).unwrap();
        let words = page.extract_words(&opts);
        let total = gp.words.len();
        if total == 0 {
            continue;
        }

        let mut used = vec![false; words.len()];
        let mut matched = 0usize;
        for gw in &gp.words {
            for (i, rw) in words.iter().enumerate() {
                if !used[i] && word_matches(rw, gw) {
                    used[i] = true;
                    matched += 1;
                    break;
                }
            }
        }

        let rate = matched as f64 / total as f64;
        eprintln!("  even page {} words: {:.1}%", gp.page_number, rate * 100.0);
        assert!(
            rate >= 0.95,
            "even page {} regressed: {:.1}% ({}/{})",
            gp.page_number,
            rate * 100.0,
            matched,
            total
        );
    }
}

// ─── Table count ──────────────────────────────────────────────────────────────

#[test]
#[ignore = "table detection on rotated pages not yet implemented; tracked in issue backlog"]
fn issue_848_table_count_all_pages() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }

    let golden = load_golden("issue-848.json");
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();
    let settings = TableSettings::default();
    let mut failures: Vec<String> = Vec::new();

    for gp in &golden.pages {
        let page = pdf.page(gp.page_number).unwrap();
        let tables = page.find_tables(&settings);
        let expected = gp.tables.len();
        let got = tables.len();

        eprintln!(
            "  page {} tables: got={} expected={}",
            gp.page_number, got, expected
        );

        if got != expected {
            failures.push(format!(
                "page {}: got {} tables, expected {}",
                gp.page_number, got, expected
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Table count mismatch:\n  {}",
        failures.join("\n  ")
    );
}

// ─── Table row accuracy ───────────────────────────────────────────────────────

#[test]
#[ignore = "table row accuracy on rotated pages not yet implemented; tracked in issue backlog"]
fn issue_848_table_row_accuracy_all_pages() {
    let pdf_path = fixtures_dir().join("pdfs/issue-848.pdf");
    if skip_if_missing(&pdf_path) {
        return;
    }

    let golden = load_golden("issue-848.json");
    let pdf = pdfplumber::Pdf::open_file(&pdf_path, None).unwrap();
    let settings = TableSettings::default();
    let mut failures: Vec<String> = Vec::new();

    for gp in &golden.pages {
        let page = pdf.page(gp.page_number).unwrap();
        let tables = page.find_tables(&settings);

        for (t_idx, gt) in gp.tables.iter().enumerate() {
            let g_nrows = gt.rows.len();
            let g_ncols = gt.rows.first().map(|r| r.len()).unwrap_or(0);

            let (r_nrows, r_ncols) = tables
                .get(t_idx)
                .map(|t| (t.rows.len(), t.rows.first().map(|r| r.len()).unwrap_or(0)))
                .unwrap_or((0, 0));

            let row_rate = r_nrows as f64 / g_nrows.max(1) as f64;
            let is_rtl = gp.page_number % 2 == 1;

            eprintln!(
                "  page {} table {}: {}r×{}c (want {}r×{}c) {:.0}% rows [{}]",
                gp.page_number,
                t_idx,
                r_nrows,
                r_ncols,
                g_nrows,
                g_ncols,
                row_rate * 100.0,
                if is_rtl { "RTL" } else { "LTR" }
            );

            if row_rate < TABLE_THRESHOLD {
                failures.push(format!(
                    "page {} table {}: {:.0}% ({}/{} rows), cols got={} want={}",
                    gp.page_number,
                    t_idx,
                    row_rate * 100.0,
                    r_nrows,
                    g_nrows,
                    r_ncols,
                    g_ncols
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Table row accuracy below {:.0}%:\n  {}",
        TABLE_THRESHOLD * 100.0,
        failures.join("\n  ")
    );
}
