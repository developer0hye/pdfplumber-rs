//! Image extraction from XObject Do operator.
//!
//! Extracts Image objects from the CTM active when the `Do` operator
//! is invoked for an Image XObject. The image is placed in a 1×1 unit
//! square that is mapped to the page via the CTM.

use crate::geometry::{BBox, Ctm, Point};

/// Metadata about an image XObject from the PDF resource dictionary.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageMetadata {
    /// Original pixel width of the image.
    pub src_width: Option<u32>,
    /// Original pixel height of the image.
    pub src_height: Option<u32>,
    /// Bits per component (e.g., 8).
    pub bits_per_component: Option<u32>,
    /// Color space name (e.g., "DeviceRGB", "DeviceGray").
    pub color_space: Option<String>,
}

/// An image extracted from a PDF page via the Do operator.
///
/// Coordinates use pdfplumber's top-left origin system.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// Left x coordinate.
    pub x0: f64,
    /// Top y coordinate (distance from top of page).
    pub top: f64,
    /// Right x coordinate.
    pub x1: f64,
    /// Bottom y coordinate (distance from top of page).
    pub bottom: f64,
    /// Display width in points.
    pub width: f64,
    /// Display height in points.
    pub height: f64,
    /// XObject name (e.g., "Im0").
    pub name: String,
    /// Original pixel width.
    pub src_width: Option<u32>,
    /// Original pixel height.
    pub src_height: Option<u32>,
    /// Bits per component.
    pub bits_per_component: Option<u32>,
    /// Color space name.
    pub color_space: Option<String>,
}

/// Extract an Image from the CTM active during a Do operator invocation.
///
/// Image XObjects are defined in a 1×1 unit square. The CTM maps this
/// unit square to the actual display area on the page. The four corners
/// of the unit square `(0,0), (1,0), (0,1), (1,1)` are transformed
/// through the CTM to compute the bounding box.
///
/// Coordinates are converted from PDF bottom-left origin to top-left origin
/// using `page_height`.
pub fn image_from_ctm(ctm: &Ctm, name: &str, page_height: f64, metadata: &ImageMetadata) -> Image {
    // Transform the 4 corners of the unit square through the CTM
    let corners = [
        ctm.transform_point(Point::new(0.0, 0.0)),
        ctm.transform_point(Point::new(1.0, 0.0)),
        ctm.transform_point(Point::new(0.0, 1.0)),
        ctm.transform_point(Point::new(1.0, 1.0)),
    ];

    // Find bounding box in PDF coordinates
    let pdf_x0 = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let pdf_x1 = corners
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let pdf_y0 = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let pdf_y1 = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);

    // Convert to top-left origin
    let top = page_height - pdf_y1;
    let bottom = page_height - pdf_y0;

    let width = pdf_x1 - pdf_x0;
    let height = bottom - top;

    Image {
        x0: pdf_x0,
        top,
        x1: pdf_x1,
        bottom,
        width,
        height,
        name: name.to_string(),
        src_width: metadata.src_width,
        src_height: metadata.src_height,
        bits_per_component: metadata.bits_per_component,
        color_space: metadata.color_space.clone(),
    }
}

impl Image {
    /// Returns the bounding box in top-left origin coordinates.
    pub fn bbox(&self) -> BBox {
        BBox::new(self.x0, self.top, self.x1, self.bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-6,
            "expected {b}, got {a}, diff={}",
            (a - b).abs()
        );
    }

    const PAGE_HEIGHT: f64 = 792.0;

    // --- Image struct ---

    #[test]
    fn test_image_bbox() {
        let img = Image {
            x0: 100.0,
            top: 200.0,
            x1: 300.0,
            bottom: 400.0,
            width: 200.0,
            height: 200.0,
            name: "Im0".to_string(),
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
        };
        let bbox = img.bbox();
        assert_approx(bbox.x0, 100.0);
        assert_approx(bbox.top, 200.0);
        assert_approx(bbox.x1, 300.0);
        assert_approx(bbox.bottom, 400.0);
    }

    // --- image_from_ctm ---

    #[test]
    fn test_image_from_ctm_simple_placement() {
        // CTM places a 200x150 image at (100, 500) in PDF coords
        // a=200 (width), d=150 (height), e=100 (x), f=500 (y)
        let ctm = Ctm::new(200.0, 0.0, 0.0, 150.0, 100.0, 500.0);
        let meta = ImageMetadata {
            src_width: Some(640),
            src_height: Some(480),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
        };

        let img = image_from_ctm(&ctm, "Im0", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 100.0);
        assert_approx(img.x1, 300.0);
        // y-flip: top = 792 - 650 = 142, bottom = 792 - 500 = 292
        assert_approx(img.top, 142.0);
        assert_approx(img.bottom, 292.0);
        assert_approx(img.width, 200.0);
        assert_approx(img.height, 150.0);
        assert_eq!(img.name, "Im0");
        assert_eq!(img.src_width, Some(640));
        assert_eq!(img.src_height, Some(480));
        assert_eq!(img.bits_per_component, Some(8));
        assert_eq!(img.color_space, Some("DeviceRGB".to_string()));
    }

    #[test]
    fn test_image_from_ctm_identity() {
        // Identity CTM: image is 1×1 at origin
        let ctm = Ctm::identity();
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im1", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 0.0);
        assert_approx(img.x1, 1.0);
        // y-flip: top = 792 - 1 = 791, bottom = 792 - 0 = 792
        assert_approx(img.top, 791.0);
        assert_approx(img.bottom, 792.0);
        assert_approx(img.width, 1.0);
        assert_approx(img.height, 1.0);
    }

    #[test]
    fn test_image_from_ctm_translation_only() {
        // 1×1 image translated to (300, 400)
        let ctm = Ctm::new(1.0, 0.0, 0.0, 1.0, 300.0, 400.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im2", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 300.0);
        assert_approx(img.x1, 301.0);
        // y-flip: top = 792 - 401 = 391, bottom = 792 - 400 = 392
        assert_approx(img.top, 391.0);
        assert_approx(img.bottom, 392.0);
    }

    #[test]
    fn test_image_from_ctm_scale_and_translate() {
        // 400×300 image at (50, 200)
        let ctm = Ctm::new(400.0, 0.0, 0.0, 300.0, 50.0, 200.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "Im3", PAGE_HEIGHT, &meta);

        assert_approx(img.x0, 50.0);
        assert_approx(img.x1, 450.0);
        // y-flip: top = 792 - 500 = 292, bottom = 792 - 200 = 592
        assert_approx(img.top, 292.0);
        assert_approx(img.bottom, 592.0);
        assert_approx(img.width, 400.0);
        assert_approx(img.height, 300.0);
    }

    #[test]
    fn test_image_from_ctm_no_metadata() {
        let ctm = Ctm::new(100.0, 0.0, 0.0, 100.0, 200.0, 300.0);
        let meta = ImageMetadata::default();

        let img = image_from_ctm(&ctm, "ImX", PAGE_HEIGHT, &meta);

        assert_eq!(img.name, "ImX");
        assert_eq!(img.src_width, None);
        assert_eq!(img.src_height, None);
        assert_eq!(img.bits_per_component, None);
        assert_eq!(img.color_space, None);
    }

    #[test]
    fn test_image_from_ctm_different_page_height() {
        // Letter-size page (11 inches = 792pt) vs A4 (842pt)
        let ctm = Ctm::new(100.0, 0.0, 0.0, 100.0, 0.0, 0.0);
        let meta = ImageMetadata::default();

        let img_letter = image_from_ctm(&ctm, "Im0", 792.0, &meta);
        let img_a4 = image_from_ctm(&ctm, "Im0", 842.0, &meta);

        // Same width
        assert_approx(img_letter.width, img_a4.width);
        // Different top due to different page height
        assert_approx(img_letter.top, 692.0); // 792 - 100
        assert_approx(img_a4.top, 742.0); // 842 - 100
    }

    #[test]
    fn test_image_metadata_default() {
        let meta = ImageMetadata::default();
        assert_eq!(meta.src_width, None);
        assert_eq!(meta.src_height, None);
        assert_eq!(meta.bits_per_component, None);
        assert_eq!(meta.color_space, None);
    }
}
