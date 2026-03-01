//! Diagnostic test: compare Rust vs Python table detection for nics-background-checks.
//!
//! Run with: `cargo test -p pdfplumber --test table_diagnostic -- --nocapture`
//!
//! This test analyzes the table detection pipeline (edges → intersections → cells → tables)
//! to identify why Rust produces fewer rows/columns than Python pdfplumber for the
//! nics-background-checks-2015-11.pdf fixture.

use pdfplumber::{Edge, Orientation, TableFinder, TableFinderDebug, TableSettings, WordOptions};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

// ─── Golden JSON types (minimal, just for tables) ──────────────────────────

#[derive(Debug, Deserialize)]
struct GoldenData {
    pages: Vec<GoldenPage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GoldenPage {
    page_number: usize,
    tables: Vec<GoldenTable>,
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

// ─── Helpers ───────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn load_golden(pdf_name: &str) -> GoldenData {
    let json_name = pdf_name.replace(".pdf", ".json");
    let path = fixtures_dir().join("golden").join(&json_name);
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read golden file {}: {}", path.display(), e));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("Failed to parse golden JSON {}: {}", path.display(), e))
}

fn open_pdf(pdf_name: &str) -> pdfplumber::Pdf {
    let path = fixtures_dir().join("pdfs").join(pdf_name);
    pdfplumber::Pdf::open_file(&path, None)
        .unwrap_or_else(|e| panic!("Failed to open PDF {}: {}", path.display(), e))
}

// ─── Diagnostic Test ───────────────────────────────────────────────────────

#[test]
fn diagnose_nics_table_detection_gap() {
    let pdf_name = "nics-background-checks-2015-11.pdf";
    let golden = load_golden(pdf_name);
    let pdf = open_pdf(pdf_name);
    let page = pdf.page(0).expect("Failed to get page 0");

    // Load golden table info
    let golden_page = &golden.pages[0];
    assert!(
        !golden_page.tables.is_empty(),
        "Golden data has no tables for page 0"
    );
    let golden_table = &golden_page.tables[0];
    let golden_rows = golden_table.rows.len();
    let golden_cols = golden_table.rows[0].len();
    let golden_cells = golden_table.rows.iter().map(|r| r.len()).sum::<usize>();

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("DIAGNOSTIC: nics-background-checks table detection gap");
    eprintln!("{}", "=".repeat(60));
    eprintln!(
        "\nGolden table: bbox=({:.1}, {:.1}, {:.1}, {:.1})",
        golden_table.bbox.x0, golden_table.bbox.top, golden_table.bbox.x1, golden_table.bbox.bottom,
    );
    eprintln!(
        "Golden dimensions: {} rows x {} cols = {} cells",
        golden_rows, golden_cols, golden_cells
    );

    // ─── 1. Edge Analysis ──────────────────────────────────────────────────

    let edges = page.edges();
    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();

    eprintln!("\n--- 1. Edge Analysis (raw from page) ---");
    eprintln!("Total edges: {}", edges.len());
    eprintln!("  Horizontal: {}", h_edges.len());
    eprintln!("  Vertical: {}", v_edges.len());

    // Unique positions before snapping
    let mut raw_x: Vec<f64> = v_edges.iter().map(|e| e.x0).collect();
    raw_x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    raw_x.dedup_by(|a, b| (*a - *b).abs() < 0.1);
    let mut raw_y: Vec<f64> = h_edges.iter().map(|e| e.top).collect();
    raw_y.sort_by(|a, b| a.partial_cmp(b).unwrap());
    raw_y.dedup_by(|a, b| (*a - *b).abs() < 0.1);
    eprintln!("  Unique x-positions (before snap): {}", raw_x.len());
    eprintln!("  Unique y-positions (before snap): {}", raw_y.len());

    // ─── 2. Pipeline Debug (after snap/join) ───────────────────────────────

    let settings = TableSettings::default();
    let words = page.extract_words(&WordOptions::default());
    let finder = TableFinder::new_with_words(edges, words, settings);
    let debug: TableFinderDebug = finder.find_tables_debug();

    let proc_h: Vec<&Edge> = debug
        .edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let proc_v: Vec<&Edge> = debug
        .edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();

    eprintln!("\n--- 2. Processed Edges (after snap + join) ---");
    eprintln!("Total edges: {}", debug.edges.len());
    eprintln!("  Horizontal: {}", proc_h.len());
    eprintln!("  Vertical: {}", proc_v.len());

    // Unique positions after processing
    let mut snap_x: Vec<f64> = proc_v.iter().map(|e| e.x0).collect();
    snap_x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    snap_x.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    let mut snap_y: Vec<f64> = proc_h.iter().map(|e| e.top).collect();
    snap_y.sort_by(|a, b| a.partial_cmp(b).unwrap());
    snap_y.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    eprintln!("  Unique x-positions (after snap): {}", snap_x.len());
    eprintln!("  Unique y-positions (after snap): {}", snap_y.len());

    // ─── 3. Intersection Analysis ──────────────────────────────────────────

    eprintln!("\n--- 3. Intersection Analysis ---");
    eprintln!("Total intersections: {}", debug.intersections.len());

    // Intersections per y-position (row)
    let mut ints_by_y: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for pt in &debug.intersections {
        let y_key = format!("{:.1}", pt.y);
        ints_by_y.entry(y_key).or_default().push(pt.x);
    }

    eprintln!("\nIntersections per y-position:");
    for (y_key, xs) in &ints_by_y {
        let coverage = if !snap_x.is_empty() {
            xs.len() as f64 / snap_x.len() as f64 * 100.0
        } else {
            0.0
        };
        let marker = if xs.len() < snap_x.len() {
            " *** INCOMPLETE"
        } else {
            ""
        };
        eprintln!(
            "  y={}: {} intersections ({:.0}% of {} x-positions){}",
            y_key,
            xs.len(),
            coverage,
            snap_x.len(),
            marker
        );
    }

    // Identify y-positions missing full-width coverage
    let incomplete_ys: Vec<(&String, usize)> = ints_by_y
        .iter()
        .filter(|(_, xs)| xs.len() < snap_x.len())
        .map(|(y, xs)| (y, xs.len()))
        .collect();

    if !incomplete_ys.is_empty() {
        eprintln!("\nY-positions missing full-width intersection coverage:");
        for (y, count) in &incomplete_ys {
            eprintln!(
                "  y={}: only {}/{} x-positions have intersections",
                y,
                count,
                snap_x.len()
            );
        }
    }

    // ─── 4. Cell / Table Analysis ──────────────────────────────────────────

    eprintln!("\n--- 4. Cell / Table Analysis ---");
    eprintln!("Total cells: {}", debug.cells.len());
    eprintln!("Total tables: {}", debug.tables.len());

    if !debug.tables.is_empty() {
        for (i, table) in debug.tables.iter().enumerate() {
            let rust_rows = table.rows.len();
            let rust_cols = if !table.rows.is_empty() {
                table.rows[0].len()
            } else {
                0
            };
            let rust_cells: usize = table.rows.iter().map(|r| r.len()).sum();
            eprintln!(
                "\n  Rust table {}: bbox=({:.1}, {:.1}, {:.1}, {:.1})",
                i, table.bbox.x0, table.bbox.top, table.bbox.x1, table.bbox.bottom,
            );
            eprintln!(
                "  Rust dimensions: {} rows x {} cols = {} cells",
                rust_rows, rust_cols, rust_cells
            );
            eprintln!(
                "  Golden dimensions: {} rows x {} cols = {} cells",
                golden_rows, golden_cols, golden_cells
            );
            eprintln!(
                "  Row diff: {} (rust {} vs golden {})",
                rust_rows as i64 - golden_rows as i64,
                rust_rows,
                golden_rows
            );
            eprintln!(
                "  Col diff: {} (rust {} vs golden {})",
                rust_cols as i64 - golden_cols as i64,
                rust_cols,
                golden_cols
            );
        }
    } else {
        eprintln!("  NO TABLES DETECTED by Rust!");
        eprintln!(
            "  Golden expected: {} rows x {} cols = {} cells",
            golden_rows, golden_cols, golden_cells
        );
    }

    // ─── 5. Edge coverage at incomplete y-positions ────────────────────────

    if !incomplete_ys.is_empty() {
        eprintln!("\n--- 5. Edge Coverage at Incomplete Y-Positions ---");
        for (y_str, _count) in &incomplete_ys {
            let y_val: f64 = y_str.parse().unwrap();
            // Which horizontal edges span this y-position?
            let spanning_h: Vec<&Edge> = proc_h
                .iter()
                .filter(|e| (e.top - y_val).abs() < 3.0)
                .copied()
                .collect();
            eprintln!(
                "\n  y={}: {} horizontal edges nearby:",
                y_str,
                spanning_h.len()
            );
            for e in &spanning_h {
                eprintln!(
                    "    H edge: x=[{:.1}, {:.1}], y={:.1}, source={:?}",
                    e.x0, e.x1, e.top, e.source
                );
            }

            // Which vertical edges cross this y-position?
            let crossing_v: Vec<&Edge> = proc_v
                .iter()
                .filter(|e| e.top <= y_val + 3.0 && e.bottom >= y_val - 3.0)
                .copied()
                .collect();
            eprintln!(
                "  y={}: {} vertical edges crossing this y:",
                y_str,
                crossing_v.len()
            );
            if crossing_v.len() <= 30 {
                for e in &crossing_v {
                    eprintln!(
                        "    V edge: x={:.1}, y=[{:.1}, {:.1}], source={:?}",
                        e.x0, e.top, e.bottom, e.source
                    );
                }
            }
        }
    }

    // ─── Summary ───────────────────────────────────────────────────────────

    eprintln!("\n{}", "=".repeat(60));
    eprintln!("SUMMARY");
    eprintln!("{}", "=".repeat(60));
    eprintln!(
        "Golden: {} rows x {} cols = {} cells",
        golden_rows, golden_cols, golden_cells
    );
    if !debug.tables.is_empty() {
        let t = &debug.tables[0];
        let r = t.rows.len();
        let c = if !t.rows.is_empty() {
            t.rows[0].len()
        } else {
            0
        };
        eprintln!(
            "Rust:   {} rows x {} cols = {} cells",
            r,
            c,
            debug.cells.len()
        );
    } else {
        eprintln!("Rust:   0 tables detected");
    }
    eprintln!("Incomplete y-positions: {}", incomplete_ys.len());
    eprintln!(
        "Root cause: intersections_to_cells requires all 4 corner \
         points, but {} y-positions lack full x-coverage",
        incomplete_ys.len()
    );

    // ─── Assertions (test must pass) ───────────────────────────────────────

    // Verify the test itself ran correctly - basic sanity checks
    assert_eq!(golden_rows, 17, "Golden should have 17 rows");
    assert_eq!(golden_cols, 25, "Golden should have 25 columns");
    assert_eq!(golden_cells, 425, "Golden should have 425 cells");

    // The diagnostic should find at least some edges and intersections
    assert!(!debug.edges.is_empty(), "Should have processed edges");
    assert!(!debug.intersections.is_empty(), "Should have intersections");

    // Verify at least one table is detected (even if dimensions are wrong)
    assert!(!debug.tables.is_empty(), "Should detect at least one table");

    // Verify that incomplete y-positions are identified
    // (This confirms the diagnostic is working - these are the root cause)
    assert!(
        !incomplete_ys.is_empty(),
        "Should identify y-positions with incomplete intersection coverage"
    );
}
