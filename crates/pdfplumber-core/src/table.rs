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

/// Snap nearby parallel edges to aligned positions.
///
/// Groups edges by orientation and clusters them along the perpendicular axis.
/// For horizontal edges, clusters by y-coordinate within `snap_y_tolerance`.
/// For vertical edges, clusters by x-coordinate within `snap_x_tolerance`.
/// Clustered edges have their perpendicular coordinates replaced with the cluster mean.
/// Diagonal edges pass through unchanged.
///
/// This does **not** merge edges — it only aligns their positions.
pub fn snap_edges(edges: Vec<Edge>, snap_x_tolerance: f64, snap_y_tolerance: f64) -> Vec<Edge> {
    use crate::geometry::Orientation;

    let mut result = Vec::with_capacity(edges.len());
    let mut horizontals: Vec<Edge> = Vec::new();
    let mut verticals: Vec<Edge> = Vec::new();

    for edge in edges {
        match edge.orientation {
            Orientation::Horizontal => horizontals.push(edge),
            Orientation::Vertical => verticals.push(edge),
            Orientation::Diagonal => result.push(edge),
        }
    }

    // Snap horizontal edges: cluster by y-coordinate (top/bottom)
    snap_group(
        &mut horizontals,
        snap_y_tolerance,
        |e| e.top,
        |e, v| {
            e.top = v;
            e.bottom = v;
        },
    );
    result.extend(horizontals);

    // Snap vertical edges: cluster by x-coordinate (x0/x1)
    snap_group(
        &mut verticals,
        snap_x_tolerance,
        |e| e.x0,
        |e, v| {
            e.x0 = v;
            e.x1 = v;
        },
    );
    result.extend(verticals);

    result
}

/// Cluster edges along a single axis and snap each cluster to its mean.
fn snap_group<F, G>(edges: &mut [Edge], tolerance: f64, key: F, mut set: G)
where
    F: Fn(&Edge) -> f64,
    G: FnMut(&mut Edge, f64),
{
    if edges.is_empty() {
        return;
    }

    // Sort by the perpendicular coordinate
    edges.sort_by(|a, b| key(a).partial_cmp(&key(b)).unwrap());

    // Build clusters of consecutive edges within tolerance
    let mut cluster_start = 0;
    for i in 1..=edges.len() {
        let end_of_cluster =
            i == edges.len() || (key(&edges[i]) - key(&edges[cluster_start])).abs() > tolerance;
        if end_of_cluster {
            // Compute mean of the cluster
            let sum: f64 = (cluster_start..i).map(|j| key(&edges[j])).sum();
            let mean = sum / (i - cluster_start) as f64;
            for edge in &mut edges[cluster_start..i] {
                set(edge, mean);
            }
            cluster_start = i;
        }
    }
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

    // --- snap_edges tests ---

    fn make_h_edge(x0: f64, y: f64, x1: f64) -> Edge {
        Edge {
            x0,
            top: y,
            x1,
            bottom: y,
            orientation: Orientation::Horizontal,
            source: crate::edges::EdgeSource::Line,
        }
    }

    fn make_v_edge(x: f64, top: f64, bottom: f64) -> Edge {
        Edge {
            x0: x,
            top,
            x1: x,
            bottom,
            orientation: Orientation::Vertical,
            source: crate::edges::EdgeSource::Line,
        }
    }

    fn assert_approx(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6,
            "expected {b}, got {a}, diff={}",
            (a - b).abs()
        );
    }

    #[test]
    fn test_snap_edges_empty() {
        let result = snap_edges(Vec::new(), 3.0, 3.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_snap_nearby_horizontal_lines() {
        // Two horizontal edges at y=50.0 and y=51.5 (within tolerance 3.0)
        // Should snap to mean = 50.75
        let edges = vec![make_h_edge(0.0, 50.0, 100.0), make_h_edge(0.0, 51.5, 100.0)];
        let result = snap_edges(edges, 3.0, 3.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        assert_eq!(horizontals.len(), 2);
        assert_approx(horizontals[0].top, 50.75);
        assert_approx(horizontals[0].bottom, 50.75);
        assert_approx(horizontals[1].top, 50.75);
        assert_approx(horizontals[1].bottom, 50.75);
    }

    #[test]
    fn test_snap_nearby_vertical_lines() {
        // Two vertical edges at x=100.0 and x=101.0 (within tolerance 3.0)
        // Should snap to mean = 100.5
        let edges = vec![
            make_v_edge(100.0, 0.0, 200.0),
            make_v_edge(101.0, 0.0, 200.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);

        let verticals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Vertical)
            .collect();
        assert_eq!(verticals.len(), 2);
        assert_approx(verticals[0].x0, 100.5);
        assert_approx(verticals[0].x1, 100.5);
        assert_approx(verticals[1].x0, 100.5);
        assert_approx(verticals[1].x1, 100.5);
    }

    #[test]
    fn test_snap_edges_far_apart_remain_unchanged() {
        // Two horizontal edges at y=50.0 and y=100.0 (far apart, beyond tolerance 3.0)
        let edges = vec![
            make_h_edge(0.0, 50.0, 100.0),
            make_h_edge(0.0, 100.0, 100.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        assert_eq!(horizontals.len(), 2);
        // They should remain at their original positions
        let mut ys: Vec<f64> = horizontals.iter().map(|e| e.top).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(ys[0], 50.0);
        assert_approx(ys[1], 100.0);
    }

    #[test]
    fn test_snap_edges_separate_x_y_tolerance() {
        // Horizontal edges within 2.0 of each other, snap_y_tolerance=1.0 (NOT within)
        // Should NOT snap
        let edges = vec![make_h_edge(0.0, 50.0, 100.0), make_h_edge(0.0, 52.0, 100.0)];
        let result = snap_edges(edges, 3.0, 1.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        let mut ys: Vec<f64> = horizontals.iter().map(|e| e.top).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(ys[0], 50.0);
        assert_approx(ys[1], 52.0);
    }

    #[test]
    fn test_snap_edges_separate_x_tolerance() {
        // Vertical edges within 2.0 of each other, snap_x_tolerance=1.0 (NOT within)
        // Should NOT snap
        let edges = vec![
            make_v_edge(100.0, 0.0, 200.0),
            make_v_edge(102.0, 0.0, 200.0),
        ];
        let result = snap_edges(edges, 1.0, 3.0);

        let verticals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Vertical)
            .collect();
        let mut xs: Vec<f64> = verticals.iter().map(|e| e.x0).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(xs[0], 100.0);
        assert_approx(xs[1], 102.0);
    }

    #[test]
    fn test_snap_edges_does_not_merge() {
        // Three horizontal edges within tolerance should snap but remain 3 separate edges
        let edges = vec![
            make_h_edge(0.0, 50.0, 100.0),
            make_h_edge(10.0, 51.0, 90.0),
            make_h_edge(20.0, 50.5, 80.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        // Still 3 edges - snap does not merge
        assert_eq!(horizontals.len(), 3);
        // All snapped to mean of 50.0, 51.0, 50.5 = 50.5
        for h in &horizontals {
            assert_approx(h.top, 50.5);
            assert_approx(h.bottom, 50.5);
        }
    }

    #[test]
    fn test_snap_edges_preserves_along_axis_coords() {
        // Snapping horizontal edges should only change y, not x
        let edges = vec![
            make_h_edge(10.0, 50.0, 200.0),
            make_h_edge(30.0, 51.0, 180.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        // x-coordinates should be unchanged
        let mut found_10 = false;
        let mut found_30 = false;
        for h in &horizontals {
            if (h.x0 - 10.0).abs() < 1e-6 {
                assert_approx(h.x1, 200.0);
                found_10 = true;
            }
            if (h.x0 - 30.0).abs() < 1e-6 {
                assert_approx(h.x1, 180.0);
                found_30 = true;
            }
        }
        assert!(found_10 && found_30, "x-coordinates should be preserved");
    }

    #[test]
    fn test_snap_edges_mixed_orientations() {
        // Mix of horizontal and vertical edges, each group snaps independently
        let edges = vec![
            make_h_edge(0.0, 50.0, 100.0),
            make_h_edge(0.0, 51.0, 100.0),
            make_v_edge(200.0, 0.0, 100.0),
            make_v_edge(201.0, 0.0, 100.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);
        assert_eq!(result.len(), 4);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        let verticals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Vertical)
            .collect();

        // Horizontal snapped to mean(50, 51) = 50.5
        for h in &horizontals {
            assert_approx(h.top, 50.5);
        }
        // Vertical snapped to mean(200, 201) = 200.5
        for v in &verticals {
            assert_approx(v.x0, 200.5);
        }
    }

    #[test]
    fn test_snap_edges_multiple_clusters() {
        // Three groups of horizontal edges, well separated
        let edges = vec![
            make_h_edge(0.0, 10.0, 100.0),
            make_h_edge(0.0, 11.0, 100.0),
            // gap
            make_h_edge(0.0, 50.0, 100.0),
            make_h_edge(0.0, 51.0, 100.0),
            // gap
            make_h_edge(0.0, 100.0, 100.0),
            make_h_edge(0.0, 101.0, 100.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        assert_eq!(horizontals.len(), 6);

        let mut ys: Vec<f64> = horizontals.iter().map(|e| e.top).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Cluster 1: mean(10, 11) = 10.5
        assert_approx(ys[0], 10.5);
        assert_approx(ys[1], 10.5);
        // Cluster 2: mean(50, 51) = 50.5
        assert_approx(ys[2], 50.5);
        assert_approx(ys[3], 50.5);
        // Cluster 3: mean(100, 101) = 100.5
        assert_approx(ys[4], 100.5);
        assert_approx(ys[5], 100.5);
    }

    #[test]
    fn test_snap_edges_single_edge_unchanged() {
        let edges = vec![make_h_edge(0.0, 50.0, 100.0)];
        let result = snap_edges(edges, 3.0, 3.0);
        assert_eq!(result.len(), 1);
        assert_approx(result[0].top, 50.0);
        assert_approx(result[0].bottom, 50.0);
    }

    #[test]
    fn test_snap_edges_diagonal_passed_through() {
        let edges = vec![
            Edge {
                x0: 0.0,
                top: 0.0,
                x1: 100.0,
                bottom: 100.0,
                orientation: Orientation::Diagonal,
                source: crate::edges::EdgeSource::Curve,
            },
            make_h_edge(0.0, 50.0, 100.0),
        ];
        let result = snap_edges(edges, 3.0, 3.0);
        assert_eq!(result.len(), 2);

        let diagonals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Diagonal)
            .collect();
        assert_eq!(diagonals.len(), 1);
        // Diagonal edge unchanged
        assert_approx(diagonals[0].x0, 0.0);
        assert_approx(diagonals[0].top, 0.0);
        assert_approx(diagonals[0].x1, 100.0);
        assert_approx(diagonals[0].bottom, 100.0);
    }

    #[test]
    fn test_snap_edges_zero_tolerance() {
        // With zero tolerance, only exact matches snap
        let edges = vec![
            make_h_edge(0.0, 50.0, 100.0),
            make_h_edge(0.0, 50.0, 100.0), // exact same y
            make_h_edge(0.0, 50.1, 100.0), // different y
        ];
        let result = snap_edges(edges, 0.0, 0.0);

        let horizontals: Vec<&Edge> = result
            .iter()
            .filter(|e| e.orientation == Orientation::Horizontal)
            .collect();
        assert_eq!(horizontals.len(), 3);
        let mut ys: Vec<f64> = horizontals.iter().map(|e| e.top).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(ys[0], 50.0);
        assert_approx(ys[1], 50.0);
        assert_approx(ys[2], 50.1);
    }
}
