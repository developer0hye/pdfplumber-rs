/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Current Transformation Matrix (CTM) — affine transform.
///
/// Represented as six values `[a, b, c, d, e, f]` corresponding to:
/// ```text
/// | a  b  0 |
/// | c  d  0 |
/// | e  f  1 |
/// ```
/// Point transformation: `(x', y') = (a*x + c*y + e, b*x + d*y + f)`
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ctm {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Ctm {
    fn default() -> Self {
        Self::identity()
    }
}

impl Ctm {
    /// Create a new CTM with the given values.
    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Identity matrix (no transformation).
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Transform a point through this CTM.
    pub fn transform_point(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.c * p.y + self.e,
            y: self.b * p.x + self.d * p.y + self.f,
        }
    }

    /// Concatenate this CTM with another: `self × other`.
    pub fn concat(&self, other: &Ctm) -> Ctm {
        Ctm {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }
}

/// Orientation of a geometric element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    Horizontal,
    Vertical,
    Diagonal,
}

/// Bounding box with top-left origin coordinate system.
///
/// Coordinates follow pdfplumber convention:
/// - `x0`: left edge
/// - `top`: top edge (distance from top of page)
/// - `x1`: right edge
/// - `bottom`: bottom edge (distance from top of page)
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BBox {
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bottom: f64,
}

impl BBox {
    pub fn new(x0: f64, top: f64, x1: f64, bottom: f64) -> Self {
        Self {
            x0,
            top,
            x1,
            bottom,
        }
    }

    /// Width of the bounding box.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// Height of the bounding box.
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    /// Compute the union of two bounding boxes.
    pub fn union(&self, other: &BBox) -> BBox {
        BBox {
            x0: self.x0.min(other.x0),
            top: self.top.min(other.top),
            x1: self.x1.max(other.x1),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point_approx(p: Point, x: f64, y: f64) {
        assert!((p.x - x).abs() < 1e-10, "x: expected {x}, got {}", p.x);
        assert!((p.y - y).abs() < 1e-10, "y: expected {y}, got {}", p.y);
    }

    // --- Point tests ---

    #[test]
    fn test_point_new() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 4.0);
    }

    // --- Ctm tests ---

    #[test]
    fn test_ctm_identity() {
        let ctm = Ctm::identity();
        assert_eq!(ctm.a, 1.0);
        assert_eq!(ctm.b, 0.0);
        assert_eq!(ctm.c, 0.0);
        assert_eq!(ctm.d, 1.0);
        assert_eq!(ctm.e, 0.0);
        assert_eq!(ctm.f, 0.0);
    }

    #[test]
    fn test_ctm_default_is_identity() {
        assert_eq!(Ctm::default(), Ctm::identity());
    }

    #[test]
    fn test_ctm_transform_identity() {
        let ctm = Ctm::identity();
        let p = ctm.transform_point(Point::new(5.0, 10.0));
        assert_point_approx(p, 5.0, 10.0);
    }

    #[test]
    fn test_ctm_transform_translation() {
        // Translation by (100, 200)
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);
        let p = ctm.transform_point(Point::new(5.0, 10.0));
        assert_point_approx(p, 105.0, 210.0);
    }

    #[test]
    fn test_ctm_transform_scaling() {
        // Scale by 2x horizontal, 3x vertical
        let ctm = Ctm::new(2.0, 0.0, 0.0, 3.0, 0.0, 0.0);
        let p = ctm.transform_point(Point::new(5.0, 10.0));
        assert_point_approx(p, 10.0, 30.0);
    }

    #[test]
    fn test_ctm_transform_scale_and_translate() {
        // Scale by 2x then translate by (10, 20)
        let ctm = Ctm::new(2.0, 0.0, 0.0, 2.0, 10.0, 20.0);
        let p = ctm.transform_point(Point::new(5.0, 10.0));
        assert_point_approx(p, 20.0, 40.0);
    }

    #[test]
    fn test_ctm_concat_identity() {
        let a = Ctm::new(2.0, 0.0, 0.0, 3.0, 10.0, 20.0);
        let id = Ctm::identity();
        assert_eq!(a.concat(&id), a);
    }

    #[test]
    fn test_ctm_concat_two_translations() {
        let a = Ctm::new(1.0, 0.0, 0.0, 1.0, 10.0, 20.0);
        let b = Ctm::new(1.0, 0.0, 0.0, 1.0, 5.0, 7.0);
        let c = a.concat(&b);
        let p = c.transform_point(Point::new(0.0, 0.0));
        assert_point_approx(p, 15.0, 27.0);
    }

    #[test]
    fn test_ctm_concat_scale_then_translate() {
        // Scale 2x, then translate by (10, 20)
        let scale = Ctm::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let translate = Ctm::new(1.0, 0.0, 0.0, 1.0, 10.0, 20.0);
        let combined = scale.concat(&translate);
        let p = combined.transform_point(Point::new(3.0, 4.0));
        // scale first: (6, 8), then translate: (16, 28)
        assert_point_approx(p, 16.0, 28.0);
    }

    // --- BBox tests ---

    #[test]
    fn test_bbox_new() {
        let bbox = BBox::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(bbox.x0, 10.0);
        assert_eq!(bbox.top, 20.0);
        assert_eq!(bbox.x1, 30.0);
        assert_eq!(bbox.bottom, 40.0);
    }

    #[test]
    fn test_bbox_dimensions() {
        let bbox = BBox::new(10.0, 20.0, 50.0, 60.0);
        assert_eq!(bbox.width(), 40.0);
        assert_eq!(bbox.height(), 40.0);
    }

    #[test]
    fn test_bbox_zero_size() {
        let bbox = BBox::new(10.0, 20.0, 10.0, 20.0);
        assert_eq!(bbox.width(), 0.0);
        assert_eq!(bbox.height(), 0.0);
    }

    // --- Orientation tests ---

    #[test]
    fn test_orientation_variants() {
        let h = Orientation::Horizontal;
        let v = Orientation::Vertical;
        let d = Orientation::Diagonal;
        assert_ne!(h, v);
        assert_ne!(v, d);
        assert_ne!(h, d);
    }

    #[test]
    fn test_orientation_clone_copy() {
        let o = Orientation::Horizontal;
        let o2 = o; // Copy
        let o3 = o.clone(); // Clone
        assert_eq!(o, o2);
        assert_eq!(o, o3);
    }

    #[test]
    fn test_bbox_union() {
        let a = BBox::new(10.0, 20.0, 30.0, 40.0);
        let b = BBox::new(5.0, 25.0, 35.0, 45.0);
        let u = a.union(&b);
        assert_eq!(u.x0, 5.0);
        assert_eq!(u.top, 20.0);
        assert_eq!(u.x1, 35.0);
        assert_eq!(u.bottom, 45.0);
    }
}
