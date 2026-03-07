use super::*;
use pdfplumber_core::{
    BBox, Color, Ctm, EdgeSource, ExplicitLines, ImageMetadata, LineOrientation, Strategy,
    TextOptions, image_from_ctm,
};

fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
    Char {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        fontname: "TestFont".to_string(),
        size: 12.0,
        doctop: top,
        upright: true,
        direction: pdfplumber_core::TextDirection::Ltr,
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

#[test]
fn test_page_creation() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    assert_eq!(page.page_number(), 0);
    assert_eq!(page.width(), 612.0);
    assert_eq!(page.height(), 792.0);
    assert!(page.chars().is_empty());
}

#[test]
fn test_page_with_chars() {
    let chars = vec![
        make_char("H", 10.0, 100.0, 20.0, 112.0),
        make_char("i", 20.0, 100.0, 30.0, 112.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    assert_eq!(page.chars().len(), 2);
    assert_eq!(page.chars()[0].text, "H");
    assert_eq!(page.chars()[1].text, "i");
}

#[test]
fn test_extract_words_default_options() {
    let chars = vec![
        make_char("H", 10.0, 100.0, 20.0, 112.0),
        make_char("e", 20.0, 100.0, 30.0, 112.0),
        make_char("l", 30.0, 100.0, 35.0, 112.0),
        make_char("l", 35.0, 100.0, 40.0, 112.0),
        make_char("o", 40.0, 100.0, 50.0, 112.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 1);
    assert_eq!(words[0].text, "Hello");
    assert_eq!(words[0].bbox, BBox::new(10.0, 100.0, 50.0, 112.0));
    assert_eq!(words[0].chars.len(), 5);
}

#[test]
fn test_extract_words_text_concatenation() {
    // "The quick fox" with spaces separating words
    let chars = vec![
        make_char("T", 10.0, 100.0, 20.0, 112.0),
        make_char("h", 20.0, 100.0, 28.0, 112.0),
        make_char("e", 28.0, 100.0, 36.0, 112.0),
        make_char(" ", 36.0, 100.0, 40.0, 112.0),
        make_char("q", 40.0, 100.0, 48.0, 112.0),
        make_char("u", 48.0, 100.0, 56.0, 112.0),
        make_char("i", 56.0, 100.0, 60.0, 112.0),
        make_char("c", 60.0, 100.0, 68.0, 112.0),
        make_char("k", 68.0, 100.0, 76.0, 112.0),
        make_char(" ", 76.0, 100.0, 80.0, 112.0),
        make_char("f", 80.0, 100.0, 88.0, 112.0),
        make_char("o", 88.0, 100.0, 96.0, 112.0),
        make_char("x", 96.0, 100.0, 104.0, 112.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 3);
    assert_eq!(words[0].text, "The");
    assert_eq!(words[1].text, "quick");
    assert_eq!(words[2].text, "fox");
}

#[test]
fn test_extract_words_bbox_calculation() {
    // Characters with varying heights; tops increase left-to-right
    // so spatial sort preserves left-to-right order.
    let chars = vec![
        make_char("A", 10.0, 97.0, 20.0, 112.0),
        make_char("b", 20.0, 98.0, 28.0, 110.0),
        make_char("C", 28.0, 99.0, 38.0, 113.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 1);
    // Union: x0=10, top=97, x1=38, bottom=113
    assert_eq!(words[0].bbox, BBox::new(10.0, 97.0, 38.0, 113.0));
}

#[test]
fn test_extract_words_multiline() {
    // Two lines of text
    let chars = vec![
        make_char("H", 10.0, 100.0, 20.0, 112.0),
        make_char("i", 20.0, 100.0, 30.0, 112.0),
        make_char("L", 10.0, 120.0, 20.0, 132.0),
        make_char("o", 20.0, 120.0, 30.0, 132.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 2);
    assert_eq!(words[0].text, "Hi");
    assert_eq!(words[1].text, "Lo");
}

#[test]
fn test_extract_words_custom_options() {
    // Two chars with gap=10, default tolerance=3 splits them, custom tolerance=15 groups them
    let chars = vec![
        make_char("A", 10.0, 100.0, 20.0, 112.0),
        make_char("B", 30.0, 100.0, 40.0, 112.0), // gap = 10
    ];
    let page = Page::new(0, 612.0, 792.0, chars);

    let default_words = page.extract_words(&WordOptions::default());
    assert_eq!(default_words.len(), 2);

    let custom_opts = WordOptions {
        x_tolerance: 15.0,
        ..WordOptions::default()
    };
    let custom_words = page.extract_words(&custom_opts);
    assert_eq!(custom_words.len(), 1);
    assert_eq!(custom_words[0].text, "AB");
}

#[test]
fn test_extract_words_empty_page() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    let words = page.extract_words(&WordOptions::default());
    assert!(words.is_empty());
}

#[test]
fn test_extract_words_constituent_chars() {
    // Verify that words contain their constituent chars
    let chars = vec![
        make_char("A", 10.0, 100.0, 20.0, 112.0),
        make_char("B", 20.0, 100.0, 30.0, 112.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars.clone());
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 1);
    assert_eq!(words[0].chars.len(), 2);
    assert_eq!(words[0].chars[0].text, "A");
    assert_eq!(words[0].chars[1].text, "B");
    assert_eq!(words[0].chars[0].bbox, BBox::new(10.0, 100.0, 20.0, 112.0));
    assert_eq!(words[0].chars[1].bbox, BBox::new(20.0, 100.0, 30.0, 112.0));
}

// --- Geometry accessors ---

#[test]
fn test_page_new_has_empty_geometry() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    assert!(page.lines().is_empty());
    assert!(page.rects().is_empty());
    assert!(page.curves().is_empty());
    assert!(page.edges().is_empty());
}

#[test]
fn test_page_with_geometry() {
    let lines = vec![make_line(
        0.0,
        50.0,
        100.0,
        50.0,
        LineOrientation::Horizontal,
    )];
    let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
    let curves = vec![make_curve(vec![
        (0.0, 100.0),
        (10.0, 50.0),
        (90.0, 50.0),
        (100.0, 100.0),
    ])];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, rects, curves);

    assert_eq!(page.lines().len(), 1);
    assert_eq!(page.rects().len(), 1);
    assert_eq!(page.curves().len(), 1);
}

#[test]
fn test_page_edges_from_lines() {
    let lines = vec![
        make_line(0.0, 50.0, 100.0, 50.0, LineOrientation::Horizontal),
        make_line(50.0, 0.0, 50.0, 100.0, LineOrientation::Vertical),
    ];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, vec![], vec![]);
    let edges = page.edges();

    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].source, EdgeSource::Line);
    assert_eq!(edges[1].source, EdgeSource::Line);
}

#[test]
fn test_page_edges_from_rects() {
    let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], rects, vec![]);
    let edges = page.edges();

    assert_eq!(edges.len(), 4);
    assert_eq!(edges[0].source, EdgeSource::RectTop);
    assert_eq!(edges[1].source, EdgeSource::RectBottom);
    assert_eq!(edges[2].source, EdgeSource::RectLeft);
    assert_eq!(edges[3].source, EdgeSource::RectRight);
}

#[test]
fn test_page_edges_combined() {
    let lines = vec![make_line(
        0.0,
        50.0,
        100.0,
        50.0,
        LineOrientation::Horizontal,
    )];
    let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
    let curves = vec![make_curve(vec![
        (0.0, 100.0),
        (10.0, 50.0),
        (90.0, 50.0),
        (100.0, 100.0),
    ])];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, rects, curves);
    let edges = page.edges();

    // 1 from line + 4 from rect + 1 from curve = 6
    assert_eq!(edges.len(), 6);
    assert_eq!(edges[0].source, EdgeSource::Line);
    assert_eq!(edges[5].source, EdgeSource::Curve);
}

// --- Image accessors ---

fn make_image(name: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Image {
    Image {
        x0,
        top,
        x1,
        bottom,
        width: x1 - x0,
        height: bottom - top,
        name: name.to_string(),
        src_width: Some(640),
        src_height: Some(480),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
        data: None,
        filter: None,
        mime_type: None,
    }
}

#[test]
fn test_page_new_has_empty_images() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    assert!(page.images().is_empty());
}

#[test]
fn test_page_with_geometry_has_empty_images() {
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], vec![], vec![]);
    assert!(page.images().is_empty());
}

#[test]
fn test_page_with_images() {
    let images = vec![
        make_image("Im0", 100.0, 200.0, 300.0, 400.0),
        make_image("Im1", 50.0, 50.0, 150.0, 100.0),
    ];
    let page =
        Page::with_geometry_and_images(0, 612.0, 792.0, vec![], vec![], vec![], vec![], images);

    assert_eq!(page.images().len(), 2);
    assert_eq!(page.images()[0].name, "Im0");
    assert_eq!(page.images()[1].name, "Im1");
}

#[test]
fn test_page_images_from_ctm() {
    // Simulate extracting an image using image_from_ctm
    let ctm = Ctm::new(200.0, 0.0, 0.0, 150.0, 100.0, 500.0);
    let meta = ImageMetadata {
        src_width: Some(640),
        src_height: Some(480),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
    };
    let img = image_from_ctm(&ctm, "Im0", 792.0, &meta);

    let page =
        Page::with_geometry_and_images(0, 612.0, 792.0, vec![], vec![], vec![], vec![], vec![img]);

    assert_eq!(page.images().len(), 1);
    let img = &page.images()[0];
    assert_eq!(img.name, "Im0");
    assert!((img.width - 200.0).abs() < 1e-6);
    assert!((img.height - 150.0).abs() < 1e-6);
    assert_eq!(img.src_width, Some(640));
    assert_eq!(img.src_height, Some(480));
}

#[test]
fn test_page_with_geometry_and_images_all_accessors() {
    let lines = vec![make_line(
        0.0,
        50.0,
        100.0,
        50.0,
        LineOrientation::Horizontal,
    )];
    let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
    let curves = vec![make_curve(vec![
        (0.0, 100.0),
        (10.0, 50.0),
        (90.0, 50.0),
        (100.0, 100.0),
    ])];
    let images = vec![make_image("Im0", 100.0, 200.0, 300.0, 400.0)];
    let chars = vec![make_char("A", 10.0, 100.0, 20.0, 112.0)];

    let page = Page::with_geometry_and_images(0, 612.0, 792.0, chars, lines, rects, curves, images);

    assert_eq!(page.chars().len(), 1);
    assert_eq!(page.lines().len(), 1);
    assert_eq!(page.rects().len(), 1);
    assert_eq!(page.curves().len(), 1);
    assert_eq!(page.images().len(), 1);
    assert_eq!(page.edges().len(), 6); // 1 + 4 + 1
}

// --- extract_text tests ---

#[test]
fn test_extract_text_simple_mode() {
    // "Hello World" on one line
    let chars = vec![
        make_char("H", 10.0, 100.0, 18.0, 112.0),
        make_char("e", 18.0, 100.0, 26.0, 112.0),
        make_char("l", 26.0, 100.0, 30.0, 112.0),
        make_char("l", 30.0, 100.0, 34.0, 112.0),
        make_char("o", 34.0, 100.0, 42.0, 112.0),
        make_char(" ", 42.0, 100.0, 46.0, 112.0),
        make_char("W", 46.0, 100.0, 56.0, 112.0),
        make_char("o", 56.0, 100.0, 64.0, 112.0),
        make_char("r", 64.0, 100.0, 70.0, 112.0),
        make_char("l", 70.0, 100.0, 74.0, 112.0),
        make_char("d", 74.0, 100.0, 82.0, 112.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let text = page.extract_text(&TextOptions::default());
    assert_eq!(text, "Hello World");
}

#[test]
fn test_extract_text_multiline_simple() {
    // Two lines of text
    let chars = vec![
        make_char("A", 10.0, 100.0, 20.0, 112.0),
        make_char("B", 20.0, 100.0, 30.0, 112.0),
        make_char("C", 10.0, 120.0, 20.0, 132.0),
        make_char("D", 20.0, 120.0, 30.0, 132.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let text = page.extract_text(&TextOptions::default());
    assert_eq!(text, "AB\nCD");
}

#[test]
fn test_extract_text_layout_single_column() {
    // Two paragraphs separated by large gap
    let chars = vec![
        // Paragraph 1, line 1
        make_char("H", 10.0, 100.0, 18.0, 112.0),
        make_char("i", 18.0, 100.0, 24.0, 112.0),
        // Paragraph 1, line 2
        make_char("T", 10.0, 115.0, 18.0, 127.0),
        make_char("o", 18.0, 115.0, 24.0, 127.0),
        // Paragraph 2 (large gap)
        make_char("B", 10.0, 200.0, 18.0, 212.0),
        make_char("y", 18.0, 200.0, 24.0, 212.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let opts = TextOptions {
        layout: true,
        ..TextOptions::default()
    };
    let text = page.extract_text(&opts);
    assert_eq!(text, "Hi\nTo\n\nBy");
}

#[test]
fn test_extract_text_layout_two_columns() {
    // Left column at x=10, right column at x=200
    let chars = vec![
        // Left column
        make_char("L", 10.0, 100.0, 18.0, 112.0),
        make_char("1", 18.0, 100.0, 26.0, 112.0),
        make_char("L", 10.0, 115.0, 18.0, 127.0),
        make_char("2", 18.0, 115.0, 26.0, 127.0),
        // Right column
        make_char("R", 200.0, 100.0, 208.0, 112.0),
        make_char("1", 208.0, 100.0, 216.0, 112.0),
        make_char("R", 200.0, 115.0, 208.0, 127.0),
        make_char("2", 208.0, 115.0, 216.0, 127.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let opts = TextOptions {
        layout: true,
        ..TextOptions::default()
    };
    let text = page.extract_text(&opts);
    assert_eq!(text, "L1\nL2\n\nR1\nR2");
}

#[test]
fn test_extract_text_empty_page() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    let text = page.extract_text(&TextOptions::default());
    assert_eq!(text, "");
}

#[test]
fn test_extract_text_layout_mixed_with_header_footer() {
    let chars = vec![
        // Header
        make_char("H", 10.0, 50.0, 18.0, 62.0),
        // Left column
        make_char("L", 10.0, 100.0, 18.0, 112.0),
        // Right column
        make_char("R", 200.0, 100.0, 208.0, 112.0),
        // Footer
        make_char("F", 10.0, 250.0, 18.0, 262.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let opts = TextOptions {
        layout: true,
        ..TextOptions::default()
    };
    let text = page.extract_text(&opts);
    assert_eq!(text, "H\n\nL\n\nR\n\nF");
}

// --- Table API tests (US-039) ---

/// Helper: create a horizontal line from (x0, y) to (x1, y)
fn hline(x0: f64, y: f64, x1: f64) -> Line {
    make_line(x0, y, x1, y, LineOrientation::Horizontal)
}

/// Helper: create a vertical line from (x, top) to (x, bottom)
fn vline(x: f64, top: f64, bottom: f64) -> Line {
    make_line(x, top, x, bottom, LineOrientation::Vertical)
}

/// Build a page with a simple 2x2 bordered table (1 row, 2 columns)
/// with text "A" in left cell and "B" in right cell.
///
/// Table grid:
/// ```text
/// (10,10)──(60,10)──(110,10)
///   │   "A"   │   "B"   │
/// (10,30)──(60,30)──(110,30)
/// ```
fn make_simple_table_page() -> Page {
    let lines = vec![
        // 3 horizontal lines
        hline(10.0, 10.0, 110.0),
        hline(10.0, 30.0, 110.0),
        // 3 vertical lines
        vline(10.0, 10.0, 30.0),
        vline(60.0, 10.0, 30.0),
        vline(110.0, 10.0, 30.0),
    ];
    let chars = vec![
        // "A" centered in left cell (10,10)-(60,30), center ~ (35,20)
        make_char("A", 30.0, 15.0, 40.0, 25.0),
        // "B" centered in right cell (60,10)-(110,30), center ~ (85,20)
        make_char("B", 80.0, 15.0, 90.0, 25.0),
    ];
    Page::with_geometry(0, 612.0, 792.0, chars, lines, vec![], vec![])
}

/// Build a page with a 2-row, 2-column bordered table:
/// ```text
/// (10,10)──(60,10)──(110,10)
///   │  "A"   │  "B"    │
/// (10,30)──(60,30)──(110,30)
///   │  "C"   │  "D"    │
/// (10,50)──(60,50)──(110,50)
/// ```
fn make_2x2_table_page() -> Page {
    let lines = vec![
        hline(10.0, 10.0, 110.0),
        hline(10.0, 30.0, 110.0),
        hline(10.0, 50.0, 110.0),
        vline(10.0, 10.0, 50.0),
        vline(60.0, 10.0, 50.0),
        vline(110.0, 10.0, 50.0),
    ];
    let chars = vec![
        make_char("A", 30.0, 15.0, 40.0, 25.0),
        make_char("B", 80.0, 15.0, 90.0, 25.0),
        make_char("C", 30.0, 35.0, 40.0, 45.0),
        make_char("D", 80.0, 35.0, 90.0, 45.0),
    ];
    Page::with_geometry(0, 612.0, 792.0, chars, lines, vec![], vec![])
}

#[test]
fn test_find_tables_simple_bordered() {
    let page = make_simple_table_page();
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.cells.len(), 2);
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].len(), 2);
}

#[test]
fn test_find_tables_cell_text() {
    let page = make_simple_table_page();
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert_eq!(tables.len(), 1);
    let row = &tables[0].rows[0];
    assert_eq!(row[0].text, Some("A".to_string()));
    assert_eq!(row[1].text, Some("B".to_string()));
}

#[test]
fn test_find_tables_2x2() {
    let page = make_2x2_table_page();
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 4);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].rows[0].len(), 2);
    assert_eq!(tables[0].rows[1].len(), 2);
}

#[test]
fn test_find_tables_empty_page() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert!(tables.is_empty());
}

#[test]
fn test_find_tables_no_lines() {
    // Page with only chars, no geometry → no tables with Lattice strategy
    let chars = vec![make_char("A", 10.0, 10.0, 20.0, 22.0)];
    let page = Page::new(0, 612.0, 792.0, chars);
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert!(tables.is_empty());
}

#[test]
fn test_find_tables_with_rects() {
    // A rect creates 4 edges (top, bottom, left, right) → should detect a 1-cell table
    let rects = vec![make_rect(10.0, 10.0, 100.0, 50.0)];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], rects, vec![]);
    let settings = TableSettings::default();
    let tables = page.find_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 1);
}

#[test]
fn test_extract_tables_simple() {
    let page = make_simple_table_page();
    let settings = TableSettings::default();
    let tables = page.extract_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 1); // 1 row
    assert_eq!(tables[0][0].len(), 2); // 2 columns
    assert_eq!(tables[0][0][0], Some("A".to_string()));
    assert_eq!(tables[0][0][1], Some("B".to_string()));
}

#[test]
fn test_extract_tables_2x2() {
    let page = make_2x2_table_page();
    let settings = TableSettings::default();
    let tables = page.extract_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 2); // 2 rows
    assert_eq!(
        tables[0][0],
        vec![Some("A".to_string()), Some("B".to_string())]
    );
    assert_eq!(
        tables[0][1],
        vec![Some("C".to_string()), Some("D".to_string())]
    );
}

#[test]
fn test_extract_tables_empty_page() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    let settings = TableSettings::default();
    let tables = page.extract_tables(&settings);

    assert!(tables.is_empty());
}

#[test]
fn test_extract_tables_empty_cells() {
    // Table with no text inside cells
    let lines = vec![
        hline(10.0, 10.0, 110.0),
        hline(10.0, 30.0, 110.0),
        vline(10.0, 10.0, 30.0),
        vline(60.0, 10.0, 30.0),
        vline(110.0, 10.0, 30.0),
    ];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, vec![], vec![]);
    let settings = TableSettings::default();
    let tables = page.extract_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0][0], vec![None, None]);
}

#[test]
fn test_extract_table_returns_largest() {
    // Two tables: a 2x2 table and a single-cell table
    let lines = vec![
        // 2x2 table at top
        hline(10.0, 10.0, 110.0),
        hline(10.0, 30.0, 110.0),
        hline(10.0, 50.0, 110.0),
        vline(10.0, 10.0, 50.0),
        vline(60.0, 10.0, 50.0),
        vline(110.0, 10.0, 50.0),
        // Single-cell table at bottom (well separated)
        hline(200.0, 200.0, 300.0),
        hline(200.0, 250.0, 300.0),
        vline(200.0, 200.0, 250.0),
        vline(300.0, 200.0, 250.0),
    ];
    let chars = vec![
        make_char("A", 30.0, 15.0, 40.0, 25.0),
        make_char("B", 80.0, 15.0, 90.0, 25.0),
        make_char("C", 30.0, 35.0, 40.0, 45.0),
        make_char("D", 80.0, 35.0, 90.0, 45.0),
        make_char("X", 240.0, 220.0, 260.0, 240.0),
    ];
    let page = Page::with_geometry(0, 612.0, 792.0, chars, lines, vec![], vec![]);
    let settings = TableSettings::default();

    let table = page.extract_table(&settings);
    assert!(table.is_some());
    let table = table.unwrap();
    // Should be the 2x2 table (4 cells > 1 cell)
    assert_eq!(table.len(), 2); // 2 rows
    assert_eq!(table[0], vec![Some("A".to_string()), Some("B".to_string())]);
    assert_eq!(table[1], vec![Some("C".to_string()), Some("D".to_string())]);
}

#[test]
fn test_extract_table_none_when_no_tables() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    let settings = TableSettings::default();

    assert!(page.extract_table(&settings).is_none());
}

#[test]
fn test_find_tables_stream_strategy() {
    // Create words that align to form a 2x2 grid (Stream detects from text alignment)
    // 4 words arranged in a grid pattern
    let chars = vec![
        // Row 1, Col 1: "AA" at (10-30, 10-22)
        make_char("A", 10.0, 10.0, 20.0, 22.0),
        make_char("A", 20.0, 10.0, 30.0, 22.0),
        // Row 1, Col 2: "BB" at (50-70, 10-22)
        make_char("B", 50.0, 10.0, 60.0, 22.0),
        make_char("B", 60.0, 10.0, 70.0, 22.0),
        // Row 2, Col 1: "CC" at (10-30, 30-42)
        make_char("C", 10.0, 30.0, 20.0, 42.0),
        make_char("C", 20.0, 30.0, 30.0, 42.0),
        // Row 2, Col 2: "DD" at (50-70, 30-42)
        make_char("D", 50.0, 30.0, 60.0, 42.0),
        make_char("D", 60.0, 30.0, 70.0, 42.0),
        // Row 3, Col 1: "EE" at (10-30, 50-62) - need 3 rows for min_words_vertical=3
        make_char("E", 10.0, 50.0, 20.0, 62.0),
        make_char("E", 20.0, 50.0, 30.0, 62.0),
        // Row 3, Col 2: "FF" at (50-70, 50-62)
        make_char("F", 50.0, 50.0, 60.0, 62.0),
        make_char("F", 60.0, 50.0, 70.0, 62.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let settings = TableSettings {
        strategy: Strategy::Stream,
        min_words_vertical: 2,
        min_words_horizontal: 1,
        ..TableSettings::default()
    };
    let tables = page.find_tables(&settings);

    // Stream strategy should detect tables from text alignment
    assert!(!tables.is_empty());
}

#[test]
fn test_find_tables_explicit_strategy() {
    let chars = vec![
        make_char("X", 30.0, 15.0, 40.0, 25.0),
        make_char("Y", 80.0, 15.0, 90.0, 25.0),
    ];
    let page = Page::new(0, 612.0, 792.0, chars);
    let settings = TableSettings {
        strategy: Strategy::Explicit,
        explicit_lines: Some(ExplicitLines {
            horizontal_lines: vec![10.0, 30.0],
            vertical_lines: vec![10.0, 60.0, 110.0],
        }),
        ..TableSettings::default()
    };
    let tables = page.find_tables(&settings);

    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].cells.len(), 2);
    // Check text extraction works with explicit strategy
    let row = &tables[0].rows[0];
    assert_eq!(row[0].text, Some("X".to_string()));
    assert_eq!(row[1].text, Some("Y".to_string()));
}

#[test]
fn test_extract_tables_multiple_tables() {
    // Two well-separated tables
    let lines = vec![
        // Table 1: 1x2 at top-left
        hline(10.0, 10.0, 110.0),
        hline(10.0, 30.0, 110.0),
        vline(10.0, 10.0, 30.0),
        vline(60.0, 10.0, 30.0),
        vline(110.0, 10.0, 30.0),
        // Table 2: 1x1 at bottom-right (well separated)
        hline(300.0, 300.0, 400.0),
        hline(300.0, 350.0, 400.0),
        vline(300.0, 300.0, 350.0),
        vline(400.0, 300.0, 350.0),
    ];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, vec![], vec![]);
    let settings = TableSettings::default();
    let tables = page.extract_tables(&settings);

    assert_eq!(tables.len(), 2);
}

#[test]
fn test_find_tables_lattice_strict() {
    // LatticeStrict should only use line edges, not rect edges
    // Create a rect (would form edges in Lattice) but not lines
    let rects = vec![make_rect(10.0, 10.0, 100.0, 50.0)];
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], rects, vec![]);

    let strict_settings = TableSettings {
        strategy: Strategy::LatticeStrict,
        ..TableSettings::default()
    };
    let tables = page.find_tables(&strict_settings);
    // Strict mode ignores rect edges, so no tables
    assert!(tables.is_empty());

    // Normal Lattice should find a table from the rect
    let lattice_settings = TableSettings::default();
    let tables = page.find_tables(&lattice_settings);
    assert_eq!(tables.len(), 1);
}

// --- Warning accessor tests ---

#[test]
fn test_page_new_has_empty_warnings() {
    let page = Page::new(0, 612.0, 792.0, vec![]);
    assert!(page.warnings().is_empty());
}

#[test]
fn test_page_with_geometry_has_empty_warnings() {
    let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], vec![], vec![]);
    assert!(page.warnings().is_empty());
}

#[test]
fn test_page_with_geometry_and_images_has_empty_warnings() {
    let page =
        Page::with_geometry_and_images(0, 612.0, 792.0, vec![], vec![], vec![], vec![], vec![]);
    assert!(page.warnings().is_empty());
}
