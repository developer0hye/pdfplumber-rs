//! Line and Rect extraction from painted paths.
//!
//! Converts painted PDF paths into geometric shapes (Line, Rect) with
//! coordinates in top-left origin system (y-flipped from PDF's bottom-left).

use crate::geometry::Point;
use crate::painting::{Color, PaintedPath};
use crate::path::PathSegment;

/// Orientation of a line segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOrientation {
    Horizontal,
    Vertical,
    Diagonal,
}

/// A line segment extracted from a painted path.
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Left x coordinate.
    pub x0: f64,
    /// Top y coordinate (distance from top of page).
    pub top: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Bottom y coordinate (distance from top of page).
    pub bottom: f64,
    /// Line width (stroke width from graphics state).
    pub line_width: f64,
    /// Stroking color.
    pub stroke_color: Color,
    /// Line orientation classification.
    pub orientation: LineOrientation,
}

/// A curve extracted from a painted path (cubic Bezier segment).
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// Bounding box left x.
    pub x0: f64,
    /// Bounding box top y (distance from top of page).
    pub top: f64,
    /// Bounding box right x.
    pub x1: f64,
    /// Bounding box bottom y (distance from top of page).
    pub bottom: f64,
    /// All points in top-left origin: [start, cp1, cp2, end].
    pub pts: Vec<(f64, f64)>,
    /// Line width (stroke width from graphics state).
    pub line_width: f64,
    /// Whether the curve is stroked.
    pub stroke: bool,
    /// Whether the curve is filled.
    pub fill: bool,
    /// Stroking color.
    pub stroke_color: Color,
    /// Fill color.
    pub fill_color: Color,
}

/// A rectangle extracted from a painted path.
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    /// Left x coordinate.
    pub x0: f64,
    /// Top y coordinate (distance from top of page).
    pub top: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Bottom y coordinate (distance from top of page).
    pub bottom: f64,
    /// Line width (stroke width from graphics state).
    pub line_width: f64,
    /// Whether the rectangle is stroked.
    pub stroke: bool,
    /// Whether the rectangle is filled.
    pub fill: bool,
    /// Stroking color.
    pub stroke_color: Color,
    /// Fill color.
    pub fill_color: Color,
}

impl Rect {
    /// Width of the rectangle.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// Height of the rectangle.
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }
}

/// Tolerance for floating-point comparison when detecting axis-aligned shapes.
const AXIS_TOLERANCE: f64 = 1e-6;

/// Classify line orientation based on start and end points (already y-flipped).
fn classify_orientation(x0: f64, y0: f64, x1: f64, y1: f64) -> LineOrientation {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    if dy < AXIS_TOLERANCE {
        LineOrientation::Horizontal
    } else if dx < AXIS_TOLERANCE {
        LineOrientation::Vertical
    } else {
        LineOrientation::Diagonal
    }
}

/// Flip a y-coordinate from PDF bottom-left origin to top-left origin.
fn flip_y(y: f64, page_height: f64) -> f64 {
    page_height - y
}

/// Try to detect an axis-aligned rectangle from a subpath's vertices.
///
/// Returns `Some((x0, top, x1, bottom))` in top-left origin if the vertices
/// form an axis-aligned rectangle, `None` otherwise.
fn try_detect_rect(vertices: &[Point], page_height: f64) -> Option<(f64, f64, f64, f64)> {
    // Need exactly 4 unique vertices for a rectangle
    if vertices.len() != 4 {
        return None;
    }

    // Check that all edges are axis-aligned (horizontal or vertical)
    for i in 0..4 {
        let a = &vertices[i];
        let b = &vertices[(i + 1) % 4];
        let dx = (b.x - a.x).abs();
        let dy = (b.y - a.y).abs();
        // Each edge must be either horizontal or vertical
        if dx > AXIS_TOLERANCE && dy > AXIS_TOLERANCE {
            return None;
        }
    }

    // Compute bounding box from all vertices
    let xs: Vec<f64> = vertices.iter().map(|p| p.x).collect();
    let ys: Vec<f64> = vertices.iter().map(|p| flip_y(p.y, page_height)).collect();

    let x0 = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let x1 = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let top = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let bottom = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Some((x0, top, x1, bottom))
}

/// Extract subpaths from a path's segments.
///
/// Each subpath starts with a MoveTo and contains subsequent segments
/// until the next MoveTo or end of segments.
fn extract_subpaths(segments: &[PathSegment]) -> Vec<&[PathSegment]> {
    let mut subpaths = Vec::new();
    let mut start = 0;

    for (i, seg) in segments.iter().enumerate() {
        if i > 0 && matches!(seg, PathSegment::MoveTo(_)) {
            if start < i {
                subpaths.push(&segments[start..i]);
            }
            start = i;
        }
    }
    if start < segments.len() {
        subpaths.push(&segments[start..]);
    }

    subpaths
}

/// Collect vertices from a subpath's segments.
///
/// Returns the list of unique vertices (endpoints of line segments).
/// ClosePath adds the first vertex as the closing point.
fn collect_vertices(subpath: &[PathSegment]) -> Vec<Point> {
    let mut vertices = Vec::new();
    let mut has_curves = false;

    for seg in subpath {
        match seg {
            PathSegment::MoveTo(p) => {
                vertices.push(*p);
            }
            PathSegment::LineTo(p) => {
                vertices.push(*p);
            }
            PathSegment::CurveTo { .. } => {
                has_curves = true;
            }
            PathSegment::ClosePath => {
                // ClosePath implicitly draws a line back to the start.
                // We don't need to add the start vertex again for detection.
            }
        }
    }

    // If there are curves, this can't be a simple rectangle or line set
    if has_curves {
        return Vec::new();
    }

    vertices
}

/// Check if a subpath is closed (has a ClosePath segment or start == end).
fn is_closed(subpath: &[PathSegment], vertices: &[Point]) -> bool {
    if subpath.iter().any(|s| matches!(s, PathSegment::ClosePath)) {
        return true;
    }
    // Also check if start and end points coincide
    if vertices.len() >= 2 {
        let first = vertices[0];
        let last = vertices[vertices.len() - 1];
        return (first.x - last.x).abs() < AXIS_TOLERANCE
            && (first.y - last.y).abs() < AXIS_TOLERANCE;
    }
    false
}

/// Check if a subpath contains any curve segments.
fn has_curves(subpath: &[PathSegment]) -> bool {
    subpath
        .iter()
        .any(|s| matches!(s, PathSegment::CurveTo { .. }))
}

/// Extract Line, Rect, and Curve objects from a painted path.
///
/// Coordinates are converted from PDF's bottom-left origin to pdfplumber's
/// top-left origin using the provided `page_height`.
///
/// Rectangle detection:
/// - Axis-aligned closed paths with exactly 4 vertices (no curves)
/// - Both from `re` operator and manual 4-line constructions
///
/// Line extraction:
/// - Each LineTo segment in a non-rectangle, non-curve subpath becomes a Line
/// - Stroked paths produce lines; non-stroked paths do not produce lines
///
/// Curve extraction:
/// - Each CurveTo segment becomes a Curve object with control points
/// - LineTo segments in curve-containing subpaths also become Lines (if stroked)
pub fn extract_shapes(
    painted: &PaintedPath,
    page_height: f64,
) -> (Vec<Line>, Vec<Rect>, Vec<Curve>) {
    let mut lines = Vec::new();
    let mut rects = Vec::new();
    let mut curves = Vec::new();

    let subpaths = extract_subpaths(&painted.path.segments);

    for subpath in subpaths {
        // If the subpath has curves, extract curve objects
        if has_curves(subpath) {
            extract_curves_from_subpath(subpath, painted, page_height, &mut curves, &mut lines);
            continue;
        }

        let vertices = collect_vertices(subpath);
        if vertices.is_empty() {
            continue;
        }

        let closed = is_closed(subpath, &vertices);

        // Try to detect rectangle from closed 4-vertex subpath
        if closed && vertices.len() == 4 {
            if let Some((x0, top, x1, bottom)) = try_detect_rect(&vertices, page_height) {
                rects.push(Rect {
                    x0,
                    top,
                    x1,
                    bottom,
                    line_width: painted.line_width,
                    stroke: painted.stroke,
                    fill: painted.fill,
                    stroke_color: painted.stroke_color,
                    fill_color: painted.fill_color,
                });
                continue;
            }
        }

        // Also check 5 vertices where the last == first (rectangle without ClosePath segment)
        if closed && vertices.len() == 5 {
            let first = vertices[0];
            let last = vertices[4];
            if (first.x - last.x).abs() < AXIS_TOLERANCE
                && (first.y - last.y).abs() < AXIS_TOLERANCE
            {
                if let Some((x0, top, x1, bottom)) = try_detect_rect(&vertices[..4], page_height) {
                    rects.push(Rect {
                        x0,
                        top,
                        x1,
                        bottom,
                        line_width: painted.line_width,
                        stroke: painted.stroke,
                        fill: painted.fill,
                        stroke_color: painted.stroke_color,
                        fill_color: painted.fill_color,
                    });
                    continue;
                }
            }
        }

        // Extract individual lines from stroked paths
        if !painted.stroke {
            continue;
        }

        extract_lines_from_subpath(subpath, &vertices, painted, page_height, &mut lines);
    }

    (lines, rects, curves)
}

/// Extract lines from a non-curve subpath.
fn extract_lines_from_subpath(
    subpath: &[PathSegment],
    vertices: &[Point],
    painted: &PaintedPath,
    page_height: f64,
    lines: &mut Vec<Line>,
) {
    let mut prev_point: Option<Point> = None;
    for seg in subpath {
        match seg {
            PathSegment::MoveTo(p) => {
                prev_point = Some(*p);
            }
            PathSegment::LineTo(p) => {
                if let Some(start) = prev_point {
                    push_line(start, *p, painted, page_height, lines);
                }
                prev_point = Some(*p);
            }
            PathSegment::ClosePath => {
                if let (Some(current), Some(start_pt)) = (prev_point, vertices.first().copied()) {
                    if (current.x - start_pt.x).abs() > AXIS_TOLERANCE
                        || (current.y - start_pt.y).abs() > AXIS_TOLERANCE
                    {
                        push_line(current, start_pt, painted, page_height, lines);
                    }
                }
                prev_point = vertices.first().copied();
            }
            PathSegment::CurveTo { .. } => {}
        }
    }
}

/// Push a Line from two points (PDF coords) into the lines vector.
fn push_line(
    start: Point,
    end: Point,
    painted: &PaintedPath,
    page_height: f64,
    lines: &mut Vec<Line>,
) {
    let fy0 = flip_y(start.y, page_height);
    let fy1 = flip_y(end.y, page_height);

    let x0 = start.x.min(end.x);
    let x1 = start.x.max(end.x);
    let top = fy0.min(fy1);
    let bottom = fy0.max(fy1);
    let orientation = classify_orientation(start.x, fy0, end.x, fy1);

    lines.push(Line {
        x0,
        top,
        x1,
        bottom,
        line_width: painted.line_width,
        stroke_color: painted.stroke_color,
        orientation,
    });
}

/// Extract curves (and lines from mixed subpaths) from a subpath containing CurveTo segments.
fn extract_curves_from_subpath(
    subpath: &[PathSegment],
    painted: &PaintedPath,
    page_height: f64,
    curves: &mut Vec<Curve>,
    lines: &mut Vec<Line>,
) {
    let mut prev_point: Option<Point> = None;
    let mut subpath_start: Option<Point> = None;

    for seg in subpath {
        match seg {
            PathSegment::MoveTo(p) => {
                prev_point = Some(*p);
                subpath_start = Some(*p);
            }
            PathSegment::LineTo(p) => {
                if painted.stroke {
                    if let Some(start) = prev_point {
                        push_line(start, *p, painted, page_height, lines);
                    }
                }
                prev_point = Some(*p);
            }
            PathSegment::CurveTo { cp1, cp2, end } => {
                if let Some(start) = prev_point {
                    // Collect all x/y coordinates for bbox
                    let all_x = [start.x, cp1.x, cp2.x, end.x];
                    let all_y = [
                        flip_y(start.y, page_height),
                        flip_y(cp1.y, page_height),
                        flip_y(cp2.y, page_height),
                        flip_y(end.y, page_height),
                    ];

                    let x0 = all_x.iter().cloned().fold(f64::INFINITY, f64::min);
                    let x1 = all_x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let top = all_y.iter().cloned().fold(f64::INFINITY, f64::min);
                    let bottom = all_y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                    curves.push(Curve {
                        x0,
                        top,
                        x1,
                        bottom,
                        pts: vec![
                            (start.x, flip_y(start.y, page_height)),
                            (cp1.x, flip_y(cp1.y, page_height)),
                            (cp2.x, flip_y(cp2.y, page_height)),
                            (end.x, flip_y(end.y, page_height)),
                        ],
                        line_width: painted.line_width,
                        stroke: painted.stroke,
                        fill: painted.fill,
                        stroke_color: painted.stroke_color,
                        fill_color: painted.fill_color,
                    });
                }
                prev_point = Some(*end);
            }
            PathSegment::ClosePath => {
                // ClosePath draws a line back to the subpath start
                if painted.stroke {
                    if let (Some(current), Some(start_pt)) = (prev_point, subpath_start) {
                        if (current.x - start_pt.x).abs() > AXIS_TOLERANCE
                            || (current.y - start_pt.y).abs() > AXIS_TOLERANCE
                        {
                            push_line(current, start_pt, painted, page_height, lines);
                        }
                    }
                }
                prev_point = subpath_start;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Ctm;
    use crate::painting::{FillRule, GraphicsState};
    use crate::path::PathBuilder;

    const PAGE_HEIGHT: f64 = 792.0;

    fn default_gs() -> GraphicsState {
        GraphicsState::default()
    }

    fn custom_gs() -> GraphicsState {
        GraphicsState {
            line_width: 2.5,
            stroke_color: Color::new(1.0, 0.0, 0.0),
            fill_color: Color::new(0.0, 0.0, 1.0),
        }
    }

    fn assert_approx(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6,
            "expected {b}, got {a}, diff={}",
            (a - b).abs()
        );
    }

    // --- Horizontal line ---

    #[test]
    fn test_horizontal_line_extraction() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 500.0);
        builder.line_to(300.0, 500.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(lines.len(), 1);
        assert!(rects.is_empty());

        let line = &lines[0];
        assert_approx(line.x0, 100.0);
        assert_approx(line.x1, 300.0);
        // y-flip: 792 - 500 = 292
        assert_approx(line.top, 292.0);
        assert_approx(line.bottom, 292.0);
        assert_eq!(line.orientation, LineOrientation::Horizontal);
        assert_approx(line.line_width, 1.0);
    }

    // --- Vertical line ---

    #[test]
    fn test_vertical_line_extraction() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(200.0, 100.0);
        builder.line_to(200.0, 400.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(lines.len(), 1);
        assert!(rects.is_empty());

        let line = &lines[0];
        assert_approx(line.x0, 200.0);
        assert_approx(line.x1, 200.0);
        // y-flip: 792-400=392 (top), 792-100=692 (bottom)
        assert_approx(line.top, 392.0);
        assert_approx(line.bottom, 692.0);
        assert_eq!(line.orientation, LineOrientation::Vertical);
    }

    // --- Diagonal line ---

    #[test]
    fn test_diagonal_line_extraction() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        builder.line_to(300.0, 400.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(lines.len(), 1);
        assert!(rects.is_empty());

        let line = &lines[0];
        assert_approx(line.x0, 100.0);
        assert_approx(line.x1, 300.0);
        // y-flip: min(792-100, 792-400) = min(692, 392) = 392
        assert_approx(line.top, 392.0);
        assert_approx(line.bottom, 692.0);
        assert_eq!(line.orientation, LineOrientation::Diagonal);
    }

    // --- Line with custom width and color ---

    #[test]
    fn test_line_with_custom_width_and_color() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 0.0);
        let painted = builder.stroke(&custom_gs());

        let (lines, _, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(lines.len(), 1);

        let line = &lines[0];
        assert_approx(line.line_width, 2.5);
        assert_eq!(line.stroke_color, Color::new(1.0, 0.0, 0.0));
    }

    // --- Rectangle from `re` operator ---

    #[test]
    fn test_rect_from_re_operator() {
        let mut builder = PathBuilder::new(Ctm::identity());
        // re(x, y, w, h) in PDF coordinates (bottom-left origin)
        builder.rectangle(100.0, 200.0, 200.0, 100.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert_eq!(rects.len(), 1);

        let rect = &rects[0];
        assert_approx(rect.x0, 100.0);
        assert_approx(rect.x1, 300.0);
        // y-flip: min(792-200, 792-300) = min(592, 492) = 492
        assert_approx(rect.top, 492.0);
        // max(792-200, 792-300) = 592
        assert_approx(rect.bottom, 592.0);
        assert!(rect.stroke);
        assert!(!rect.fill);
    }

    // --- Rectangle from 4-line closed path ---

    #[test]
    fn test_rect_from_four_line_closed_path() {
        let mut builder = PathBuilder::new(Ctm::identity());
        // Manually construct a rectangle without using `re`
        builder.move_to(50.0, 100.0);
        builder.line_to(250.0, 100.0);
        builder.line_to(250.0, 300.0);
        builder.line_to(50.0, 300.0);
        builder.close_path();
        let painted = builder.fill(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert_eq!(rects.len(), 1);

        let rect = &rects[0];
        assert_approx(rect.x0, 50.0);
        assert_approx(rect.x1, 250.0);
        // y-flip: min(792-100, 792-300) = min(692, 492) = 492
        assert_approx(rect.top, 492.0);
        assert_approx(rect.bottom, 692.0);
        assert!(!rect.stroke);
        assert!(rect.fill);
    }

    // --- Fill+stroke rectangle ---

    #[test]
    fn test_rect_fill_and_stroke() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.rectangle(10.0, 20.0, 100.0, 50.0);
        let painted = builder.fill_and_stroke(&custom_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert_eq!(rects.len(), 1);

        let rect = &rects[0];
        assert!(rect.stroke);
        assert!(rect.fill);
        assert_approx(rect.line_width, 2.5);
        assert_eq!(rect.stroke_color, Color::new(1.0, 0.0, 0.0));
        assert_eq!(rect.fill_color, Color::new(0.0, 0.0, 1.0));
    }

    // --- Rect dimensions ---

    #[test]
    fn test_rect_width_and_height() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.rectangle(100.0, 200.0, 150.0, 80.0);
        let painted = builder.stroke(&default_gs());

        let (_, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(rects.len(), 1);

        let rect = &rects[0];
        assert_approx(rect.width(), 150.0);
        assert_approx(rect.height(), 80.0);
    }

    // --- Non-rectangular closed path produces lines ---

    #[test]
    fn test_non_rect_closed_path_produces_lines() {
        // A triangle (3 vertices, not 4) — not a rectangle
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        builder.line_to(200.0, 100.0);
        builder.line_to(150.0, 200.0);
        builder.close_path(); // closes back to (100, 100)
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(rects.is_empty());
        // 3 lines: (100,100)→(200,100), (200,100)→(150,200), (150,200)→(100,100)
        assert_eq!(lines.len(), 3);

        // First line is horizontal
        assert_eq!(lines[0].orientation, LineOrientation::Horizontal);
        // Other two are diagonal
        assert_eq!(lines[1].orientation, LineOrientation::Diagonal);
        assert_eq!(lines[2].orientation, LineOrientation::Diagonal);
    }

    // --- Non-axis-aligned 4-vertex path produces lines ---

    #[test]
    fn test_non_axis_aligned_quadrilateral_produces_lines() {
        // A diamond/rhombus shape — 4 vertices but not axis-aligned
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(150.0, 100.0);
        builder.line_to(200.0, 200.0);
        builder.line_to(150.0, 300.0);
        builder.line_to(100.0, 200.0);
        builder.close_path();
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(rects.is_empty());
        assert_eq!(lines.len(), 4); // 4 diagonal lines
    }

    // --- Fill-only path does not produce lines ---

    #[test]
    fn test_fill_only_does_not_produce_lines() {
        // A non-rectangle filled path should not produce lines
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        builder.line_to(200.0, 100.0);
        builder.line_to(150.0, 200.0);
        builder.close_path();
        let painted = builder.fill(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty()); // fill-only, no stroked lines
        assert!(rects.is_empty()); // not a rectangle
    }

    // --- Multiple subpaths ---

    #[test]
    fn test_multiple_subpaths_lines() {
        let mut builder = PathBuilder::new(Ctm::identity());
        // First subpath: horizontal line
        builder.move_to(0.0, 100.0);
        builder.line_to(200.0, 100.0);
        // Second subpath: vertical line
        builder.move_to(100.0, 0.0);
        builder.line_to(100.0, 200.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(lines.len(), 2);
        assert!(rects.is_empty());
        assert_eq!(lines[0].orientation, LineOrientation::Horizontal);
        assert_eq!(lines[1].orientation, LineOrientation::Vertical);
    }

    // --- Multiple subpaths: rect + line ---

    #[test]
    fn test_multiple_subpaths_rect_and_line() {
        let mut builder = PathBuilder::new(Ctm::identity());
        // First subpath: rectangle
        builder.rectangle(10.0, 10.0, 100.0, 50.0);
        // Second subpath: a line
        builder.move_to(0.0, 100.0);
        builder.line_to(200.0, 100.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(rects.len(), 1);
        assert_eq!(lines.len(), 1);
    }

    // --- n (end path, no painting) produces nothing ---

    #[test]
    fn test_end_path_produces_nothing() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.rectangle(10.0, 10.0, 100.0, 50.0);
        let result = builder.end_path();
        assert!(result.is_none());
        // No painted path means nothing to extract
    }

    // --- Orientation classification ---

    #[test]
    fn test_classify_orientation_horizontal() {
        assert_eq!(
            classify_orientation(0.0, 100.0, 200.0, 100.0),
            LineOrientation::Horizontal
        );
    }

    #[test]
    fn test_classify_orientation_vertical() {
        assert_eq!(
            classify_orientation(100.0, 0.0, 100.0, 200.0),
            LineOrientation::Vertical
        );
    }

    #[test]
    fn test_classify_orientation_diagonal() {
        assert_eq!(
            classify_orientation(0.0, 0.0, 100.0, 200.0),
            LineOrientation::Diagonal
        );
    }

    // --- Y-flip ---

    #[test]
    fn test_y_flip() {
        assert_approx(flip_y(0.0, 792.0), 792.0);
        assert_approx(flip_y(792.0, 792.0), 0.0);
        assert_approx(flip_y(396.0, 792.0), 396.0);
        assert_approx(flip_y(100.0, 792.0), 692.0);
    }

    // --- Edge case: empty path ---

    #[test]
    fn test_empty_path_produces_nothing() {
        let painted = PaintedPath {
            path: crate::path::Path {
                segments: Vec::new(),
            },
            stroke: true,
            fill: false,
            fill_rule: FillRule::NonZeroWinding,
            line_width: 1.0,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        };

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert!(rects.is_empty());
    }

    // --- Edge case: single MoveTo ---

    #[test]
    fn test_single_moveto_produces_nothing() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert!(rects.is_empty());
    }

    // --- Path with curves produces curves, not rects ---

    #[test]
    fn test_path_with_curves_no_rect_detection() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        builder.close_path();
        let painted = builder.stroke(&default_gs());

        let (lines, rects, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(rects.is_empty());
        // ClosePath generates a line back to (0,0) since path is stroked
        assert_eq!(lines.len(), 1);
        assert_eq!(curves.len(), 1);
    }

    // --- Rectangle with CTM transformation ---

    #[test]
    fn test_rect_with_ctm_scale() {
        // CTM scales by 2x
        let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.rectangle(50.0, 100.0, 100.0, 50.0);
        let painted = builder.stroke(&default_gs());

        let (lines, rects, _) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert_eq!(rects.len(), 1);

        let rect = &rects[0];
        // Scaled: x: 100..300, y: 200..300
        assert_approx(rect.x0, 100.0);
        assert_approx(rect.x1, 300.0);
        // y-flip: 792-300=492 (top), 792-200=592 (bottom)
        assert_approx(rect.top, 492.0);
        assert_approx(rect.bottom, 592.0);
    }

    // ==================== Curve extraction tests (US-024) ====================

    #[test]
    fn test_curve_extraction_simple() {
        // Simple cubic Bezier from (0,0) to (100,0) with control points at (10,50) and (90,50)
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);

        let curve = &curves[0];
        // 4 points: start, cp1, cp2, end
        assert_eq!(curve.pts.len(), 4);
        // Start: (0, flip(0)) = (0, 792)
        assert_approx(curve.pts[0].0, 0.0);
        assert_approx(curve.pts[0].1, 792.0);
        // CP1: (10, flip(50)) = (10, 742)
        assert_approx(curve.pts[1].0, 10.0);
        assert_approx(curve.pts[1].1, 742.0);
        // CP2: (90, flip(50)) = (90, 742)
        assert_approx(curve.pts[2].0, 90.0);
        assert_approx(curve.pts[2].1, 742.0);
        // End: (100, flip(0)) = (100, 792)
        assert_approx(curve.pts[3].0, 100.0);
        assert_approx(curve.pts[3].1, 792.0);
    }

    #[test]
    fn test_curve_bbox() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        let curve = &curves[0];

        // x: min(0, 10, 90, 100) = 0, max = 100
        assert_approx(curve.x0, 0.0);
        assert_approx(curve.x1, 100.0);
        // y (flipped): min(792, 742, 742, 792) = 742, max = 792
        assert_approx(curve.top, 742.0);
        assert_approx(curve.bottom, 792.0);
    }

    #[test]
    fn test_curve_captures_graphics_state() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 20.0, 30.0, 40.0, 50.0, 0.0);
        let painted = builder.stroke(&custom_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);

        let curve = &curves[0];
        assert_approx(curve.line_width, 2.5);
        assert!(curve.stroke);
        assert!(!curve.fill);
        assert_eq!(curve.stroke_color, Color::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_curve_fill_only() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        builder.close_path();
        let painted = builder.fill(&default_gs());

        let (lines, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);
        assert!(curves[0].fill);
        assert!(!curves[0].stroke);
        // Fill-only: no lines from ClosePath
        assert!(lines.is_empty());
    }

    #[test]
    fn test_multiple_curves_in_subpath() {
        // Two curve segments in one subpath
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 40.0, 50.0, 50.0, 0.0);
        builder.curve_to(60.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 2);

        // First curve: (0,0) -> (50,0)
        assert_approx(curves[0].pts[0].0, 0.0);
        assert_approx(curves[0].pts[3].0, 50.0);
        // Second curve: (50,0) -> (100,0)
        assert_approx(curves[1].pts[0].0, 50.0);
        assert_approx(curves[1].pts[3].0, 100.0);
    }

    #[test]
    fn test_mixed_line_and_curve_subpath() {
        // Subpath with both LineTo and CurveTo: line + curve + line
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(50.0, 0.0);
        builder.curve_to(60.0, 0.0, 70.0, 10.0, 70.0, 20.0);
        builder.line_to(70.0, 50.0);
        let painted = builder.stroke(&default_gs());

        let (lines, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);
        assert_eq!(lines.len(), 2); // line_to(50,0) and line_to(70,50)
    }

    #[test]
    fn test_curve_with_ctm_transform() {
        // CTM scales by 2x
        let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 25.0, 40.0, 25.0, 50.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);

        let curve = &curves[0];
        // Coords are CTM-transformed: (0,0)->(0,0), (10,25)->(20,50), (40,25)->(80,50), (50,0)->(100,0)
        assert_approx(curve.pts[0].0, 0.0);
        assert_approx(curve.pts[1].0, 20.0);
        assert_approx(curve.pts[2].0, 80.0);
        assert_approx(curve.pts[3].0, 100.0);
    }
}
