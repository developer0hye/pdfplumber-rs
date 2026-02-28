//! Serde serialization/deserialization round-trip tests.
//!
//! These tests verify that all public data types can be serialized to JSON
//! and deserialized back, producing equal values.

#![cfg(feature = "serde")]

use pdfplumber_core::*;

/// Helper: serialize to JSON string, deserialize back, assert equality.
fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    let restored: T = serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(*value, restored, "round-trip mismatch for JSON: {json}");
}

// --- Geometry types ---

#[test]
fn test_serde_point() {
    roundtrip(&Point::new(3.14, 2.72));
}

#[test]
fn test_serde_ctm() {
    roundtrip(&Ctm::new(2.0, 0.0, 0.0, 3.0, 10.0, 20.0));
    roundtrip(&Ctm::identity());
}

#[test]
fn test_serde_orientation() {
    roundtrip(&Orientation::Horizontal);
    roundtrip(&Orientation::Vertical);
    roundtrip(&Orientation::Diagonal);
}

#[test]
fn test_serde_bbox() {
    roundtrip(&BBox::new(10.0, 20.0, 300.0, 400.0));
}

// --- Text types ---

#[test]
fn test_serde_text_direction() {
    roundtrip(&TextDirection::Ltr);
    roundtrip(&TextDirection::Rtl);
    roundtrip(&TextDirection::Ttb);
    roundtrip(&TextDirection::Btt);
}

#[test]
fn test_serde_char() {
    let ch = Char {
        text: "A".to_string(),
        bbox: BBox::new(10.0, 20.0, 20.0, 32.0),
        fontname: "Helvetica".to_string(),
        size: 12.0,
        doctop: 20.0,
        upright: true,
        direction: TextDirection::Ltr,
        stroking_color: Some(Color::Rgb(1.0, 0.0, 0.0)),
        non_stroking_color: Some(Color::Gray(0.0)),
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: 65,
        mcid: None,
        tag: None,
    };
    roundtrip(&ch);
}

#[test]
fn test_serde_char_no_colors() {
    let ch = Char {
        text: "Z".to_string(),
        bbox: BBox::new(0.0, 0.0, 10.0, 12.0),
        fontname: "Courier".to_string(),
        size: 10.0,
        doctop: 0.0,
        upright: false,
        direction: TextDirection::Ttb,
        stroking_color: None,
        non_stroking_color: None,
        ctm: [0.0, 1.0, -1.0, 0.0, 50.0, 100.0],
        char_code: 90,
        mcid: None,
        tag: None,
    };
    roundtrip(&ch);
}

// --- Color types ---

#[test]
fn test_serde_color_gray() {
    roundtrip(&Color::Gray(0.5));
}

#[test]
fn test_serde_color_rgb() {
    roundtrip(&Color::Rgb(0.1, 0.2, 0.3));
}

#[test]
fn test_serde_color_cmyk() {
    roundtrip(&Color::Cmyk(0.1, 0.2, 0.3, 0.4));
}

#[test]
fn test_serde_color_other() {
    roundtrip(&Color::Other(vec![0.1, 0.2, 0.3, 0.4, 0.5]));
}

// --- Painting types ---

#[test]
fn test_serde_fill_rule() {
    roundtrip(&FillRule::NonZeroWinding);
    roundtrip(&FillRule::EvenOdd);
}

#[test]
fn test_serde_dash_pattern() {
    roundtrip(&DashPattern::solid());
    roundtrip(&DashPattern::new(vec![5.0, 3.0, 1.0], 2.0));
}

// --- Shape types ---

#[test]
fn test_serde_line() {
    let line = Line {
        x0: 10.0,
        top: 20.0,
        x1: 100.0,
        bottom: 20.0,
        line_width: 1.5,
        stroke_color: Color::Rgb(1.0, 0.0, 0.0),
        orientation: Orientation::Horizontal,
    };
    roundtrip(&line);
}

#[test]
fn test_serde_rect() {
    let rect = Rect {
        x0: 50.0,
        top: 100.0,
        x1: 200.0,
        bottom: 300.0,
        line_width: 2.0,
        stroke: true,
        fill: true,
        stroke_color: Color::Gray(0.0),
        fill_color: Color::Cmyk(0.0, 1.0, 1.0, 0.0),
    };
    roundtrip(&rect);
}

#[test]
fn test_serde_curve() {
    let curve = Curve {
        x0: 0.0,
        top: 50.0,
        x1: 100.0,
        bottom: 100.0,
        pts: vec![(0.0, 100.0), (30.0, 50.0), (70.0, 50.0), (100.0, 100.0)],
        line_width: 1.0,
        stroke: true,
        fill: false,
        stroke_color: Color::black(),
        fill_color: Color::black(),
    };
    roundtrip(&curve);
}

// --- Edge types ---

#[test]
fn test_serde_edge_source() {
    roundtrip(&EdgeSource::Line);
    roundtrip(&EdgeSource::RectTop);
    roundtrip(&EdgeSource::RectBottom);
    roundtrip(&EdgeSource::RectLeft);
    roundtrip(&EdgeSource::RectRight);
    roundtrip(&EdgeSource::Curve);
    roundtrip(&EdgeSource::Stream);
    roundtrip(&EdgeSource::Explicit);
}

#[test]
fn test_serde_edge() {
    let edge = Edge {
        x0: 10.0,
        top: 20.0,
        x1: 200.0,
        bottom: 20.0,
        orientation: Orientation::Horizontal,
        source: EdgeSource::Line,
    };
    roundtrip(&edge);
}

// --- Image types ---

#[test]
fn test_serde_image_metadata() {
    let meta = ImageMetadata {
        src_width: Some(1920),
        src_height: Some(1080),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
    };
    roundtrip(&meta);
}

#[test]
fn test_serde_image() {
    let img = Image {
        x0: 72.0,
        top: 100.0,
        x1: 272.0,
        bottom: 250.0,
        width: 200.0,
        height: 150.0,
        name: "Im0".to_string(),
        src_width: Some(1920),
        src_height: Some(1080),
        bits_per_component: Some(8),
        color_space: Some("DeviceRGB".to_string()),
        data: None,
        filter: None,
        mime_type: None,
    };
    roundtrip(&img);
}

// --- Word type ---

#[test]
fn test_serde_word() {
    let word = Word {
        text: "Hello".to_string(),
        bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
        doctop: 100.0,
        direction: TextDirection::Ltr,
        chars: vec![Char {
            text: "H".to_string(),
            bbox: BBox::new(10.0, 100.0, 20.0, 112.0),
            fontname: "Helvetica".to_string(),
            size: 12.0,
            doctop: 100.0,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 72,
            mcid: None,
            tag: None,
        }],
    };
    roundtrip(&word);
}

// --- Table types ---

#[test]
fn test_serde_cell() {
    let cell = Cell {
        bbox: BBox::new(10.0, 20.0, 110.0, 40.0),
        text: Some("content".to_string()),
    };
    roundtrip(&cell);

    let empty_cell = Cell {
        bbox: BBox::new(0.0, 0.0, 50.0, 20.0),
        text: None,
    };
    roundtrip(&empty_cell);
}

#[test]
fn test_serde_table() {
    let cell = Cell {
        bbox: BBox::new(10.0, 20.0, 110.0, 40.0),
        text: Some("data".to_string()),
    };
    let table = Table {
        bbox: BBox::new(10.0, 20.0, 110.0, 40.0),
        cells: vec![cell.clone()],
        rows: vec![vec![cell.clone()]],
        columns: vec![vec![cell]],
    };
    roundtrip(&table);
}

#[test]
fn test_serde_strategy() {
    roundtrip(&Strategy::Lattice);
    roundtrip(&Strategy::LatticeStrict);
    roundtrip(&Strategy::Stream);
    roundtrip(&Strategy::Explicit);
}

// --- Path types ---

#[test]
fn test_serde_path_segment() {
    roundtrip(&PathSegment::MoveTo(Point::new(10.0, 20.0)));
    roundtrip(&PathSegment::LineTo(Point::new(30.0, 40.0)));
    roundtrip(&PathSegment::CurveTo {
        cp1: Point::new(10.0, 20.0),
        cp2: Point::new(30.0, 40.0),
        end: Point::new(50.0, 60.0),
    });
    roundtrip(&PathSegment::ClosePath);
}

#[test]
fn test_serde_path() {
    let path = Path {
        segments: vec![
            PathSegment::MoveTo(Point::new(0.0, 0.0)),
            PathSegment::LineTo(Point::new(100.0, 0.0)),
            PathSegment::CurveTo {
                cp1: Point::new(100.0, 50.0),
                cp2: Point::new(50.0, 100.0),
                end: Point::new(0.0, 100.0),
            },
            PathSegment::ClosePath,
        ],
    };
    roundtrip(&path);
}

// --- Layout types ---

#[test]
fn test_serde_text_line() {
    let line = TextLine {
        words: vec![Word {
            text: "Hello".to_string(),
            bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
            doctop: 100.0,
            direction: TextDirection::Ltr,
            chars: vec![],
        }],
        bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
    };
    roundtrip(&line);
}

#[test]
fn test_serde_text_block() {
    let block = TextBlock {
        lines: vec![TextLine {
            words: vec![],
            bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
        }],
        bbox: BBox::new(10.0, 100.0, 200.0, 200.0),
    };
    roundtrip(&block);
}

// --- Intersection type ---

#[test]
fn test_serde_intersection() {
    let i = Intersection { x: 10.0, y: 20.0 };
    roundtrip(&i);
}

// --- JSON structure verification ---

#[test]
fn test_char_json_fields() {
    let ch = Char {
        text: "X".to_string(),
        bbox: BBox::new(1.0, 2.0, 3.0, 4.0),
        fontname: "Arial".to_string(),
        size: 14.0,
        doctop: 2.0,
        upright: true,
        direction: TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: None,
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: 88,
        mcid: None,
        tag: None,
    };
    let json: serde_json::Value = serde_json::to_value(&ch).unwrap();
    assert_eq!(json["text"], "X");
    assert_eq!(json["fontname"], "Arial");
    assert_eq!(json["size"], 14.0);
    assert_eq!(json["upright"], true);
    assert_eq!(json["char_code"], 88);
    assert!(json["bbox"].is_object());
    assert_eq!(json["bbox"]["x0"], 1.0);
    assert_eq!(json["bbox"]["top"], 2.0);
}

#[test]
fn test_color_json_tagged() {
    // Verify Color enum serializes with tag/content
    let gray = Color::Gray(0.5);
    let json = serde_json::to_string(&gray).unwrap();
    let restored: Color = serde_json::from_str(&json).unwrap();
    assert_eq!(gray, restored);
}
