//! Table detection types and pipeline.
//!
//! This module provides the configuration types, data structures, and orchestration
//! for detecting tables in PDF pages using Lattice, Stream, or Explicit strategies.

use crate::edges::{Edge, EdgeSource};
use crate::geometry::{BBox, Orientation};
use crate::text::Char;
use crate::words::{Word, WordExtractor, WordOptions};

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
pub struct Intersection {
    /// X coordinate of the intersection point.
    pub x: f64,
    /// Y coordinate of the intersection point.
    pub y: f64,
}

/// Find all intersection points between horizontal and vertical edges.
///
/// An intersection exists when a vertical edge's x-coordinate falls within a
/// horizontal edge's x-span (within `x_tolerance`) AND the horizontal edge's
/// y-coordinate falls within the vertical edge's y-span (within `y_tolerance`).
///
/// Only considers actual overlapping segments, not infinite line extensions.
/// Diagonal edges are ignored.
pub fn edges_to_intersections(
    edges: &[Edge],
    x_tolerance: f64,
    y_tolerance: f64,
) -> Vec<Intersection> {
    let horizontals: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let verticals: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();

    let mut intersections = Vec::new();

    for h in &horizontals {
        let h_y = h.top; // horizontal edge: top == bottom
        for v in &verticals {
            let v_x = v.x0; // vertical edge: x0 == x1

            // Check that the vertical's x is within the horizontal's x-span (with tolerance)
            // and the horizontal's y is within the vertical's y-span (with tolerance)
            if v_x >= h.x0 - x_tolerance
                && v_x <= h.x1 + x_tolerance
                && h_y >= v.top - y_tolerance
                && h_y <= v.bottom + y_tolerance
            {
                intersections.push(Intersection { x: v_x, y: h_y });
            }
        }
    }

    // Sort and deduplicate intersection points at the same location
    intersections.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap()
            .then_with(|| a.y.partial_cmp(&b.y).unwrap())
    });
    intersections.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);

    intersections
}

/// Construct rectangular cells from a grid of intersection points.
///
/// Groups intersection points into a grid of unique y-rows and x-columns (sorted).
/// For each pair of adjacent rows and adjacent columns, checks if all 4 corner
/// intersections exist. If so, creates a [`Cell`] with the corresponding bounding box.
/// Missing corners are skipped gracefully.
pub fn intersections_to_cells(intersections: &[Intersection]) -> Vec<Cell> {
    if intersections.is_empty() {
        return Vec::new();
    }

    // Collect unique x and y coordinates (sorted)
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();

    for pt in intersections {
        if !xs.iter().any(|&x| (x - pt.x).abs() < 1e-9) {
            xs.push(pt.x);
        }
        if !ys.iter().any(|&y| (y - pt.y).abs() < 1e-9) {
            ys.push(pt.y);
        }
    }

    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Helper to check if an intersection exists at (x, y)
    let has_point = |x: f64, y: f64| -> bool {
        intersections
            .iter()
            .any(|pt| (pt.x - x).abs() < 1e-9 && (pt.y - y).abs() < 1e-9)
    };

    let mut cells = Vec::new();

    // For each pair of adjacent rows and columns, check all 4 corners
    for yi in 0..ys.len().saturating_sub(1) {
        for xi in 0..xs.len().saturating_sub(1) {
            let x0 = xs[xi];
            let x1 = xs[xi + 1];
            let top = ys[yi];
            let bottom = ys[yi + 1];

            if has_point(x0, top)
                && has_point(x1, top)
                && has_point(x0, bottom)
                && has_point(x1, bottom)
            {
                cells.push(Cell {
                    bbox: BBox::new(x0, top, x1, bottom),
                    text: None,
                });
            }
        }
    }

    cells
}

/// Group adjacent cells into distinct tables.
///
/// Cells that share an edge (same x-boundary or y-boundary) are grouped into
/// the same table using a union-find algorithm. Each table receives:
/// - A `bbox` that is the union of all its cells' bounding boxes
/// - `rows`: cells organized by y-coordinate (top-to-bottom), sorted left-to-right within each row
/// - `columns`: cells organized by x-coordinate (left-to-right), sorted top-to-bottom within each column
pub fn cells_to_tables(cells: Vec<Cell>) -> Vec<Table> {
    if cells.is_empty() {
        return Vec::new();
    }

    let n = cells.len();

    // Union-Find to group cells sharing edges
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]]; // path compression
            i = parent[i];
        }
        i
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb] = ra;
        }
    }

    // Two cells share an edge if they have a common boundary segment:
    // - Same x0/x1 boundary AND overlapping y-ranges, or
    // - Same top/bottom boundary AND overlapping x-ranges
    for i in 0..n {
        for j in (i + 1)..n {
            if cells_share_edge(&cells[i], &cells[j]) {
                union(&mut parent, i, j);
            }
        }
    }

    // Group cells by their root
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    // Build a Table from each group
    let mut tables: Vec<Table> = groups
        .into_values()
        .map(|indices| {
            let group_cells: Vec<Cell> = indices.iter().map(|&i| cells[i].clone()).collect();

            // Compute union bbox
            let mut bbox = group_cells[0].bbox;
            for cell in &group_cells[1..] {
                bbox = bbox.union(&cell.bbox);
            }

            // Organize into rows: group by top coordinate, sort left-to-right
            let mut row_map: std::collections::BTreeMap<i64, Vec<Cell>> =
                std::collections::BTreeMap::new();
            for cell in &group_cells {
                let key = float_key(cell.bbox.top);
                row_map.entry(key).or_default().push(cell.clone());
            }
            let rows: Vec<Vec<Cell>> = row_map
                .into_values()
                .map(|mut row| {
                    row.sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap());
                    row
                })
                .collect();

            // Organize into columns: group by x0 coordinate, sort top-to-bottom
            let mut col_map: std::collections::BTreeMap<i64, Vec<Cell>> =
                std::collections::BTreeMap::new();
            for cell in &group_cells {
                let key = float_key(cell.bbox.x0);
                col_map.entry(key).or_default().push(cell.clone());
            }
            let columns: Vec<Vec<Cell>> = col_map
                .into_values()
                .map(|mut col| {
                    col.sort_by(|a, b| a.bbox.top.partial_cmp(&b.bbox.top).unwrap());
                    col
                })
                .collect();

            Table {
                bbox,
                cells: group_cells,
                rows,
                columns,
            }
        })
        .collect();

    // Sort tables by position for deterministic output (top-to-bottom, left-to-right)
    tables.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then_with(|| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });

    tables
}

/// Check if two cells share an edge (a common boundary segment).
fn cells_share_edge(a: &Cell, b: &Cell) -> bool {
    let eps = 1e-6;

    // Check for shared vertical boundary (one cell's x1 == other's x0 or vice versa)
    // with overlapping y-ranges
    let shared_vertical = ((a.bbox.x1 - b.bbox.x0).abs() < eps
        || (a.bbox.x0 - b.bbox.x1).abs() < eps)
        && a.bbox.top < b.bbox.bottom + eps
        && b.bbox.top < a.bbox.bottom + eps;

    // Check for shared horizontal boundary (one cell's bottom == other's top or vice versa)
    // with overlapping x-ranges
    let shared_horizontal = ((a.bbox.bottom - b.bbox.top).abs() < eps
        || (a.bbox.top - b.bbox.bottom).abs() < eps)
        && a.bbox.x0 < b.bbox.x1 + eps
        && b.bbox.x0 < a.bbox.x1 + eps;

    shared_vertical || shared_horizontal
}

/// Convert a float to an integer key for grouping (multiply by 1000 to preserve 3 decimal places).
fn float_key(v: f64) -> i64 {
    (v * 1000.0).round() as i64
}

/// Compute the length of an edge along its primary axis.
fn edge_length(edge: &Edge) -> f64 {
    let dx = edge.x1 - edge.x0;
    let dy = edge.bottom - edge.top;
    (dx * dx + dy * dy).sqrt()
}

/// Extract text content for each cell by finding characters within the cell bbox.
///
/// For each cell, finds all [`Char`]s whose bbox center point falls within the
/// cell's bounding box. Characters are grouped into words using [`WordExtractor`],
/// then joined into text with spaces between words on the same line and newlines
/// between lines.
///
/// Cells with no matching characters get `text = None`.
pub fn extract_text_for_cells(cells: &mut [Cell], chars: &[Char]) {
    let options = WordOptions::default();

    for cell in cells.iter_mut() {
        // Find chars whose bbox center falls within this cell
        let cell_chars: Vec<Char> = chars
            .iter()
            .filter(|ch| {
                let cx = (ch.bbox.x0 + ch.bbox.x1) / 2.0;
                let cy = (ch.bbox.top + ch.bbox.bottom) / 2.0;
                cx >= cell.bbox.x0
                    && cx <= cell.bbox.x1
                    && cy >= cell.bbox.top
                    && cy <= cell.bbox.bottom
            })
            .cloned()
            .collect();

        if cell_chars.is_empty() {
            cell.text = None;
            continue;
        }

        // Group chars into words
        let words = WordExtractor::extract(&cell_chars, &options);
        if words.is_empty() {
            cell.text = None;
            continue;
        }

        // Group words into lines by y-coordinate proximity
        let mut sorted_words: Vec<&crate::words::Word> = words.iter().collect();
        sorted_words.sort_by(|a, b| {
            a.bbox
                .top
                .partial_cmp(&b.bbox.top)
                .unwrap()
                .then_with(|| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
        });

        let mut lines: Vec<Vec<&crate::words::Word>> = Vec::new();
        for word in &sorted_words {
            let added = lines.last_mut().and_then(|line| {
                let last_top = line[0].bbox.top;
                if (word.bbox.top - last_top).abs() <= options.y_tolerance {
                    line.push(word);
                    Some(())
                } else {
                    None
                }
            });
            if added.is_none() {
                lines.push(vec![word]);
            }
        }

        // Join: words within a line separated by spaces, lines separated by newlines
        let text: String = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");

        cell.text = Some(text);
    }
}

/// Generate synthetic edges from text alignment patterns for the Stream strategy.
///
/// Analyzes word positions to detect vertical and horizontal text alignments:
/// - Words sharing similar x0 or x1 coordinates → synthetic vertical edges
/// - Words sharing similar top or bottom coordinates → synthetic horizontal edges
///
/// Groups must meet the minimum word count thresholds (`min_words_vertical` for
/// vertical edges, `min_words_horizontal` for horizontal edges) to produce an edge.
///
/// Each synthetic edge spans the full extent of the aligned words in the
/// perpendicular direction.
pub fn words_to_edges_stream(
    words: &[Word],
    text_x_tolerance: f64,
    text_y_tolerance: f64,
    min_words_vertical: usize,
    min_words_horizontal: usize,
) -> Vec<Edge> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut edges = Vec::new();

    // Vertical edges from x0 alignment (left edges of words)
    edges.extend(cluster_words_to_edges(
        words,
        |w| w.bbox.x0,
        text_x_tolerance,
        min_words_vertical,
        EdgeKind::Vertical,
    ));

    // Vertical edges from x1 alignment (right edges of words)
    edges.extend(cluster_words_to_edges(
        words,
        |w| w.bbox.x1,
        text_x_tolerance,
        min_words_vertical,
        EdgeKind::Vertical,
    ));

    // Horizontal edges from top alignment
    edges.extend(cluster_words_to_edges(
        words,
        |w| w.bbox.top,
        text_y_tolerance,
        min_words_horizontal,
        EdgeKind::Horizontal,
    ));

    // Horizontal edges from bottom alignment
    edges.extend(cluster_words_to_edges(
        words,
        |w| w.bbox.bottom,
        text_y_tolerance,
        min_words_horizontal,
        EdgeKind::Horizontal,
    ));

    edges
}

/// Internal enum to specify what kind of edge to produce from word clusters.
enum EdgeKind {
    Vertical,
    Horizontal,
}

/// Cluster words by a coordinate accessor, then produce synthetic edges for qualifying clusters.
fn cluster_words_to_edges<F>(
    words: &[Word],
    key: F,
    tolerance: f64,
    min_words: usize,
    kind: EdgeKind,
) -> Vec<Edge>
where
    F: Fn(&Word) -> f64,
{
    if words.is_empty() || min_words == 0 {
        return Vec::new();
    }

    // Sort word indices by the key coordinate
    let mut indices: Vec<usize> = (0..words.len()).collect();
    indices.sort_by(|&a, &b| key(&words[a]).partial_cmp(&key(&words[b])).unwrap());

    let mut edges = Vec::new();
    let mut cluster_start = 0;

    for i in 1..=indices.len() {
        let end_of_cluster = i == indices.len()
            || (key(&words[indices[i]]) - key(&words[indices[cluster_start]])).abs() > tolerance;

        if end_of_cluster {
            let cluster_size = i - cluster_start;
            if cluster_size >= min_words {
                // Compute the mean position for the cluster
                let sum: f64 = (cluster_start..i).map(|j| key(&words[indices[j]])).sum();
                let mean_pos = sum / cluster_size as f64;

                // Compute the span in the perpendicular direction
                let cluster_words: Vec<&Word> =
                    (cluster_start..i).map(|j| &words[indices[j]]).collect();

                match kind {
                    EdgeKind::Vertical => {
                        let min_top = cluster_words
                            .iter()
                            .map(|w| w.bbox.top)
                            .fold(f64::INFINITY, f64::min);
                        let max_bottom = cluster_words
                            .iter()
                            .map(|w| w.bbox.bottom)
                            .fold(f64::NEG_INFINITY, f64::max);
                        edges.push(Edge {
                            x0: mean_pos,
                            top: min_top,
                            x1: mean_pos,
                            bottom: max_bottom,
                            orientation: Orientation::Vertical,
                            source: EdgeSource::Stream,
                        });
                    }
                    EdgeKind::Horizontal => {
                        let min_x0 = cluster_words
                            .iter()
                            .map(|w| w.bbox.x0)
                            .fold(f64::INFINITY, f64::min);
                        let max_x1 = cluster_words
                            .iter()
                            .map(|w| w.bbox.x1)
                            .fold(f64::NEG_INFINITY, f64::max);
                        edges.push(Edge {
                            x0: min_x0,
                            top: mean_pos,
                            x1: max_x1,
                            bottom: mean_pos,
                            orientation: Orientation::Horizontal,
                            source: EdgeSource::Stream,
                        });
                    }
                }
            }
            cluster_start = i;
        }
    }

    edges
}

/// Convert user-provided explicit line coordinates into edges.
///
/// Horizontal lines (y-coordinates) become horizontal edges spanning the full
/// x-range of the vertical lines. Vertical lines (x-coordinates) become
/// vertical edges spanning the full y-range of the horizontal lines.
///
/// Returns an empty Vec if either list is empty (a grid requires both).
pub fn explicit_lines_to_edges(explicit: &ExplicitLines) -> Vec<Edge> {
    if explicit.horizontal_lines.is_empty() || explicit.vertical_lines.is_empty() {
        return Vec::new();
    }

    let min_x = explicit
        .vertical_lines
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_x = explicit
        .vertical_lines
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = explicit
        .horizontal_lines
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let max_y = explicit
        .horizontal_lines
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    let mut edges = Vec::new();

    // Horizontal edges: each y-coordinate spans from min_x to max_x
    for &y in &explicit.horizontal_lines {
        edges.push(Edge {
            x0: min_x,
            top: y,
            x1: max_x,
            bottom: y,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Explicit,
        });
    }

    // Vertical edges: each x-coordinate spans from min_y to max_y
    for &x in &explicit.vertical_lines {
        edges.push(Edge {
            x0: x,
            top: min_y,
            x1: x,
            bottom: max_y,
            orientation: Orientation::Vertical,
            source: EdgeSource::Explicit,
        });
    }

    edges
}

/// Orchestrator for the table detection pipeline.
///
/// Takes edges (and optionally words/chars) and settings, then runs
/// the appropriate detection strategy to produce tables.
pub struct TableFinder {
    /// Edges available for table detection.
    edges: Vec<Edge>,
    /// Words for Stream strategy text alignment detection.
    words: Vec<Word>,
    /// Configuration settings.
    settings: TableSettings,
}

impl TableFinder {
    /// Create a new TableFinder with the given edges and settings.
    pub fn new(edges: Vec<Edge>, settings: TableSettings) -> Self {
        Self {
            edges,
            words: Vec::new(),
            settings,
        }
    }

    /// Create a new TableFinder with edges, words, and settings.
    ///
    /// The words are used by the Stream strategy to generate synthetic edges
    /// from text alignment patterns.
    pub fn new_with_words(edges: Vec<Edge>, words: Vec<Word>, settings: TableSettings) -> Self {
        Self {
            edges,
            words,
            settings,
        }
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
    /// Pipeline: filter edges → snap → join → intersections → cells → tables.
    ///
    /// For **Lattice** strategy, all edges (lines + rect edges) are used.
    /// For **LatticeStrict** strategy, only line-sourced edges are used (no rect edges).
    /// For **Stream** strategy, synthetic edges are generated from word alignment patterns.
    /// For **Explicit** strategy, edges from user-provided coordinates are used,
    /// combined with any detected edges passed to the finder (mixing).
    pub fn find_tables(&self) -> Vec<Table> {
        // Step 1: Select edges based on strategy
        let edges: Vec<Edge> = match self.settings.strategy {
            Strategy::LatticeStrict => self
                .edges
                .iter()
                .filter(|e| e.source == EdgeSource::Line)
                .cloned()
                .collect(),
            Strategy::Stream => {
                // Generate synthetic edges from word alignment patterns
                words_to_edges_stream(
                    &self.words,
                    self.settings.text_x_tolerance,
                    self.settings.text_y_tolerance,
                    self.settings.min_words_vertical,
                    self.settings.min_words_horizontal,
                )
            }
            Strategy::Explicit => {
                // Start with detected edges (for mixing)
                let mut edges = self.edges.clone();

                if let Some(ref explicit) = self.settings.explicit_lines {
                    // Compute the overall bounding range from detected edges + explicit coords
                    let mut min_x = f64::INFINITY;
                    let mut max_x = f64::NEG_INFINITY;
                    let mut min_y = f64::INFINITY;
                    let mut max_y = f64::NEG_INFINITY;

                    for e in &edges {
                        min_x = min_x.min(e.x0);
                        max_x = max_x.max(e.x1);
                        min_y = min_y.min(e.top);
                        max_y = max_y.max(e.bottom);
                    }
                    for &x in &explicit.vertical_lines {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                    for &y in &explicit.horizontal_lines {
                        min_y = min_y.min(y);
                        max_y = max_y.max(y);
                    }

                    if min_x <= max_x && min_y <= max_y {
                        for &y in &explicit.horizontal_lines {
                            edges.push(Edge {
                                x0: min_x,
                                top: y,
                                x1: max_x,
                                bottom: y,
                                orientation: Orientation::Horizontal,
                                source: EdgeSource::Explicit,
                            });
                        }
                        for &x in &explicit.vertical_lines {
                            edges.push(Edge {
                                x0: x,
                                top: min_y,
                                x1: x,
                                bottom: max_y,
                                orientation: Orientation::Vertical,
                                source: EdgeSource::Explicit,
                            });
                        }
                    }
                }

                edges
            }
            // Lattice (default): use all edges
            Strategy::Lattice => self.edges.clone(),
        };

        // Step 2: Filter edges by minimum length
        let min_len = self.settings.edge_min_length;
        let edges: Vec<Edge> = edges
            .into_iter()
            .filter(|e| edge_length(e) >= min_len)
            .collect();

        if edges.is_empty() {
            return Vec::new();
        }

        // Step 3: Snap nearby parallel edges
        let edges = snap_edges(
            edges,
            self.settings.snap_x_tolerance,
            self.settings.snap_y_tolerance,
        );

        // Step 4: Join collinear edge segments
        let edges = join_edge_group(
            edges,
            self.settings.join_x_tolerance,
            self.settings.join_y_tolerance,
        );

        // Step 5: Find intersections
        let intersections = edges_to_intersections(
            &edges,
            self.settings.intersection_x_tolerance,
            self.settings.intersection_y_tolerance,
        );

        // Step 6: Build cells from intersections
        let cells = intersections_to_cells(&intersections);

        // Step 7: Group cells into tables
        cells_to_tables(cells)
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
}
