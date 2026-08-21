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

/// The compatible object family emitted for a painted-path subpath.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// A single straight segment.
    Line,
    /// A closed axis-aligned rectangle.
    Rect,
    /// Any other painted path.
    Curve,
}

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

/// Extract Line, Rect, and Curve objects from a painted path.
///
/// Coordinates are converted from PDF's bottom-left origin to pdfplumber's
/// top-left origin using the provided `page_height`.
///
/// Each subpath becomes exactly one object, classified by its shape the way
/// pdfminer does:
///
/// * a single straight segment is a [`Line`],
/// * a closed, axis-aligned four-corner loop is a [`Rect`],
/// * anything else — a polyline, a Bézier, an open triangle — is a [`Curve`].
///
/// Whether the path was stroked or filled does not affect the classification:
/// a filled shape is as much on the page as a stroked one. A curve keeps the
/// endpoint of each segment in `pts`; Bézier control points are not endpoints
/// and are left out.
pub fn extract_shapes(
    painted: &PaintedPath,
    page_height: f64,
) -> (Vec<Line>, Vec<Rect>, Vec<Curve>) {
    let (lines, rects, curves, _) = extract_shapes_with_order(painted, page_height);
    (lines, rects, curves)
}

/// Extract shapes while retaining the emitted subpath-family order.
pub fn extract_shapes_with_order(
    painted: &PaintedPath,
    page_height: f64,
) -> (Vec<Line>, Vec<Rect>, Vec<Curve>, Vec<ShapeKind>) {
    let mut lines = Vec::new();
    let mut rects = Vec::new();
    let mut curves = Vec::new();
    let mut order = Vec::new();

    for subpath in extract_subpaths(&painted.path.segments) {
        let Some((shape, points)) = shape_of(subpath) else {
            continue;
        };

        match shape.as_str() {
            "ml" | "mlh" => {
                push_line(points[0], points[1], painted, page_height, &mut lines);
                order.push(ShapeKind::Line);
            }
            "mlllh" | "mllll" => {
                let closed_loop = points_coincide(points[0], points[4]);
                match try_detect_rect(&points[..4], page_height).filter(|_| closed_loop) {
                    Some((x0, top, x1, bottom)) => {
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
                        order.push(ShapeKind::Rect);
                    }
                    None => {
                        curves.push(curve_from_points(&points, painted, page_height));
                        order.push(ShapeKind::Curve);
                    }
                }
            }
            _ => {
                curves.push(curve_from_points(&points, painted, page_height));
                order.push(ShapeKind::Curve);
            }
        }
    }

    (lines, rects, curves, order)
}

/// Describe a subpath as pdfminer does: a shape string and one point per
/// segment.
///
/// The shape string uses the PDF operator letters — `m` move, `l` line, `c`
/// curve, `h` close — and each segment contributes its endpoint, with `h`
/// contributing the point the subpath started from. A trailing line back to
/// the start before a close is redundant and dropped, so a rectangle drawn
/// either way describes the same shape.
///
/// Returns `None` for a subpath that does not start with a move or is too
/// short to describe anything.
fn shape_of(subpath: &[PathSegment]) -> Option<(String, Vec<Point>)> {
    let start = match subpath.first()? {
        PathSegment::MoveTo(p) => *p,
        _ => return None,
    };

    let mut shape = String::with_capacity(subpath.len());
    let mut points = Vec::with_capacity(subpath.len());

    for segment in subpath {
        match segment {
            PathSegment::MoveTo(p) => {
                shape.push('m');
                points.push(*p);
            }
            PathSegment::LineTo(p) => {
                shape.push('l');
                points.push(*p);
            }
            PathSegment::CurveTo { end, .. } => {
                shape.push('c');
                points.push(*end);
            }
            PathSegment::ClosePath => {
                shape.push('h');
                points.push(start);
            }
        }
    }

    if shape.len() > 3 && shape.ends_with("lh") && points_coincide(points[points.len() - 2], start)
    {
        shape.truncate(shape.len() - 2);
        shape.push('h');
        points.remove(points.len() - 2);
    }

    if points.len() < 2 {
        return None;
    }

    Some((shape, points))
}

/// Whether two points are the same, allowing for float noise.
fn points_coincide(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < AXIS_TOLERANCE && (a.y - b.y).abs() < AXIS_TOLERANCE
}

/// Build a curve spanning the given segment endpoints.
fn curve_from_points(points: &[Point], painted: &PaintedPath, page_height: f64) -> Curve {
    let pts: Vec<(f64, f64)> = points
        .iter()
        .map(|p| (p.x, flip_y(p.y, page_height)))
        .collect();

    let x0 = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let x1 = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let top = pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let bottom = pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    Curve {
        x0,
        top,
        x1,
        bottom,
        pts,
        line_width: painted.line_width,
        stroke: painted.stroke,
        fill: painted.fill,
        stroke_color: painted.stroke_color.clone(),
        fill_color: painted.fill_color.clone(),
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

    lines.push(Line {
        x0: start.x.min(end.x),
        top: fy0.min(fy1),
        x1: start.x.max(end.x),
        bottom: fy0.max(fy1),
        line_width: painted.line_width,
        stroke_color: painted.stroke_color.clone(),
        orientation: classify_orientation(start.x, fy0, end.x, fy1),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Ctm;
    use crate::painting::{DashPattern, FillRule, GraphicsState};
    use crate::path::PathBuilder;

    const PAGE_HEIGHT: f64 = 792.0;

    // --- Direct construction and field access tests ---

    #[test]
    fn test_line_construction_and_field_access() {
        let line = Line {
            x0: 10.0,
            top: 20.0,
            x1: 100.0,
            bottom: 20.0,
            line_width: 1.5,
            stroke_color: Color::Rgb(1.0, 0.0, 0.0),
            orientation: Orientation::Horizontal,
        };
        assert_eq!(line.x0, 10.0);
        assert_eq!(line.top, 20.0);
        assert_eq!(line.x1, 100.0);
        assert_eq!(line.bottom, 20.0);
        assert_eq!(line.line_width, 1.5);
        assert_eq!(line.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
        assert_eq!(line.orientation, Orientation::Horizontal);
    }

    #[test]
    fn test_rect_construction_and_field_access() {
        let rect = Rect {
            x0: 50.0,
            top: 100.0,
            x1: 200.0,
            bottom: 300.0,
            line_width: 2.0,
            stroke: true,
            fill: true,
            stroke_color: Color::Gray(0.0),
            fill_color: Color::Cmyk(0.0, 1.0, 1.0, 0.0),
        };
        assert_eq!(rect.x0, 50.0);
        assert_eq!(rect.top, 100.0);
        assert_eq!(rect.x1, 200.0);
        assert_eq!(rect.bottom, 300.0);
        assert_eq!(rect.line_width, 2.0);
        assert!(rect.stroke);
        assert!(rect.fill);
        assert_eq!(rect.stroke_color, Color::Gray(0.0));
        assert_eq!(rect.fill_color, Color::Cmyk(0.0, 1.0, 1.0, 0.0));
        assert_eq!(rect.width(), 150.0);
        assert_eq!(rect.height(), 200.0);
    }

    #[test]
    fn test_curve_construction_and_field_access() {
        let curve = Curve {
            x0: 0.0,
            top: 50.0,
            x1: 100.0,
            bottom: 100.0,
            pts: vec![(0.0, 100.0), (30.0, 50.0), (70.0, 50.0), (100.0, 100.0)],
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        };
        assert_eq!(curve.x0, 0.0);
        assert_eq!(curve.top, 50.0);
        assert_eq!(curve.x1, 100.0);
        assert_eq!(curve.bottom, 100.0);
        assert_eq!(curve.pts.len(), 4);
        assert_eq!(curve.pts[0], (0.0, 100.0));
        assert_eq!(curve.pts[3], (100.0, 100.0));
        assert_eq!(curve.line_width, 1.0);
        assert!(curve.stroke);
        assert!(!curve.fill);
    }

    fn default_gs() -> GraphicsState {
        GraphicsState::default()
    }

    fn custom_gs() -> GraphicsState {
        GraphicsState {
            line_width: 2.5,
            stroke_color: Color::Rgb(1.0, 0.0, 0.0),
            fill_color: Color::Rgb(0.0, 0.0, 1.0),
            ..GraphicsState::default()
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
        assert_eq!(line.orientation, Orientation::Horizontal);
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
        assert_eq!(line.orientation, Orientation::Vertical);
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
        assert_eq!(line.orientation, Orientation::Diagonal);
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
        assert_eq!(line.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
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
        assert_eq!(rect.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
        assert_eq!(rect.fill_color, Color::Rgb(0.0, 0.0, 1.0));
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
    fn test_non_rect_closed_path_is_a_curve() {
        // A triangle (3 vertices, not 4) — not a rectangle
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        builder.line_to(200.0, 100.0);
        builder.line_to(150.0, 200.0);
        builder.close_path(); // closes back to (100, 100)
        let painted = builder.stroke(&default_gs());

        let (lines, rects, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(rects.is_empty());
        assert!(lines.is_empty());

        // One shape, holding the corners it was drawn through and back again.
        assert_eq!(curves.len(), 1);
        assert_eq!(curves[0].pts.len(), 4);
    }

    // --- Non-axis-aligned 4-vertex path produces lines ---

    #[test]
    fn test_non_axis_aligned_quadrilateral_is_a_curve() {
        // A diamond/rhombus shape — 4 vertices but not axis-aligned
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(150.0, 100.0);
        builder.line_to(200.0, 200.0);
        builder.line_to(150.0, 300.0);
        builder.line_to(100.0, 200.0);
        builder.close_path();
        let painted = builder.stroke(&default_gs());

        let (lines, rects, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(rects.is_empty());
        assert!(lines.is_empty());
        assert_eq!(curves.len(), 1);
    }

    // --- Fill-only path does not produce lines ---

    #[test]
    fn test_fill_only_shape_is_still_extracted() {
        // A filled triangle is on the page even though nothing was stroked.
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(100.0, 100.0);
        builder.line_to(200.0, 100.0);
        builder.line_to(150.0, 200.0);
        builder.close_path();
        let painted = builder.fill(&default_gs());

        let (lines, rects, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert!(lines.is_empty());
        assert!(rects.is_empty()); // not a rectangle
        assert_eq!(curves.len(), 1);
        assert!(curves[0].fill);
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
        assert_eq!(lines[0].orientation, Orientation::Horizontal);
        assert_eq!(lines[1].orientation, Orientation::Vertical);
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

        let (lines, rects, _, order) = extract_shapes_with_order(&painted, PAGE_HEIGHT);
        assert_eq!(rects.len(), 1);
        assert_eq!(lines.len(), 1);
        assert_eq!(order, vec![ShapeKind::Rect, ShapeKind::Line]);
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
            Orientation::Horizontal
        );
    }

    #[test]
    fn test_classify_orientation_vertical() {
        assert_eq!(
            classify_orientation(100.0, 0.0, 100.0, 200.0),
            Orientation::Vertical
        );
    }

    #[test]
    fn test_classify_orientation_diagonal() {
        assert_eq!(
            classify_orientation(0.0, 0.0, 100.0, 200.0),
            Orientation::Diagonal
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
            dash_pattern: DashPattern::solid(),
            stroke_alpha: 1.0,
            fill_alpha: 1.0,
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
        assert!(lines.is_empty());
        assert_eq!(curves.len(), 1);
        // Start, the curve's endpoint, and the close back to the start.
        assert_eq!(curves[0].pts.len(), 3);
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
        // Segment endpoints only: where the path starts and where it lands.
        // Control points steer the curve but are not points on it.
        assert_eq!(curve.pts.len(), 2);
        assert_approx(curve.pts[0].0, 0.0);
        assert_approx(curve.pts[0].1, 792.0);
        assert_approx(curve.pts[1].0, 100.0);
        assert_approx(curve.pts[1].1, 792.0);
    }

    #[test]
    fn test_curve_bbox() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        let curve = &curves[0];

        // The box spans the endpoints: x from 0 to 100, both at y = 792.
        assert_approx(curve.x0, 0.0);
        assert_approx(curve.x1, 100.0);
        assert_approx(curve.top, 792.0);
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
        assert_eq!(curve.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
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
        // Two curve segments in one subpath are one object, not two: a subpath
        // is a single shape however many segments draw it.
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 50.0, 40.0, 50.0, 50.0, 0.0);
        builder.curve_to(60.0, 50.0, 90.0, 50.0, 100.0, 0.0);
        let painted = builder.stroke(&default_gs());

        let (_, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);

        let pts = &curves[0].pts;
        assert_eq!(pts.len(), 3);
        assert_approx(pts[0].0, 0.0);
        assert_approx(pts[1].0, 50.0);
        assert_approx(pts[2].0, 100.0);
    }

    #[test]
    fn test_mixed_line_and_curve_subpath() {
        // One subpath of line, curve and line is one shape, and a shape with a
        // curve in it is never a Line object.
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(50.0, 0.0);
        builder.curve_to(60.0, 0.0, 70.0, 10.0, 70.0, 20.0);
        builder.line_to(70.0, 50.0);
        let painted = builder.stroke(&default_gs());

        let (lines, _, curves) = extract_shapes(&painted, PAGE_HEIGHT);
        assert_eq!(curves.len(), 1);
        assert!(lines.is_empty());
        assert_eq!(curves[0].pts.len(), 4);
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
        // The endpoints are CTM-transformed: (0,0) -> (0,0), (50,0) -> (100,0)
        assert_eq!(curve.pts.len(), 2);
        assert_approx(curve.pts[0].0, 0.0);
        assert_approx(curve.pts[1].0, 100.0);
    }
}
