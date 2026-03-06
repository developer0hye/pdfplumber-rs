//! Table detection types and pipeline.
//!
//! This module provides the configuration types, data structures, and orchestration
//! for detecting tables in PDF pages using Lattice, Stream, or Explicit strategies.

use crate::edges::{Edge, EdgeSource};
use crate::geometry::{BBox, Orientation};
use crate::text::{Char, TextDirection};
use crate::words::{Word, WordExtractor, WordOptions};

/// Strategy for table detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Minimum accuracy threshold for auto-filtering low-quality tables (0.0 to 1.0).
    /// Tables with accuracy below this threshold are discarded. Default: None (no filtering).
    pub min_accuracy: Option<f64>,
    /// When true, cells spanning multiple grid positions have their text duplicated
    /// to all sub-cells. This normalizes merged/spanning cells so every row has the
    /// same number of columns. Default: false.
    pub duplicate_merged_content: bool,
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
            min_accuracy: None,
            duplicate_merged_content: false,
        }
    }
}

/// User-provided line coordinates for Explicit strategy.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExplicitLines {
    /// Y-coordinates for horizontal lines.
    pub horizontal_lines: Vec<f64>,
    /// X-coordinates for vertical lines.
    pub vertical_lines: Vec<f64>,
}

/// A detected table cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cell {
    /// Bounding box of the cell.
    pub bbox: BBox,
    /// Text content within the cell, if any.
    pub text: Option<String>,
}

/// Quality metrics for a detected table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableQuality {
    /// Percentage of cells with non-empty text (0.0 to 1.0).
    pub accuracy: f64,
    /// Average ratio of whitespace in cell text (0.0 to 1.0, lower is better).
    pub whitespace: f64,
}

/// A detected table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

impl Table {
    /// Percentage of cells with non-empty text (0.0 to 1.0).
    ///
    /// Returns 0.0 if the table has no cells.
    pub fn accuracy(&self) -> f64 {
        if self.cells.is_empty() {
            return 0.0;
        }
        let filled = self
            .cells
            .iter()
            .filter(|c| c.text.as_ref().is_some_and(|t| !t.trim().is_empty()))
            .count();
        filled as f64 / self.cells.len() as f64
    }

    /// Average ratio of whitespace characters in cell text (0.0 to 1.0, lower is better).
    ///
    /// Only considers cells that have text. Returns 0.0 if no cells have text.
    pub fn whitespace(&self) -> f64 {
        let ratios: Vec<f64> = self
            .cells
            .iter()
            .filter_map(|c| c.text.as_ref())
            .filter(|t| !t.is_empty())
            .map(|t| {
                let ws = t.chars().filter(|ch| ch.is_whitespace()).count();
                ws as f64 / t.len() as f64
            })
            .collect();
        if ratios.is_empty() {
            return 0.0;
        }
        ratios.iter().sum::<f64>() / ratios.len() as f64
    }

    /// Combined quality metrics for the table.
    pub fn quality(&self) -> TableQuality {
        TableQuality {
            accuracy: self.accuracy(),
            whitespace: self.whitespace(),
        }
    }
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

/// Merge overlapping or adjacent collinear edge segments.
///
/// Groups edges by orientation and collinear position, then merges segments
/// within each group when their gap is within the join tolerance.
/// For horizontal edges, segments on the same y-line merge when the gap along x
/// is within `join_x_tolerance`. For vertical edges, segments on the same x-line
/// merge when the gap along y is within `join_y_tolerance`.
/// Diagonal edges pass through unchanged.
pub fn join_edge_group(
    edges: Vec<Edge>,
    join_x_tolerance: f64,
    join_y_tolerance: f64,
) -> Vec<Edge> {
    let mut result: Vec<Edge> = Vec::new();
    let mut horizontals: Vec<Edge> = Vec::new();
    let mut verticals: Vec<Edge> = Vec::new();

    for edge in edges {
        match edge.orientation {
            Orientation::Horizontal => horizontals.push(edge),
            Orientation::Vertical => verticals.push(edge),
            Orientation::Diagonal => result.push(edge),
        }
    }

    // Join horizontal edges: group by y-coordinate, merge along x-axis
    result.extend(join_collinear(
        horizontals,
        |e| e.top,
        |e| (e.x0, e.x1),
        |proto, start, end| Edge {
            x0: start,
            top: proto.top,
            x1: end,
            bottom: proto.bottom,
            orientation: proto.orientation,
            source: proto.source,
        },
        join_x_tolerance,
    ));

    // Join vertical edges: group by x-coordinate, merge along y-axis
    result.extend(join_collinear(
        verticals,
        |e| e.x0,
        |e| (e.top, e.bottom),
        |proto, start, end| Edge {
            x0: proto.x0,
            top: start,
            x1: proto.x1,
            bottom: end,
            orientation: proto.orientation,
            source: proto.source,
        },
        join_y_tolerance,
    ));

    result
}

/// Group edges by a collinear key, then merge overlapping/adjacent segments within each group.
fn join_collinear<K, S, B>(
    mut edges: Vec<Edge>,
    key: K,
    span: S,
    build: B,
    tolerance: f64,
) -> Vec<Edge>
where
    K: Fn(&Edge) -> f64,
    S: Fn(&Edge) -> (f64, f64),
    B: Fn(&Edge, f64, f64) -> Edge,
{
    if edges.is_empty() {
        return Vec::new();
    }

    // Sort by collinear key first, then by span start
    edges.sort_by(|a, b| {
        key(a)
            .partial_cmp(&key(b))
            .unwrap()
            .then_with(|| span(a).0.partial_cmp(&span(b).0).unwrap())
    });

    let mut result = Vec::new();
    let mut i = 0;

    while i < edges.len() {
        // Collect group of edges on the same collinear line (exact match after snapping)
        let group_key = key(&edges[i]);
        let mut j = i + 1;
        while j < edges.len() && (key(&edges[j]) - group_key).abs() < 1e-9 {
            j += 1;
        }

        // Merge segments within this collinear group
        let (mut cur_start, mut cur_end) = span(&edges[i]);
        let mut proto_idx = i;

        for k in (i + 1)..j {
            let (s, e) = span(&edges[k]);
            if s <= cur_end + tolerance {
                // Overlapping or within tolerance — extend
                if e > cur_end {
                    cur_end = e;
                }
            } else {
                // Gap too large — emit current merged edge, start new one
                result.push(build(&edges[proto_idx], cur_start, cur_end));
                cur_start = s;
                cur_end = e;
                proto_idx = k;
            }
        }
        result.push(build(&edges[proto_idx], cur_start, cur_end));

        i = j;
    }

    result
}

/// An intersection point between horizontal and vertical edges.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Intersection {
    /// X coordinate of the intersection point.
    pub x: f64,
    /// Y coordinate of the intersection point.
    pub y: f64,
}


/// Convert a float to an integer key for grouping (multiply by 1000 to preserve 3 decimal places).
pub(super) fn float_key(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

mod algorithms;
pub use algorithms::{
    cells_to_tables, duplicate_merged_content_in_table, edges_to_cells, edges_to_intersections,
    intersections_to_cells, normalize_table_columns,
};

mod extraction;
pub use extraction::{
    TableFinder, TableFinderDebug, explicit_lines_to_edges, extract_text_for_cells,
    extract_text_for_cells_with_options, words_to_edges_stream,
};

#[cfg(test)]
mod tests;
