//! Render a [`Table`] to a pipe-delimited text string for inclusion in a chunk.
//!
//! Format: one row per line, cells separated by ` | `. Empty cells render as
//! empty string between pipes. This format is LLM-friendly — most models trained
//! on markdown understand pipe tables natively.
//!
//! Example:
//! ```text
//! Header A | Header B | Header C
//! value 1  | value 2  | value 3
//! total    | 42       |
//! ```

use pdfplumber_core::Table;

/// Render `table` as a pipe-delimited text string.
///
/// - Each row becomes one line.
/// - Cells are separated by ` | `.
/// - `None` cells (empty/merged) render as empty string.
/// - Leading and trailing whitespace is stripped from each cell.
/// - Completely empty rows (all cells empty) are omitted.
pub fn render(table: &Table) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(table.rows.len());

    for row in &table.rows {
        let cells: Vec<&str> = row
            .iter()
            .map(|cell| cell.text.as_deref().unwrap_or("").trim())
            .collect();

        // Skip rows where all cells are empty.
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }

        lines.push(cells.join(" | "));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Cell, Table};

    fn make_cell(text: Option<&str>) -> Cell {
        Cell {
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            text: text.map(|s| s.to_string()),
        }
    }

    fn make_table(rows: Vec<Vec<Option<&str>>>) -> Table {
        let cells: Vec<Vec<Cell>> = rows
            .iter()
            .map(|row| row.iter().map(|t| make_cell(*t)).collect())
            .collect();
        let all_cells: Vec<Cell> = cells.iter().flatten().cloned().collect();
        Table {
            bbox: BBox::new(0.0, 0.0, 300.0, 200.0),
            cells: all_cells,
            rows: cells,
            columns: vec![],
        }
    }

    #[test]
    fn render_simple_2x2() {
        let table = make_table(vec![
            vec![Some("A"), Some("B")],
            vec![Some("1"), Some("2")],
        ]);
        let text = render(&table);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "A | B");
        assert_eq!(lines[1], "1 | 2");
    }

    #[test]
    fn render_none_cells_are_empty() {
        let table = make_table(vec![
            vec![Some("Header"), None],
            vec![None, Some("value")],
        ]);
        let text = render(&table);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Header | ");
        assert_eq!(lines[1], " | value");
    }

    #[test]
    fn render_skips_all_empty_rows() {
        let table = make_table(vec![
            vec![Some("A"), Some("B")],
            vec![None, None],
            vec![Some("C"), Some("D")],
        ]);
        let text = render(&table);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "A | B");
        assert_eq!(lines[1], "C | D");
    }

    #[test]
    fn render_trims_whitespace() {
        let table = make_table(vec![vec![Some("  hello  "), Some("  world  ")]]);
        let text = render(&table);
        assert_eq!(text, "hello | world");
    }

    #[test]
    fn render_empty_table() {
        let table = make_table(vec![]);
        assert_eq!(render(&table), "");
    }
}
