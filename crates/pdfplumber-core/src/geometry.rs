/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

impl Point {
    /// Create a new point at `(x, y)`.
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
    /// Scale X (horizontal scaling).
    pub a: f64,
    /// Shear Y.
    pub b: f64,
    /// Shear X.
    pub c: f64,
    /// Scale Y (vertical scaling).
    pub d: f64,
    /// Translate X.
    pub e: f64,
    /// Translate Y.
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
    /// Horizontal (left-to-right or right-to-left).
    Horizontal,
    /// Vertical (top-to-bottom or bottom-to-top).
    Vertical,
    /// Diagonal (neither purely horizontal nor vertical).
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
    /// Left edge x coordinate.
    pub x0: f64,
    /// Top edge y coordinate (distance from top of page).
    pub top: f64,
    /// Right edge x coordinate.
    pub x1: f64,
    /// Bottom edge y coordinate (distance from top of page).
    pub bottom: f64,
}

impl BBox {
    /// Create a new bounding box from `(x0, top)` to `(x1, bottom)`.
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

    // ═══════════════════════════════════════════════════════════════════════
    // CTM ROTATION TESTS — these are the transforms that broke upright detection
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_ctm_90_degree_rotation() {
        // 90° CCW: [cos90, sin90, -sin90, cos90, 0, 0] = [0, 1, -1, 0, 0, 0]
        let ctm = Ctm::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0);
        let p = ctm.transform_point(Point::new(10.0, 0.0));
        assert_point_approx(p, 0.0, 10.0);
    }

    #[test]
    fn test_ctm_180_degree_rotation() {
        // 180°: [-1, 0, 0, -1, 0, 0]
        let ctm = Ctm::new(-1.0, 0.0, 0.0, -1.0, 0.0, 0.0);
        let p = ctm.transform_point(Point::new(10.0, 5.0));
        assert_point_approx(p, -10.0, -5.0);
    }

    #[test]
    fn test_ctm_270_degree_rotation() {
        // 270° CCW (= 90° CW): [0, -1, 1, 0, 0, 0]
        let ctm = Ctm::new(0.0, -1.0, 1.0, 0.0, 0.0, 0.0);
        let p = ctm.transform_point(Point::new(10.0, 0.0));
        assert_point_approx(p, 0.0, -10.0);
    }

    #[test]
    fn test_ctm_mirror_x() {
        // Mirror across Y axis: [-1, 0, 0, 1, 0, 0]
        // This is the 180° rotation that broke upright detection (trm.a < 0)
        let ctm = Ctm::new(-1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let p = ctm.transform_point(Point::new(10.0, 5.0));
        assert_point_approx(p, -10.0, 5.0);
        // upright check: b≈0, c≈0, a>0 → false (a=-1 < 0)
        assert!(ctm.b.abs() < 1e-6);
        assert!(ctm.c.abs() < 1e-6);
        assert!(ctm.a < 0.0, "mirrored x-scale must be negative");
    }

    #[test]
    fn test_ctm_concat_rotation_then_translation() {
        // Common pattern: rotate 90° then translate to page position
        let rot90 = Ctm::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0);
        let translate = Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);
        let combined = rot90.concat(&translate);
        let p = combined.transform_point(Point::new(10.0, 0.0));
        // rot90: (0, 10), then translate: (100, 210)
        assert_point_approx(p, 100.0, 210.0);
    }

    #[test]
    fn test_ctm_concat_scale_rotation_translation() {
        // Full PDF text matrix: scale 12pt, rotate 90°, translate to position
        let scale = Ctm::new(12.0, 0.0, 0.0, 12.0, 0.0, 0.0);
        let rot90 = Ctm::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0);
        let translate = Ctm::new(1.0, 0.0, 0.0, 1.0, 72.0, 720.0);
        let combined = scale.concat(&rot90).concat(&translate);
        let p = combined.transform_point(Point::new(1.0, 0.0));
        // scale: (12, 0), rot90: (0, 12), translate: (72, 732)
        assert_point_approx(p, 72.0, 732.0);
    }

    #[test]
    fn test_ctm_concat_is_associative() {
        let a = Ctm::new(2.0, 1.0, -1.0, 3.0, 10.0, 20.0);
        let b = Ctm::new(0.5, -0.3, 0.3, 0.5, 5.0, 7.0);
        let c = Ctm::new(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);

        let ab_c = a.concat(&b).concat(&c);
        let a_bc = a.concat(&b.concat(&c));

        let p = Point::new(3.0, 4.0);
        let p1 = ab_c.transform_point(p);
        let p2 = a_bc.transform_point(p);
        assert_point_approx(p2, p1.x, p1.y);
    }

    #[test]
    fn test_ctm_identity_is_left_identity() {
        let a = Ctm::new(2.0, 0.5, -0.5, 3.0, 10.0, 20.0);
        let id = Ctm::identity();
        let result = id.concat(&a);
        // id.concat(a) should equal a
        let p = Point::new(7.0, 11.0);
        let p1 = a.transform_point(p);
        let p2 = result.transform_point(p);
        assert_point_approx(p2, p1.x, p1.y);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BBOX PROPERTY TESTS
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_bbox_union_is_commutative() {
        let a = BBox::new(10.0, 20.0, 30.0, 40.0);
        let b = BBox::new(5.0, 25.0, 35.0, 45.0);
        assert_eq!(a.union(&b), b.union(&a));
    }

    #[test]
    fn test_bbox_union_is_associative() {
        let a = BBox::new(10.0, 20.0, 30.0, 40.0);
        let b = BBox::new(5.0, 25.0, 35.0, 45.0);
        let c = BBox::new(0.0, 15.0, 25.0, 50.0);
        assert_eq!(a.union(&b).union(&c), a.union(&b.union(&c)));
    }

    #[test]
    fn test_bbox_union_with_self_is_identity() {
        let a = BBox::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(a.union(&a), a);
    }

    #[test]
    fn test_bbox_union_contained_box_unchanged() {
        // If b is entirely inside a, union(a, b) == a
        let a = BBox::new(0.0, 0.0, 100.0, 100.0);
        let b = BBox::new(10.0, 10.0, 50.0, 50.0);
        assert_eq!(a.union(&b), a);
    }

    #[test]
    fn test_bbox_negative_dimensions() {
        // x1 < x0 should produce negative width
        let bbox = BBox::new(30.0, 40.0, 10.0, 20.0);
        assert_eq!(bbox.width(), -20.0);
        assert_eq!(bbox.height(), -20.0);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // UPRIGHT DETECTION HELPERS — the exact logic that caused 3 test failures
    // ═══════════════════════════════════════════════════════════════════════

    /// Simulates the upright check from char_extraction.rs
    fn is_upright(ctm: &Ctm) -> bool {
        ctm.b.abs() < 1e-6 && ctm.c.abs() < 1e-6 && ctm.a > 0.0
    }

    #[test]
    fn test_upright_identity() {
        assert!(is_upright(&Ctm::identity()));
    }

    #[test]
    fn test_upright_positive_scale() {
        assert!(is_upright(&Ctm::new(12.0, 0.0, 0.0, 12.0, 72.0, 720.0)));
    }

    #[test]
    fn test_not_upright_negative_x_scale() {
        // 180° rotation with translation — common in rot180 PDFs
        assert!(!is_upright(&Ctm::new(-12.0, 0.0, 0.0, -12.0, 500.0, 700.0)));
    }

    #[test]
    fn test_not_upright_mirror_x() {
        // Mirror across Y axis
        assert!(!is_upright(&Ctm::new(-1.0, 0.0, 0.0, 1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_not_upright_90_rotation() {
        assert!(!is_upright(&Ctm::new(0.0, 12.0, -12.0, 0.0, 0.0, 0.0)));
    }

    #[test]
    fn test_not_upright_270_rotation() {
        assert!(!is_upright(&Ctm::new(0.0, -12.0, 12.0, 0.0, 0.0, 0.0)));
    }

    #[test]
    fn test_not_upright_slight_rotation() {
        // 5° rotation — small but non-zero b and c
        let angle = 5.0_f64.to_radians();
        let ctm = Ctm::new(angle.cos(), angle.sin(), -angle.sin(), angle.cos(), 0.0, 0.0);
        assert!(!is_upright(&ctm), "5° rotation should not be upright");
    }

    #[test]
    fn test_upright_very_small_shear() {
        // Floating point noise: b and c are ~1e-15 (below threshold)
        let ctm = Ctm::new(12.0, 1e-15, -1e-15, 12.0, 72.0, 720.0);
        assert!(is_upright(&ctm), "FP noise below 1e-6 should still be upright");
    }

    #[test]
    fn test_not_upright_zero_a() {
        // a = 0 means no x-scale component — degenerate
        assert!(!is_upright(&Ctm::new(0.0, 0.0, 0.0, 12.0, 0.0, 0.0)));
    }
}
