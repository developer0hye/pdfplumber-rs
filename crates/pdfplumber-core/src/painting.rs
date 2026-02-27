//! Path painting operators and types.
//!
//! Implements PDF path painting operators (S, s, f, F, f*, B, B*, b, b*, n)
//! that determine how constructed paths are rendered.

use crate::path::{Path, PathBuilder};

/// Simple RGB color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color {
    /// Create a new RGB color with values in [0.0, 1.0].
    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// Black color (0, 0, 0).
    pub fn black() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

/// Fill rule for path painting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FillRule {
    /// Nonzero winding number rule (default).
    #[default]
    NonZeroWinding,
    /// Even-odd rule.
    EvenOdd,
}

/// Graphics state relevant to path painting.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsState {
    /// Current line width (default: 1.0 per PDF spec).
    pub line_width: f64,
    /// Current stroking color.
    pub stroke_color: Color,
    /// Current non-stroking (fill) color.
    pub fill_color: Color,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        }
    }
}

/// A painted path — the result of a painting operator applied to a constructed path.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintedPath {
    /// The path segments.
    pub path: Path,
    /// Whether the path is stroked.
    pub stroke: bool,
    /// Whether the path is filled.
    pub fill: bool,
    /// Fill rule used (only meaningful when `fill` is true).
    pub fill_rule: FillRule,
    /// Line width at the time of painting.
    pub line_width: f64,
    /// Stroking color at the time of painting.
    pub stroke_color: Color,
    /// Fill color at the time of painting.
    pub fill_color: Color,
}

impl PathBuilder {
    /// `S` operator: stroke the current path.
    ///
    /// Paints the path outline using the current stroking color and line width.
    /// Clears the current path after painting.
    pub fn stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke: true,
            fill: false,
            fill_rule: FillRule::NonZeroWinding,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color,
            fill_color: gs.fill_color,
        }
    }

    /// `s` operator: close the current subpath, then stroke.
    ///
    /// Equivalent to `h S`.
    pub fn close_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.stroke(gs)
    }

    /// `f` or `F` operator: fill the current path using the nonzero winding rule.
    ///
    /// Any open subpaths are implicitly closed before filling.
    pub fn fill(&mut self, gs: &GraphicsState) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke: false,
            fill: true,
            fill_rule: FillRule::NonZeroWinding,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color,
            fill_color: gs.fill_color,
        }
    }

    /// `f*` operator: fill the current path using the even-odd rule.
    pub fn fill_even_odd(&mut self, gs: &GraphicsState) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke: false,
            fill: true,
            fill_rule: FillRule::EvenOdd,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color,
            fill_color: gs.fill_color,
        }
    }

    /// `B` operator: fill then stroke the current path (nonzero winding).
    pub fn fill_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke: true,
            fill: true,
            fill_rule: FillRule::NonZeroWinding,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color,
            fill_color: gs.fill_color,
        }
    }

    /// `B*` operator: fill (even-odd) then stroke the current path.
    pub fn fill_even_odd_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke: true,
            fill: true,
            fill_rule: FillRule::EvenOdd,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color,
            fill_color: gs.fill_color,
        }
    }

    /// `b` operator: close, fill (nonzero winding), then stroke.
    ///
    /// Equivalent to `h B`.
    pub fn close_fill_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.fill_and_stroke(gs)
    }

    /// `b*` operator: close, fill (even-odd), then stroke.
    ///
    /// Equivalent to `h B*`.
    pub fn close_fill_even_odd_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.fill_even_odd_and_stroke(gs)
    }

    /// `n` operator: end the path without painting.
    ///
    /// Discards the current path. Used primarily for clipping paths.
    /// Returns `None` since no painted path is produced.
    pub fn end_path(&mut self) -> Option<PaintedPath> {
        self.take_path();
        None
    }

    /// Take the current path segments and reset the builder for the next path.
    fn take_path(&mut self) -> Path {
        self.take_and_reset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Ctm, Point};
    use crate::path::PathSegment;

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

    fn build_triangle(builder: &mut PathBuilder) {
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 0.0);
        builder.line_to(50.0, 80.0);
    }

    fn build_rectangle(builder: &mut PathBuilder) {
        builder.rectangle(10.0, 20.0, 100.0, 50.0);
    }

    // --- Color tests ---

    #[test]
    fn test_color_new() {
        let c = Color::new(0.5, 0.6, 0.7);
        assert_eq!(c.r, 0.5);
        assert_eq!(c.g, 0.6);
        assert_eq!(c.b, 0.7);
    }

    #[test]
    fn test_color_black() {
        let c = Color::black();
        assert_eq!(c.r, 0.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn test_color_default_is_black() {
        assert_eq!(Color::default(), Color::black());
    }

    // --- FillRule tests ---

    #[test]
    fn test_fill_rule_default() {
        assert_eq!(FillRule::default(), FillRule::NonZeroWinding);
    }

    // --- GraphicsState tests ---

    #[test]
    fn test_graphics_state_default() {
        let gs = GraphicsState::default();
        assert_eq!(gs.line_width, 1.0);
        assert_eq!(gs.stroke_color, Color::black());
        assert_eq!(gs.fill_color, Color::black());
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
        assert_eq!(painted.stroke_color, Color::new(1.0, 0.0, 0.0));
        assert_eq!(painted.fill_color, Color::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_stroke_clears_builder() {
        let mut builder = PathBuilder::new(Ctm::identity());
        build_triangle(&mut builder);
        let _ = builder.stroke(&default_gs());

        assert!(builder.is_empty());
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
        assert_eq!(painted.stroke_color, Color::new(1.0, 0.0, 0.0));
        assert_eq!(painted.fill_color, Color::new(0.0, 0.0, 1.0));
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
            stroke_color: Color::new(1.0, 0.0, 0.0),
            fill_color: Color::black(),
        };
        let first = builder.stroke(&gs1);

        // Second paint with different graphics state
        builder.move_to(10.0, 10.0);
        builder.line_to(60.0, 60.0);
        let gs2 = GraphicsState {
            line_width: 3.0,
            stroke_color: Color::new(0.0, 1.0, 0.0),
            fill_color: Color::black(),
        };
        let second = builder.stroke(&gs2);

        // Each painted path should have its own state
        assert_eq!(first.line_width, 1.0);
        assert_eq!(first.stroke_color, Color::new(1.0, 0.0, 0.0));
        assert_eq!(second.line_width, 3.0);
        assert_eq!(second.stroke_color, Color::new(0.0, 1.0, 0.0));
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
}
