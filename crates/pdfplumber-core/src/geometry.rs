/// Bounding box with top-left origin coordinate system.
///
/// Coordinates follow pdfplumber convention:
/// - `x0`: left edge
/// - `top`: top edge (distance from top of page)
/// - `x1`: right edge
/// - `bottom`: bottom edge (distance from top of page)
#[derive(Debug, Clone, Copy, PartialEq)]
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
