use super::*;
use crate::geometry::{Ctm, Point};
use crate::path::PathSegment;

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

fn build_triangle(builder: &mut PathBuilder) {
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);
    builder.line_to(50.0, 80.0);
}

fn build_rectangle(builder: &mut PathBuilder) {
    builder.rectangle(10.0, 20.0, 100.0, 50.0);
}

// --- Color::to_rgb tests ---

#[test]
fn test_gray_to_rgb() {
    let c = Color::Gray(0.5);
    assert_eq!(c.to_rgb(), Some((0.5, 0.5, 0.5)));
}

#[test]
fn test_gray_black_to_rgb() {
    let c = Color::Gray(0.0);
    assert_eq!(c.to_rgb(), Some((0.0, 0.0, 0.0)));
}

#[test]
fn test_gray_white_to_rgb() {
    let c = Color::Gray(1.0);
    assert_eq!(c.to_rgb(), Some((1.0, 1.0, 1.0)));
}

#[test]
fn test_rgb_to_rgb_identity() {
    let c = Color::Rgb(0.2, 0.4, 0.6);
    assert_eq!(c.to_rgb(), Some((0.2, 0.4, 0.6)));
}

#[test]
fn test_cmyk_to_rgb() {
    // Pure cyan: C=1, M=0, Y=0, K=0 → R=0, G=1, B=1
    let c = Color::Cmyk(1.0, 0.0, 0.0, 0.0);
    let (r, g, b) = c.to_rgb().unwrap();
    assert!((r - 0.0).abs() < 0.01, "r={r}");
    assert!((g - 1.0).abs() < 0.01, "g={g}");
    assert!((b - 1.0).abs() < 0.01, "b={b}");
}

#[test]
fn test_cmyk_black_to_rgb() {
    // K=1 → all black
    let c = Color::Cmyk(0.0, 0.0, 0.0, 1.0);
    let (r, g, b) = c.to_rgb().unwrap();
    assert!((r - 0.0).abs() < 0.01, "r={r}");
    assert!((g - 0.0).abs() < 0.01, "g={g}");
    assert!((b - 0.0).abs() < 0.01, "b={b}");
}

#[test]
fn test_cmyk_white_to_rgb() {
    // All zero → white
    let c = Color::Cmyk(0.0, 0.0, 0.0, 0.0);
    let (r, g, b) = c.to_rgb().unwrap();
    assert!((r - 1.0).abs() < 0.01, "r={r}");
    assert!((g - 1.0).abs() < 0.01, "g={g}");
    assert!((b - 1.0).abs() < 0.01, "b={b}");
}

#[test]
fn test_other_to_rgb_returns_none() {
    let c = Color::Other(vec![0.1, 0.2]);
    assert_eq!(c.to_rgb(), None);
}

// --- Color tests ---

#[test]
fn test_color_gray() {
    let c = Color::Gray(0.5);
    assert_eq!(c, Color::Gray(0.5));
}

#[test]
fn test_color_rgb() {
    let c = Color::Rgb(0.5, 0.6, 0.7);
    assert_eq!(c, Color::Rgb(0.5, 0.6, 0.7));
}

#[test]
fn test_color_cmyk() {
    let c = Color::Cmyk(0.1, 0.2, 0.3, 0.4);
    assert_eq!(c, Color::Cmyk(0.1, 0.2, 0.3, 0.4));
}

#[test]
fn test_color_other() {
    let c = Color::Other(vec![0.1, 0.2, 0.3, 0.4, 0.5]);
    if let Color::Other(ref v) = c {
        assert_eq!(v.len(), 5);
    } else {
        panic!("expected Color::Other");
    }
}

#[test]
fn test_color_black() {
    let c = Color::black();
    assert_eq!(c, Color::Gray(0.0));
}

#[test]
fn test_color_default_is_black() {
    assert_eq!(Color::default(), Color::black());
}

#[test]
fn test_color_clone() {
    let c = Color::Rgb(1.0, 0.0, 0.0);
    let c2 = c.clone();
    assert_eq!(c, c2);
}

// --- FillRule tests ---

#[test]
fn test_fill_rule_default() {
    assert_eq!(FillRule::default(), FillRule::NonZeroWinding);
}

// --- DashPattern tests ---

#[test]
fn test_dash_pattern_solid() {
    let dp = DashPattern::solid();
    assert!(dp.dash_array.is_empty());
    assert_eq!(dp.dash_phase, 0.0);
    assert!(dp.is_solid());
}

#[test]
fn test_dash_pattern_default_is_solid() {
    assert_eq!(DashPattern::default(), DashPattern::solid());
}

#[test]
fn test_dash_pattern_new() {
    let dp = DashPattern::new(vec![3.0, 2.0], 1.0);
    assert_eq!(dp.dash_array, vec![3.0, 2.0]);
    assert_eq!(dp.dash_phase, 1.0);
    assert!(!dp.is_solid());
}

#[test]
fn test_dash_pattern_complex() {
    let dp = DashPattern::new(vec![5.0, 2.0, 1.0, 2.0], 0.0);
    assert_eq!(dp.dash_array.len(), 4);
    assert!(!dp.is_solid());
}

// --- GraphicsState tests ---

#[test]
fn test_graphics_state_default() {
    let gs = GraphicsState::default();
    assert_eq!(gs.line_width, 1.0);
    assert_eq!(gs.stroke_color, Color::black());
    assert_eq!(gs.fill_color, Color::black());
    assert!(gs.dash_pattern.is_solid());
    assert_eq!(gs.stroke_alpha, 1.0);
    assert_eq!(gs.fill_alpha, 1.0);
}

#[test]
fn test_set_dash_pattern() {
    let mut gs = GraphicsState::default();
    gs.set_dash_pattern(vec![4.0, 2.0], 0.5);

    assert_eq!(gs.dash_pattern.dash_array, vec![4.0, 2.0]);
    assert_eq!(gs.dash_pattern.dash_phase, 0.5);
    assert!(!gs.dash_pattern.is_solid());
}

#[test]
fn test_set_dash_pattern_back_to_solid() {
    let mut gs = GraphicsState::default();
    gs.set_dash_pattern(vec![4.0, 2.0], 0.5);
    assert!(!gs.dash_pattern.is_solid());

    gs.set_dash_pattern(vec![], 0.0);
    assert!(gs.dash_pattern.is_solid());
}

// --- ExtGState tests ---

#[test]
fn test_ext_gstate_default_is_all_none() {
    let ext = ExtGState::default();
    assert!(ext.line_width.is_none());
    assert!(ext.dash_pattern.is_none());
    assert!(ext.stroke_alpha.is_none());
    assert!(ext.fill_alpha.is_none());
    assert!(ext.font.is_none());
}

#[test]
fn test_apply_ext_gstate_line_width() {
    let mut gs = GraphicsState::default();
    assert_eq!(gs.line_width, 1.0);

    let ext = ExtGState {
        line_width: Some(3.5),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.line_width, 3.5);
    // Other fields unchanged
    assert!(gs.dash_pattern.is_solid());
    assert_eq!(gs.stroke_alpha, 1.0);
    assert_eq!(gs.fill_alpha, 1.0);
}

#[test]
fn test_apply_ext_gstate_dash_pattern() {
    let mut gs = GraphicsState::default();
    assert!(gs.dash_pattern.is_solid());

    let ext = ExtGState {
        dash_pattern: Some(DashPattern::new(vec![6.0, 3.0], 0.0)),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.dash_pattern.dash_array, vec![6.0, 3.0]);
    assert_eq!(gs.dash_pattern.dash_phase, 0.0);
    // Other fields unchanged
    assert_eq!(gs.line_width, 1.0);
}

#[test]
fn test_apply_ext_gstate_stroke_alpha() {
    let mut gs = GraphicsState::default();
    assert_eq!(gs.stroke_alpha, 1.0);

    let ext = ExtGState {
        stroke_alpha: Some(0.5),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.stroke_alpha, 0.5);
    assert_eq!(gs.fill_alpha, 1.0); // unchanged
}

#[test]
fn test_apply_ext_gstate_fill_alpha() {
    let mut gs = GraphicsState::default();
    assert_eq!(gs.fill_alpha, 1.0);

    let ext = ExtGState {
        fill_alpha: Some(0.75),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.fill_alpha, 0.75);
    assert_eq!(gs.stroke_alpha, 1.0); // unchanged
}

#[test]
fn test_apply_ext_gstate_multiple_fields() {
    let mut gs = GraphicsState::default();

    let ext = ExtGState {
        line_width: Some(2.0),
        dash_pattern: Some(DashPattern::new(vec![1.0, 1.0], 0.0)),
        stroke_alpha: Some(0.8),
        fill_alpha: Some(0.6),
        font: Some(("Helvetica".to_string(), 14.0)),
    };
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.line_width, 2.0);
    assert_eq!(gs.dash_pattern.dash_array, vec![1.0, 1.0]);
    assert_eq!(gs.stroke_alpha, 0.8);
    assert_eq!(gs.fill_alpha, 0.6);
    // Font is stored in ExtGState but not in GraphicsState directly
    // (it's a text state concern — callers read ext.font separately)
}

#[test]
fn test_apply_ext_gstate_none_fields_preserve_state() {
    let mut gs = GraphicsState {
        line_width: 5.0,
        stroke_alpha: 0.3,
        fill_alpha: 0.4,
        dash_pattern: DashPattern::new(vec![2.0], 0.0),
        ..GraphicsState::default()
    };

    // Apply empty ExtGState — nothing should change
    let ext = ExtGState::default();
    gs.apply_ext_gstate(&ext);

    assert_eq!(gs.line_width, 5.0);
    assert_eq!(gs.stroke_alpha, 0.3);
    assert_eq!(gs.fill_alpha, 0.4);
    assert_eq!(gs.dash_pattern.dash_array, vec![2.0]);
}

#[test]
fn test_apply_ext_gstate_sequential() {
    let mut gs = GraphicsState::default();

    // First ExtGState: set line width
    let ext1 = ExtGState {
        line_width: Some(2.0),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext1);
    assert_eq!(gs.line_width, 2.0);

    // Second ExtGState: set alpha, line width stays
    let ext2 = ExtGState {
        stroke_alpha: Some(0.5),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext2);
    assert_eq!(gs.line_width, 2.0); // preserved
    assert_eq!(gs.stroke_alpha, 0.5);
}

#[test]
fn test_ext_gstate_font_override() {
    let ext = ExtGState {
        font: Some(("CourierNew".to_string(), 10.0)),
        ..ExtGState::default()
    };
    let (name, size) = ext.font.as_ref().unwrap();
    assert_eq!(name, "CourierNew");
    assert_eq!(*size, 10.0);
}

// --- S operator (stroke) ---

#[test]
fn test_stroke_produces_stroke_only() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.stroke(&default_gs());
    assert!(painted.stroke);
    assert!(!painted.fill);
    assert_eq!(painted.fill_rule, FillRule::NonZeroWinding);
}

#[test]
fn test_stroke_captures_path_segments() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.stroke(&default_gs());
    assert_eq!(painted.path.segments.len(), 3); // moveto + 2 lineto
    assert_eq!(
        painted.path.segments[0],
        PathSegment::MoveTo(Point::new(0.0, 0.0))
    );
}

#[test]
fn test_stroke_captures_graphics_state() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let gs = custom_gs();
    let painted = builder.stroke(&gs);
    assert_eq!(painted.line_width, 2.5);
    assert_eq!(painted.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
    assert_eq!(painted.fill_color, Color::Rgb(0.0, 0.0, 1.0));
}

#[test]
fn test_stroke_clears_builder() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);
    let _ = builder.stroke(&default_gs());

    assert!(builder.is_empty());
}

// --- Stroke captures dash pattern and alpha ---

#[test]
fn test_stroke_captures_dash_pattern() {
    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let gs = GraphicsState {
        dash_pattern: DashPattern::new(vec![5.0, 3.0], 1.0),
        ..GraphicsState::default()
    };
    let painted = builder.stroke(&gs);
    assert_eq!(painted.dash_pattern.dash_array, vec![5.0, 3.0]);
    assert_eq!(painted.dash_pattern.dash_phase, 1.0);
}

#[test]
fn test_stroke_captures_alpha() {
    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let gs = GraphicsState {
        stroke_alpha: 0.7,
        fill_alpha: 0.3,
        ..GraphicsState::default()
    };
    let painted = builder.stroke(&gs);
    assert_eq!(painted.stroke_alpha, 0.7);
    assert_eq!(painted.fill_alpha, 0.3);
}

#[test]
fn test_stroke_default_gs_has_solid_dash_and_full_alpha() {
    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let painted = builder.stroke(&default_gs());
    assert!(painted.dash_pattern.is_solid());
    assert_eq!(painted.stroke_alpha, 1.0);
    assert_eq!(painted.fill_alpha, 1.0);
}

// --- s operator (close + stroke) ---

#[test]
fn test_close_and_stroke_includes_closepath() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.close_and_stroke(&default_gs());
    assert!(painted.stroke);
    assert!(!painted.fill);
    // Should have: moveto + 2 lineto + closepath = 4 segments
    assert_eq!(painted.path.segments.len(), 4);
    assert_eq!(painted.path.segments[3], PathSegment::ClosePath);
}

// --- f/F operator (fill, nonzero winding) ---

#[test]
fn test_fill_produces_fill_only() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.fill(&default_gs());
    assert!(!painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::NonZeroWinding);
}

#[test]
fn test_fill_captures_path() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_rectangle(&mut builder);

    let painted = builder.fill(&default_gs());
    assert_eq!(painted.path.segments.len(), 5); // moveto + 3 lineto + closepath
}

#[test]
fn test_fill_clears_builder() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);
    let _ = builder.fill(&default_gs());

    assert!(builder.is_empty());
}

// --- f* operator (fill, even-odd) ---

#[test]
fn test_fill_even_odd_uses_even_odd_rule() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.fill_even_odd(&default_gs());
    assert!(!painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::EvenOdd);
}

// --- B operator (fill + stroke) ---

#[test]
fn test_fill_and_stroke() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.fill_and_stroke(&default_gs());
    assert!(painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::NonZeroWinding);
}

#[test]
fn test_fill_and_stroke_captures_custom_gs() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let gs = custom_gs();
    let painted = builder.fill_and_stroke(&gs);
    assert_eq!(painted.line_width, 2.5);
    assert_eq!(painted.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
    assert_eq!(painted.fill_color, Color::Rgb(0.0, 0.0, 1.0));
}

// --- B* operator (fill even-odd + stroke) ---

#[test]
fn test_fill_even_odd_and_stroke() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.fill_even_odd_and_stroke(&default_gs());
    assert!(painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::EvenOdd);
}

// --- b operator (close + fill + stroke) ---

#[test]
fn test_close_fill_and_stroke() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.close_fill_and_stroke(&default_gs());
    assert!(painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::NonZeroWinding);
    // Should have closepath
    assert_eq!(painted.path.segments.len(), 4);
    assert_eq!(painted.path.segments[3], PathSegment::ClosePath);
}

// --- b* operator (close + fill even-odd + stroke) ---

#[test]
fn test_close_fill_even_odd_and_stroke() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let painted = builder.close_fill_even_odd_and_stroke(&default_gs());
    assert!(painted.stroke);
    assert!(painted.fill);
    assert_eq!(painted.fill_rule, FillRule::EvenOdd);
    // Should have closepath
    assert_eq!(painted.path.segments.len(), 4);
    assert_eq!(painted.path.segments[3], PathSegment::ClosePath);
}

// --- n operator (end path, no painting) ---

#[test]
fn test_end_path_returns_none() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);

    let result = builder.end_path();
    assert!(result.is_none());
}

#[test]
fn test_end_path_clears_builder() {
    let mut builder = PathBuilder::new(Ctm::identity());
    build_triangle(&mut builder);
    let _ = builder.end_path();

    assert!(builder.is_empty());
}

// --- Sequential painting operations ---

#[test]
fn test_paint_then_build_new_path() {
    let mut builder = PathBuilder::new(Ctm::identity());

    // First path: stroke a line
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);
    let first = builder.stroke(&default_gs());
    assert_eq!(first.path.segments.len(), 2);

    // Second path: fill a rectangle
    build_rectangle(&mut builder);
    let second = builder.fill(&default_gs());
    assert_eq!(second.path.segments.len(), 5);
    assert!(second.fill);
    assert!(!second.stroke);
}

#[test]
fn test_multiple_paints_independent() {
    let mut builder = PathBuilder::new(Ctm::identity());

    // First paint with one graphics state
    builder.move_to(0.0, 0.0);
    builder.line_to(50.0, 50.0);
    let gs1 = GraphicsState {
        line_width: 1.0,
        stroke_color: Color::Rgb(1.0, 0.0, 0.0),
        fill_color: Color::black(),
        ..GraphicsState::default()
    };
    let first = builder.stroke(&gs1);

    // Second paint with different graphics state
    builder.move_to(10.0, 10.0);
    builder.line_to(60.0, 60.0);
    let gs2 = GraphicsState {
        line_width: 3.0,
        stroke_color: Color::Rgb(0.0, 1.0, 0.0),
        fill_color: Color::black(),
        ..GraphicsState::default()
    };
    let second = builder.stroke(&gs2);

    // Each painted path should have its own state
    assert_eq!(first.line_width, 1.0);
    assert_eq!(first.stroke_color, Color::Rgb(1.0, 0.0, 0.0));
    assert_eq!(second.line_width, 3.0);
    assert_eq!(second.stroke_color, Color::Rgb(0.0, 1.0, 0.0));
}

// --- Painting with CTM-transformed paths ---

#[test]
fn test_stroke_with_ctm_transformed_path() {
    let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 10.0, 10.0);
    let mut builder = PathBuilder::new(ctm);
    builder.move_to(0.0, 0.0);
    builder.line_to(50.0, 0.0);

    let painted = builder.stroke(&default_gs());
    // Coordinates should already be CTM-transformed
    assert_eq!(
        painted.path.segments[0],
        PathSegment::MoveTo(Point::new(10.0, 10.0))
    );
    assert_eq!(
        painted.path.segments[1],
        PathSegment::LineTo(Point::new(110.0, 10.0))
    );
}

// --- Painting with curves ---

#[test]
fn test_stroke_path_with_curves() {
    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.curve_to(10.0, 20.0, 30.0, 40.0, 50.0, 0.0);

    let painted = builder.stroke(&default_gs());
    assert_eq!(painted.path.segments.len(), 2);
    assert_eq!(
        painted.path.segments[1],
        PathSegment::CurveTo {
            cp1: Point::new(10.0, 20.0),
            cp2: Point::new(30.0, 40.0),
            end: Point::new(50.0, 0.0),
        }
    );
    assert!(painted.stroke);
}

#[test]
fn test_fill_path_with_curves() {
    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.curve_to(10.0, 50.0, 90.0, 50.0, 100.0, 0.0);
    builder.close_path();

    let painted = builder.fill(&default_gs());
    assert!(painted.fill);
    assert!(!painted.stroke);
    assert_eq!(painted.path.segments.len(), 3); // moveto + curveto + closepath
}

// --- d operator (inline dash pattern) ---

#[test]
fn test_d_operator_sets_dash() {
    let mut gs = GraphicsState::default();
    assert!(gs.dash_pattern.is_solid());

    // d operator: [3 2] 0 d
    gs.set_dash_pattern(vec![3.0, 2.0], 0.0);
    assert_eq!(gs.dash_pattern.dash_array, vec![3.0, 2.0]);
    assert_eq!(gs.dash_pattern.dash_phase, 0.0);
}

#[test]
fn test_d_operator_with_phase() {
    let mut gs = GraphicsState::default();
    gs.set_dash_pattern(vec![6.0, 3.0, 1.0, 3.0], 2.0);
    assert_eq!(gs.dash_pattern.dash_array, vec![6.0, 3.0, 1.0, 3.0]);
    assert_eq!(gs.dash_pattern.dash_phase, 2.0);
}

#[test]
fn test_d_operator_propagates_to_painted_path() {
    let mut gs = GraphicsState::default();
    gs.set_dash_pattern(vec![4.0, 2.0], 0.0);

    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let painted = builder.stroke(&gs);
    assert_eq!(painted.dash_pattern.dash_array, vec![4.0, 2.0]);
    assert!(!painted.dash_pattern.is_solid());
}

// --- gs operator (ExtGState application) scenarios ---

#[test]
fn test_gs_operator_line_width_propagates_to_paint() {
    let mut gs = GraphicsState::default();
    let ext = ExtGState {
        line_width: Some(4.0),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let painted = builder.stroke(&gs);
    assert_eq!(painted.line_width, 4.0);
}

#[test]
fn test_gs_operator_dash_propagates_to_paint() {
    let mut gs = GraphicsState::default();
    let ext = ExtGState {
        dash_pattern: Some(DashPattern::new(vec![10.0, 5.0], 0.0)),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 0.0);

    let painted = builder.stroke(&gs);
    assert_eq!(painted.dash_pattern.dash_array, vec![10.0, 5.0]);
}

#[test]
fn test_gs_operator_opacity_propagates_to_paint() {
    let mut gs = GraphicsState::default();
    let ext = ExtGState {
        stroke_alpha: Some(0.5),
        fill_alpha: Some(0.25),
        ..ExtGState::default()
    };
    gs.apply_ext_gstate(&ext);

    let mut builder = PathBuilder::new(Ctm::identity());
    builder.move_to(0.0, 0.0);
    builder.line_to(100.0, 100.0);
    builder.close_path();

    let painted = builder.fill_and_stroke(&gs);
    assert_eq!(painted.stroke_alpha, 0.5);
    assert_eq!(painted.fill_alpha, 0.25);
}
