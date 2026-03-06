#[cfg(test)]
mod tests {
    use super::*;
    use crate::Page;
    use pdfplumber_core::{Color, LineOrientation, TextDirection};

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
            mcid: None,
            tag: None,
        }
    }

    fn make_line(x0: f64, top: f64, x1: f64, bottom: f64, orient: LineOrientation) -> Line {
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

    fn make_image(name: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Image {
        Image {
            x0,
            top,
            x1,
            bottom,
            width: x1 - x0,
            height: bottom - top,
            name: name.to_string(),
            src_width: Some(100),
            src_height: Some(100),
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_string()),
            data: None,
            filter: None,
            mime_type: None,
        }
    }

    /// Build a test page with chars at known positions:
    ///   "A" at (10, 10)-(20, 22)  center=(15, 16)
    ///   "B" at (50, 10)-(60, 22)  center=(55, 16)
    ///   "C" at (10, 50)-(20, 62)  center=(15, 56)
    ///   "D" at (50, 50)-(60, 62)  center=(55, 56)
    fn make_test_page() -> Page {
        let chars = vec![
            make_char("A", 10.0, 10.0, 20.0, 22.0),
            make_char("B", 50.0, 10.0, 60.0, 22.0),
            make_char("C", 10.0, 50.0, 20.0, 62.0),
            make_char("D", 50.0, 50.0, 60.0, 62.0),
        ];
        let lines = vec![
            make_line(10.0, 0.0, 60.0, 0.0, LineOrientation::Horizontal),
            make_line(10.0, 70.0, 60.0, 70.0, LineOrientation::Horizontal),
        ];
        let rects = vec![make_rect(10.0, 10.0, 60.0, 62.0)];
        let curves = vec![make_curve(vec![
            (10.0, 80.0),
            (20.0, 75.0),
            (50.0, 75.0),
            (60.0, 80.0),
        ])];
        let images = vec![make_image("Im0", 10.0, 10.0, 30.0, 30.0)];
        Page::with_geometry_and_images(0, 100.0, 100.0, chars, lines, rects, curves, images)
    }

    // ---- crop tests ----

    #[test]
    fn test_crop_includes_chars_with_center_inside() {
        let page = make_test_page();
        // Crop to left half: x0=0, top=0, x1=35, bottom=70
        // "A" center=(15,16) → inside, "B" center=(55,16) → outside
        // "C" center=(15,56) → inside, "D" center=(55,56) → outside
        let cropped = page.crop(BBox::new(0.0, 0.0, 35.0, 70.0));

        assert_eq!(cropped.chars().len(), 2);
        assert_eq!(cropped.chars()[0].text, "A");
        assert_eq!(cropped.chars()[1].text, "C");
    }

    #[test]
    fn test_crop_coordinate_adjustment() {
        let page = make_test_page();
        // Crop to region starting at (10, 10)
        let cropped = page.crop(BBox::new(10.0, 10.0, 70.0, 70.0));

        // "A" was at (10,10)-(20,22), should now be at (0,0)-(10,12)
        let a = &cropped.chars()[0];
        assert_eq!(a.text, "A");
        assert!((a.bbox.x0 - 0.0).abs() < 1e-10);
        assert!((a.bbox.top - 0.0).abs() < 1e-10);
        assert!((a.bbox.x1 - 10.0).abs() < 1e-10);
        assert!((a.bbox.bottom - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_crop_dimensions() {
        let page = make_test_page();
        let cropped = page.crop(BBox::new(10.0, 20.0, 50.0, 60.0));

        assert!((cropped.width() - 40.0).abs() < 1e-10);
        assert!((cropped.height() - 40.0).abs() < 1e-10);
        assert_eq!(cropped.bbox(), BBox::new(0.0, 0.0, 40.0, 40.0));
    }

    #[test]
    fn test_crop_filters_lines() {
        let page = make_test_page();
        // Line at y=0 center=(35, 0), line at y=70 center=(35, 70)
        // Crop to top half: y 0..40
        let cropped = page.crop(BBox::new(0.0, 0.0, 100.0, 40.0));

        // Line at y=0: center y=0, in bbox → included
        // Line at y=70: center y=70, outside bbox → excluded
        assert_eq!(cropped.lines().len(), 1);
        assert!((cropped.lines()[0].top - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_crop_filters_rects() {
        let page = make_test_page();
        // Rect at (10,10)-(60,62), center=(35, 36)
        // Crop to small region that doesn't contain the center
        let cropped = page.crop(BBox::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(cropped.rects().len(), 0);

        // Crop that contains the center
        let cropped = page.crop(BBox::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(cropped.rects().len(), 1);
    }

    #[test]
    fn test_crop_filters_curves() {
        let page = make_test_page();
        // Curve bbox (10,75)-(60,80), center=(35, 77.5)
        // Crop that includes y=77.5
        let cropped = page.crop(BBox::new(0.0, 70.0, 100.0, 100.0));
        assert_eq!(cropped.curves().len(), 1);
        // Adjusted coordinates: curve top was 75, offset=70 → 5
        assert!((cropped.curves()[0].top - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_crop_filters_images() {
        let page = make_test_page();
        // Image at (10,10)-(30,30), center=(20, 20)
        let cropped = page.crop(BBox::new(0.0, 0.0, 25.0, 25.0));
        assert_eq!(cropped.images().len(), 1);

        let cropped = page.crop(BBox::new(50.0, 50.0, 100.0, 100.0));
        assert_eq!(cropped.images().len(), 0);
    }

    #[test]
    fn test_crop_adjusts_curve_points() {
        let page = make_test_page();
        let cropped = page.crop(BBox::new(0.0, 70.0, 100.0, 100.0));

        let curve = &cropped.curves()[0];
        // Original pts: (10,80), (20,75), (50,75), (60,80)
        // Offset: dx=0, dy=70
        assert!((curve.pts[0].0 - 10.0).abs() < 1e-10);
        assert!((curve.pts[0].1 - 10.0).abs() < 1e-10);
        assert!((curve.pts[1].1 - 5.0).abs() < 1e-10);
    }

    // ---- within_bbox tests ----

    #[test]
    fn test_within_bbox_strict_containment() {
        let page = make_test_page();
        // "A" at (10,10)-(20,22) — fully within (5,5)-(25,25)
        // "B" at (50,10)-(60,22) — NOT fully within (5,5)-(25,25)
        let cropped = page.within_bbox(BBox::new(5.0, 5.0, 25.0, 25.0));

        assert_eq!(cropped.chars().len(), 1);
        assert_eq!(cropped.chars()[0].text, "A");
    }

    #[test]
    fn test_within_bbox_partial_overlap_excluded() {
        let page = make_test_page();
        // "A" at (10,10)-(20,22)
        // Filter box (12,12)-(25,25) — A's x0=10 < 12, so NOT fully within
        let cropped = page.within_bbox(BBox::new(12.0, 12.0, 25.0, 25.0));
        assert_eq!(cropped.chars().len(), 0);
    }

    #[test]
    fn test_within_bbox_all_objects() {
        let page = make_test_page();
        // Everything is within (0,0)-(100,100)
        let cropped = page.within_bbox(BBox::new(0.0, 0.0, 100.0, 100.0));

        assert_eq!(cropped.chars().len(), 4);
        assert_eq!(cropped.lines().len(), 2);
        assert_eq!(cropped.rects().len(), 1);
        assert_eq!(cropped.curves().len(), 1);
        assert_eq!(cropped.images().len(), 1);
    }

    #[test]
    fn test_within_bbox_coordinate_adjustment() {
        let page = make_test_page();
        // "A" at (10,10)-(20,22), within_bbox at origin (5,5)
        let cropped = page.within_bbox(BBox::new(5.0, 5.0, 25.0, 25.0));

        let a = &cropped.chars()[0];
        // Adjusted: (10-5, 10-5) = (5, 5)
        assert!((a.bbox.x0 - 5.0).abs() < 1e-10);
        assert!((a.bbox.top - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_within_bbox_image_fully_inside() {
        let page = make_test_page();
        // Image at (10,10)-(30,30), within (5,5)-(35,35) → fully inside
        let cropped = page.within_bbox(BBox::new(5.0, 5.0, 35.0, 35.0));
        assert_eq!(cropped.images().len(), 1);

        // Image at (10,10)-(30,30), within (15,15)-(35,35) → NOT fully inside (x0=10 < 15)
        let cropped = page.within_bbox(BBox::new(15.0, 15.0, 35.0, 35.0));
        assert_eq!(cropped.images().len(), 0);
    }

    // ---- outside_bbox tests ----

    #[test]
    fn test_outside_bbox_excludes_overlapping() {
        let page = make_test_page();
        // "A" at (10,10)-(20,22) overlaps with (0,0)-(25,25)
        // "B" at (50,10)-(60,22) does NOT overlap with (0,0)-(25,25)
        // "C" at (10,50)-(20,62) does NOT overlap (only x overlaps, not y)
        // "D" at (50,50)-(60,62) does NOT overlap
        let cropped = page.outside_bbox(BBox::new(0.0, 0.0, 25.0, 25.0));

        assert_eq!(cropped.chars().len(), 3);
        let texts: Vec<&str> = cropped.chars().iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"B"));
        assert!(texts.contains(&"C"));
        assert!(texts.contains(&"D"));
        assert!(!texts.contains(&"A"));
    }

    #[test]
    fn test_outside_bbox_all_outside() {
        let page = make_test_page();
        // Filter box at a region with no objects
        let cropped = page.outside_bbox(BBox::new(200.0, 200.0, 300.0, 300.0));
        assert_eq!(cropped.chars().len(), 4);
    }

    #[test]
    fn test_outside_bbox_none_outside() {
        let page = make_test_page();
        // Filter box covers entire page
        let cropped = page.outside_bbox(BBox::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(cropped.chars().len(), 0);
    }

    #[test]
    fn test_outside_bbox_coordinate_adjustment() {
        let page = make_test_page();
        // outside_bbox still adjusts to the filter bbox origin
        let cropped = page.outside_bbox(BBox::new(0.0, 0.0, 25.0, 25.0));

        // "B" at (50,10)-(60,22), offset dx=0, dy=0
        let b = cropped.chars().iter().find(|c| c.text == "B").unwrap();
        assert!((b.bbox.x0 - 50.0).abs() < 1e-10);
        assert!((b.bbox.top - 10.0).abs() < 1e-10);
    }

    // ---- chained filtering tests ----

    #[test]
    fn test_chained_crop() {
        let page = make_test_page();
        // First crop to left half
        let cropped1 = page.crop(BBox::new(0.0, 0.0, 35.0, 70.0));
        assert_eq!(cropped1.chars().len(), 2); // A, C

        // Then crop to top half of the already-cropped page
        // A is now at (10,10)-(20,22) adjusted by (0,0) → same, center=(15,16)
        // C is now at (10,50)-(20,62) adjusted by (0,0) → same, center=(15,56)
        // Crop to top 35px
        let cropped2 = cropped1.crop(BBox::new(0.0, 0.0, 35.0, 35.0));
        assert_eq!(cropped2.chars().len(), 1);
        assert_eq!(cropped2.chars()[0].text, "A");
    }

    #[test]
    fn test_chained_within_then_crop() {
        let page = make_test_page();
        // within_bbox for top-left quadrant
        let cropped1 = page.within_bbox(BBox::new(0.0, 0.0, 70.0, 70.0));
        // All 4 chars are within (0,0)-(70,70)
        assert_eq!(cropped1.chars().len(), 4);

        // Now crop to only the left side (centers with x < 35)
        let cropped2 = cropped1.crop(BBox::new(0.0, 0.0, 35.0, 70.0));
        assert_eq!(cropped2.chars().len(), 2);
    }

    // ---- extract_words / extract_text on CroppedPage ----

    #[test]
    fn test_cropped_page_extract_words() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("i", 20.0, 100.0, 30.0, 112.0),
            make_char("B", 50.0, 100.0, 60.0, 112.0),
            make_char("y", 60.0, 100.0, 70.0, 112.0),
        ];
        let page = Page::new(0, 100.0, 200.0, chars);

        // Crop to only "Hi" region
        let cropped = page.crop(BBox::new(0.0, 90.0, 40.0, 120.0));
        let words = cropped.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Hi");
    }

    #[test]
    fn test_cropped_page_extract_text() {
        let chars = vec![
            make_char("A", 10.0, 10.0, 20.0, 22.0),
            make_char("B", 20.0, 10.0, 30.0, 22.0),
            make_char("X", 50.0, 10.0, 60.0, 22.0),
        ];
        let page = Page::new(0, 100.0, 100.0, chars);

        let cropped = page.crop(BBox::new(0.0, 0.0, 35.0, 30.0));
        let text = cropped.extract_text(&TextOptions::default());

        assert_eq!(text, "AB");
    }

    // ---- edge derivation on CroppedPage ----

    #[test]
    fn test_cropped_page_edges() {
        let page = make_test_page();
        // Crop to include the rect but not the curve
        let cropped = page.crop(BBox::new(0.0, 0.0, 70.0, 70.0));

        // Rect produces 4 edges, lines produce some too
        let edges = cropped.edges();
        assert!(!edges.is_empty());
    }

    // ---- helper function tests ----

    #[test]
    fn test_bbox_center_calculation() {
        let (cx, cy) = bbox_center(10.0, 20.0, 30.0, 40.0);
        assert!((cx - 20.0).abs() < 1e-10);
        assert!((cy - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_in_bbox_inside() {
        let bbox = BBox::new(10.0, 20.0, 30.0, 40.0);
        assert!(point_in_bbox(20.0, 30.0, &bbox));
    }

    #[test]
    fn test_point_in_bbox_on_boundary() {
        let bbox = BBox::new(10.0, 20.0, 30.0, 40.0);
        assert!(point_in_bbox(10.0, 20.0, &bbox)); // top-left corner
        assert!(point_in_bbox(30.0, 40.0, &bbox)); // bottom-right corner
    }

    #[test]
    fn test_point_in_bbox_outside() {
        let bbox = BBox::new(10.0, 20.0, 30.0, 40.0);
        assert!(!point_in_bbox(5.0, 30.0, &bbox));
        assert!(!point_in_bbox(35.0, 30.0, &bbox));
    }

    #[test]
    fn test_fully_within_true() {
        let bbox = BBox::new(0.0, 0.0, 100.0, 100.0);
        assert!(fully_within(10.0, 10.0, 20.0, 20.0, &bbox));
    }

    #[test]
    fn test_fully_within_false_partial() {
        let bbox = BBox::new(15.0, 15.0, 100.0, 100.0);
        // Object starts at x=10 which is before bbox.x0=15
        assert!(!fully_within(10.0, 10.0, 20.0, 20.0, &bbox));
    }

    #[test]
    fn test_fully_outside_true() {
        let bbox = BBox::new(0.0, 0.0, 10.0, 10.0);
        // Object is entirely to the right
        assert!(fully_outside(20.0, 20.0, 30.0, 30.0, &bbox));
    }

    #[test]
    fn test_fully_outside_false_overlapping() {
        let bbox = BBox::new(0.0, 0.0, 25.0, 25.0);
        // Object overlaps
        assert!(!fully_outside(10.0, 10.0, 20.0, 20.0, &bbox));
    }

    #[test]
    fn test_crop_empty_page() {
        let page = Page::new(0, 100.0, 100.0, vec![]);
        let cropped = page.crop(BBox::new(10.0, 10.0, 50.0, 50.0));
        assert!(cropped.chars().is_empty());
        assert!((cropped.width() - 40.0).abs() < 1e-10);
        assert!((cropped.height() - 40.0).abs() < 1e-10);
    }
}
