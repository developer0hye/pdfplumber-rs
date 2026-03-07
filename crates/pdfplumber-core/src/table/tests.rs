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

// --- join_edge_group tests ---

#[test]
fn test_join_edge_group_empty() {
    let result = join_edge_group(Vec::new(), 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_join_edge_group_single_edge_unchanged() {
    let edges = vec![make_h_edge(10.0, 50.0, 80.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].x0, 10.0);
    assert_approx(result[0].x1, 80.0);
}

#[test]
fn test_join_two_overlapping_horizontal_edges() {
    // Two horizontal edges at y=50 that overlap: [10..60] and [40..90]
    // Should merge into [10..90]
    let edges = vec![make_h_edge(10.0, 50.0, 60.0), make_h_edge(40.0, 50.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].x0, 10.0);
    assert_approx(result[0].x1, 90.0);
    assert_approx(result[0].top, 50.0);
}

#[test]
fn test_join_two_adjacent_horizontal_edges_within_tolerance() {
    // Two horizontal edges at y=50: [10..50] and [52..90]
    // Gap is 2.0, within join_x_tolerance=3.0 → merge to [10..90]
    let edges = vec![make_h_edge(10.0, 50.0, 50.0), make_h_edge(52.0, 50.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].x0, 10.0);
    assert_approx(result[0].x1, 90.0);
}

#[test]
fn test_join_distant_horizontal_edges_not_merged() {
    // Two horizontal edges at y=50: [10..40] and [60..90]
    // Gap is 20.0, beyond tolerance → remain separate
    let edges = vec![make_h_edge(10.0, 50.0, 40.0), make_h_edge(60.0, 50.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_join_chain_of_three_horizontal_segments() {
    // Three segments on y=50: [10..40], [38..70], [68..100]
    // All overlap pairwise → chain merge to [10..100]
    let edges = vec![
        make_h_edge(10.0, 50.0, 40.0),
        make_h_edge(38.0, 50.0, 70.0),
        make_h_edge(68.0, 50.0, 100.0),
    ];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].x0, 10.0);
    assert_approx(result[0].x1, 100.0);
}

#[test]
fn test_join_two_overlapping_vertical_edges() {
    // Two vertical edges at x=50: [10..60] and [40..90]
    // Should merge into [10..90]
    let edges = vec![make_v_edge(50.0, 10.0, 60.0), make_v_edge(50.0, 40.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].top, 10.0);
    assert_approx(result[0].bottom, 90.0);
    assert_approx(result[0].x0, 50.0);
}

#[test]
fn test_join_adjacent_vertical_edges_within_tolerance() {
    // Two vertical edges at x=50: [10..50] and [52..90]
    // Gap is 2.0, within join_y_tolerance=3.0 → merge
    let edges = vec![make_v_edge(50.0, 10.0, 50.0), make_v_edge(50.0, 52.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert_approx(result[0].top, 10.0);
    assert_approx(result[0].bottom, 90.0);
}

#[test]
fn test_join_groups_by_collinear_position() {
    // Two groups of horizontal edges at different y positions
    // Group 1: y=50, [10..50] and [48..90] → merge to [10..90]
    // Group 2: y=100, [10..40] and [60..90] → gap too big, stay separate
    let edges = vec![
        make_h_edge(10.0, 50.0, 50.0),
        make_h_edge(48.0, 50.0, 90.0),
        make_h_edge(10.0, 100.0, 40.0),
        make_h_edge(60.0, 100.0, 90.0),
    ];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 3);

    let at_50: Vec<&Edge> = result
        .iter()
        .filter(|e| (e.top - 50.0).abs() < 1e-6)
        .collect();
    assert_eq!(at_50.len(), 1);
    assert_approx(at_50[0].x0, 10.0);
    assert_approx(at_50[0].x1, 90.0);

    let at_100: Vec<&Edge> = result
        .iter()
        .filter(|e| (e.top - 100.0).abs() < 1e-6)
        .collect();
    assert_eq!(at_100.len(), 2);
}

#[test]
fn test_join_mixed_orientations() {
    // Mix of horizontal and vertical edges: each group joins independently
    let edges = vec![
        make_h_edge(10.0, 50.0, 50.0),
        make_h_edge(48.0, 50.0, 90.0),
        make_v_edge(200.0, 10.0, 50.0),
        make_v_edge(200.0, 48.0, 90.0),
    ];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 2);

    let horizontals: Vec<&Edge> = result
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert_eq!(horizontals.len(), 1);
    assert_approx(horizontals[0].x0, 10.0);
    assert_approx(horizontals[0].x1, 90.0);

    let verticals: Vec<&Edge> = result
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert_eq!(verticals.len(), 1);
    assert_approx(verticals[0].top, 10.0);
    assert_approx(verticals[0].bottom, 90.0);
}

#[test]
fn test_join_separate_x_y_tolerance() {
    // Horizontal edges: gap=4.0, join_x_tolerance=3.0 → NOT merged
    let edges = vec![make_h_edge(10.0, 50.0, 40.0), make_h_edge(44.0, 50.0, 80.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 2);

    // Vertical edges: gap=4.0, join_y_tolerance=5.0 → merged
    let edges = vec![make_v_edge(50.0, 10.0, 40.0), make_v_edge(50.0, 44.0, 80.0)];
    let result = join_edge_group(edges, 3.0, 5.0);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_join_diagonal_edges_pass_through() {
    let diag = Edge {
        x0: 0.0,
        top: 0.0,
        x1: 100.0,
        bottom: 100.0,
        orientation: Orientation::Diagonal,
        source: crate::edges::EdgeSource::Curve,
    };
    let edges = vec![diag.clone(), make_h_edge(10.0, 50.0, 90.0)];
    let result = join_edge_group(edges, 3.0, 3.0);
    assert_eq!(result.len(), 2);

    let diagonals: Vec<&Edge> = result
        .iter()
        .filter(|e| e.orientation == Orientation::Diagonal)
        .collect();
    assert_eq!(diagonals.len(), 1);
    assert_approx(diagonals[0].x0, 0.0);
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

// --- edges_to_intersections tests ---

fn has_intersection(intersections: &[Intersection], x: f64, y: f64) -> bool {
    intersections
        .iter()
        .any(|i| (i.x - x).abs() < 1e-6 && (i.y - y).abs() < 1e-6)
}

#[test]
fn test_intersections_empty_edges() {
    let result = edges_to_intersections(&[], 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_simple_cross() {
    // Horizontal edge at y=50 from x=0 to x=100
    // Vertical edge at x=50 from y=0 to y=100
    // Should intersect at (50, 50)
    let edges = vec![make_h_edge(0.0, 50.0, 100.0), make_v_edge(50.0, 0.0, 100.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_t_intersection() {
    // Horizontal edge at y=50 from x=0 to x=100
    // Vertical edge at x=50 from y=50 to y=100 (starts at the horizontal edge)
    // Should intersect at (50, 50)
    let edges = vec![
        make_h_edge(0.0, 50.0, 100.0),
        make_v_edge(50.0, 50.0, 100.0),
    ];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_l_intersection_corner() {
    // Horizontal edge at y=50 from x=50 to x=100
    // Vertical edge at x=50 from y=0 to y=50
    // Corner at (50, 50)
    let edges = vec![make_h_edge(50.0, 50.0, 100.0), make_v_edge(50.0, 0.0, 50.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_no_intersection_parallel() {
    // Two parallel horizontal edges — no intersections
    let edges = vec![make_h_edge(0.0, 50.0, 100.0), make_h_edge(0.0, 80.0, 100.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_no_intersection_non_overlapping() {
    // Horizontal edge at y=50 from x=0 to x=40
    // Vertical edge at x=60 from y=0 to y=100
    // They don't overlap in x-range (40 < 60 with tolerance 3 → 40+3=43 < 60)
    let edges = vec![make_h_edge(0.0, 50.0, 40.0), make_v_edge(60.0, 0.0, 100.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_tolerance_based() {
    // Horizontal edge at y=50 from x=0 to x=48
    // Vertical edge at x=50 from y=0 to y=100
    // Gap in x: 50 - 48 = 2, within tolerance 3 → should intersect
    let edges = vec![make_h_edge(0.0, 50.0, 48.0), make_v_edge(50.0, 0.0, 100.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_tolerance_y_based() {
    // Horizontal edge at y=50 from x=0 to x=100
    // Vertical edge at x=50 from y=0 to y=48
    // Gap in y: 50 - 48 = 2, within tolerance 3 → should intersect
    let edges = vec![make_h_edge(0.0, 50.0, 100.0), make_v_edge(50.0, 0.0, 48.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_beyond_tolerance_no_match() {
    // Horizontal edge at y=50 from x=0 to x=45
    // Vertical edge at x=50 from y=0 to y=100
    // Gap in x: 50 - 45 = 5, beyond tolerance 3 → no intersection
    let edges = vec![make_h_edge(0.0, 50.0, 45.0), make_v_edge(50.0, 0.0, 100.0)];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_grid_2x2() {
    // 2x2 grid: 3 horizontal edges × 3 vertical edges = 9 intersections
    // H: y=0, y=50, y=100 (all from x=0 to x=100)
    // V: x=0, x=50, x=100 (all from y=0 to y=100)
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        make_h_edge(0.0, 100.0, 100.0),
        make_v_edge(0.0, 0.0, 100.0),
        make_v_edge(50.0, 0.0, 100.0),
        make_v_edge(100.0, 0.0, 100.0),
    ];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 9);
    // Check corners
    assert!(has_intersection(&result, 0.0, 0.0));
    assert!(has_intersection(&result, 100.0, 0.0));
    assert!(has_intersection(&result, 0.0, 100.0));
    assert!(has_intersection(&result, 100.0, 100.0));
    // Check center
    assert!(has_intersection(&result, 50.0, 50.0));
}

#[test]
fn test_intersections_ignores_diagonal_edges() {
    // Diagonal edge should be ignored
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
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_multiple_h_one_v() {
    // Three horizontal edges at y=10, y=50, y=90 (x=0..100)
    // One vertical edge at x=50 (y=0..100)
    // Should yield 3 intersections at (50,10), (50,50), (50,90)
    let edges = vec![
        make_h_edge(0.0, 10.0, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        make_h_edge(0.0, 90.0, 100.0),
        make_v_edge(50.0, 0.0, 100.0),
    ];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 3);
    assert!(has_intersection(&result, 50.0, 10.0));
    assert!(has_intersection(&result, 50.0, 50.0));
    assert!(has_intersection(&result, 50.0, 90.0));
}

#[test]
fn test_intersections_separate_x_y_tolerance() {
    // Horizontal edge at y=50, x=0..48
    // Vertical edge at x=50, y=0..100
    // Gap in x is 2.0. With x_tolerance=1.0, should NOT intersect
    let edges = vec![make_h_edge(0.0, 50.0, 48.0), make_v_edge(50.0, 0.0, 100.0)];
    let result = edges_to_intersections(&edges, 1.0, 3.0);
    assert!(result.is_empty());

    // Same setup but with x_tolerance=3.0, should intersect
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_intersections_no_duplicate_points() {
    // Two horizontal edges at the same y, one vertical edge
    // Both horizontals cross the vertical at the same point
    // Should produce only one intersection point (deduplicated)
    let edges = vec![
        make_h_edge(0.0, 50.0, 100.0),
        make_h_edge(20.0, 50.0, 80.0),
        make_v_edge(50.0, 0.0, 100.0),
    ];
    let result = edges_to_intersections(&edges, 3.0, 3.0);
    // Both horizontals at y=50 cross vertical at x=50 → same point (50, 50)
    // Should be deduplicated to 1 intersection
    assert_eq!(result.len(), 1);
    assert!(has_intersection(&result, 50.0, 50.0));
}

// --- intersections_to_cells tests ---

fn make_intersection(x: f64, y: f64) -> Intersection {
    Intersection { x, y }
}

#[test]
fn test_intersections_to_cells_empty() {
    let result = intersections_to_cells(&[]);
    assert!(result.is_empty());
}

#[test]
fn test_intersections_to_cells_simple_2x2_grid() {
    // 2x2 grid of intersections → 1 cell
    // (0,0) (100,0)
    // (0,50) (100,50)
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let cells = intersections_to_cells(&intersections);
    assert_eq!(cells.len(), 1);
    assert_approx(cells[0].bbox.x0, 0.0);
    assert_approx(cells[0].bbox.top, 0.0);
    assert_approx(cells[0].bbox.x1, 100.0);
    assert_approx(cells[0].bbox.bottom, 50.0);
    assert!(cells[0].text.is_none());
}

#[test]
fn test_intersections_to_cells_3x3_grid() {
    // 3x3 grid of intersections → 4 cells
    //  (0,0)  (50,0)  (100,0)
    //  (0,30) (50,30) (100,30)
    //  (0,60) (50,60) (100,60)
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(50.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 30.0),
        make_intersection(50.0, 30.0),
        make_intersection(100.0, 30.0),
        make_intersection(0.0, 60.0),
        make_intersection(50.0, 60.0),
        make_intersection(100.0, 60.0),
    ];
    let cells = intersections_to_cells(&intersections);
    assert_eq!(cells.len(), 4);

    // Top-left cell
    assert!(cells.iter().any(|c| (c.bbox.x0 - 0.0).abs() < 1e-6
        && (c.bbox.top - 0.0).abs() < 1e-6
        && (c.bbox.x1 - 50.0).abs() < 1e-6
        && (c.bbox.bottom - 30.0).abs() < 1e-6));
    // Top-right cell
    assert!(cells.iter().any(|c| (c.bbox.x0 - 50.0).abs() < 1e-6
        && (c.bbox.top - 0.0).abs() < 1e-6
        && (c.bbox.x1 - 100.0).abs() < 1e-6
        && (c.bbox.bottom - 30.0).abs() < 1e-6));
    // Bottom-left cell
    assert!(cells.iter().any(|c| (c.bbox.x0 - 0.0).abs() < 1e-6
        && (c.bbox.top - 30.0).abs() < 1e-6
        && (c.bbox.x1 - 50.0).abs() < 1e-6
        && (c.bbox.bottom - 60.0).abs() < 1e-6));
    // Bottom-right cell
    assert!(cells.iter().any(|c| (c.bbox.x0 - 50.0).abs() < 1e-6
        && (c.bbox.top - 30.0).abs() < 1e-6
        && (c.bbox.x1 - 100.0).abs() < 1e-6
        && (c.bbox.bottom - 60.0).abs() < 1e-6));
}

#[test]
fn test_intersections_to_cells_missing_corner() {
    // 2x2 grid but missing the bottom-right corner → 0 cells
    // (0,0) (100,0)
    // (0,50) ---missing---
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
    ];
    let cells = intersections_to_cells(&intersections);
    assert!(cells.is_empty());
}

#[test]
fn test_intersections_to_cells_irregular_grid() {
    // 3x3 grid but missing center intersection → only corners that form complete rectangles
    // (0,0)  (50,0)  (100,0)
    // (0,30) ---X--- (100,30)
    // (0,60) (50,60) (100,60)
    // Without (50,30): top-left and bottom-left cells lose a corner.
    // Only (0,0)-(100,0)-(0,30)-(100,30) is complete → 1 big cell top row
    // And (0,30)-(100,30)-(0,60)-(100,60) is complete → 1 big cell bottom row
    // Plus (0,60)-(50,60) and (50,60)-(100,60) don't have top corners at 50,30
    // So we get: cells that have all 4 corners present
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(50.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 30.0),
        // (50, 30) missing
        make_intersection(100.0, 30.0),
        make_intersection(0.0, 60.0),
        make_intersection(50.0, 60.0),
        make_intersection(100.0, 60.0),
    ];
    let cells = intersections_to_cells(&intersections);
    // Top row: (0,0)-(50,0)-(0,30)-(50,30)? No, (50,30) missing → skip
    //          (50,0)-(100,0)-(50,30)-(100,30)? No, (50,30) missing → skip
    //          (0,0)-(100,0)-(0,30)-(100,30)? The grid only checks adjacent columns.
    //            xs = [0, 50, 100], adjacent pairs are (0,50) and (50,100)
    //            So this cell would not be formed from the adjacent pair logic.
    // Bottom row: (0,30)-(50,30)? (50,30) missing → skip
    //             (50,30)-(100,30)? (50,30) missing → skip
    // Bottom row with y=30..60: (0,30)-(50,30) missing → skip; (50,30)-(100,30) missing → skip
    //   But (0,30)-(100,30)-(0,60)-(100,60) is NOT adjacent columns
    // Result: 0 cells (because the missing center breaks all adjacent cell formations)
    // Wait - let me reconsider:
    // xs = [0, 50, 100], ys = [0, 30, 60]
    // (0,50) x (0,30): corners (0,0),(50,0),(0,30),(50,30) → (50,30) missing → skip
    // (50,100) x (0,30): corners (50,0),(100,0),(50,30),(100,30) → (50,30) missing → skip
    // (0,50) x (30,60): corners (0,30),(50,30),(0,60),(50,60) → (50,30) missing → skip
    // (50,100) x (30,60): corners (50,30),(100,30),(50,60),(100,60) → (50,30) missing → skip
    // All cells need (50,30) which is missing → 0 cells
    assert_eq!(cells.len(), 0);
}

#[test]
fn test_intersections_to_cells_partial_grid_with_valid_cells() {
    // L-shaped grid where some cells are complete
    // (0,0) (50,0)
    // (0,30) (50,30) (100,30)
    //                (100,60)
    // Only the top-left cell (0,0)-(50,0)-(0,30)-(50,30) has all 4 corners
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(50.0, 0.0),
        make_intersection(0.0, 30.0),
        make_intersection(50.0, 30.0),
        make_intersection(100.0, 30.0),
        make_intersection(100.0, 60.0),
    ];
    let cells = intersections_to_cells(&intersections);
    assert_eq!(cells.len(), 1);
    assert_approx(cells[0].bbox.x0, 0.0);
    assert_approx(cells[0].bbox.top, 0.0);
    assert_approx(cells[0].bbox.x1, 50.0);
    assert_approx(cells[0].bbox.bottom, 30.0);
}

#[test]
fn test_intersections_to_cells_single_point() {
    // Single intersection point → no cells
    let intersections = vec![make_intersection(50.0, 50.0)];
    let cells = intersections_to_cells(&intersections);
    assert!(cells.is_empty());
}

#[test]
fn test_intersections_to_cells_collinear_points() {
    // Points along a single line (no area) → no cells
    let intersections = vec![
        make_intersection(0.0, 50.0),
        make_intersection(50.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let cells = intersections_to_cells(&intersections);
    assert!(cells.is_empty());
}

#[test]
fn test_intersections_to_cells_4x3_grid() {
    // 4 columns × 3 rows → 3×2 = 6 cells
    let mut intersections = Vec::new();
    for &x in &[0.0, 40.0, 80.0, 120.0] {
        for &y in &[0.0, 30.0, 60.0] {
            intersections.push(make_intersection(x, y));
        }
    }
    let cells = intersections_to_cells(&intersections);
    assert_eq!(cells.len(), 6);
}

#[test]
fn test_intersections_to_cells_text_is_none() {
    // All cells should have text = None (text extraction is a separate step)
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let cells = intersections_to_cells(&intersections);
    for cell in &cells {
        assert!(cell.text.is_none());
    }
}

// --- edges_to_cells tests ---

#[test]
fn test_edges_to_cells_complete_grid() {
    // Complete 2x2 grid: 4 corners, 4 edges → 1 cell
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        make_v_edge(0.0, 0.0, 50.0),
        make_v_edge(100.0, 0.0, 50.0),
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    assert_eq!(cells.len(), 1);
    assert_approx(cells[0].bbox.x0, 0.0);
    assert_approx(cells[0].bbox.top, 0.0);
    assert_approx(cells[0].bbox.x1, 100.0);
    assert_approx(cells[0].bbox.bottom, 50.0);
}

#[test]
fn test_edges_to_cells_partial_intersections_with_spanning_edges() {
    // Simulates the nics-background-checks scenario:
    // Only outer corners have intersections, but edges span the full width.
    // 3 columns, 2 rows, but only outer border intersections at y=0.
    //
    //  (0,0)                 (100,0)   <- only 2 intersections at y=0
    //        H edge spans [0, 100] at y=0
    //  (0,30) (50,30) (100,30)         <- all 3 intersections at y=30
    //        H edge spans [0, 100] at y=30
    //
    // Vertical edges at x=0,50,100 span [0,30].
    // With edge coverage, cells at y=[0,30] should be created for x=[0,50] and x=[50,100]
    // because the horizontal edge at y=0 spans [0,100] covering both cells.
    let intersections = vec![
        make_intersection(0.0, 0.0),
        // no intersection at (50, 0)
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 30.0),
        make_intersection(50.0, 30.0),
        make_intersection(100.0, 30.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),  // top spans full width
        make_h_edge(0.0, 30.0, 100.0), // bottom spans full width
        make_v_edge(0.0, 0.0, 30.0),   // left border
        make_v_edge(50.0, 0.0, 30.0),  // middle divider
        make_v_edge(100.0, 0.0, 30.0), // right border
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    // Should produce 2 cells: (0,0)-(50,30) and (50,0)-(100,30)
    assert_eq!(cells.len(), 2);
    assert!(cells.iter().any(|c| (c.bbox.x0 - 0.0).abs() < 1e-6
        && (c.bbox.top - 0.0).abs() < 1e-6
        && (c.bbox.x1 - 50.0).abs() < 1e-6
        && (c.bbox.bottom - 30.0).abs() < 1e-6));
    assert!(cells.iter().any(|c| (c.bbox.x0 - 50.0).abs() < 1e-6
        && (c.bbox.top - 0.0).abs() < 1e-6
        && (c.bbox.x1 - 100.0).abs() < 1e-6
        && (c.bbox.bottom - 30.0).abs() < 1e-6));
}

#[test]
fn test_edges_to_cells_no_edges_no_cells() {
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let cells = edges_to_cells(&intersections, &[], 3.0, 3.0);
    assert!(cells.is_empty());
}

#[test]
fn test_edges_to_cells_empty_intersections() {
    let cells = edges_to_cells(&[], &[], 3.0, 3.0);
    assert!(cells.is_empty());
}

#[test]
fn test_edges_to_cells_single_row_table() {
    // Single row with 3 columns, all edges present
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(30.0, 0.0),
        make_intersection(60.0, 0.0),
        make_intersection(90.0, 0.0),
        make_intersection(0.0, 20.0),
        make_intersection(30.0, 20.0),
        make_intersection(60.0, 20.0),
        make_intersection(90.0, 20.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 0.0, 90.0),
        make_h_edge(0.0, 20.0, 90.0),
        make_v_edge(0.0, 0.0, 20.0),
        make_v_edge(30.0, 0.0, 20.0),
        make_v_edge(60.0, 0.0, 20.0),
        make_v_edge(90.0, 0.0, 20.0),
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    assert_eq!(cells.len(), 3);
}

#[test]
fn test_edges_to_cells_missing_vertical_no_cell() {
    // Missing vertical edge at x=50 means cells adjacent to x=50 are invalid
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(50.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 30.0),
        make_intersection(50.0, 30.0),
        make_intersection(100.0, 30.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 30.0, 100.0),
        make_v_edge(0.0, 0.0, 30.0),
        // no vertical at x=50
        make_v_edge(100.0, 0.0, 30.0),
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    // Cell (0,0)-(50,30): V left OK, V right at x=50 missing → skip
    // Cell (50,0)-(100,30): V left at x=50 missing → skip
    assert_eq!(cells.len(), 0);
}

#[test]
fn test_edges_to_cells_tolerance_matching() {
    // Edges slightly off from intersection positions, within tolerance
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 1.5, 100.0),  // y=1.5, within 3.0 of y=0
        make_h_edge(0.0, 48.5, 100.0), // y=48.5, within 3.0 of y=50
        make_v_edge(1.0, 0.0, 50.0),   // x=1.0, within 3.0 of x=0
        make_v_edge(99.0, 0.0, 50.0),  // x=99.0, within 3.0 of x=100
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    assert_eq!(cells.len(), 1);
}

#[test]
fn test_edges_to_cells_text_is_none() {
    let intersections = vec![
        make_intersection(0.0, 0.0),
        make_intersection(100.0, 0.0),
        make_intersection(0.0, 50.0),
        make_intersection(100.0, 50.0),
    ];
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        make_v_edge(0.0, 0.0, 50.0),
        make_v_edge(100.0, 0.0, 50.0),
    ];
    let cells = edges_to_cells(&intersections, &edges, 3.0, 3.0);
    for cell in &cells {
        assert!(cell.text.is_none());
    }
}

// --- normalize_table_columns tests ---

#[test]
fn test_normalize_table_columns_uniform_grid() {
    // 2x2 uniform grid: no merged cells → should be unchanged
    let cells = vec![
        Cell {
            bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
            text: Some("A".to_string()),
        },
        Cell {
            bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
            text: Some("B".to_string()),
        },
        Cell {
            bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
            text: Some("C".to_string()),
        },
        Cell {
            bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
            text: Some("D".to_string()),
        },
    ];
    let table = cells_to_tables(cells);
    assert_eq!(table.len(), 1);
    let normalized = normalize_table_columns(&table[0]);
    assert_eq!(normalized.rows.len(), 2);
    assert_eq!(normalized.rows[0].len(), 2);
    assert_eq!(normalized.rows[1].len(), 2);
    assert_eq!(normalized.rows[0][0].text.as_deref(), Some("A"));
    assert_eq!(normalized.rows[0][1].text.as_deref(), Some("B"));
    assert_eq!(normalized.rows[1][0].text.as_deref(), Some("C"));
    assert_eq!(normalized.rows[1][1].text.as_deref(), Some("D"));
}

#[test]
fn test_normalize_table_columns_merged_header() {
    // Row 0: 1 wide cell spanning full width (merged header)
    // Row 1: 2 normal cells
    // After normalization: row 0 should have 2 cells (text in first, None in second)
    let cells = vec![
        Cell {
            bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
            text: Some("Title".to_string()),
        },
        Cell {
            bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
            text: Some("C".to_string()),
        },
        Cell {
            bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
            text: Some("D".to_string()),
        },
    ];
    let table = cells_to_tables(cells);
    assert_eq!(table.len(), 1);
    let normalized = normalize_table_columns(&table[0]);
    assert_eq!(normalized.rows.len(), 2);
    // Row 0: merged cell split into 2, text in first only
    assert_eq!(normalized.rows[0].len(), 2);
    assert_eq!(normalized.rows[0][0].text.as_deref(), Some("Title"));
    assert!(normalized.rows[0][1].text.is_none());
    // Row 1: unchanged
    assert_eq!(normalized.rows[1].len(), 2);
    assert_eq!(normalized.rows[1][0].text.as_deref(), Some("C"));
    assert_eq!(normalized.rows[1][1].text.as_deref(), Some("D"));
}

#[test]
fn test_normalize_table_columns_empty_table() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 100.0),
        cells: vec![],
        rows: vec![],
        columns: vec![],
    };
    let normalized = normalize_table_columns(&table);
    assert!(normalized.cells.is_empty());
}

// --- cells_to_tables tests ---

fn make_cell(x0: f64, top: f64, x1: f64, bottom: f64) -> Cell {
    Cell {
        bbox: BBox::new(x0, top, x1, bottom),
        text: None,
    }
}

#[test]
fn test_cells_to_tables_empty() {
    let tables = cells_to_tables(Vec::new());
    assert!(tables.is_empty());
}

#[test]
fn test_cells_to_tables_single_cell() {
    // A single cell forms a single table
    let cells = vec![make_cell(0.0, 0.0, 50.0, 30.0)];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    assert_approx(tables[0].bbox.x0, 0.0);
    assert_approx(tables[0].bbox.top, 0.0);
    assert_approx(tables[0].bbox.x1, 50.0);
    assert_approx(tables[0].bbox.bottom, 30.0);
    assert_eq!(tables[0].cells.len(), 1);
    assert_eq!(tables[0].rows.len(), 1);
    assert_eq!(tables[0].rows[0].len(), 1);
    assert_eq!(tables[0].columns.len(), 1);
    assert_eq!(tables[0].columns[0].len(), 1);
}

#[test]
fn test_cells_to_tables_single_table_2x2() {
    // 2x2 grid: 4 cells sharing edges → 1 table
    let cells = vec![
        make_cell(0.0, 0.0, 50.0, 30.0),
        make_cell(50.0, 0.0, 100.0, 30.0),
        make_cell(0.0, 30.0, 50.0, 60.0),
        make_cell(50.0, 30.0, 100.0, 60.0),
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    assert_approx(tables[0].bbox.x0, 0.0);
    assert_approx(tables[0].bbox.top, 0.0);
    assert_approx(tables[0].bbox.x1, 100.0);
    assert_approx(tables[0].bbox.bottom, 60.0);
    assert_eq!(tables[0].cells.len(), 4);
    // 2 rows, each with 2 cells
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0].len(), 2);
    assert_eq!(tables[0].rows[1].len(), 2);
    // 2 columns, each with 2 cells
    assert_eq!(tables[0].columns.len(), 2);
    assert_eq!(tables[0].columns[0].len(), 2);
    assert_eq!(tables[0].columns[1].len(), 2);
}

#[test]
fn test_cells_to_tables_single_table_rows_ordered() {
    // Verify rows are ordered top-to-bottom, left-to-right
    let cells = vec![
        make_cell(50.0, 30.0, 100.0, 60.0), // bottom-right (given first to test ordering)
        make_cell(0.0, 0.0, 50.0, 30.0),    // top-left
        make_cell(50.0, 0.0, 100.0, 30.0),  // top-right
        make_cell(0.0, 30.0, 50.0, 60.0),   // bottom-left
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    // Row 0 (top): left then right
    assert_approx(tables[0].rows[0][0].bbox.x0, 0.0);
    assert_approx(tables[0].rows[0][1].bbox.x0, 50.0);
    // Row 1 (bottom): left then right
    assert_approx(tables[0].rows[1][0].bbox.x0, 0.0);
    assert_approx(tables[0].rows[1][1].bbox.x0, 50.0);
}

#[test]
fn test_cells_to_tables_single_table_columns_ordered() {
    // Verify columns are ordered left-to-right, top-to-bottom
    let cells = vec![
        make_cell(0.0, 0.0, 50.0, 30.0),
        make_cell(50.0, 0.0, 100.0, 30.0),
        make_cell(0.0, 30.0, 50.0, 60.0),
        make_cell(50.0, 30.0, 100.0, 60.0),
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    // Column 0 (left): top then bottom
    assert_approx(tables[0].columns[0][0].bbox.top, 0.0);
    assert_approx(tables[0].columns[0][1].bbox.top, 30.0);
    // Column 1 (right): top then bottom
    assert_approx(tables[0].columns[1][0].bbox.top, 0.0);
    assert_approx(tables[0].columns[1][1].bbox.top, 30.0);
}

#[test]
fn test_cells_to_tables_two_separate_tables() {
    // Two tables far apart on the same page
    // Table 1: top-left area
    // Table 2: bottom-right area (no shared edges)
    let cells = vec![
        // Table 1
        make_cell(0.0, 0.0, 50.0, 30.0),
        make_cell(50.0, 0.0, 100.0, 30.0),
        // Table 2
        make_cell(200.0, 200.0, 250.0, 230.0),
        make_cell(250.0, 200.0, 300.0, 230.0),
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 2);

    // Sort tables by x0 to get deterministic order
    let mut tables = tables;
    tables.sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap());

    // Table 1
    assert_approx(tables[0].bbox.x0, 0.0);
    assert_approx(tables[0].bbox.x1, 100.0);
    assert_eq!(tables[0].cells.len(), 2);
    assert_eq!(tables[0].rows.len(), 1);
    assert_eq!(tables[0].columns.len(), 2);

    // Table 2
    assert_approx(tables[1].bbox.x0, 200.0);
    assert_approx(tables[1].bbox.x1, 300.0);
    assert_eq!(tables[1].cells.len(), 2);
    assert_eq!(tables[1].rows.len(), 1);
    assert_eq!(tables[1].columns.len(), 2);
}

#[test]
fn test_cells_to_tables_3x3_grid() {
    // 3 cols × 3 rows = 9 cells, all connected → 1 table
    let mut cells = Vec::new();
    for row in 0..3 {
        for col in 0..3 {
            let x0 = col as f64 * 40.0;
            let top = row as f64 * 30.0;
            cells.push(make_cell(x0, top, x0 + 40.0, top + 30.0));
        }
    }
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 9);
    assert_eq!(tables[0].rows.len(), 3);
    for row in &tables[0].rows {
        assert_eq!(row.len(), 3);
    }
    assert_eq!(tables[0].columns.len(), 3);
    for col in &tables[0].columns {
        assert_eq!(col.len(), 3);
    }
    assert_approx(tables[0].bbox.x0, 0.0);
    assert_approx(tables[0].bbox.top, 0.0);
    assert_approx(tables[0].bbox.x1, 120.0);
    assert_approx(tables[0].bbox.bottom, 90.0);
}

#[test]
fn test_cells_to_tables_single_row() {
    // 3 cells in a single row → 1 table with 1 row, 3 columns
    let cells = vec![
        make_cell(0.0, 0.0, 40.0, 30.0),
        make_cell(40.0, 0.0, 80.0, 30.0),
        make_cell(80.0, 0.0, 120.0, 30.0),
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 1);
    assert_eq!(tables[0].rows[0].len(), 3);
    assert_eq!(tables[0].columns.len(), 3);
    for col in &tables[0].columns {
        assert_eq!(col.len(), 1);
    }
}

#[test]
fn test_cells_to_tables_single_column() {
    // 3 cells in a single column → 1 table with 3 rows, 1 column
    let cells = vec![
        make_cell(0.0, 0.0, 50.0, 30.0),
        make_cell(0.0, 30.0, 50.0, 60.0),
        make_cell(0.0, 60.0, 50.0, 90.0),
    ];
    let tables = cells_to_tables(cells);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 3);
    for row in &tables[0].rows {
        assert_eq!(row.len(), 1);
    }
    assert_eq!(tables[0].columns.len(), 1);
    assert_eq!(tables[0].columns[0].len(), 3);
}

// --- US-035: Lattice strategy - full pipeline tests ---

fn make_h_edge_src(x0: f64, y: f64, x1: f64, source: crate::edges::EdgeSource) -> Edge {
    Edge {
        x0,
        top: y,
        x1,
        bottom: y,
        orientation: Orientation::Horizontal,
        source,
    }
}

fn make_v_edge_src(x: f64, top: f64, bottom: f64, source: crate::edges::EdgeSource) -> Edge {
    Edge {
        x0: x,
        top,
        x1: x,
        bottom,
        orientation: Orientation::Vertical,
        source,
    }
}

#[test]
fn test_lattice_simple_bordered_table() {
    // Simple 2x2 table from line edges forming a grid:
    // 3 horizontal lines at y=0, y=30, y=60 (from x=0 to x=100)
    // 3 vertical lines at x=0, x=50, x=100 (from y=0 to y=60)
    // Should produce 1 table with 4 cells (2 rows × 2 cols)
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 30.0, 100.0),
        make_h_edge(0.0, 60.0, 100.0),
        make_v_edge(0.0, 0.0, 60.0),
        make_v_edge(50.0, 0.0, 60.0),
        make_v_edge(100.0, 0.0, 60.0),
    ];
    let settings = TableSettings::default();
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 4);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0].len(), 2);
    assert_eq!(tables[0].rows[1].len(), 2);
    assert_approx(tables[0].bbox.x0, 0.0);
    assert_approx(tables[0].bbox.top, 0.0);
    assert_approx(tables[0].bbox.x1, 100.0);
    assert_approx(tables[0].bbox.bottom, 60.0);
}

#[test]
fn test_lattice_with_rect_edges() {
    // Lattice strategy includes rect-sourced edges.
    // Build edges from rect sources that form a 1-cell table.
    let edges = vec![
        make_h_edge_src(0.0, 0.0, 100.0, crate::edges::EdgeSource::RectTop),
        make_h_edge_src(0.0, 50.0, 100.0, crate::edges::EdgeSource::RectBottom),
        make_v_edge_src(0.0, 0.0, 50.0, crate::edges::EdgeSource::RectLeft),
        make_v_edge_src(100.0, 0.0, 50.0, crate::edges::EdgeSource::RectRight),
    ];
    let settings = TableSettings {
        strategy: Strategy::Lattice,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    // Lattice includes rect edges → should find 1 table with 1 cell
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_lattice_strict_excludes_rect_edges() {
    // LatticeStrict should exclude rect-sourced edges.
    // Only line-sourced edges should be used.
    let edges = vec![
        // These rect-sourced edges form a grid by themselves
        make_h_edge_src(0.0, 0.0, 100.0, crate::edges::EdgeSource::RectTop),
        make_h_edge_src(0.0, 50.0, 100.0, crate::edges::EdgeSource::RectBottom),
        make_v_edge_src(0.0, 0.0, 50.0, crate::edges::EdgeSource::RectLeft),
        make_v_edge_src(100.0, 0.0, 50.0, crate::edges::EdgeSource::RectRight),
    ];
    let settings = TableSettings {
        strategy: Strategy::LatticeStrict,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    // LatticeStrict excludes rect edges → no line edges → no tables
    assert!(tables.is_empty());
}

#[test]
fn test_lattice_strict_with_line_edges() {
    // LatticeStrict with line-sourced edges should detect tables.
    let edges = vec![
        make_h_edge_src(0.0, 0.0, 100.0, crate::edges::EdgeSource::Line),
        make_h_edge_src(0.0, 50.0, 100.0, crate::edges::EdgeSource::Line),
        make_v_edge_src(0.0, 0.0, 50.0, crate::edges::EdgeSource::Line),
        make_v_edge_src(100.0, 0.0, 50.0, crate::edges::EdgeSource::Line),
    ];
    let settings = TableSettings {
        strategy: Strategy::LatticeStrict,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_lattice_edge_min_length_filtering() {
    // Edges shorter than edge_min_length should be filtered out.
    // Short edges (length 2.0) should be removed with min_length=3.0
    let edges = vec![
        // These form a valid grid
        make_h_edge(0.0, 0.0, 100.0),  // length 100, kept
        make_h_edge(0.0, 50.0, 100.0), // length 100, kept
        make_v_edge(0.0, 0.0, 50.0),   // length 50, kept
        make_v_edge(100.0, 0.0, 50.0), // length 50, kept
        // Short edges that should be filtered
        make_h_edge(200.0, 0.0, 201.0), // length 1, filtered
        make_v_edge(200.0, 0.0, 2.0),   // length 2, filtered
    ];
    let settings = TableSettings {
        edge_min_length: 3.0,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    // Only the main grid edges remain → 1 table
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_lattice_edge_min_length_filters_all() {
    // If all edges are too short, no tables should be detected.
    let edges = vec![
        make_h_edge(0.0, 0.0, 2.0),   // length 2
        make_h_edge(0.0, 50.0, 1.5),  // length 1.5
        make_v_edge(0.0, 0.0, 2.5),   // length 2.5
        make_v_edge(100.0, 0.0, 1.0), // length 1
    ];
    let settings = TableSettings {
        edge_min_length: 3.0,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    assert!(tables.is_empty());
}

#[test]
fn test_lattice_full_pipeline_snap_and_join() {
    // Edges that are slightly misaligned and segmented.
    // After snap and join, they should form a valid grid.
    //
    // Two horizontal edges at y≈0 (slightly off) and y≈50:
    //   h1: y=0.5, x=0..60
    //   h2: y=-0.3, x=55..100  (same line as h1 after snap, overlapping after join)
    //   h3: y=50.0, x=0..100
    //
    // Two vertical edges at x≈0 and x≈100:
    //   v1: x=0.0, y=0..50
    //   v2: x=100.2, y=0..25
    //   v3: x=99.8, y=23..50  (same line as v2 after snap, overlapping after join)
    let edges = vec![
        make_h_edge(0.0, 0.5, 60.0),
        make_h_edge(55.0, -0.3, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        make_v_edge(0.0, 0.0, 50.0),
        make_v_edge(100.2, 0.0, 25.0),
        make_v_edge(99.8, 23.0, 50.0),
    ];
    let settings = TableSettings::default(); // snap/join tolerances = 3.0
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    // After snap+join, should form 1 table with 1 cell
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_lattice_empty_edges() {
    // No edges → no tables
    let finder = TableFinder::new(Vec::new(), TableSettings::default());
    let tables = finder.find_tables();
    assert!(tables.is_empty());
}

#[test]
fn test_lattice_no_intersections() {
    // Parallel edges that don't intersect → no tables
    let edges = vec![
        make_h_edge(0.0, 0.0, 100.0),
        make_h_edge(0.0, 50.0, 100.0),
        // No vertical edges → no intersections
    ];
    let finder = TableFinder::new(edges, TableSettings::default());
    let tables = finder.find_tables();
    assert!(tables.is_empty());
}

#[test]
fn test_lattice_strict_mixed_line_and_rect_edges() {
    // LatticeStrict should use line edges but not rect edges.
    // Mix of both: only line edges should be used.
    let edges = vec![
        // Line edges forming top/bottom
        make_h_edge_src(0.0, 0.0, 100.0, crate::edges::EdgeSource::Line),
        make_h_edge_src(0.0, 50.0, 100.0, crate::edges::EdgeSource::Line),
        // Line edges forming left/right
        make_v_edge_src(0.0, 0.0, 50.0, crate::edges::EdgeSource::Line),
        make_v_edge_src(100.0, 0.0, 50.0, crate::edges::EdgeSource::Line),
        // Rect edge adding a middle vertical line (should be ignored in strict mode)
        make_v_edge_src(50.0, 0.0, 50.0, crate::edges::EdgeSource::RectLeft),
    ];
    let settings = TableSettings {
        strategy: Strategy::LatticeStrict,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(edges, settings);
    let tables = finder.find_tables();

    // Only line edges used → 1 table with 1 cell (not 2 cells)
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

// --- extract_text_for_cells tests (US-036) ---

fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
    Char {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        fontname: "TestFont".to_string(),
        size: 12.0,
        doctop: top,
        upright: true,
        direction: crate::text::TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: None,
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: 0,
        mcid: None,
        tag: None,
    }
}

#[test]
fn test_extract_text_single_word_in_cell() {
    // Cell: (0,0)-(100,50), chars spelling "Hi" centered in cell
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
        text: None,
    }];
    let chars = vec![
        make_char("H", 10.0, 15.0, 20.0, 27.0),
        make_char("i", 20.0, 15.0, 28.0, 27.0),
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("Hi".to_string()));
}

#[test]
fn test_extract_text_empty_cell() {
    // Cell with no characters inside → text should remain None
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
        text: None,
    }];
    let chars: Vec<Char> = vec![];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, None);
}

#[test]
fn test_extract_text_chars_outside_cell() {
    // All characters are outside the cell bbox → text should be None
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
        text: None,
    }];
    // Chars at x=200, far outside cell
    let chars = vec![
        make_char("A", 200.0, 10.0, 210.0, 22.0),
        make_char("B", 210.0, 10.0, 220.0, 22.0),
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, None);
}

#[test]
fn test_extract_text_center_point_containment() {
    // Char bbox partially overlaps cell but center is outside → not included
    // Cell: (0,0)-(50,30)
    // Char bbox: (48,10)-(60,22) → center = (54, 16) → outside cell
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
        text: None,
    }];
    let chars = vec![make_char("X", 48.0, 10.0, 60.0, 22.0)];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, None);
}

#[test]
fn test_extract_text_center_inside_cell() {
    // Char bbox extends past cell border but center is inside → included
    // Cell: (0,0)-(50,30)
    // Char bbox: (40,10)-(52,22) → center = (46, 16) → inside cell
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
        text: None,
    }];
    let chars = vec![make_char("Y", 40.0, 10.0, 52.0, 22.0)];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("Y".to_string()));
}

#[test]
fn test_extract_text_multiple_words_in_cell() {
    // Cell with two words separated by a space char
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 200.0, 50.0),
        text: None,
    }];
    let chars = vec![
        make_char("H", 10.0, 15.0, 20.0, 27.0),
        make_char("i", 20.0, 15.0, 28.0, 27.0),
        make_char(" ", 28.0, 15.0, 33.0, 27.0),
        make_char("B", 33.0, 15.0, 43.0, 27.0),
        make_char("o", 43.0, 15.0, 51.0, 27.0),
        make_char("b", 51.0, 15.0, 59.0, 27.0),
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("Hi Bob".to_string()));
}

#[test]
fn test_extract_text_multiple_lines_in_cell() {
    // Cell with text on two lines (different y positions)
    let mut cells = vec![Cell {
        bbox: BBox::new(0.0, 0.0, 200.0, 80.0),
        text: None,
    }];
    let chars = vec![
        // Line 1: "AB" at y=10
        make_char("A", 10.0, 10.0, 20.0, 22.0),
        make_char("B", 20.0, 10.0, 30.0, 22.0),
        // Line 2: "CD" at y=40
        make_char("C", 10.0, 40.0, 20.0, 52.0),
        make_char("D", 20.0, 40.0, 30.0, 52.0),
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("AB\nCD".to_string()));
}

#[test]
fn test_extract_text_two_cells() {
    // Two cells, each with different text
    let mut cells = vec![
        Cell {
            bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
            text: None,
        },
        Cell {
            bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
            text: None,
        },
    ];
    let chars = vec![
        // "A" in cell 0 (center at (15, 16))
        make_char("A", 10.0, 10.0, 20.0, 22.0),
        // "B" in cell 1 (center at (65, 16))
        make_char("B", 60.0, 10.0, 70.0, 22.0),
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("A".to_string()));
    assert_eq!(cells[1].text, Some("B".to_string()));
}

#[test]
fn test_extract_text_no_cells() {
    // Empty cells slice → should not panic
    let mut cells: Vec<Cell> = vec![];
    let chars = vec![make_char("A", 10.0, 10.0, 20.0, 22.0)];
    extract_text_for_cells(&mut cells, &chars);
    assert!(cells.is_empty());
}

#[test]
fn test_extract_text_mixed_empty_and_populated_cells() {
    // Three cells: first has text, second is empty, third has text
    let mut cells = vec![
        Cell {
            bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
            text: None,
        },
        Cell {
            bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
            text: None,
        },
        Cell {
            bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
            text: None,
        },
    ];
    let chars = vec![
        make_char("X", 10.0, 10.0, 20.0, 22.0), // in cell 0
        make_char("Z", 110.0, 10.0, 120.0, 22.0), // in cell 2
                                                // No chars in cell 1
    ];
    extract_text_for_cells(&mut cells, &chars);
    assert_eq!(cells[0].text, Some("X".to_string()));
    assert_eq!(cells[1].text, None);
    assert_eq!(cells[2].text, Some("Z".to_string()));
}

// --- Stream strategy tests (US-037) ---

fn make_word(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Word {
    Word {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        doctop: top,
        direction: crate::text::TextDirection::Ltr,
        chars: vec![],
    }
}

#[test]
fn test_words_to_edges_stream_empty() {
    let edges = words_to_edges_stream(&[], 3.0, 3.0, 3, 1);
    assert!(edges.is_empty());
}

#[test]
fn test_words_to_edges_stream_vertical_x0_alignment() {
    // 3 words with x0 at ~10.0 (within tolerance 3.0)
    // Should produce a vertical edge at ~10.0
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 10.0, 30.0, 35.0, 42.0),
        make_word("C", 10.0, 50.0, 40.0, 62.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    // Should have at least one vertical edge from x0 alignment
    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert!(
        !v_edges.is_empty(),
        "Should produce vertical edges from x0 alignment"
    );

    // The vertical edge at x0≈10.0 should span from top=10.0 to bottom=62.0
    let v_edge = v_edges
        .iter()
        .find(|e| (e.x0 - 10.0).abs() < 1.0)
        .expect("Should have a vertical edge near x=10");
    assert!((v_edge.top - 10.0).abs() < 0.01);
    assert!((v_edge.bottom - 62.0).abs() < 0.01);
    assert_eq!(v_edge.source, EdgeSource::Stream);
}

#[test]
fn test_words_to_edges_stream_vertical_x1_alignment() {
    // 3 words with x1 at ~50.0
    let words = vec![
        make_word("A", 10.0, 10.0, 50.0, 22.0),
        make_word("B", 20.0, 30.0, 50.0, 42.0),
        make_word("C", 15.0, 50.0, 50.0, 62.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert!(
        !v_edges.is_empty(),
        "Should produce vertical edges from x1 alignment"
    );

    let v_edge = v_edges
        .iter()
        .find(|e| (e.x0 - 50.0).abs() < 1.0)
        .expect("Should have a vertical edge near x=50");
    assert!((v_edge.top - 10.0).abs() < 0.01);
    assert!((v_edge.bottom - 62.0).abs() < 0.01);
}

#[test]
fn test_words_to_edges_stream_horizontal_top_alignment() {
    // 3 words with top at ~10.0 (min_words_horizontal = 1, but 3 words align)
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 40.0, 10.0, 60.0, 22.0),
        make_word("C", 70.0, 10.0, 90.0, 22.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert!(
        !h_edges.is_empty(),
        "Should produce horizontal edges from top alignment"
    );

    // The horizontal edge at y≈10.0 should span from x0=10.0 to x1=90.0
    let h_edge = h_edges
        .iter()
        .find(|e| (e.top - 10.0).abs() < 1.0)
        .expect("Should have a horizontal edge near y=10");
    assert!((h_edge.x0 - 10.0).abs() < 0.01);
    assert!((h_edge.x1 - 90.0).abs() < 0.01);
}

#[test]
fn test_words_to_edges_stream_horizontal_bottom_alignment() {
    // 3 words with bottom at ~22.0
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 40.0, 12.0, 60.0, 22.0),
        make_word("C", 70.0, 8.0, 90.0, 22.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert!(
        !h_edges.is_empty(),
        "Should produce horizontal edges from bottom alignment"
    );

    let h_edge = h_edges
        .iter()
        .find(|e| (e.top - 22.0).abs() < 1.0)
        .expect("Should have a horizontal edge near y=22");
    assert!((h_edge.x0 - 10.0).abs() < 0.01);
    assert!((h_edge.x1 - 90.0).abs() < 0.01);
}

#[test]
fn test_words_to_edges_stream_threshold_filtering_vertical() {
    // Only 2 words with aligned x0, but min_words_vertical=3 → no vertical edge
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 10.0, 30.0, 35.0, 42.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert!(
        v_edges.is_empty(),
        "Should not produce vertical edges below threshold"
    );
}

#[test]
fn test_words_to_edges_stream_threshold_filtering_horizontal() {
    // Only 2 words with aligned top, but min_words_horizontal=3 → no horizontal edge
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 40.0, 10.0, 60.0, 22.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 3);

    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert!(
        h_edges.is_empty(),
        "Should not produce horizontal edges below threshold"
    );
}

#[test]
fn test_words_to_edges_stream_tolerance_grouping() {
    // Words with x0 slightly different but within tolerance
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 11.5, 30.0, 35.0, 42.0),
        make_word("C", 12.0, 50.0, 40.0, 62.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    // Should group x0 values 10.0, 11.5, 12.0 into one cluster (all within 3.0 tolerance)
    assert!(
        !v_edges.is_empty(),
        "Should group nearby x0 values within tolerance"
    );
}

#[test]
fn test_words_to_edges_stream_no_grouping_beyond_tolerance() {
    // Words with x0 values far apart → no cluster of 3
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 50.0, 30.0, 70.0, 42.0),
        make_word("C", 90.0, 50.0, 110.0, 62.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert!(
        v_edges.is_empty(),
        "Should not group x0 values that are far apart"
    );
}

#[test]
fn test_stream_strategy_full_pipeline() {
    // Simulate a 3-column borderless table with 3 rows:
    //   Row 0: "A"  "B"  "C"  (top=10, bottom=22)
    //   Row 1: "D"  "E"  "F"  (top=30, bottom=42)
    //   Row 2: "G"  "H"  "I"  (top=50, bottom=62)
    // Columns: x0=10, x0=50, x0=90 → left edges
    //          x1=30, x1=70, x1=110 → right edges
    let words = vec![
        // Row 0
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 50.0, 10.0, 70.0, 22.0),
        make_word("C", 90.0, 10.0, 110.0, 22.0),
        // Row 1
        make_word("D", 10.0, 30.0, 30.0, 42.0),
        make_word("E", 50.0, 30.0, 70.0, 42.0),
        make_word("F", 90.0, 30.0, 110.0, 42.0),
        // Row 2
        make_word("G", 10.0, 50.0, 30.0, 62.0),
        make_word("H", 50.0, 50.0, 70.0, 62.0),
        make_word("I", 90.0, 50.0, 110.0, 62.0),
    ];

    let settings = TableSettings {
        strategy: Strategy::Stream,
        min_words_vertical: 3,
        min_words_horizontal: 3,
        ..TableSettings::default()
    };

    let finder = TableFinder::new_with_words(vec![], words, settings);
    let tables = finder.find_tables();

    // Should detect at least one table
    assert!(!tables.is_empty(), "Stream strategy should detect a table");

    // The table should have cells
    assert!(
        !tables[0].cells.is_empty(),
        "Table should have detected cells"
    );
}

#[test]
fn test_stream_strategy_with_no_words() {
    // Empty words → no synthetic edges → no tables
    let settings = TableSettings {
        strategy: Strategy::Stream,
        ..TableSettings::default()
    };
    let finder = TableFinder::new_with_words(vec![], vec![], settings);
    let tables = finder.find_tables();
    assert!(tables.is_empty());
}

#[test]
fn test_stream_edge_source_is_stream() {
    // All synthetic edges from Stream should have EdgeSource::Stream
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 10.0, 30.0, 35.0, 42.0),
        make_word("C", 10.0, 50.0, 40.0, 62.0),
    ];
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);
    for edge in &edges {
        assert_eq!(
            edge.source,
            EdgeSource::Stream,
            "All stream edges should have EdgeSource::Stream"
        );
    }
}

#[test]
fn test_stream_strategy_min_words_horizontal_default() {
    // Default min_words_horizontal=1 means even a single row produces horizontal edges
    let words = vec![
        make_word("A", 10.0, 10.0, 30.0, 22.0),
        make_word("B", 50.0, 10.0, 70.0, 22.0),
        make_word("C", 90.0, 10.0, 110.0, 22.0),
    ];
    // min_words_horizontal=1 means each group of 1+ word at same y produces horizontal edges
    let edges = words_to_edges_stream(&words, 3.0, 3.0, 3, 1);

    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert!(
        !h_edges.is_empty(),
        "min_words_horizontal=1 should produce horizontal edges for 3 aligned words"
    );
}

// --- US-038: Explicit strategy tests ---

#[test]
fn test_explicit_lines_to_edges_basic() {
    // A 3x3 grid (3 horizontal + 3 vertical lines) should produce edges
    let explicit = ExplicitLines {
        horizontal_lines: vec![10.0, 30.0, 50.0],
        vertical_lines: vec![100.0, 200.0, 300.0],
    };
    let edges = explicit_lines_to_edges(&explicit);

    // 3 horizontal + 3 vertical = 6 edges
    assert_eq!(edges.len(), 6);

    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();
    assert_eq!(h_edges.len(), 3);
    assert_eq!(v_edges.len(), 3);

    // Horizontal edges span from min_x to max_x of vertical lines
    for h in &h_edges {
        assert_eq!(h.x0, 100.0);
        assert_eq!(h.x1, 300.0);
    }
    // Vertical edges span from min_y to max_y of horizontal lines
    for v in &v_edges {
        assert_eq!(v.top, 10.0);
        assert_eq!(v.bottom, 50.0);
    }
}

#[test]
fn test_explicit_lines_to_edges_empty_horizontal() {
    let explicit = ExplicitLines {
        horizontal_lines: vec![],
        vertical_lines: vec![100.0, 200.0],
    };
    let edges = explicit_lines_to_edges(&explicit);
    // No horizontal lines means no span for verticals either → no edges
    assert!(edges.is_empty());
}

#[test]
fn test_explicit_lines_to_edges_empty_vertical() {
    let explicit = ExplicitLines {
        horizontal_lines: vec![10.0, 20.0],
        vertical_lines: vec![],
    };
    let edges = explicit_lines_to_edges(&explicit);
    // No vertical lines means no span for horizontals either → no edges
    assert!(edges.is_empty());
}

#[test]
fn test_explicit_lines_to_edges_both_empty() {
    let explicit = ExplicitLines {
        horizontal_lines: vec![],
        vertical_lines: vec![],
    };
    let edges = explicit_lines_to_edges(&explicit);
    assert!(edges.is_empty());
}

#[test]
fn test_explicit_edge_source_is_explicit() {
    let explicit = ExplicitLines {
        horizontal_lines: vec![10.0, 50.0],
        vertical_lines: vec![100.0, 200.0],
    };
    let edges = explicit_lines_to_edges(&explicit);
    for edge in &edges {
        assert_eq!(edge.source, EdgeSource::Explicit);
    }
}

#[test]
fn test_explicit_grid_detection() {
    // A 3x3 grid should produce 4 cells
    let explicit = ExplicitLines {
        horizontal_lines: vec![0.0, 20.0, 40.0],
        vertical_lines: vec![0.0, 50.0, 100.0],
    };
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(explicit),
        ..TableSettings::default()
    };
    let finder = TableFinder::new(vec![], settings);
    let tables = finder.find_tables();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 4);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].columns.len(), 2);
}

#[test]
fn test_explicit_2x2_grid() {
    // A 2x2 grid (2 horizontal + 2 vertical) → 1 cell
    let explicit = ExplicitLines {
        horizontal_lines: vec![10.0, 50.0],
        vertical_lines: vec![100.0, 300.0],
    };
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(explicit),
        ..TableSettings::default()
    };
    let finder = TableFinder::new(vec![], settings);
    let tables = finder.find_tables();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
    let cell = &tables[0].cells[0];
    assert_eq!(cell.bbox.x0, 100.0);
    assert_eq!(cell.bbox.top, 10.0);
    assert_eq!(cell.bbox.x1, 300.0);
    assert_eq!(cell.bbox.bottom, 50.0);
}

#[test]
fn test_explicit_strategy_no_explicit_lines() {
    // Explicit strategy with no explicit_lines should return no tables
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: None,
        ..TableSettings::default()
    };
    let finder = TableFinder::new(vec![], settings);
    let tables = finder.find_tables();
    assert!(tables.is_empty());
}

#[test]
fn test_explicit_mixing_with_detected_edges() {
    // Detected edges form partial grid; explicit lines complete it
    // Detected: two vertical edges at x=0 and x=100
    let detected_edges = vec![make_v_edge(0.0, 0.0, 40.0), make_v_edge(100.0, 0.0, 40.0)];
    // Explicit: add horizontal lines at y=0 and y=40
    let explicit = ExplicitLines {
        horizontal_lines: vec![0.0, 40.0],
        vertical_lines: vec![], // no explicit verticals
    };
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(explicit),
        ..TableSettings::default()
    };
    let finder = TableFinder::new(detected_edges, settings);
    let tables = finder.find_tables();

    // The explicit horizontal lines + detected vertical edges form a complete grid → 1 cell
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_explicit_single_line_each() {
    // Only 1 horizontal + 1 vertical → no cells (need at least 2×2 grid)
    let explicit = ExplicitLines {
        horizontal_lines: vec![10.0],
        vertical_lines: vec![100.0],
    };
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(explicit),
        ..TableSettings::default()
    };
    let finder = TableFinder::new(vec![], settings);
    let tables = finder.find_tables();
    assert!(tables.is_empty());
}

#[test]
fn test_explicit_unsorted_coordinates() {
    // Coordinates provided in unsorted order should still work
    let explicit = ExplicitLines {
        horizontal_lines: vec![40.0, 0.0, 20.0],
        vertical_lines: vec![100.0, 0.0, 50.0],
    };
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(explicit),
        ..TableSettings::default()
    };
    let finder = TableFinder::new(vec![], settings);
    let tables = finder.find_tables();

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 4); // 3x3 grid → 4 cells
}

// --- US-069 tests: TableFinderDebug ---

#[test]
fn test_find_tables_debug_returns_intermediate_results() {
    // Build a simple 2x2 grid
    let edges = vec![
        // Horizontal edges
        Edge {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 0.0,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Line,
        },
        Edge {
            x0: 0.0,
            top: 50.0,
            x1: 100.0,
            bottom: 50.0,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Line,
        },
        Edge {
            x0: 0.0,
            top: 100.0,
            x1: 100.0,
            bottom: 100.0,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Line,
        },
        // Vertical edges
        Edge {
            x0: 0.0,
            top: 0.0,
            x1: 0.0,
            bottom: 100.0,
            orientation: Orientation::Vertical,
            source: EdgeSource::Line,
        },
        Edge {
            x0: 50.0,
            top: 0.0,
            x1: 50.0,
            bottom: 100.0,
            orientation: Orientation::Vertical,
            source: EdgeSource::Line,
        },
        Edge {
            x0: 100.0,
            top: 0.0,
            x1: 100.0,
            bottom: 100.0,
            orientation: Orientation::Vertical,
            source: EdgeSource::Line,
        },
    ];

    let finder = TableFinder::new(edges, TableSettings::default());
    let debug = finder.find_tables_debug();

    // Should have edges from the pipeline
    assert!(!debug.edges.is_empty(), "Should have processed edges");
    // Should have intersections (6 edges in a grid = 9 intersections)
    assert!(!debug.intersections.is_empty(), "Should have intersections");
    // Should have cells
    assert!(!debug.cells.is_empty(), "Should have cells");
    // Should have tables
    assert!(!debug.tables.is_empty(), "Should have tables");
    // The tables from debug should match find_tables()
    let tables = finder.find_tables();
    assert_eq!(debug.tables.len(), tables.len());
}

#[test]
fn test_find_tables_debug_no_edges() {
    let finder = TableFinder::new(vec![], TableSettings::default());
    let debug = finder.find_tables_debug();

    assert!(debug.edges.is_empty());
    assert!(debug.intersections.is_empty());
    assert!(debug.cells.is_empty());
    assert!(debug.tables.is_empty());
}

#[test]
fn test_find_tables_debug_struct_fields() {
    let edges = vec![
        Edge {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 0.0,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Line,
        },
        Edge {
            x0: 0.0,
            top: 0.0,
            x1: 0.0,
            bottom: 100.0,
            orientation: Orientation::Vertical,
            source: EdgeSource::Line,
        },
    ];

    let finder = TableFinder::new(edges, TableSettings::default());
    let debug = finder.find_tables_debug();

    // Should have edges (at least the 2 input edges after processing)
    assert!(!debug.edges.is_empty());
    // Should have at least one intersection (where the edges meet)
    assert!(!debug.intersections.is_empty());
    assert_eq!(debug.intersections[0].x, 0.0);
    assert_eq!(debug.intersections[0].y, 0.0);
}

// ---- TableQuality / accuracy / whitespace tests ----

#[test]
fn test_accuracy_all_cells_filled() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("B".into()),
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("C".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("D".into()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.accuracy() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_accuracy_half_empty() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: None,
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("C".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: None,
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.accuracy() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_accuracy_all_empty() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: None,
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: None,
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.accuracy()).abs() < f64::EPSILON);
}

#[test]
fn test_accuracy_whitespace_only_treated_as_empty() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("  ".into()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.accuracy() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_accuracy_no_cells() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.accuracy()).abs() < f64::EPSILON);
}

#[test]
fn test_whitespace_no_whitespace() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("ABC".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("DEF".into()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.whitespace()).abs() < f64::EPSILON);
}

#[test]
fn test_whitespace_all_spaces() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
        cells: vec![Cell {
            bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
            text: Some("   ".into()),
        }],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.whitespace() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_whitespace_mixed() {
    // "A B" = 1 whitespace / 3 chars = 0.333...
    // "CD"  = 0 whitespace / 2 chars = 0.0
    // average = (0.333... + 0.0) / 2 = 0.1666...
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A B".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("CD".into()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    let expected = (1.0 / 3.0 + 0.0) / 2.0;
    assert!((table.whitespace() - expected).abs() < 1e-10);
}

#[test]
fn test_whitespace_skips_empty_cells() {
    // Only cells with text contribute to the whitespace average
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("ABC".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: None,
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.whitespace()).abs() < f64::EPSILON);
}

#[test]
fn test_whitespace_no_text_cells() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: None,
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: None,
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    assert!((table.whitespace()).abs() < f64::EPSILON);
}

#[test]
fn test_quality_combined() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("Hello".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: None,
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("World".into()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("Test".into()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };
    let q = table.quality();
    assert!((q.accuracy - 0.75).abs() < f64::EPSILON);
    assert!((q.whitespace).abs() < f64::EPSILON);
}

#[test]
fn test_min_accuracy_filtering() {
    // Test that TableSettings::min_accuracy exists and defaults to None
    let settings = TableSettings::default();
    assert_eq!(settings.min_accuracy, None);

    // Test that min_accuracy can be set
    let settings = TableSettings {
        min_accuracy: Some(0.5),
        ..TableSettings::default()
    };
    assert_eq!(settings.min_accuracy, Some(0.5));
}

// --- duplicate_merged_content tests ---

#[test]
fn test_duplicate_merged_content_default_false() {
    let settings = TableSettings::default();
    assert!(!settings.duplicate_merged_content);
}

#[test]
fn test_horizontal_merge_duplicated() {
    // Table: 2 rows x 3 columns, row 0 has a cell spanning columns 0-1
    // +------ merged ------+-----+
    // |      "AB"          | "C" |
    // +----------+---------+-----+
    // |   "D"    |  "E"   | "F" |
    // +----------+---------+-----+
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 150.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
                text: Some("AB".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
                text: Some("C".to_string()),
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("D".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("E".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 30.0, 150.0, 60.0),
                text: Some("F".to_string()),
            },
        ],
        rows: vec![
            vec![
                Cell {
                    bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
                    text: Some("AB".to_string()),
                },
                Cell {
                    bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
                    text: Some("C".to_string()),
                },
            ],
            vec![
                Cell {
                    bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                    text: Some("D".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                    text: Some("E".to_string()),
                },
                Cell {
                    bbox: BBox::new(100.0, 30.0, 150.0, 60.0),
                    text: Some("F".to_string()),
                },
            ],
        ],
        columns: vec![],
    };

    let result = duplicate_merged_content_in_table(&table);

    // After duplication, row 0 should have 3 cells, with the merged cell's text in both
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0].len(), 3);
    assert_eq!(result.rows[0][0].text.as_deref(), Some("AB"));
    assert_eq!(result.rows[0][1].text.as_deref(), Some("AB")); // duplicated
    assert_eq!(result.rows[0][2].text.as_deref(), Some("C"));

    // Row 1 unchanged
    assert_eq!(result.rows[1].len(), 3);
    assert_eq!(result.rows[1][0].text.as_deref(), Some("D"));
    assert_eq!(result.rows[1][1].text.as_deref(), Some("E"));
    assert_eq!(result.rows[1][2].text.as_deref(), Some("F"));
}

#[test]
fn test_vertical_merge_duplicated() {
    // Table: 3 rows x 2 columns, column 0 rows 0-1 merged vertically
    // +-----+-----+
    // | "A" | "B" |
    // |     +-----+
    // |     | "D" |
    // +-----+-----+
    // | "E" | "F" |
    // +-----+-----+
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 90.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 60.0),
                text: Some("A".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("B".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("D".to_string()),
            },
            Cell {
                bbox: BBox::new(0.0, 60.0, 50.0, 90.0),
                text: Some("E".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 60.0, 100.0, 90.0),
                text: Some("F".to_string()),
            },
        ],
        rows: vec![
            vec![
                Cell {
                    bbox: BBox::new(0.0, 0.0, 50.0, 60.0),
                    text: Some("A".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                    text: Some("B".to_string()),
                },
            ],
            vec![Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("D".to_string()),
            }],
            vec![
                Cell {
                    bbox: BBox::new(0.0, 60.0, 50.0, 90.0),
                    text: Some("E".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 60.0, 100.0, 90.0),
                    text: Some("F".to_string()),
                },
            ],
        ],
        columns: vec![],
    };

    let result = duplicate_merged_content_in_table(&table);

    // After duplication, should have 3 rows x 2 columns
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.rows[0].len(), 2);
    assert_eq!(result.rows[0][0].text.as_deref(), Some("A"));
    assert_eq!(result.rows[0][1].text.as_deref(), Some("B"));

    assert_eq!(result.rows[1].len(), 2);
    assert_eq!(result.rows[1][0].text.as_deref(), Some("A")); // duplicated from vertical merge
    assert_eq!(result.rows[1][1].text.as_deref(), Some("D"));

    assert_eq!(result.rows[2].len(), 2);
    assert_eq!(result.rows[2][0].text.as_deref(), Some("E"));
    assert_eq!(result.rows[2][1].text.as_deref(), Some("F"));
}

#[test]
fn test_2x2_merge_duplicated() {
    // Table: 2 rows x 2 columns, top-left 2x2 block is merged
    // +---- merged ----+-----+
    // |                | "C" |
    // |    "AB"        +-----+
    // |                | "F" |
    // +-------+--------+-----+
    // | "G"   | "H"   | "I" |
    // +-------+--------+-----+
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 150.0, 90.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
                text: Some("AB".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
                text: Some("C".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 30.0, 150.0, 60.0),
                text: Some("F".to_string()),
            },
            Cell {
                bbox: BBox::new(0.0, 60.0, 50.0, 90.0),
                text: Some("G".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 60.0, 100.0, 90.0),
                text: Some("H".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 60.0, 150.0, 90.0),
                text: Some("I".to_string()),
            },
        ],
        rows: vec![],
        columns: vec![],
    };

    let result = duplicate_merged_content_in_table(&table);

    // Row 0: AB duplicated to 2 positions, plus C
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.rows[0].len(), 3);
    assert_eq!(result.rows[0][0].text.as_deref(), Some("AB"));
    assert_eq!(result.rows[0][1].text.as_deref(), Some("AB"));
    assert_eq!(result.rows[0][2].text.as_deref(), Some("C"));

    // Row 1: AB duplicated to 2 positions, plus F
    assert_eq!(result.rows[1].len(), 3);
    assert_eq!(result.rows[1][0].text.as_deref(), Some("AB"));
    assert_eq!(result.rows[1][1].text.as_deref(), Some("AB"));
    assert_eq!(result.rows[1][2].text.as_deref(), Some("F"));

    // Row 2: normal
    assert_eq!(result.rows[2].len(), 3);
    assert_eq!(result.rows[2][0].text.as_deref(), Some("G"));
    assert_eq!(result.rows[2][1].text.as_deref(), Some("H"));
    assert_eq!(result.rows[2][2].text.as_deref(), Some("I"));
}

#[test]
fn test_no_merge_table_unchanged() {
    // Regular 2x2 table with no merges
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 100.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                text: Some("A".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                text: Some("B".to_string()),
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("C".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("D".to_string()),
            },
        ],
        rows: vec![
            vec![
                Cell {
                    bbox: BBox::new(0.0, 0.0, 50.0, 30.0),
                    text: Some("A".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 0.0, 100.0, 30.0),
                    text: Some("B".to_string()),
                },
            ],
            vec![
                Cell {
                    bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                    text: Some("C".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                    text: Some("D".to_string()),
                },
            ],
        ],
        columns: vec![],
    };

    let result = duplicate_merged_content_in_table(&table);

    // Structure unchanged - 2 rows x 2 columns
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0].len(), 2);
    assert_eq!(result.rows[1].len(), 2);
    assert_eq!(result.rows[0][0].text.as_deref(), Some("A"));
    assert_eq!(result.rows[0][1].text.as_deref(), Some("B"));
    assert_eq!(result.rows[1][0].text.as_deref(), Some("C"));
    assert_eq!(result.rows[1][1].text.as_deref(), Some("D"));
}

#[test]
fn test_disabled_option_returns_none_for_merged() {
    // When duplicate_merged_content is false (default), merged cells
    // are left as-is — the wide cell has text, but no sub-cells are created
    let settings = TableSettings::default();
    assert!(!settings.duplicate_merged_content);

    // A table with a horizontal merge: row 0 has 2 cells, row 1 has 3 cells
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 150.0, 60.0),
        cells: vec![
            Cell {
                bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
                text: Some("AB".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
                text: Some("C".to_string()),
            },
            Cell {
                bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                text: Some("D".to_string()),
            },
            Cell {
                bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                text: Some("E".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 30.0, 150.0, 60.0),
                text: Some("F".to_string()),
            },
        ],
        rows: vec![
            vec![
                Cell {
                    bbox: BBox::new(0.0, 0.0, 100.0, 30.0),
                    text: Some("AB".to_string()),
                },
                Cell {
                    bbox: BBox::new(100.0, 0.0, 150.0, 30.0),
                    text: Some("C".to_string()),
                },
            ],
            vec![
                Cell {
                    bbox: BBox::new(0.0, 30.0, 50.0, 60.0),
                    text: Some("D".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 30.0, 100.0, 60.0),
                    text: Some("E".to_string()),
                },
                Cell {
                    bbox: BBox::new(100.0, 30.0, 150.0, 60.0),
                    text: Some("F".to_string()),
                },
            ],
        ],
        columns: vec![],
    };

    // Without duplicate_merged_content, the table stays as-is
    // Row 0 has 2 cells (the wide merged cell + C), Row 1 has 3 cells
    assert_eq!(table.rows[0].len(), 2);
    assert_eq!(table.rows[1].len(), 3);
}

#[test]
fn test_empty_table_duplicate() {
    let table = Table {
        bbox: BBox::new(0.0, 0.0, 0.0, 0.0),
        cells: vec![],
        rows: vec![],
        columns: vec![],
    };

    let result = duplicate_merged_content_in_table(&table);
    assert!(result.cells.is_empty());
    assert!(result.rows.is_empty());
}
