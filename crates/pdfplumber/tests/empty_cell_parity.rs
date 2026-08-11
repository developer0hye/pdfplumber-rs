//! Parity tests for how empty table cells are reported.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! pdfplumber distinguishes two kinds of blank: a cell that exists but holds no
//! characters reads as `""`, while a position no cell covers — because a
//! neighbour spans it — reads as `None`. Consumers rely on the difference to
//! tell "this field was left blank" from "this row has fewer columns".

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
fn a_blank_cell_reads_as_an_empty_string() {
    let tables = tables("table_lattice.pdf");

    assert_eq!(tables.len(), 1);
    // Row 2 has no Stock value, row 4 no Price: both cells exist and are blank.
    assert_eq!(
        tables[0][2],
        row(&[
            Some("2"),
            Some("Gadget B"),
            Some("Electronics"),
            Some("$25.50"),
            Some("")
        ])
    );
    assert_eq!(
        tables[0][4],
        row(&[
            Some("4"),
            Some("Part D"),
            Some("Components"),
            Some(""),
            Some("200")
        ])
    );
}

#[test]
fn a_position_covered_by_a_merged_cell_reads_as_none() {
    let tables = tables("table_merged_cells.pdf");

    // The full-width title leaves three positions uncovered, and "North" spans
    // into the row below.
    assert_eq!(
        tables[0][0],
        row(&[Some("Quarterly Report"), None, None, None])
    );
    assert_eq!(
        tables[0][3],
        row(&[None, Some("110"), Some("160"), Some("210")])
    );
}

#[test]
fn no_row_mixes_up_the_two_kinds_of_blank() {
    // Every table_lattice cell exists, so nothing in it should be None.
    let tables = tables("table_lattice.pdf");

    for row in &tables[0] {
        assert!(
            row.iter().all(|cell| cell.is_some()),
            "a fully ruled table should have no uncovered positions: {row:?}"
        );
    }
}
