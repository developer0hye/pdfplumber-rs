//! Table detection types and pipeline.
//!
//! This module provides the configuration types, data structures, and orchestration
//! for detecting tables in PDF pages using Lattice, Stream, or Explicit strategies.

use crate::edges::Edge;
use crate::geometry::BBox;

/// Strategy for table detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Detect tables using visible lines and rect edges.
    #[default]
    Lattice,
    /// Detect tables using only visible lines (no rect edges).
    LatticeStrict,
    /// Detect tables from text alignment patterns (no visible borders needed).
    Stream,
    /// Detect tables using user-provided line coordinates.
    Explicit,
}

/// Configuration for table detection.
///
/// All tolerance values default to 3.0, matching Python pdfplumber defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct TableSettings {
    /// Table detection strategy.
    pub strategy: Strategy,
    /// General snap tolerance for aligning nearby edges.
    pub snap_tolerance: f64,
    /// Snap tolerance for horizontal alignment.
    pub snap_x_tolerance: f64,
    /// Snap tolerance for vertical alignment.
    pub snap_y_tolerance: f64,
    /// General join tolerance for merging collinear edges.
    pub join_tolerance: f64,
    /// Join tolerance for horizontal edges.
    pub join_x_tolerance: f64,
    /// Join tolerance for vertical edges.
    pub join_y_tolerance: f64,
    /// Minimum edge length to consider for table detection.
    pub edge_min_length: f64,
    /// Minimum number of words sharing a vertical alignment for Stream strategy.
    pub min_words_vertical: usize,
    /// Minimum number of words sharing a horizontal alignment for Stream strategy.
    pub min_words_horizontal: usize,
    /// General text tolerance for assigning text to cells.
    pub text_tolerance: f64,
    /// Text tolerance along x-axis.
    pub text_x_tolerance: f64,
    /// Text tolerance along y-axis.
    pub text_y_tolerance: f64,
    /// General intersection tolerance for detecting edge crossings.
    pub intersection_tolerance: f64,
    /// Intersection tolerance along x-axis.
    pub intersection_x_tolerance: f64,
    /// Intersection tolerance along y-axis.
    pub intersection_y_tolerance: f64,
    /// Optional explicit line coordinates for Explicit strategy.
    pub explicit_lines: Option<ExplicitLines>,
}

impl Default for TableSettings {
    fn default() -> Self {
        Self {
            strategy: Strategy::default(),
            snap_tolerance: 3.0,
            snap_x_tolerance: 3.0,
            snap_y_tolerance: 3.0,
            join_tolerance: 3.0,
            join_x_tolerance: 3.0,
            join_y_tolerance: 3.0,
            edge_min_length: 3.0,
            min_words_vertical: 3,
            min_words_horizontal: 1,
            text_tolerance: 3.0,
            text_x_tolerance: 3.0,
            text_y_tolerance: 3.0,
            intersection_tolerance: 3.0,
            intersection_x_tolerance: 3.0,
            intersection_y_tolerance: 3.0,
            explicit_lines: None,
        }
    }
}

/// User-provided line coordinates for Explicit strategy.
#[derive(Debug, Clone, PartialEq)]
pub struct ExplicitLines {
    /// Y-coordinates for horizontal lines.
    pub horizontal_lines: Vec<f64>,
    /// X-coordinates for vertical lines.
    pub vertical_lines: Vec<f64>,
}

/// A detected table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Bounding box of the cell.
    pub bbox: BBox,
    /// Text content within the cell, if any.
    pub text: Option<String>,
}

/// A detected table.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Bounding box enclosing the entire table.
    pub bbox: BBox,
    /// All cells in the table.
    pub cells: Vec<Cell>,
    /// Cells organized into rows (top-to-bottom, left-to-right within each row).
    pub rows: Vec<Vec<Cell>>,
    /// Cells organized into columns (left-to-right, top-to-bottom within each column).
    pub columns: Vec<Vec<Cell>>,
}

/// Orchestrator for the table detection pipeline.
///
/// Takes edges (and optionally words/chars) and settings, then runs
/// the appropriate detection strategy to produce tables.
pub struct TableFinder {
    /// Edges available for table detection.
    edges: Vec<Edge>,
    /// Configuration settings.
    settings: TableSettings,
}

impl TableFinder {
    /// Create a new TableFinder with the given edges and settings.
    pub fn new(edges: Vec<Edge>, settings: TableSettings) -> Self {
        Self { edges, settings }
    }

    /// Get a reference to the settings.
    pub fn settings(&self) -> &TableSettings {
        &self.settings
    }

    /// Get a reference to the edges.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Run the table detection pipeline and return detected tables.
    ///
    /// This is a placeholder that will be filled in by subsequent user stories.
    pub fn find_tables(&self) -> Vec<Table> {
        // Pipeline will be implemented in US-030 through US-036:
        // 1. Filter edges by min_length (US-035)
        // 2. snap_edges (US-030)
        // 3. join_edges (US-031)
        // 4. edges_to_intersections (US-032)
        // 5. intersections_to_cells (US-033)
        // 6. cells_to_tables (US-034)
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Orientation;

    // --- Strategy tests ---

    #[test]
    fn test_strategy_default_is_lattice() {
        assert_eq!(Strategy::default(), Strategy::Lattice);
    }

    #[test]
    fn test_strategy_variants_are_distinct() {
        let strategies = [
            Strategy::Lattice,
            Strategy::LatticeStrict,
            Strategy::Stream,
            Strategy::Explicit,
        ];
        for i in 0..strategies.len() {
            for j in (i + 1)..strategies.len() {
                assert_ne!(strategies[i], strategies[j]);
            }
        }
    }

    #[test]
    fn test_strategy_copy() {
        let s = Strategy::Stream;
        let s2 = s;
        assert_eq!(s, s2);
    }

    // --- TableSettings tests ---

    #[test]
    fn test_table_settings_default_values() {
        let settings = TableSettings::default();
        assert_eq!(settings.strategy, Strategy::Lattice);
        assert_eq!(settings.snap_tolerance, 3.0);
        assert_eq!(settings.snap_x_tolerance, 3.0);
        assert_eq!(settings.snap_y_tolerance, 3.0);
        assert_eq!(settings.join_tolerance, 3.0);
        assert_eq!(settings.join_x_tolerance, 3.0);
        assert_eq!(settings.join_y_tolerance, 3.0);
        assert_eq!(settings.edge_min_length, 3.0);
        assert_eq!(settings.min_words_vertical, 3);
        assert_eq!(settings.min_words_horizontal, 1);
        assert_eq!(settings.text_tolerance, 3.0);
        assert_eq!(settings.text_x_tolerance, 3.0);
        assert_eq!(settings.text_y_tolerance, 3.0);
        assert_eq!(settings.intersection_tolerance, 3.0);
        assert_eq!(settings.intersection_x_tolerance, 3.0);
        assert_eq!(settings.intersection_y_tolerance, 3.0);
        assert!(settings.explicit_lines.is_none());
    }

    #[test]
    fn test_table_settings_custom_construction() {
        let settings = TableSettings {
            strategy: Strategy::Stream,
            snap_tolerance: 5.0,
            min_words_vertical: 5,
            min_words_horizontal: 2,
            ..TableSettings::default()
        };
        assert_eq!(settings.strategy, Strategy::Stream);
        assert_eq!(settings.snap_tolerance, 5.0);
        assert_eq!(settings.min_words_vertical, 5);
        assert_eq!(settings.min_words_horizontal, 2);
        // Other fields should still be defaults
        assert_eq!(settings.join_tolerance, 3.0);
        assert_eq!(settings.edge_min_length, 3.0);
    }

    #[test]
    fn test_table_settings_with_explicit_lines() {
        let settings = TableSettings {
            strategy: Strategy::Explicit,
            explicit_lines: Some(ExplicitLines {
                horizontal_lines: vec![10.0, 50.0, 100.0],
                vertical_lines: vec![20.0, 80.0, 140.0],
            }),
            ..TableSettings::default()
        };
        assert_eq!(settings.strategy, Strategy::Explicit);
        let lines = settings.explicit_lines.as_ref().unwrap();
        assert_eq!(lines.horizontal_lines.len(), 3);
        assert_eq!(lines.vertical_lines.len(), 3);
    }

    #[test]
    fn test_table_settings_strategy_selection() {
        for strategy in [
            Strategy::Lattice,
            Strategy::LatticeStrict,
            Strategy::Stream,
            Strategy::Explicit,
        ] {
            let settings = TableSettings {
                strategy,
                ..TableSettings::default()
            };
            assert_eq!(settings.strategy, strategy);
        }
    }

    // --- Cell tests ---

    #[test]
    fn test_cell_with_text() {
        let cell = Cell {
            bbox: BBox::new(10.0, 20.0, 100.0, 40.0),
            text: Some("Hello".to_string()),
        };
        assert_eq!(cell.bbox.x0, 10.0);
        assert_eq!(cell.text.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_cell_without_text() {
        let cell = Cell {
            bbox: BBox::new(10.0, 20.0, 100.0, 40.0),
            text: None,
        };
        assert!(cell.text.is_none());
    }

    // --- Table tests ---

    #[test]
    fn test_table_construction() {
        let cells = vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("B".to_string()),
            },
        ];
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
            cells: cells.clone(),
            rows: vec![cells.clone()],
            columns: vec![vec![cells[0].clone()], vec![cells[1].clone()]],
        };
        assert_eq!(table.bbox.x0, 0.0);
        assert_eq!(table.bbox.x1, 100.0);
        assert_eq!(table.cells.len(), 2);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].len(), 2);
        assert_eq!(table.columns.len(), 2);
    }

    #[test]
    fn test_table_multi_row() {
        let row1 = vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A1".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("B1".to_string()),
            },
        ];
        let row2 = vec![
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("A2".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("B2".to_string()),
            },
        ];
        let all_cells: Vec<Cell> = row1.iter().chain(row2.iter()).cloned().collect();
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
            cells: all_cells,
            rows: vec![row1, row2],
            columns: vec![
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                        text: Some("A1".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                        text: Some("A2".to_string()),
                    },
                ],
                vec![
                    Cell {
                        bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                        text: Some("B1".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                        text: Some("B2".to_string()),
                    },
                ],
            ],
        };
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.cells.len(), 4);
    }

    // --- TableFinder tests ---

    #[test]
    fn test_table_finder_construction() {
        let edges = vec![Edge {
            x0: 0.0,
            top: 50.0,
            x1: 100.0,
            bottom: 50.0,
            orientation: Orientation::Horizontal,
            source: crate::edges::EdgeSource::Line,
        }];
        let settings = TableSettings::default();
        let finder = TableFinder::new(edges.clone(), settings.clone());

        assert_eq!(finder.edges().len(), 1);
        assert_eq!(finder.settings().strategy, Strategy::Lattice);
    }

    #[test]
    fn test_table_finder_empty_edges() {
        let finder = TableFinder::new(Vec::new(), TableSettings::default());
        assert!(finder.edges().is_empty());
        let tables = finder.find_tables();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_table_finder_custom_settings() {
        let settings = TableSettings {
            strategy: Strategy::LatticeStrict,
            snap_tolerance: 5.0,
            ..TableSettings::default()
        };
        let finder = TableFinder::new(Vec::new(), settings);
        assert_eq!(finder.settings().strategy, Strategy::LatticeStrict);
        assert_eq!(finder.settings().snap_tolerance, 5.0);
    }

    // --- ExplicitLines tests ---

    #[test]
    fn test_explicit_lines_construction() {
        let lines = ExplicitLines {
            horizontal_lines: vec![0.0, 30.0, 60.0],
            vertical_lines: vec![0.0, 50.0, 100.0],
        };
        assert_eq!(lines.horizontal_lines.len(), 3);
        assert_eq!(lines.vertical_lines.len(), 3);
        assert_eq!(lines.horizontal_lines[1], 30.0);
        assert_eq!(lines.vertical_lines[2], 100.0);
    }

    #[test]
    fn test_explicit_lines_empty() {
        let lines = ExplicitLines {
            horizontal_lines: Vec::new(),
            vertical_lines: Vec::new(),
        };
        assert!(lines.horizontal_lines.is_empty());
        assert!(lines.vertical_lines.is_empty());
    }
}
