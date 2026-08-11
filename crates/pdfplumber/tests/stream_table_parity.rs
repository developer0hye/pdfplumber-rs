//! End-to-end parity checks for the Stream (text-alignment) table strategy.
//!
//! The expected tables were produced by Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables({"vertical_strategy": "text", "horizontal_strategy": "text"})
//! ```
//!
//! Stream detection is sensitive to how synthetic edges are placed, so a whole
//! extracted table is the observable outcome worth pinning: a column boundary
//! that lands one alignment off splits words across cells.

use std::path::PathBuf;

use pdfplumber::{Pdf, Strategy, TableSettings};

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated")
        .join(name)
}

fn stream_tables(name: &str) -> Vec<Vec<Vec<String>>> {
    let pdf = Pdf::open_file(generated(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    let settings = TableSettings {
        strategy: Strategy::Stream,
        ..TableSettings::default()
    };
    page.extract_tables(&settings)
        .into_iter()
        .map(|table| {
            table
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| cell.unwrap_or_default())
                        .collect()
                })
                .collect()
        })
        .collect()
}

#[test]
fn borderless_table_matches_python_pdfplumber_stream_output() {
    let tables = stream_tables("table_borderless.pdf");

    assert_eq!(tables.len(), 1, "expected exactly one detected table");

    // Blank rows are pdfplumber's own output: every text row contributes both a
    // top and a bottom edge, and the gap between them forms a zero-content row.
    let blank = ["", "", "", "", "", ""];
    let expected: Vec<Vec<&str>> = vec![
        vec!["ID", "Name", "", "Category", "Price", "Stock"],
        blank.to_vec(),
        vec!["1", "Widget", "A", "Hardware", "$10.00", "100"],
        blank.to_vec(),
        vec!["2", "Gadget", "B", "Electronics", "$25.50", ""],
        blank.to_vec(),
        vec!["3", "Tool C", "", "Hardware", "$7.25", "50"],
        blank.to_vec(),
        vec!["4", "Part D", "", "Components", "", "200"],
        blank.to_vec(),
        vec!["5", "Device", "E", "Electronics", "$99.99", "12"],
        blank.to_vec(),
        vec!["6", "Supply", "F", "Materials", "$3.00", "500"],
        blank.to_vec(),
        vec!["7", "Item G", "", "Misc", "$15.75", ""],
    ];

    assert_eq!(tables[0], expected);
}
