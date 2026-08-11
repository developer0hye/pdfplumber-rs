//! Parity tests for tables containing merged cells.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! A cell that spans two rows sits in the row it starts in; the row below gets
//! `None` at that position rather than losing the column altogether.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated")
        .join(name)
}

fn tables(name: &str) -> Vec<Vec<Vec<Option<String>>>> {
    let pdf = Pdf::open_file(generated(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())
}

fn row(cells: &[Option<&str>]) -> Vec<Option<String>> {
    cells
        .iter()
        .map(|cell| cell.map(|text| text.to_string()))
        .collect()
}

#[test]
fn merged_cells_match_python_pdfplumber() {
    let tables = tables("table_merged_cells.pdf");

    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0],
        vec![
            // A full-width title: one cell, three empty positions beside it.
            row(&[Some("Quarterly Report"), None, None, None]),
            row(&[Some("Region"), Some("Q1"), Some("Q2"), Some("Q3")]),
            // "North" spans this row and the next.
            row(&[Some("North"), Some("100"), Some("150"), Some("200")]),
            row(&[None, Some("110"), Some("160"), Some("210")]),
            row(&[Some("South"), Some("200"), Some("250"), Some("300")]),
        ]
    );
}

#[test]
fn every_row_has_the_same_width() {
    // The point of the None positions: a merged table still reads as a grid.
    let tables = tables("table_merged_cells.pdf");

    for row in &tables[0] {
        assert_eq!(row.len(), 4);
    }
}

#[test]
fn a_table_without_merges_is_unchanged() {
    let tables = tables("table_lattice.pdf");

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 8);
    assert_eq!(
        tables[0][0],
        row(&[
            Some("ID"),
            Some("Name"),
            Some("Category"),
            Some("Price"),
            Some("Stock")
        ])
    );
}
