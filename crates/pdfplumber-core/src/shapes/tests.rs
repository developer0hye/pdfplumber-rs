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
    assert_eq!(lines[0].orientation, Orientation::Horizontal);
    // Other two are diagonal
    assert_eq!(lines[1].orientation, Orientation::Diagonal);
    assert_eq!(lines[2].orientation, Orientation::Diagonal);
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
