use crate::geometry::{Ctm, Point};

/// A segment of a PDF path.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PathSegment {
    /// Move to a new point (starts a new subpath).
    MoveTo(Point),
    /// Straight line from current point to target.
    LineTo(Point),
    /// Cubic Bezier curve with two control points and an endpoint.
    CurveTo {
        /// First control point.
        cp1: Point,
        /// Second control point.
        cp2: Point,
        /// Endpoint of the curve.
        end: Point,
    },
    /// Close the current subpath (line back to the subpath start).
    ClosePath,
}

/// A complete path consisting of segments.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Path {
    /// The path segments.
    pub segments: Vec<PathSegment>,
}

/// Builder for constructing paths from PDF path operators.
///
/// Coordinates are transformed through the CTM before storage.
#[derive(Debug, Clone)]
pub struct PathBuilder {
    segments: Vec<PathSegment>,
    current_point: Option<Point>,
    subpath_start: Option<Point>,
    ctm: Ctm,
}

impl PathBuilder {
    /// Create a new PathBuilder with the given CTM.
    pub fn new(ctm: Ctm) -> Self {
        Self {
            segments: Vec::new(),
            current_point: None,
            subpath_start: None,
            ctm,
        }
    }

    /// Update the CTM.
    pub fn set_ctm(&mut self, ctm: Ctm) {
        self.ctm = ctm;
    }

    /// Get the current CTM.
    pub fn ctm(&self) -> &Ctm {
        &self.ctm
    }

    /// `m` operator: move to a new point, starting a new subpath.
    pub fn move_to(&mut self, x: f64, y: f64) {
        let p = self.ctm.transform_point(Point::new(x, y));
        self.segments.push(PathSegment::MoveTo(p));
        self.current_point = Some(p);
        self.subpath_start = Some(p);
    }

    /// `l` operator: straight line from current point to `(x, y)`.
    pub fn line_to(&mut self, x: f64, y: f64) {
        let p = self.ctm.transform_point(Point::new(x, y));
        self.segments.push(PathSegment::LineTo(p));
        self.current_point = Some(p);
    }

    /// `c` operator: cubic Bezier curve with three coordinate pairs.
    pub fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let cp1 = self.ctm.transform_point(Point::new(x1, y1));
        let cp2 = self.ctm.transform_point(Point::new(x2, y2));
        let end = self.ctm.transform_point(Point::new(x3, y3));
        self.segments.push(PathSegment::CurveTo { cp1, cp2, end });
        self.current_point = Some(end);
    }

    /// `v` operator: cubic Bezier where first control point equals current point.
    pub fn curve_to_v(&mut self, x2: f64, y2: f64, x3: f64, y3: f64) {
        let Some(cp1) = self.current_point else {
            return;
        };
        let cp2 = self.ctm.transform_point(Point::new(x2, y2));
        let end = self.ctm.transform_point(Point::new(x3, y3));
        self.segments.push(PathSegment::CurveTo { cp1, cp2, end });
        self.current_point = Some(end);
    }

    /// `y` operator: cubic Bezier where last control point equals endpoint.
    pub fn curve_to_y(&mut self, x1: f64, y1: f64, x3: f64, y3: f64) {
        let cp1 = self.ctm.transform_point(Point::new(x1, y1));
        let end = self.ctm.transform_point(Point::new(x3, y3));
        self.segments
            .push(PathSegment::CurveTo { cp1, cp2: end, end });
        self.current_point = Some(end);
    }

    /// `h` operator: close the current subpath.
    pub fn close_path(&mut self) {
        self.segments.push(PathSegment::ClosePath);
        if let Some(start) = self.subpath_start {
            self.current_point = Some(start);
        }
    }

    /// `re` operator: append a rectangle as moveto + 3 lineto + closepath.
    pub fn rectangle(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.move_to(x, y);
        self.line_to(x + width, y);
        self.line_to(x + width, y + height);
        self.line_to(x, y + height);
        self.close_path();
    }

    /// Get the current point (already CTM-transformed).
    pub fn current_point(&self) -> Option<Point> {
        self.current_point
    }

    /// Consume the builder and return the constructed path.
    pub fn build(self) -> Path {
        Path {
            segments: self.segments,
        }
    }

    /// Check if the builder has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Take the accumulated segments as a `Path` and reset the builder.
    ///
    /// After this call, the builder is empty and ready for a new path.
    /// The current point and subpath start are also reset.
    pub fn take_and_reset(&mut self) -> Path {
        let segments = std::mem::take(&mut self.segments);
        self.current_point = None;
        self.subpath_start = None;
        Path { segments }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point_approx(p: Point, x: f64, y: f64) {
        assert!((p.x - x).abs() < 1e-10, "x: expected {x}, got {}", p.x);
        assert!((p.y - y).abs() < 1e-10, "y: expected {y}, got {}", p.y);
    }

    // --- PathBuilder: empty state ---

    #[test]
    fn test_new_builder_is_empty() {
        let builder = PathBuilder::new(Ctm::identity());
        assert!(builder.is_empty());
        assert!(builder.current_point().is_none());
    }

    // --- m (moveto) ---

    #[test]
    fn test_move_to() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(10.0, 20.0);

        assert!(!builder.is_empty());
        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 10.0, 20.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 1);
        assert_eq!(
            path.segments[0],
            PathSegment::MoveTo(Point::new(10.0, 20.0))
        );
    }

    #[test]
    fn test_move_to_updates_subpath_start() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(10.0, 20.0);
        builder.line_to(30.0, 40.0);
        builder.close_path();

        // After close, current point should return to subpath start (10, 20)
        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 10.0, 20.0);
    }

    // --- l (lineto) ---

    #[test]
    fn test_line_to() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 50.0);

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 100.0, 50.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 2);
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(100.0, 50.0))
        );
    }

    // --- c (curveto) ---

    #[test]
    fn test_curve_to() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 20.0, 30.0, 40.0, 50.0, 60.0);

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 50.0, 60.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 2);
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(10.0, 20.0),
                cp2: Point::new(30.0, 40.0),
                end: Point::new(50.0, 60.0),
            }
        );
    }

    // --- v (curveto variant: first CP = current point) ---

    #[test]
    fn test_curve_to_v() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(5.0, 10.0);
        builder.curve_to_v(30.0, 40.0, 50.0, 60.0);

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 50.0, 60.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 2);
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(5.0, 10.0), // first CP = current point at time of call
                cp2: Point::new(30.0, 40.0),
                end: Point::new(50.0, 60.0),
            }
        );
    }

    #[test]
    fn test_curve_to_v_without_current_point_is_noop() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.curve_to_v(30.0, 40.0, 50.0, 60.0);

        assert!(builder.is_empty());
        assert!(builder.current_point().is_none());
    }

    // --- y (curveto variant: last CP = endpoint) ---

    #[test]
    fn test_curve_to_y() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.curve_to_y(10.0, 20.0, 50.0, 60.0);

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 50.0, 60.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 2);
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(10.0, 20.0),
                cp2: Point::new(50.0, 60.0), // last CP = endpoint
                end: Point::new(50.0, 60.0),
            }
        );
    }

    // --- h (closepath) ---

    #[test]
    fn test_close_path() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(10.0, 20.0);
        builder.line_to(30.0, 40.0);
        builder.line_to(50.0, 20.0);
        builder.close_path();

        // Current point returns to subpath start
        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 10.0, 20.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 4);
        assert_eq!(path.segments[3], PathSegment::ClosePath);
    }

    // --- re (rectangle) ---

    #[test]
    fn test_rectangle() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.rectangle(10.0, 20.0, 100.0, 50.0);

        let path = builder.build();
        // re produces: moveto + 3 lineto + closepath = 5 segments
        assert_eq!(path.segments.len(), 5);
        assert_eq!(
            path.segments[0],
            PathSegment::MoveTo(Point::new(10.0, 20.0))
        );
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(110.0, 20.0))
        );
        assert_eq!(
            path.segments[2],
            PathSegment::LineTo(Point::new(110.0, 70.0))
        );
        assert_eq!(
            path.segments[3],
            PathSegment::LineTo(Point::new(10.0, 70.0))
        );
        assert_eq!(path.segments[4], PathSegment::ClosePath);
    }

    #[test]
    fn test_rectangle_current_point_at_start() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.rectangle(10.0, 20.0, 100.0, 50.0);

        // After re + close, current point is the rectangle origin
        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 10.0, 20.0);
    }

    // --- Combined path construction ---

    #[test]
    fn test_combined_path_triangle() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 0.0);
        builder.line_to(50.0, 80.0);
        builder.close_path();

        let path = builder.build();
        assert_eq!(path.segments.len(), 4);
        assert_eq!(path.segments[0], PathSegment::MoveTo(Point::new(0.0, 0.0)));
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(100.0, 0.0))
        );
        assert_eq!(
            path.segments[2],
            PathSegment::LineTo(Point::new(50.0, 80.0))
        );
        assert_eq!(path.segments[3], PathSegment::ClosePath);
    }

    #[test]
    fn test_combined_path_with_curves() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(50.0, 0.0);
        builder.curve_to(60.0, 0.0, 70.0, 10.0, 70.0, 20.0);
        builder.line_to(70.0, 50.0);
        builder.close_path();

        let path = builder.build();
        assert_eq!(path.segments.len(), 5);
    }

    #[test]
    fn test_multiple_subpaths() {
        let mut builder = PathBuilder::new(Ctm::identity());
        // First subpath: a line
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 0.0);
        // Second subpath: another line
        builder.move_to(0.0, 50.0);
        builder.line_to(100.0, 50.0);

        let path = builder.build();
        assert_eq!(path.segments.len(), 4);

        // After second moveto, close should go back to second subpath start
    }

    #[test]
    fn test_multiple_subpaths_close_returns_to_latest_start() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(0.0, 0.0);
        builder.line_to(100.0, 100.0);
        builder.move_to(200.0, 200.0);
        builder.line_to(300.0, 300.0);
        builder.close_path();

        // Close returns to the most recent moveto (200, 200)
        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 200.0, 200.0);
    }

    // --- CTM-transformed paths ---

    #[test]
    fn test_ctm_translation_moveto() {
        // CTM translates by (100, 200)
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(10.0, 20.0);

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 110.0, 220.0);
    }

    #[test]
    fn test_ctm_scaling_lineto() {
        // CTM scales by 2x
        let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(5.0, 10.0);
        builder.line_to(15.0, 25.0);

        let path = builder.build();
        assert_eq!(
            path.segments[0],
            PathSegment::MoveTo(Point::new(10.0, 20.0))
        );
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(30.0, 50.0))
        );
    }

    #[test]
    fn test_ctm_transformed_rectangle() {
        // CTM: scale 2x + translate (10, 10)
        let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 10.0, 10.0);
        let mut builder = PathBuilder::new(ctm);
        builder.rectangle(0.0, 0.0, 50.0, 30.0);

        let path = builder.build();
        // (0,0) -> (10, 10)
        assert_eq!(
            path.segments[0],
            PathSegment::MoveTo(Point::new(10.0, 10.0))
        );
        // (50,0) -> (110, 10)
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(110.0, 10.0))
        );
        // (50,30) -> (110, 70)
        assert_eq!(
            path.segments[2],
            PathSegment::LineTo(Point::new(110.0, 70.0))
        );
        // (0,30) -> (10, 70)
        assert_eq!(
            path.segments[3],
            PathSegment::LineTo(Point::new(10.0, 70.0))
        );
        assert_eq!(path.segments[4], PathSegment::ClosePath);
    }

    #[test]
    fn test_ctm_transformed_curveto() {
        // CTM scales x by 2, y by 3
        let ctm = Ctm::new(2.0, 0.0, 0.0, 3.0, 0.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(0.0, 0.0);
        builder.curve_to(10.0, 10.0, 20.0, 20.0, 30.0, 30.0);

        let path = builder.build();
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(20.0, 30.0),
                cp2: Point::new(40.0, 60.0),
                end: Point::new(60.0, 90.0),
            }
        );
    }

    #[test]
    fn test_ctm_transformed_curve_to_v() {
        // CTM translates by (100, 0)
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(5.0, 10.0); // transformed: (105, 10)
        builder.curve_to_v(30.0, 40.0, 50.0, 60.0);

        let path = builder.build();
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(105.0, 10.0), // current point (already transformed)
                cp2: Point::new(130.0, 40.0),
                end: Point::new(150.0, 60.0),
            }
        );
    }

    #[test]
    fn test_ctm_transformed_curve_to_y() {
        // CTM scales by 0.5
        let ctm = Ctm::new(0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(0.0, 0.0);
        builder.curve_to_y(20.0, 40.0, 60.0, 80.0);

        let path = builder.build();
        assert_eq!(
            path.segments[1],
            PathSegment::CurveTo {
                cp1: Point::new(10.0, 20.0),
                cp2: Point::new(30.0, 40.0), // same as endpoint
                end: Point::new(30.0, 40.0),
            }
        );
    }

    #[test]
    fn test_ctm_close_path_returns_to_transformed_start() {
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 50.0, 50.0);
        let mut builder = PathBuilder::new(ctm);
        builder.move_to(10.0, 20.0); // transformed: (60, 70)
        builder.line_to(100.0, 100.0);
        builder.close_path();

        let cp = builder.current_point().unwrap();
        assert_point_approx(cp, 60.0, 70.0);
    }

    #[test]
    fn test_set_ctm() {
        let mut builder = PathBuilder::new(Ctm::identity());
        builder.move_to(10.0, 20.0); // no transform

        // Change CTM to translate by (100, 100)
        builder.set_ctm(Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 100.0));
        builder.line_to(10.0, 20.0); // now transformed: (110, 120)

        let path = builder.build();
        assert_eq!(
            path.segments[0],
            PathSegment::MoveTo(Point::new(10.0, 20.0))
        );
        assert_eq!(
            path.segments[1],
            PathSegment::LineTo(Point::new(110.0, 120.0))
        );
    }

    #[test]
    fn test_ctm_accessor() {
        let ctm = Ctm::new(2.0, 0.0, 0.0, 3.0, 10.0, 20.0);
        let builder = PathBuilder::new(ctm);
        assert_eq!(*builder.ctm(), ctm);
    }
}
