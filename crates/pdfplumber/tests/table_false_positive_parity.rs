//! Pages that Python pdfplumber reports no tables for.
//!
//! A lone bordered box — a button, a page frame, a call-out — produces one cell
//! and no table in pdfplumber:
//!
//! ```python
//! page.extract_tables()  # []
//! ```
//!
//! These fixtures each contain such boxes, so they are the cheapest guard
//! against detecting decoration as data.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn downloaded(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/downloaded")
        .join(name)
}

fn tables(name: &str) -> Vec<Vec<Vec<Option<String>>>> {
    let pdf = Pdf::open_file(downloaded(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())
}

#[test]
fn isolated_button_outlines_are_not_tables() {
    // pdffill-demo.pdf frames its "First Page" / "Next Page" / "Print" buttons
    // in rectangles. Each is one standalone cell, so pdfplumber returns [].
    assert_eq!(
        tables("pdffill-demo.pdf"),
        Vec::<Vec<Vec<Option<String>>>>::new()
    );
}

#[test]
fn a_page_border_is_not_a_table() {
    // annotations-unicode-issues.pdf draws a rectangle around the whole page:
    // four edges, four intersections, one cell, no table.
    assert_eq!(
        tables("annotations-unicode-issues.pdf"),
        Vec::<Vec<Vec<Option<String>>>>::new()
    );
}

#[test]
fn a_real_grid_is_still_detected() {
    // The counterpart check: a bordered table on the same code path survives.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated/table_lattice.pdf");
    let pdf = Pdf::open_file(path, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.extract_tables(&TableSettings::default());

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 8);
}
