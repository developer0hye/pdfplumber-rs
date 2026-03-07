//! Table text extraction, edge stream analysis, and TableFinder orchestration.

use super::algorithms::{cells_to_tables, edges_to_cells, edges_to_intersections};
use super::{Cell, ExplicitLines, Intersection, Table, TableSettings};
use super::{join_edge_group, snap_edges};
use crate::edges::{Edge, EdgeSource};
use crate::geometry::Orientation;
use crate::table::Strategy;
use crate::text::{Char, TextDirection};
use crate::words::{Word, WordExtractor, WordOptions};

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
    extract_text_for_cells_with_options(cells, chars, &WordOptions::default());
}

/// Like [`extract_text_for_cells`] but with explicit [`WordOptions`] so the
/// caller can supply a rotation-adjusted text direction.
pub fn extract_text_for_cells_with_options(
    cells: &mut [Cell],
    chars: &[Char],
    options: &WordOptions,
) {
    let is_vertical = matches!(
        options.text_direction,
        TextDirection::Ttb | TextDirection::Btt
    );

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
        let words = WordExtractor::extract(&cell_chars, options);
        if words.is_empty() {
            cell.text = None;
            continue;
        }

        // Group words into lines:
        // - For horizontal text (LTR/RTL): group by y-coordinate (top)
        // - For vertical text (TTB/BTT): group by x-coordinate (x0)
        let mut sorted_words: Vec<&crate::words::Word> = words.iter().collect();
        if is_vertical {
            sorted_words.sort_by(|a, b| {
                a.bbox
                    .x0
                    .partial_cmp(&b.bbox.x0)
                    .unwrap()
                    .then_with(|| a.bbox.top.partial_cmp(&b.bbox.top).unwrap())
            });
        } else {
            sorted_words.sort_by(|a, b| {
                a.bbox
                    .top
                    .partial_cmp(&b.bbox.top)
                    .unwrap()
                    .then_with(|| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
            });
        }

        let tolerance = if is_vertical {
            options.x_tolerance
        } else {
            options.y_tolerance
        };

        let mut lines: Vec<Vec<&crate::words::Word>> = Vec::new();
        for word in &sorted_words {
            let added = lines.last_mut().and_then(|line| {
                let last_key = if is_vertical {
                    line[0].bbox.x0
                } else {
                    line[0].bbox.top
                };
                let word_key = if is_vertical {
                    word.bbox.x0
                } else {
                    word.bbox.top
                };
                if (word_key - last_key).abs() <= tolerance {
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

/// Intermediate results from the table detection pipeline.
///
/// Returned by [`TableFinder::find_tables_debug`] to expose every stage of the
/// pipeline for visual debugging (edges, intersections, cells, tables).
#[derive(Debug, Clone)]
pub struct TableFinderDebug {
    /// Processed edges after filtering, snapping, and joining.
    pub edges: Vec<Edge>,
    /// Intersection points between horizontal and vertical edges.
    pub intersections: Vec<Intersection>,
    /// Cells constructed from the intersection grid.
    pub cells: Vec<Cell>,
    /// Final tables grouped from adjacent cells.
    pub tables: Vec<Table>,
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

        // Step 6: Build cells from intersections using edge coverage
        let cells = edges_to_cells(
            &intersections,
            &edges,
            self.settings.intersection_x_tolerance,
            self.settings.intersection_y_tolerance,
        );

        // Step 7: Group cells into tables
        cells_to_tables(cells)
    }

    /// Run the table detection pipeline and return intermediate results for debugging.
    ///
    /// Returns a [`TableFinderDebug`] containing the processed edges, intersections,
    /// cells, and tables from each pipeline stage. This is used by the visual
    /// debugging system to render the table detection process.
    pub fn find_tables_debug(&self) -> TableFinderDebug {
        // Step 1: Select edges based on strategy (same as find_tables)
        let edges: Vec<Edge> = match self.settings.strategy {
            Strategy::LatticeStrict => self
                .edges
                .iter()
                .filter(|e| e.source == EdgeSource::Line)
                .cloned()
                .collect(),
            Strategy::Stream => words_to_edges_stream(
                &self.words,
                self.settings.text_x_tolerance,
                self.settings.text_y_tolerance,
                self.settings.min_words_vertical,
                self.settings.min_words_horizontal,
            ),
            Strategy::Explicit => {
                let mut edges = self.edges.clone();
                if let Some(ref explicit) = self.settings.explicit_lines {
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
            Strategy::Lattice => self.edges.clone(),
        };

        // Step 2: Filter by minimum length
        let min_len = self.settings.edge_min_length;
        let edges: Vec<Edge> = edges
            .into_iter()
            .filter(|e| edge_length(e) >= min_len)
            .collect();

        if edges.is_empty() {
            return TableFinderDebug {
                edges: Vec::new(),
                intersections: Vec::new(),
                cells: Vec::new(),
                tables: Vec::new(),
            };
        }

        // Step 3: Snap
        let edges = snap_edges(
            edges,
            self.settings.snap_x_tolerance,
            self.settings.snap_y_tolerance,
        );

        // Step 4: Join
        let edges = join_edge_group(
            edges,
            self.settings.join_x_tolerance,
            self.settings.join_y_tolerance,
        );

        // Step 5: Intersections
        let intersections = edges_to_intersections(
            &edges,
            self.settings.intersection_x_tolerance,
            self.settings.intersection_y_tolerance,
        );

        // Step 6: Cells (using edge coverage)
        let cells = edges_to_cells(
            &intersections,
            &edges,
            self.settings.intersection_x_tolerance,
            self.settings.intersection_y_tolerance,
        );

        // Step 7: Tables
        let tables = cells_to_tables(cells.clone());

        TableFinderDebug {
            edges,
            intersections,
            cells,
            tables,
        }
    }
}
