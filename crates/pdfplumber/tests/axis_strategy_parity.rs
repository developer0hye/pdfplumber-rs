//! Parity tests for choosing a table strategy per axis.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! page.extract_tables({"vertical_strategy": "text", "horizontal_strategy": "lines"})
//! ```
//!
//! Mixed strategies are how pdfplumber handles the common half-ruled table:
//! rules between rows but nothing between columns, or the reverse.

use std::path::PathBuf;

use pdfplumber::{ExplicitLines, Pdf, Strategy, TableSettings};

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated")
        .join(name)
}

fn extract(name: &str, settings: &TableSettings) -> Vec<Vec<Vec<String>>> {
    let pdf = Pdf::open_file(generated(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_tables(settings)
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

fn axes(vertical: Strategy, horizontal: Strategy) -> TableSettings {
    TableSettings {
        vertical_strategy: Some(vertical),
        horizontal_strategy: Some(horizontal),
        ..TableSettings::default()
    }
}

#[test]
fn text_columns_with_ruled_rows_match_python_pdfplumber() {
    // Python: 1 table, 6 rows of 6 columns. Column boundaries come from text
    // alignment, so "Widget A" splits across two cells.
    let tables = extract(
        "table_lattice.pdf",
        &axes(Strategy::Stream, Strategy::Lattice),
    );

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 6);
    assert_eq!(
        tables[0][0],
        ["1", "Widget", "A", "Hardware", "$10.00", "100"]
    );
    assert_eq!(
        tables[0][5],
        ["6", "Supply", "F", "Materials", "$3.00", "500"]
    );
}

#[test]
fn ruled_columns_with_text_rows_match_python_pdfplumber() {
    // Python: 1 table, 15 rows of 3 columns. Every text row contributes a top
    // and a bottom edge, so blank rows fall between the content rows.
    let tables = extract(
        "table_lattice.pdf",
        &axes(Strategy::Lattice, Strategy::Stream),
    );

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 15);
    assert_eq!(tables[0][0], ["Name", "Category", "Price"]);
    assert_eq!(tables[0][1], ["", "", ""]);
    assert_eq!(tables[0][2], ["Widget A", "Hardware", "$10.00"]);
    assert_eq!(tables[0][14], ["Item G", "Misc", "$15.75"]);
}

#[test]
fn both_axes_ruled_is_unchanged() {
    let tables = extract(
        "table_lattice.pdf",
        &axes(Strategy::Lattice, Strategy::Lattice),
    );

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 8);
    assert_eq!(tables[0][0], ["ID", "Name", "Category", "Price", "Stock"]);
}

#[test]
fn an_unset_axis_falls_back_to_the_general_strategy() {
    let per_axis = extract(
        "table_lattice.pdf",
        &axes(Strategy::Lattice, Strategy::Lattice),
    );
    let general = extract("table_lattice.pdf", &TableSettings::default());

    assert_eq!(per_axis, general);
}

#[test]
fn explicit_vertical_lines_replace_the_detected_ones() {
    // With vertical_strategy = explicit, the ruled column edges are ignored and
    // only the three given x positions divide the table: 8 rows of 2 columns.
    // The line at x=50 cuts through "ID", which is why the header reads "D".
    let settings = TableSettings {
        vertical_strategy: Some(Strategy::Explicit),
        horizontal_strategy: Some(Strategy::Lattice),
        explicit_lines: Some(ExplicitLines {
            vertical_lines: vec![50.0, 300.0, 500.0],
            horizontal_lines: vec![],
        }),
        ..TableSettings::default()
    };
    let tables = extract("table_lattice.pdf", &settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 8);
    assert_eq!(tables[0][0], ["D Name Category", "Price Stock"]);
}

#[test]
fn explicit_lines_are_added_to_a_detected_grid() {
    // pdfplumber always adds explicit lines on top of whatever the axis
    // strategy found, so an extra rule at x=300 splits the Category column.
    let settings = TableSettings {
        explicit_lines: Some(ExplicitLines {
            vertical_lines: vec![300.0],
            horizontal_lines: vec![],
        }),
        ..TableSettings::default()
    };
    let tables = extract("table_lattice.pdf", &settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(
        tables[0][0],
        ["ID", "Name", "Category", "", "Price", "Stock"]
    );
}

#[test]
fn a_borderless_table_needs_text_on_both_axes() {
    // Python finds nothing with either axis ruled, and one 15-row table with
    // both axes driven by text.
    for settings in [
        axes(Strategy::Lattice, Strategy::Lattice),
        axes(Strategy::Stream, Strategy::Lattice),
        axes(Strategy::Lattice, Strategy::Stream),
    ] {
        assert!(extract("table_borderless.pdf", &settings).is_empty());
    }

    let tables = extract(
        "table_borderless.pdf",
        &axes(Strategy::Stream, Strategy::Stream),
    );
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 15);
}
