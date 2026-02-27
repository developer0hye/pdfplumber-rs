//! Page coordinate normalization — rotation and CropBox transforms.
//!
//! Transforms coordinates from PDF native space (bottom-left origin)
//! to the user-visible page coordinate system (top-left origin),
//! accounting for page rotation (`/Rotate`) and CropBox.

use pdfplumber_core::geometry::BBox;

/// Page coordinate normalization configuration.
///
/// Combines MediaBox, optional CropBox, and page rotation to provide
/// a unified transform from PDF native space to top-left origin
/// user-visible space.
///
/// # Coordinate Transform Pipeline
///
/// 1. Offset from MediaBox origin
/// 2. Apply rotation (0°/90°/180°/270° clockwise)
/// 3. Offset by CropBox position (in rotated space)
/// 4. Y-flip (bottom-left → top-left origin)
///
/// # Example
///
/// ```
/// use pdfplumber_core::geometry::BBox;
/// use pdfplumber_parse::page_geometry::PageGeometry;
///
/// // US Letter page, no crop, no rotation
/// let media_box = BBox::new(0.0, 0.0, 612.0, 792.0);
/// let geo = PageGeometry::new(media_box, None, 0);
///
/// assert_eq!(geo.width(), 612.0);
/// assert_eq!(geo.height(), 792.0);
///
/// // Point near top in PDF space (y=720) → near top in display (y=72)
/// let (x, y) = geo.normalize_point(72.0, 720.0);
/// assert!((x - 72.0).abs() < 0.01);
/// assert!((y - 72.0).abs() < 0.01);
/// ```
pub struct PageGeometry {
    rotation: i32,
    media_x0: f64,
    media_y0: f64,
    native_width: f64,
    native_height: f64,
    crop_rx0: f64,
    crop_ry0: f64,
    display_width: f64,
    display_height: f64,
}

impl PageGeometry {
    /// Create a new `PageGeometry` from page metadata.
    ///
    /// # Arguments
    ///
    /// * `media_box` - Page MediaBox as raw PDF coordinates in a [`BBox`].
    ///   The BBox fields map to PDF array values:
    ///   `x0` = left, `top` = y-min (PDF bottom), `x1` = right, `bottom` = y-max (PDF top).
    /// * `crop_box` - Optional CropBox (same coordinate convention as MediaBox).
    ///   If `None`, MediaBox is used as the visible viewport.
    /// * `rotation` - Page `/Rotate` value. Normalized to 0, 90, 180, or 270.
    pub fn new(media_box: BBox, crop_box: Option<BBox>, rotation: i32) -> Self {
        let rotation = rotation.rem_euclid(360);

        let media_x0 = media_box.x0;
        let media_y0 = media_box.top;
        let native_width = media_box.width();
        let native_height = media_box.height();

        let crop = crop_box.unwrap_or(media_box);

        // Adjust CropBox relative to MediaBox origin
        let cx0 = crop.x0 - media_x0;
        let cy0 = crop.top - media_y0;
        let cx1 = crop.x1 - media_x0;
        let cy1 = crop.bottom - media_y0;

        // Rotate CropBox corners to display space
        let (crop_rx0, crop_ry0, crop_rx1, crop_ry1) = match rotation {
            90 => (cy0, native_width - cx1, cy1, native_width - cx0),
            180 => (
                native_width - cx1,
                native_height - cy1,
                native_width - cx0,
                native_height - cy0,
            ),
            270 => (native_height - cy1, cx0, native_height - cy0, cx1),
            _ => (cx0, cy0, cx1, cy1), // 0 or fallback
        };

        Self {
            rotation,
            media_x0,
            media_y0,
            native_width,
            native_height,
            crop_rx0,
            crop_ry0,
            display_width: crop_rx1 - crop_rx0,
            display_height: crop_ry1 - crop_ry0,
        }
    }

    /// Visible page width after rotation and cropping.
    pub fn width(&self) -> f64 {
        self.display_width
    }

    /// Visible page height after rotation and cropping.
    pub fn height(&self) -> f64 {
        self.display_height
    }

    /// Page rotation in degrees (normalized to 0, 90, 180, or 270).
    pub fn rotation(&self) -> i32 {
        self.rotation
    }

    /// Transform a point from PDF native space to top-left origin display space.
    ///
    /// Applies: MediaBox offset → rotation → CropBox offset → y-flip.
    pub fn normalize_point(&self, x: f64, y: f64) -> (f64, f64) {
        // Step 1: Offset from MediaBox origin
        let px = x - self.media_x0;
        let py = y - self.media_y0;

        // Step 2: Apply rotation (clockwise)
        let (rx, ry) = match self.rotation {
            90 => (py, self.native_width - px),
            180 => (self.native_width - px, self.native_height - py),
            270 => (self.native_height - py, px),
            _ => (px, py), // 0 or fallback
        };

        // Step 3: CropBox offset
        let cx = rx - self.crop_rx0;
        let cy = ry - self.crop_ry0;

        // Step 4: Y-flip (bottom-left → top-left)
        (cx, self.display_height - cy)
    }

    /// Transform a bounding box from PDF native space to top-left origin display space.
    ///
    /// Takes min/max corners in native PDF coordinates and returns a [`BBox`]
    /// in display space with top-left origin. Corners are re-normalized after
    /// transformation since rotation may swap min/max.
    pub fn normalize_bbox(&self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> BBox {
        let (x0, y0) = self.normalize_point(min_x, min_y);
        let (x1, y1) = self.normalize_point(max_x, max_y);
        BBox::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Standard US Letter page dimensions
    const LETTER_W: f64 = 612.0;
    const LETTER_H: f64 = 792.0;

    fn letter_media_box() -> BBox {
        BBox::new(0.0, 0.0, LETTER_W, LETTER_H)
    }

    fn letter_crop_box() -> BBox {
        // 0.5-inch margins (36pt from each edge)
        BBox::new(36.0, 36.0, 576.0, 756.0)
    }

    fn assert_approx(actual: f64, expected: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{msg}: expected {expected}, got {actual}"
        );
    }

    fn assert_point_approx(actual: (f64, f64), expected: (f64, f64), msg: &str) {
        assert_approx(actual.0, expected.0, &format!("{msg} x"));
        assert_approx(actual.1, expected.1, &format!("{msg} y"));
    }

    // ===== Rotation 0 (identity + y-flip) =====

    #[test]
    fn rotate_0_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        assert_approx(geo.width(), 612.0, "width");
        assert_approx(geo.height(), 792.0, "height");
    }

    #[test]
    fn rotate_0_point_near_top() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        // Point near top in PDF space (high y)
        let p = geo.normalize_point(72.0, 720.0);
        // y-flip: (72, 792-720) = (72, 72) — near display top
        assert_point_approx(p, (72.0, 72.0), "near top");
    }

    #[test]
    fn rotate_0_point_near_bottom() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        // Point near bottom in PDF space (low y)
        let p = geo.normalize_point(72.0, 72.0);
        // y-flip: (72, 792-72) = (72, 720) — near display bottom
        assert_point_approx(p, (72.0, 720.0), "near bottom");
    }

    #[test]
    fn rotate_0_bbox() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        let bbox = geo.normalize_bbox(72.0, 717.0, 80.0, 729.0);
        assert_approx(bbox.x0, 72.0, "x0");
        assert_approx(bbox.top, 63.0, "top"); // 792 - 729
        assert_approx(bbox.x1, 80.0, "x1");
        assert_approx(bbox.bottom, 75.0, "bottom"); // 792 - 717
    }

    #[test]
    fn rotate_0_equivalent_to_simple_y_flip() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        // For rotation 0, normalize_point produces the same result as simple y-flip
        let (x, y) = geo.normalize_point(72.0, 720.0);
        assert_approx(x, 72.0, "x unchanged");
        assert_approx(y, LETTER_H - 720.0, "y matches simple y-flip");
    }

    // ===== Rotation 90 (CW) =====

    #[test]
    fn rotate_90_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), None, 90);
        // Width and height swap
        assert_approx(geo.width(), 792.0, "width swapped");
        assert_approx(geo.height(), 612.0, "height swapped");
    }

    #[test]
    fn rotate_90_point() {
        let geo = PageGeometry::new(letter_media_box(), None, 90);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate: (720, 612-72) = (720, 540) → y-flip: (720, 612-540) = (720, 72)
        assert_point_approx(p, (720.0, 72.0), "90° point");
    }

    #[test]
    fn rotate_90_bbox() {
        let geo = PageGeometry::new(letter_media_box(), None, 90);
        let bbox = geo.normalize_bbox(72.0, 717.0, 80.0, 729.0);
        // (72, 717) → (717, 540) → (717, 72)
        // (80, 729) → (729, 532) → (729, 80)
        assert_approx(bbox.x0, 717.0, "x0");
        assert_approx(bbox.top, 72.0, "top");
        assert_approx(bbox.x1, 729.0, "x1");
        assert_approx(bbox.bottom, 80.0, "bottom");
        // Original 8×12 box becomes 12×8 after 90° rotation
        assert_approx(bbox.width(), 12.0, "width");
        assert_approx(bbox.height(), 8.0, "height");
    }

    // ===== Rotation 180 =====

    #[test]
    fn rotate_180_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), None, 180);
        // Width and height stay the same
        assert_approx(geo.width(), 612.0, "width unchanged");
        assert_approx(geo.height(), 792.0, "height unchanged");
    }

    #[test]
    fn rotate_180_point() {
        let geo = PageGeometry::new(letter_media_box(), None, 180);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate: (612-72, 792-720) = (540, 72) → y-flip: (540, 792-72) = (540, 720)
        assert_point_approx(p, (540.0, 720.0), "180° point");
    }

    #[test]
    fn rotate_180_bbox() {
        let geo = PageGeometry::new(letter_media_box(), None, 180);
        let bbox = geo.normalize_bbox(72.0, 717.0, 80.0, 729.0);
        // (72, 717) → (540, 75) → (540, 717)
        // (80, 729) → (532, 63) → (532, 729)
        assert_approx(bbox.x0, 532.0, "x0");
        assert_approx(bbox.top, 717.0, "top");
        assert_approx(bbox.x1, 540.0, "x1");
        assert_approx(bbox.bottom, 729.0, "bottom");
        // Same dimensions as original
        assert_approx(bbox.width(), 8.0, "width");
        assert_approx(bbox.height(), 12.0, "height");
    }

    // ===== Rotation 270 (= 90° CCW) =====

    #[test]
    fn rotate_270_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), None, 270);
        // Width and height swap
        assert_approx(geo.width(), 792.0, "width swapped");
        assert_approx(geo.height(), 612.0, "height swapped");
    }

    #[test]
    fn rotate_270_point() {
        let geo = PageGeometry::new(letter_media_box(), None, 270);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate: (792-720, 72) = (72, 72) → y-flip: (72, 612-72) = (72, 540)
        assert_point_approx(p, (72.0, 540.0), "270° point");
    }

    #[test]
    fn rotate_270_bbox() {
        let geo = PageGeometry::new(letter_media_box(), None, 270);
        let bbox = geo.normalize_bbox(72.0, 717.0, 80.0, 729.0);
        // (72, 717) → (75, 72) → (75, 540)
        // (80, 729) → (63, 80) → (63, 532)
        assert_approx(bbox.x0, 63.0, "x0");
        assert_approx(bbox.top, 532.0, "top");
        assert_approx(bbox.x1, 75.0, "x1");
        assert_approx(bbox.bottom, 540.0, "bottom");
        // Original 8×12 box becomes 12×8 after 270° rotation
        assert_approx(bbox.width(), 12.0, "width");
        assert_approx(bbox.height(), 8.0, "height");
    }

    // ===== CropBox offset (rotation 0) =====

    #[test]
    fn cropbox_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 0);
        // CropBox: [36,36,576,756] → 540×720
        assert_approx(geo.width(), 540.0, "cropped width");
        assert_approx(geo.height(), 720.0, "cropped height");
    }

    #[test]
    fn cropbox_offset_point() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 0);
        let p = geo.normalize_point(72.0, 720.0);
        // crop offset: cx=72-36=36, cy=720-36=684
        // y-flip: (36, 720-684) = (36, 36)
        assert_point_approx(p, (36.0, 36.0), "cropped point");
    }

    #[test]
    fn cropbox_offset_bbox() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 0);
        let bbox = geo.normalize_bbox(72.0, 717.0, 80.0, 729.0);
        // (72, 717) → cx=36, cy=681 → (36, 720-681) = (36, 39)
        // (80, 729) → cx=44, cy=693 → (44, 720-693) = (44, 27)
        assert_approx(bbox.x0, 36.0, "x0");
        assert_approx(bbox.top, 27.0, "top");
        assert_approx(bbox.x1, 44.0, "x1");
        assert_approx(bbox.bottom, 39.0, "bottom");
    }

    // ===== Combined rotation + CropBox =====

    #[test]
    fn cropbox_with_rotation_90_dimensions() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 90);
        // CropBox [36,36,576,756] rotated 90°:
        // rx0=36, ry0=612-576=36, rx1=756, ry1=612-36=576
        // display: 720×540
        assert_approx(geo.width(), 720.0, "rotated+cropped width");
        assert_approx(geo.height(), 540.0, "rotated+cropped height");
    }

    #[test]
    fn cropbox_with_rotation_90_point() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 90);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate 90: (720, 612-72) = (720, 540)
        // crop offset: (720-36, 540-36) = (684, 504)
        // y-flip: (684, 540-504) = (684, 36)
        assert_point_approx(p, (684.0, 36.0), "90° + crop");
    }

    #[test]
    fn cropbox_with_rotation_180_point() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 180);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate 180: (612-72, 792-720) = (540, 72)
        // CropBox 180°: rx0=612-576=36, ry0=792-756=36, rx1=612-36=576, ry1=792-36=756
        // crop offset: (540-36, 72-36) = (504, 36)
        // display height = 720
        // y-flip: (504, 720-36) = (504, 684)
        assert_point_approx(p, (504.0, 684.0), "180° + crop");
    }

    #[test]
    fn cropbox_with_rotation_270_point() {
        let geo = PageGeometry::new(letter_media_box(), Some(letter_crop_box()), 270);
        let p = geo.normalize_point(72.0, 720.0);
        // rotate 270: (792-720, 72) = (72, 72)
        // CropBox 270°: rx0=792-756=36, ry0=36, rx1=792-36=756, ry1=576
        // crop offset: (72-36, 72-36) = (36, 36)
        // display height = 540
        // y-flip: (36, 540-36) = (36, 504)
        assert_point_approx(p, (36.0, 504.0), "270° + crop");
    }

    // ===== Non-zero MediaBox origin =====

    #[test]
    fn non_zero_mediabox_origin() {
        let media_box = BBox::new(100.0, 100.0, 712.0, 892.0);
        let geo = PageGeometry::new(media_box, None, 0);
        assert_approx(geo.width(), 612.0, "width");
        assert_approx(geo.height(), 792.0, "height");

        let p = geo.normalize_point(172.0, 820.0);
        // Offset: (172-100, 820-100) = (72, 720)
        // y-flip: (72, 792-720) = (72, 72)
        assert_point_approx(p, (72.0, 72.0), "shifted origin");
    }

    #[test]
    fn non_zero_mediabox_with_rotation_90() {
        let media_box = BBox::new(50.0, 50.0, 662.0, 842.0);
        let geo = PageGeometry::new(media_box, None, 90);
        assert_approx(geo.width(), 792.0, "width swapped");
        assert_approx(geo.height(), 612.0, "height swapped");

        let p = geo.normalize_point(122.0, 770.0);
        // Offset: (122-50, 770-50) = (72, 720)
        // rotate 90: (720, 612-72) = (720, 540)
        // y-flip: (720, 612-540) = (720, 72)
        assert_point_approx(p, (720.0, 72.0), "shifted + 90°");
    }

    // ===== Rotation normalization =====

    #[test]
    fn negative_rotation_normalized() {
        let geo = PageGeometry::new(letter_media_box(), None, -90);
        assert_eq!(geo.rotation(), 270);
        assert_approx(geo.width(), 792.0, "width for -90°");
        assert_approx(geo.height(), 612.0, "height for -90°");
    }

    #[test]
    fn rotation_360_normalized_to_0() {
        let geo = PageGeometry::new(letter_media_box(), None, 360);
        assert_eq!(geo.rotation(), 0);
        assert_approx(geo.width(), 612.0, "width for 360°");
        assert_approx(geo.height(), 792.0, "height for 360°");
    }

    #[test]
    fn rotation_450_normalized_to_90() {
        let geo = PageGeometry::new(letter_media_box(), None, 450);
        assert_eq!(geo.rotation(), 90);
    }

    // ===== Page origin at (0,0) =====

    #[test]
    fn origin_point_transforms_correctly() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        let p = geo.normalize_point(0.0, 0.0);
        // PDF origin (bottom-left) → display bottom-left → (0, 792)
        assert_point_approx(p, (0.0, 792.0), "origin");
    }

    #[test]
    fn top_right_corner_transforms_correctly() {
        let geo = PageGeometry::new(letter_media_box(), None, 0);
        let p = geo.normalize_point(612.0, 792.0);
        // PDF top-right → display top-right → (612, 0)
        assert_point_approx(p, (612.0, 0.0), "top-right corner");
    }

    // ===== Accessor =====

    #[test]
    fn rotation_accessor() {
        assert_eq!(PageGeometry::new(letter_media_box(), None, 0).rotation(), 0);
        assert_eq!(
            PageGeometry::new(letter_media_box(), None, 90).rotation(),
            90
        );
        assert_eq!(
            PageGeometry::new(letter_media_box(), None, 180).rotation(),
            180
        );
        assert_eq!(
            PageGeometry::new(letter_media_box(), None, 270).rotation(),
            270
        );
    }

    // ===== CropBox smaller than MediaBox =====

    #[test]
    fn small_cropbox_clips_dimensions() {
        // CropBox is a small region of the page
        let crop = BBox::new(100.0, 200.0, 300.0, 500.0);
        let geo = PageGeometry::new(letter_media_box(), Some(crop), 0);
        assert_approx(geo.width(), 200.0, "small crop width");
        assert_approx(geo.height(), 300.0, "small crop height");
    }

    #[test]
    fn small_cropbox_offsets_coordinates() {
        let crop = BBox::new(100.0, 200.0, 300.0, 500.0);
        let geo = PageGeometry::new(letter_media_box(), Some(crop), 0);
        // Point at crop origin → display bottom-left → (0, 300)
        let p = geo.normalize_point(100.0, 200.0);
        assert_point_approx(p, (0.0, 300.0), "crop origin");

        // Point at crop top-right → display top-right → (200, 0)
        let p2 = geo.normalize_point(300.0, 500.0);
        assert_point_approx(p2, (200.0, 0.0), "crop top-right");
    }

    // ===== Square page =====

    #[test]
    fn square_page_rotate_90() {
        let media = BBox::new(0.0, 0.0, 500.0, 500.0);
        let geo = PageGeometry::new(media, None, 90);
        // Square: width and height remain the same after rotation
        assert_approx(geo.width(), 500.0, "width");
        assert_approx(geo.height(), 500.0, "height");
    }
}
