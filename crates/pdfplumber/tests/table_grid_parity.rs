//! Parity tests for the grid a table's cells are laid out on.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [(len(t), len(t[0])) for t in page.extract_tables()]
//! ```
//!
//! A row has one position per distinct cell left edge in the table. A cell that
//! spans several columns occupies the position it starts at and leaves the rest
//! of its span empty — it does not split into one position per boundary it
//! happens to cross.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

/// (row count, column count) for each table on page 1.
fn shapes(name: &str) -> Vec<(usize, usize)> {
    let pdf = Pdf::open_file(fixture(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())
        .iter()
        .map(|table| (table.len(), table.first().map_or(0, Vec::len)))
        .collect()
}

#[test]
fn single_column_tables_stay_single_column() {
    // Python pdfplumber: 6x1 and 9x1.
    assert_eq!(shapes("issue-848.pdf"), vec![(6, 1), (9, 1)]);
}

#[test]
fn a_ruled_table_keeps_its_own_shape() {
    // Python pdfplumber: the second table on this page is 8x2.
    assert_eq!(shapes("pr-138-example.pdf")[1], (8, 2));
}

#[test]
fn merged_cells_still_read_correctly() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated/table_merged_cells.pdf");
    let pdf = Pdf::open_file(path, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.extract_tables(&TableSettings::default());

    assert_eq!(
        tables[0],
        vec![
            vec![Some("Quarterly Report".to_string()), None, None, None],
            vec![
                Some("Region".to_string()),
                Some("Q1".to_string()),
                Some("Q2".to_string()),
                Some("Q3".to_string())
            ],
            vec![
                Some("North".to_string()),
                Some("100".to_string()),
                Some("150".to_string()),
                Some("200".to_string())
            ],
            vec![
                None,
                Some("110".to_string()),
                Some("160".to_string()),
                Some("210".to_string())
            ],
            vec![
                Some("South".to_string()),
                Some("200".to_string()),
                Some("250".to_string()),
                Some("300".to_string())
            ],
        ]
    );
}
