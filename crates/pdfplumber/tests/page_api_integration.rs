//! Integration tests for the Page API.
//!
//! These tests exercise the full Page API with all object types (chars, words,
//! lines, rects, curves, edges, images) working together, simulating the output
//! of a real PDF extraction pipeline.

use pdfplumber::{
    BBox, Char, Color, Ctm, Curve, Image, ImageMetadata, Line, LineOrientation, Page, Rect,
    WordOptions, image_from_ctm,
};

/// Helper: create a Char.
fn char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
    Char {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        fontname: "Helvetica".to_string(),
        size: 12.0,
        doctop: top,
        upright: true,
        direction: pdfplumber::TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: None,
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: 0,
        mcid: None,
        tag: None,
    }
}

/// Helper: create a horizontal Line.
fn hline(x0: f64, y: f64, x1: f64, width: f64) -> Line {
    Line {
        x0,
        top: y,
        x1,
        bottom: y,
        line_width: width,
        stroke_color: Color::black(),
        orientation: LineOrientation::Horizontal,
    }
}

/// Helper: create a vertical Line.
fn vline(x: f64, top: f64, bottom: f64, width: f64) -> Line {
    Line {
        x0: x,
        top,
        x1: x,
        bottom,
        line_width: width,
        stroke_color: Color::black(),
        orientation: LineOrientation::Vertical,
    }
}

/// Helper: create a Rect.
fn rect(x0: f64, top: f64, x1: f64, bottom: f64) -> Rect {
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

/// Helper: create a Curve.
fn curve(pts: Vec<(f64, f64)>) -> Curve {
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

/// Simulate a PDF page that contains:
/// - Text: "Hello World" on line 1, "Test PDF" on line 2
/// - A table border: 2x2 grid of rectangles
/// - Two horizontal and two vertical separator lines
/// - A decorative curve
/// - An embedded image
///
/// This mirrors what a real PDF extraction pipeline would produce.
#[test]
fn test_page_with_all_object_types() {
    let page_width = 612.0;
    let page_height = 792.0;

    // Text characters (top-left origin, pre-extracted)
    let chars = vec![
        // "Hello" at y=72
        char("H", 72.0, 72.0, 80.0, 84.0),
        char("e", 80.0, 72.0, 88.0, 84.0),
        char("l", 88.0, 72.0, 93.0, 84.0),
        char("l", 93.0, 72.0, 98.0, 84.0),
        char("o", 98.0, 72.0, 106.0, 84.0),
        // Space
        char(" ", 106.0, 72.0, 110.0, 84.0),
        // "World"
        char("W", 110.0, 72.0, 122.0, 84.0),
        char("o", 122.0, 72.0, 130.0, 84.0),
        char("r", 130.0, 72.0, 136.0, 84.0),
        char("l", 136.0, 72.0, 141.0, 84.0),
        char("d", 141.0, 72.0, 149.0, 84.0),
        // "Test" on line 2 (y=96)
        char("T", 72.0, 96.0, 80.0, 108.0),
        char("e", 80.0, 96.0, 88.0, 108.0),
        char("s", 88.0, 96.0, 94.0, 108.0),
        char("t", 94.0, 96.0, 100.0, 108.0),
    ];

    // Table border lines (simulating a 2-column table)
    let lines = vec![
        hline(72.0, 150.0, 540.0, 1.0),  // top border
        hline(72.0, 200.0, 540.0, 0.5),  // middle separator
        hline(72.0, 250.0, 540.0, 1.0),  // bottom border
        vline(72.0, 150.0, 250.0, 1.0),  // left border
        vline(306.0, 150.0, 250.0, 0.5), // middle separator
        vline(540.0, 150.0, 250.0, 1.0), // right border
    ];

    // Table cell rectangles (background fills)
    let rects = vec![
        rect(72.0, 150.0, 306.0, 200.0),  // top-left cell
        rect(306.0, 150.0, 540.0, 200.0), // top-right cell
    ];

    // Decorative curve
    let curves = vec![curve(vec![
        (72.0, 300.0),
        (150.0, 280.0),
        (400.0, 280.0),
        (540.0, 300.0),
    ])];

    // Embedded image (simulating a 200x150pt image at position (72, 350))
    let ctm = Ctm::new(200.0, 0.0, 0.0, 150.0, 72.0, 792.0 - 350.0 - 150.0);
    let meta = ImageMetadata {
        src_width: Some(1920),
        src_height: Some(1080),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
    };
    let img = image_from_ctm(&ctm, "Im0", page_height, &meta);

    // Construct the page
    let page = Page::with_geometry_and_images(
        0,
        page_width,
        page_height,
        chars,
        lines,
        rects,
        curves,
        vec![img],
    );

    // --- Verify all accessors ---

    // Page metadata
    assert_eq!(page.page_number(), 0);
    assert_eq!(page.width(), 612.0);
    assert_eq!(page.height(), 792.0);

    // Characters
    assert_eq!(page.chars().len(), 15);

    // Words
    let words = page.extract_words(&WordOptions::default());
    assert_eq!(words.len(), 3); // "Hello", "World", "Test"
    assert_eq!(words[0].text, "Hello");
    assert_eq!(words[1].text, "World");
    assert_eq!(words[2].text, "Test");

    // Lines
    assert_eq!(page.lines().len(), 6);

    // Rects
    assert_eq!(page.rects().len(), 2);

    // Curves
    assert_eq!(page.curves().len(), 1);

    // Edges: 6 from lines + 8 from rects (4 per rect) + 1 from curve = 15
    let edges = page.edges();
    assert_eq!(edges.len(), 15);

    // Images
    assert_eq!(page.images().len(), 1);
    let img = &page.images()[0];
    assert_eq!(img.name, "Im0");
    assert!((img.width - 200.0).abs() < 1e-6);
    assert!((img.height - 150.0).abs() < 1e-6);
    assert_eq!(img.src_width, Some(1920));
    assert_eq!(img.src_height, Some(1080));
    assert_eq!(img.color_space, Some("DeviceRGB".to_string()));
}

/// Test that image_from_ctm correctly handles typical PDF image placements.
#[test]
fn test_image_extraction_from_ctm_typical_pdf() {
    let page_height = 792.0;

    // A typical image placement: 300x200 image at bottom-left (72, 72) in PDF coords
    // In PDF: CTM = [300 0 0 200 72 72]
    // This means the image spans x: 72..372, y: 72..272 in PDF coords
    let ctm = Ctm::new(300.0, 0.0, 0.0, 200.0, 72.0, 72.0);
    let meta = ImageMetadata {
        src_width: Some(3000),
        src_height: Some(2000),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
    };

    let img = image_from_ctm(&ctm, "photo", page_height, &meta);

    assert_eq!(img.name, "photo");
    assert!((img.x0 - 72.0).abs() < 1e-6);
    assert!((img.x1 - 372.0).abs() < 1e-6);
    // y-flip: top = 792 - 272 = 520, bottom = 792 - 72 = 720
    assert!((img.top - 520.0).abs() < 1e-6);
    assert!((img.bottom - 720.0).abs() < 1e-6);
    assert!((img.width - 300.0).abs() < 1e-6);
    assert!((img.height - 200.0).abs() < 1e-6);
    assert_eq!(img.src_width, Some(3000));
    assert_eq!(img.src_height, Some(2000));
}

/// Test that multiple images on the same page are accessible.
#[test]
fn test_page_with_multiple_images() {
    let page_height = 792.0;

    let images: Vec<Image> = (0..3)
        .map(|i| {
            let x_offset = 72.0 + (i as f64) * 200.0;
            let ctm = Ctm::new(150.0, 0.0, 0.0, 100.0, x_offset, 400.0);
            let meta = ImageMetadata {
                src_width: Some(800),
                src_height: Some(600),
                bits_per_component: Some(8),
                color_space: Some("DeviceRGB".to_string()),
            };
            image_from_ctm(&ctm, &format!("Im{i}"), page_height, &meta)
        })
        .collect();

    let page = Page::with_geometry_and_images(
        0,
        612.0,
        page_height,
        vec![],
        vec![],
        vec![],
        vec![],
        images,
    );

    assert_eq!(page.images().len(), 3);
    assert_eq!(page.images()[0].name, "Im0");
    assert_eq!(page.images()[1].name, "Im1");
    assert_eq!(page.images()[2].name, "Im2");

    // Each image should be 150pt wide and 100pt tall
    for img in page.images() {
        assert!((img.width - 150.0).abs() < 1e-6);
        assert!((img.height - 100.0).abs() < 1e-6);
    }

    // Images should be at different x positions
    assert!((page.images()[0].x0 - 72.0).abs() < 1e-6);
    assert!((page.images()[1].x0 - 272.0).abs() < 1e-6);
    assert!((page.images()[2].x0 - 472.0).abs() < 1e-6);
}
