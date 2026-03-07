//! Line and Rect extraction from painted paths.
//!
//! Converts painted PDF paths into geometric shapes (Line, Rect) with
//! coordinates in top-left origin system (y-flipped from PDF's bottom-left).

use crate::geometry::{Orientation, Point};
use crate::painting::{Color, PaintedPath};
use crate::path::PathSegment;

/// Type alias preserving backward compatibility.
pub type LineOrientation = Orientation;

/// A line segment extracted from a painted path.
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    pub orientation: Orientation,
}

/// A curve extracted from a painted path (cubic Bezier segment).
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
fn classify_orientation(x0: f64, y0: f64, x1: f64, y1: f64) -> Orientation {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    if dy < AXIS_TOLERANCE {
        Orientation::Horizontal
    } else if dx < AXIS_TOLERANCE {
        Orientation::Vertical
    } else {
        Orientation::Diagonal
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
                    stroke_color: painted.stroke_color.clone(),
                    fill_color: painted.fill_color.clone(),
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
                        stroke_color: painted.stroke_color.clone(),
                        fill_color: painted.fill_color.clone(),
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
        stroke_color: painted.stroke_color.clone(),
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
                        stroke_color: painted.stroke_color.clone(),
                        fill_color: painted.fill_color.clone(),
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
mod tests;
