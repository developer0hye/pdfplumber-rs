//! Edge derivation from geometric primitives.
//!
//! Edges are line segments derived from Lines, Rects, and Curves for
//! use in table detection algorithms.

use crate::geometry::Orientation;
use crate::shapes::{Curve, Line, Rect};

/// Source of an edge, tracking which geometric primitive it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSource {
    /// Derived directly from a Line object.
    Line,
    /// Top edge of a Rect.
    RectTop,
    /// Bottom edge of a Rect.
    RectBottom,
    /// Left edge of a Rect.
    RectLeft,
    /// Right edge of a Rect.
    RectRight,
    /// Approximated from a Curve (chord from start to end).
    Curve,
    /// Synthetic edge generated from text alignment patterns (Stream strategy).
    Stream,
}

/// A line segment edge for table detection.
///
/// Edges are derived from Lines, Rects, and Curves and are used
/// by the table detection algorithm to find cell boundaries.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Left x coordinate.
    pub x0: f64,
    /// Top y coordinate (distance from top of page).
    pub top: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Bottom y coordinate (distance from top of page).
    pub bottom: f64,
    /// Edge orientation.
    pub orientation: Orientation,
    /// Where this edge was derived from.
    pub source: EdgeSource,
}

/// Derive an Edge from a Line (direct conversion).
pub fn edge_from_line(line: &Line) -> Edge {
    Edge {
        x0: line.x0,
        top: line.top,
        x1: line.x1,
        bottom: line.bottom,
        orientation: line.orientation,
        source: EdgeSource::Line,
    }
}

/// Derive 4 Edges from a Rect (top, bottom, left, right).
pub fn edges_from_rect(rect: &Rect) -> Vec<Edge> {
    vec![
        Edge {
            x0: rect.x0,
            top: rect.top,
            x1: rect.x1,
            bottom: rect.top,
            orientation: Orientation::Horizontal,
            source: EdgeSource::RectTop,
        },
        Edge {
            x0: rect.x0,
            top: rect.bottom,
            x1: rect.x1,
            bottom: rect.bottom,
            orientation: Orientation::Horizontal,
            source: EdgeSource::RectBottom,
        },
        Edge {
            x0: rect.x0,
            top: rect.top,
            x1: rect.x0,
            bottom: rect.bottom,
            orientation: Orientation::Vertical,
            source: EdgeSource::RectLeft,
        },
        Edge {
            x0: rect.x1,
            top: rect.top,
            x1: rect.x1,
            bottom: rect.bottom,
            orientation: Orientation::Vertical,
            source: EdgeSource::RectRight,
        },
    ]
}

/// Tolerance for floating-point comparison when classifying edge orientation.
const EDGE_AXIS_TOLERANCE: f64 = 1e-6;

/// Classify orientation for an edge from two points.
fn classify_edge_orientation(x0: f64, y0: f64, x1: f64, y1: f64) -> Orientation {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    if dy < EDGE_AXIS_TOLERANCE {
        Orientation::Horizontal
    } else if dx < EDGE_AXIS_TOLERANCE {
        Orientation::Vertical
    } else {
        Orientation::Diagonal
    }
}

/// Derive an Edge from a Curve using chord approximation (start to end).
pub fn edge_from_curve(curve: &Curve) -> Edge {
    let (start_x, start_y) = curve.pts[0];
    let (end_x, end_y) = curve.pts[curve.pts.len() - 1];

    let x0 = start_x.min(end_x);
    let x1 = start_x.max(end_x);
    let top = start_y.min(end_y);
    let bottom = start_y.max(end_y);
    let orientation = classify_edge_orientation(start_x, start_y, end_x, end_y);

    Edge {
        x0,
        top,
        x1,
        bottom,
        orientation,
        source: EdgeSource::Curve,
    }
}

/// Derive all edges from collections of lines, rects, and curves.
pub fn derive_edges(lines: &[Line], rects: &[Rect], curves: &[Curve]) -> Vec<Edge> {
    let mut edges = Vec::new();

    for line in lines {
        edges.push(edge_from_line(line));
    }

    for rect in rects {
        edges.extend(edges_from_rect(rect));
    }

    for curve in curves {
        edges.push(edge_from_curve(curve));
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::painting::Color;

    #[test]
    fn test_edge_construction_and_field_access() {
        let edge = Edge {
            x0: 10.0,
            top: 20.0,
            x1: 200.0,
            bottom: 20.0,
            orientation: Orientation::Horizontal,
            source: EdgeSource::Line,
        };
        assert_eq!(edge.x0, 10.0);
        assert_eq!(edge.top, 20.0);
        assert_eq!(edge.x1, 200.0);
        assert_eq!(edge.bottom, 20.0);
        assert_eq!(edge.orientation, Orientation::Horizontal);
        assert_eq!(edge.source, EdgeSource::Line);
    }

    #[test]
    fn test_edge_source_variants() {
        let sources = [
            EdgeSource::Line,
            EdgeSource::RectTop,
            EdgeSource::RectBottom,
            EdgeSource::RectLeft,
            EdgeSource::RectRight,
            EdgeSource::Curve,
        ];
        // EdgeSource derives Copy
        for source in sources {
            let copy = source;
            assert_eq!(source, copy);
        }
    }

    fn assert_approx(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6,
            "expected {b}, got {a}, diff={}",
            (a - b).abs()
        );
    }

    fn make_line(x0: f64, top: f64, x1: f64, bottom: f64, orient: Orientation) -> Line {
        Line {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            stroke_color: Color::black(),
            orientation: orient,
        }
    }

    fn make_rect(x0: f64, top: f64, x1: f64, bottom: f64) -> Rect {
        Rect {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        }
    }

    fn make_curve(pts: Vec<(f64, f64)>) -> Curve {
        let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        Curve {
            x0: xs.iter().cloned().fold(f64::INFINITY, f64::min),
            top: ys.iter().cloned().fold(f64::INFINITY, f64::min),
            x1: xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            bottom: ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            pts,
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        }
    }

    // --- Edge from Line ---

    #[test]
    fn test_edge_from_horizontal_line() {
        let line = make_line(10.0, 50.0, 100.0, 50.0, Orientation::Horizontal);
        let edge = edge_from_line(&line);

        assert_approx(edge.x0, 10.0);
        assert_approx(edge.top, 50.0);
        assert_approx(edge.x1, 100.0);
        assert_approx(edge.bottom, 50.0);
        assert_eq!(edge.orientation, Orientation::Horizontal);
        assert_eq!(edge.source, EdgeSource::Line);
    }

    #[test]
    fn test_edge_from_vertical_line() {
        let line = make_line(50.0, 10.0, 50.0, 200.0, Orientation::Vertical);
        let edge = edge_from_line(&line);

        assert_approx(edge.x0, 50.0);
        assert_approx(edge.top, 10.0);
        assert_approx(edge.x1, 50.0);
        assert_approx(edge.bottom, 200.0);
        assert_eq!(edge.orientation, Orientation::Vertical);
        assert_eq!(edge.source, EdgeSource::Line);
    }

    #[test]
    fn test_edge_from_diagonal_line() {
        let line = make_line(10.0, 20.0, 100.0, 200.0, Orientation::Diagonal);
        let edge = edge_from_line(&line);

        assert_eq!(edge.orientation, Orientation::Diagonal);
        assert_eq!(edge.source, EdgeSource::Line);
    }

    // --- Edges from Rect ---

    #[test]
    fn test_edges_from_rect_count() {
        let rect = make_rect(10.0, 20.0, 110.0, 70.0);
        let edges = edges_from_rect(&rect);
        assert_eq!(edges.len(), 4);
    }

    #[test]
    fn test_edges_from_rect_top() {
        let rect = make_rect(10.0, 20.0, 110.0, 70.0);
        let edges = edges_from_rect(&rect);
        let top_edge = &edges[0];

        assert_approx(top_edge.x0, 10.0);
        assert_approx(top_edge.top, 20.0);
        assert_approx(top_edge.x1, 110.0);
        assert_approx(top_edge.bottom, 20.0);
        assert_eq!(top_edge.orientation, Orientation::Horizontal);
        assert_eq!(top_edge.source, EdgeSource::RectTop);
    }

    #[test]
    fn test_edges_from_rect_bottom() {
        let rect = make_rect(10.0, 20.0, 110.0, 70.0);
        let edges = edges_from_rect(&rect);
        let bottom_edge = &edges[1];

        assert_approx(bottom_edge.x0, 10.0);
        assert_approx(bottom_edge.top, 70.0);
        assert_approx(bottom_edge.x1, 110.0);
        assert_approx(bottom_edge.bottom, 70.0);
        assert_eq!(bottom_edge.orientation, Orientation::Horizontal);
        assert_eq!(bottom_edge.source, EdgeSource::RectBottom);
    }

    #[test]
    fn test_edges_from_rect_left() {
        let rect = make_rect(10.0, 20.0, 110.0, 70.0);
        let edges = edges_from_rect(&rect);
        let left_edge = &edges[2];

        assert_approx(left_edge.x0, 10.0);
        assert_approx(left_edge.top, 20.0);
        assert_approx(left_edge.x1, 10.0);
        assert_approx(left_edge.bottom, 70.0);
        assert_eq!(left_edge.orientation, Orientation::Vertical);
        assert_eq!(left_edge.source, EdgeSource::RectLeft);
    }

    #[test]
    fn test_edges_from_rect_right() {
        let rect = make_rect(10.0, 20.0, 110.0, 70.0);
        let edges = edges_from_rect(&rect);
        let right_edge = &edges[3];

        assert_approx(right_edge.x0, 110.0);
        assert_approx(right_edge.top, 20.0);
        assert_approx(right_edge.x1, 110.0);
        assert_approx(right_edge.bottom, 70.0);
        assert_eq!(right_edge.orientation, Orientation::Vertical);
        assert_eq!(right_edge.source, EdgeSource::RectRight);
    }

    // --- Edge from Curve (chord approximation) ---

    #[test]
    fn test_edge_from_curve_horizontal_chord() {
        // Curve from (0, 100) to (100, 100) — chord is horizontal
        let curve = make_curve(vec![
            (0.0, 100.0),
            (10.0, 50.0),
            (90.0, 50.0),
            (100.0, 100.0),
        ]);
        let edge = edge_from_curve(&curve);

        assert_approx(edge.x0, 0.0);
        assert_approx(edge.x1, 100.0);
        assert_approx(edge.top, 100.0);
        assert_approx(edge.bottom, 100.0);
        assert_eq!(edge.orientation, Orientation::Horizontal);
        assert_eq!(edge.source, EdgeSource::Curve);
    }

    #[test]
    fn test_edge_from_curve_vertical_chord() {
        // Curve from (50, 0) to (50, 100) — chord is vertical
        let curve = make_curve(vec![
            (50.0, 0.0),
            (100.0, 30.0),
            (100.0, 70.0),
            (50.0, 100.0),
        ]);
        let edge = edge_from_curve(&curve);

        assert_approx(edge.x0, 50.0);
        assert_approx(edge.x1, 50.0);
        assert_approx(edge.top, 0.0);
        assert_approx(edge.bottom, 100.0);
        assert_eq!(edge.orientation, Orientation::Vertical);
        assert_eq!(edge.source, EdgeSource::Curve);
    }

    #[test]
    fn test_edge_from_curve_diagonal_chord() {
        // Curve from (0, 0) to (100, 100) — chord is diagonal
        let curve = make_curve(vec![(0.0, 0.0), (30.0, 70.0), (70.0, 30.0), (100.0, 100.0)]);
        let edge = edge_from_curve(&curve);

        assert_approx(edge.x0, 0.0);
        assert_approx(edge.x1, 100.0);
        assert_approx(edge.top, 0.0);
        assert_approx(edge.bottom, 100.0);
        assert_eq!(edge.orientation, Orientation::Diagonal);
        assert_eq!(edge.source, EdgeSource::Curve);
    }

    // --- derive_edges (combined) ---

    #[test]
    fn test_derive_edges_empty_inputs() {
        let edges = derive_edges(&[], &[], &[]);
        assert!(edges.is_empty());
    }

    #[test]
    fn test_derive_edges_lines_only() {
        let lines = vec![
            make_line(0.0, 50.0, 100.0, 50.0, Orientation::Horizontal),
            make_line(50.0, 0.0, 50.0, 100.0, Orientation::Vertical),
        ];
        let edges = derive_edges(&lines, &[], &[]);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].source, EdgeSource::Line);
        assert_eq!(edges[1].source, EdgeSource::Line);
    }

    #[test]
    fn test_derive_edges_rects_only() {
        let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
        let edges = derive_edges(&[], &rects, &[]);
        assert_eq!(edges.len(), 4); // 4 edges per rect
    }

    #[test]
    fn test_derive_edges_curves_only() {
        let curves = vec![make_curve(vec![
            (0.0, 100.0),
            (10.0, 50.0),
            (90.0, 50.0),
            (100.0, 100.0),
        ])];
        let edges = derive_edges(&[], &[], &curves);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, EdgeSource::Curve);
    }

    #[test]
    fn test_derive_edges_mixed() {
        let lines = vec![make_line(0.0, 50.0, 100.0, 50.0, Orientation::Horizontal)];
        let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
        let curves = vec![make_curve(vec![
            (0.0, 100.0),
            (10.0, 50.0),
            (90.0, 50.0),
            (100.0, 100.0),
        ])];
        let edges = derive_edges(&lines, &rects, &curves);
        // 1 from line + 4 from rect + 1 from curve = 6
        assert_eq!(edges.len(), 6);
        assert_eq!(edges[0].source, EdgeSource::Line);
        assert_eq!(edges[1].source, EdgeSource::RectTop);
        assert_eq!(edges[4].source, EdgeSource::RectRight);
        assert_eq!(edges[5].source, EdgeSource::Curve);
    }

    #[test]
    fn test_derive_edges_multiple_rects() {
        let rects = vec![
            make_rect(10.0, 20.0, 110.0, 70.0),
            make_rect(200.0, 300.0, 350.0, 400.0),
        ];
        let edges = derive_edges(&[], &rects, &[]);
        assert_eq!(edges.len(), 8); // 4 edges per rect × 2
    }
}
