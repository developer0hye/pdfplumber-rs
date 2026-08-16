//! Parity tests for telling neighbouring tables apart.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables()
//! ```
//!
//! Two tables set side by side share the rule between them. pdfplumber keeps
//! them apart because it groups cells that meet at a corner, not cells that
//! merely run alongside each other — so a totals box whose rows are ruled at
//! different heights than the line items beside it stays its own table.

use std::path::PathBuf;

use pdfplumber::{Pdf, TableSettings};

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated")
        .join(name)
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

fn extract(name: &str) -> Vec<Vec<Vec<String>>> {
    let pdf = Pdf::open_file(generated(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(&TableSettings::default())
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
fn a_totals_box_beside_line_items_is_its_own_table() {
    let tables = extract("table_side_by_side.pdf");

    assert_eq!(
        tables.len(),
        2,
        "expected the line items and the totals box to be two tables, got {}",
        tables.len()
    );

    assert_eq!(
        tables[0],
        vec![
            vec!["Design retainer", "40.00"],
            vec!["Implementation", "128.50"],
            vec!["Code review", "16.00"],
            vec!["Deployment", "8.25"],
        ],
    );
    assert_eq!(
        tables[1],
        vec![
            vec!["Subtotal", "192.75"],
            vec!["Tax", "19.28"],
            vec!["Total", "212.03"],
        ],
    );
}

#[test]
fn an_empty_results_table_keeps_its_own_row() {
    // issue-140-example.pdf holds a stack of narrow tables. The "No results"
    // table below the document list touches the one above it along a rule
    // without meeting it at a corner; joining the two gave the empty table a
    // third row and pushed its header cells onto the wrong line.
    let pdf = Pdf::open_file(fixture("issue-140-example.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.extract_tables(&TableSettings::default());

    // Python pdfplumber: two rows, a header and a single "No results" cell.
    let documents = &tables[3];
    assert_eq!(documents.len(), 2, "expected a header and one row");
    assert_eq!(
        documents[0],
        vec![
            Some("Document type".to_string()),
            Some("Document name".to_string()),
            Some("Uploaded by".to_string()),
            Some("Updated on".to_string()),
            Some("Buyer/ supplier".to_string()),
            Some("Document visibility".to_string()),
        ],
    );
    assert_eq!(
        documents[1],
        vec![Some("No results".to_string()), None, None, None, None, None],
    );
}

#[test]
fn neighbouring_tables_keep_their_own_bounds() {
    let pdf = Pdf::open_file(generated("table_side_by_side.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert_eq!(tables.len(), 2);

    // Python pdfplumber: (42.52, 113.39, 354.33, 249.45) and
    // (354.33, 130.39, 538.58, 266.45). They meet at the shared rule without
    // either spilling into the other.
    let items = &tables[0].bbox;
    let totals = &tables[1].bbox;
    assert!((items.x1 - totals.x0).abs() < 0.01, "they share the rule");
    assert!(
        items.top < totals.top,
        "the totals box starts lower than the line items",
    );
    assert!(
        items.bottom < totals.bottom,
        "the totals box also ends lower",
    );
}
