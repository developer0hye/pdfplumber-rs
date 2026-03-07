//! Table detection algorithms — edge snapping, intersection finding, cell extraction.

use super::{Cell, Intersection, Table, float_key};
use crate::edges::Edge;
use crate::geometry::{BBox, Orientation};

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

/// Construct rectangular cells using edge coverage with grid completion.
///
/// Uses a two-phase approach:
///
/// **Phase 1 (strict edge coverage):** For each candidate cell (consecutive x-pair and
/// y-pair from intersection grid), check all 4 edges: horizontal edges span \[x0, x1\]
/// at top and bottom y, AND vertical edges span \[top, bottom\] at left and right x.
///
/// **Phase 2 (merged cell completion):** For rows not fully covered by Phase 1, identify
/// x-positions that have vertical edge coverage at the current y-range. Between consecutive
/// such x-positions, create one merged cell if horizontal edges span the range at both
/// top and bottom y. This produces wider cells for merged header/footer rows (matching
/// Python pdfplumber behavior). Use [`normalize_table_columns`] after text extraction
/// to split merged cells into uniform grid columns.
pub fn edges_to_cells(
    intersections: &[Intersection],
    edges: &[Edge],
    x_tolerance: f64,
    y_tolerance: f64,
) -> Vec<Cell> {
    if intersections.is_empty() || edges.is_empty() {
        return Vec::new();
    }

    // Collect unique x and y coordinates (sorted) from intersections
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

    // Separate edges by orientation
    let horizontals: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let verticals: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();

    // Check if a horizontal edge covers the x-range [x0, x1] at y-position
    let has_h_coverage = |x0: f64, x1: f64, y: f64| -> bool {
        horizontals.iter().any(|e| {
            (e.top - y).abs() <= y_tolerance && e.x0 <= x0 + x_tolerance && e.x1 >= x1 - x_tolerance
        })
    };

    // Check if a vertical edge covers the y-range [top, bottom] at x-position
    let has_v_coverage = |x: f64, top: f64, bottom: f64| -> bool {
        verticals.iter().any(|e| {
            (e.x0 - x).abs() <= x_tolerance
                && e.top <= top + y_tolerance
                && e.bottom >= bottom - y_tolerance
        })
    };

    // Phase 1: strict edge coverage (all 4 edges required)
    let mut cells = Vec::new();
    // Track which column boundaries (x-positions) are established by phase-1 cells
    let mut established_xs = std::collections::HashSet::new();

    for yi in 0..ys.len().saturating_sub(1) {
        for xi in 0..xs.len().saturating_sub(1) {
            let x0 = xs[xi];
            let x1 = xs[xi + 1];
            let top = ys[yi];
            let bottom = ys[yi + 1];

            if has_h_coverage(x0, x1, top)
                && has_h_coverage(x0, x1, bottom)
                && has_v_coverage(x0, top, bottom)
                && has_v_coverage(x1, top, bottom)
            {
                cells.push(Cell {
                    bbox: BBox::new(x0, top, x1, bottom),
                    text: None,
                });
                // Record that x0 and x1 are established column boundaries
                // Use integer key (scaled by 1000) to avoid float hash issues
                established_xs.insert((x0 * 1000.0).round() as i64);
                established_xs.insert((x1 * 1000.0).round() as i64);
            }
        }
    }

    // Phase 2: grid completion with merged cells — for rows with missing vertical edges,
    // create merged cells spanning between consecutive x-positions that have vertical
    // edge coverage at the current y-range. This produces wider cells for merged header/
    // footer rows (matching Python pdfplumber behavior) instead of narrow cells that
    // fragment text.
    let is_established_x =
        |x: f64| -> bool { established_xs.contains(&((x * 1000.0).round() as i64)) };

    for yi in 0..ys.len().saturating_sub(1) {
        let top = ys[yi];
        let bottom = ys[yi + 1];

        // Check if this row is already fully covered by Phase 1
        let phase1_count = cells
            .iter()
            .filter(|c| (c.bbox.top - top).abs() < 1e-9)
            .count();
        let max_cells = xs.len().saturating_sub(1);
        if phase1_count >= max_cells {
            continue;
        }

        // Find x-positions with vertical edge coverage at this y-range
        let v_xs: Vec<f64> = xs
            .iter()
            .filter(|&&x| is_established_x(x) && has_v_coverage(x, top, bottom))
            .copied()
            .collect();

        // Create merged cells between consecutive V-boundary positions
        for vi in 0..v_xs.len().saturating_sub(1) {
            let cell_x0 = v_xs[vi];
            let cell_x1 = v_xs[vi + 1];

            // Skip if Phase 1 already created a matching cell
            let already_exists = cells.iter().any(|c| {
                (c.bbox.x0 - cell_x0).abs() < 1e-9
                    && (c.bbox.top - top).abs() < 1e-9
                    && (c.bbox.x1 - cell_x1).abs() < 1e-9
                    && (c.bbox.bottom - bottom).abs() < 1e-9
            });
            if already_exists {
                continue;
            }

            // Check H edge coverage at top and bottom
            if has_h_coverage(cell_x0, cell_x1, top) && has_h_coverage(cell_x0, cell_x1, bottom) {
                cells.push(Cell {
                    bbox: BBox::new(cell_x0, top, cell_x1, bottom),
                    text: None,
                });
            }
        }
    }

    cells
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

/// Normalize a table by splitting merged cells into sub-cells with duplicated content.
///
/// Determines the full grid from all unique x-coordinates and y-coordinates across
/// all cells in the table. Cells that span multiple grid positions (merged cells) are
/// split into individual sub-cells, each receiving the text of the original merged cell.
///
/// This ensures every row has the same number of columns, which is useful for data
/// pipeline consumers that expect uniform table structures.
pub fn duplicate_merged_content_in_table(table: &Table) -> Table {
    if table.cells.is_empty() {
        return table.clone();
    }

    // Collect all unique x-coordinates and y-coordinates from cell boundaries
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();

    for cell in &table.cells {
        if !xs.iter().any(|&x| (x - cell.bbox.x0).abs() < 1e-6) {
            xs.push(cell.bbox.x0);
        }
        if !xs.iter().any(|&x| (x - cell.bbox.x1).abs() < 1e-6) {
            xs.push(cell.bbox.x1);
        }
        if !ys.iter().any(|&y| (y - cell.bbox.top).abs() < 1e-6) {
            ys.push(cell.bbox.top);
        }
        if !ys.iter().any(|&y| (y - cell.bbox.bottom).abs() < 1e-6) {
            ys.push(cell.bbox.bottom);
        }
    }

    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // For each grid position, find the enclosing cell and create a sub-cell
    let mut new_cells: Vec<Cell> = Vec::new();

    for yi in 0..ys.len().saturating_sub(1) {
        for xi in 0..xs.len().saturating_sub(1) {
            let sub_x0 = xs[xi];
            let sub_x1 = xs[xi + 1];
            let sub_top = ys[yi];
            let sub_bottom = ys[yi + 1];
            let sub_cx = (sub_x0 + sub_x1) / 2.0;
            let sub_cy = (sub_top + sub_bottom) / 2.0;

            // Find which existing cell contains this grid position's center
            let enclosing_cell = table.cells.iter().find(|c| {
                sub_cx >= c.bbox.x0 - 1e-6
                    && sub_cx <= c.bbox.x1 + 1e-6
                    && sub_cy >= c.bbox.top - 1e-6
                    && sub_cy <= c.bbox.bottom + 1e-6
            });

            if let Some(cell) = enclosing_cell {
                new_cells.push(Cell {
                    bbox: BBox::new(sub_x0, sub_top, sub_x1, sub_bottom),
                    text: cell.text.clone(),
                });
            }
        }
    }

    // Organize into rows (group by top, sort by x0)
    let mut row_map: std::collections::BTreeMap<i64, Vec<Cell>> = std::collections::BTreeMap::new();
    for cell in &new_cells {
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

    // Organize into columns (group by x0, sort by top)
    let mut col_map: std::collections::BTreeMap<i64, Vec<Cell>> = std::collections::BTreeMap::new();
    for cell in &new_cells {
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
        bbox: table.bbox,
        cells: new_cells,
        rows,
        columns,
    }
}

/// Normalize a table so all rows have equal column count by splitting merged cells.
///
/// Similar to [`duplicate_merged_content_in_table`], but text is placed only in the
/// first sub-cell of each merged group (top-left corner) instead of being duplicated
/// to all sub-cells. This matches Python pdfplumber's behavior where merged header
/// cells have text in the first column position and empty strings in the rest.
///
/// Should be called after `extract_text_for_cells` so merged cells already have
/// their text content populated.
pub fn normalize_table_columns(table: &Table) -> Table {
    if table.cells.is_empty() {
        return table.clone();
    }

    // Collect all unique x-coordinates and y-coordinates from cell boundaries
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();

    for cell in &table.cells {
        if !xs.iter().any(|&x| (x - cell.bbox.x0).abs() < 1e-6) {
            xs.push(cell.bbox.x0);
        }
        if !xs.iter().any(|&x| (x - cell.bbox.x1).abs() < 1e-6) {
            xs.push(cell.bbox.x1);
        }
        if !ys.iter().any(|&y| (y - cell.bbox.top).abs() < 1e-6) {
            ys.push(cell.bbox.top);
        }
        if !ys.iter().any(|&y| (y - cell.bbox.bottom).abs() < 1e-6) {
            ys.push(cell.bbox.bottom);
        }
    }

    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // For each grid position, find the enclosing cell and create a sub-cell
    let mut new_cells: Vec<Cell> = Vec::new();

    for yi in 0..ys.len().saturating_sub(1) {
        for xi in 0..xs.len().saturating_sub(1) {
            let sub_x0 = xs[xi];
            let sub_x1 = xs[xi + 1];
            let sub_top = ys[yi];
            let sub_bottom = ys[yi + 1];
            let sub_cx = (sub_x0 + sub_x1) / 2.0;
            let sub_cy = (sub_top + sub_bottom) / 2.0;

            // Find which existing cell contains this grid position's center
            let enclosing_cell = table.cells.iter().find(|c| {
                sub_cx >= c.bbox.x0 - 1e-6
                    && sub_cx <= c.bbox.x1 + 1e-6
                    && sub_cy >= c.bbox.top - 1e-6
                    && sub_cy <= c.bbox.bottom + 1e-6
            });

            if let Some(cell) = enclosing_cell {
                // Text goes in first sub-cell only (top-left corner of the enclosing cell)
                let is_first =
                    (sub_x0 - cell.bbox.x0).abs() < 1e-6 && (sub_top - cell.bbox.top).abs() < 1e-6;
                new_cells.push(Cell {
                    bbox: BBox::new(sub_x0, sub_top, sub_x1, sub_bottom),
                    text: if is_first { cell.text.clone() } else { None },
                });
            }
        }
    }

    // Organize into rows (group by top, sort by x0)
    let mut row_map: std::collections::BTreeMap<i64, Vec<Cell>> = std::collections::BTreeMap::new();
    for cell in &new_cells {
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

    // Organize into columns (group by x0, sort by top)
    let mut col_map: std::collections::BTreeMap<i64, Vec<Cell>> = std::collections::BTreeMap::new();
    for cell in &new_cells {
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
        bbox: table.bbox,
        cells: new_cells,
        rows,
        columns,
    }
}
