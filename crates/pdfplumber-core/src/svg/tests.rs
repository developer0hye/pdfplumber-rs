use super::*;
use crate::edges::EdgeSource;
use crate::geometry::Orientation;
use crate::painting::Color;
use crate::table::Cell;
use crate::text::TextDirection;

// --- Existing US-067 tests ---

#[test]
fn test_svg_default_options() {
    let opts = SvgOptions::default();
    assert!(opts.width.is_none());
    assert!(opts.height.is_none());
    assert!((opts.scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_svg_generation_simple_page() {
    let renderer = SvgRenderer::new(612.0, 792.0); // US Letter
    let svg = renderer.to_svg(&SvgOptions::default());

    // Must be valid SVG with proper namespace
    assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("version=\"1.1\""));
    // Must start with <svg and end with </svg>
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
}

#[test]
fn test_svg_has_correct_viewbox() {
    let renderer = SvgRenderer::new(612.0, 792.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("viewBox=\"0 0 612 792\""));
}

#[test]
fn test_svg_has_correct_dimensions_default() {
    let renderer = SvgRenderer::new(612.0, 792.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // Default scale=1.0, so SVG width/height match page dimensions
    assert!(svg.contains("width=\"612\""));
    assert!(svg.contains("height=\"792\""));
}

#[test]
fn test_svg_has_correct_dimensions_with_scale() {
    let renderer = SvgRenderer::new(612.0, 792.0);
    let svg = renderer.to_svg(&SvgOptions {
        scale: 2.0,
        ..Default::default()
    });

    // Scale=2.0, so SVG width/height are doubled
    assert!(svg.contains("width=\"1224\""));
    assert!(svg.contains("height=\"1584\""));
    // viewBox stays the same (page coordinates)
    assert!(svg.contains("viewBox=\"0 0 612 792\""));
}

#[test]
fn test_svg_has_correct_dimensions_with_explicit_size() {
    let renderer = SvgRenderer::new(612.0, 792.0);
    let svg = renderer.to_svg(&SvgOptions {
        width: Some(800.0),
        height: Some(600.0),
        scale: 1.0,
    });

    assert!(svg.contains("width=\"800\""));
    assert!(svg.contains("height=\"600\""));
    // viewBox still matches page dimensions
    assert!(svg.contains("viewBox=\"0 0 612 792\""));
}

#[test]
fn test_svg_has_page_boundary_rect() {
    let renderer = SvgRenderer::new(612.0, 792.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // Must contain a rectangle for the page boundary
    assert!(svg.contains("<rect"));
    assert!(svg.contains("width=\"612\""));
    assert!(svg.contains("height=\"792\""));
    assert!(svg.contains("fill=\"white\""));
    assert!(svg.contains("stroke=\"black\""));
}

#[test]
fn test_svg_valid_markup() {
    let renderer = SvgRenderer::new(100.0, 200.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // Basic structural validity
    let open_tags = svg.matches("<svg").count();
    let close_tags = svg.matches("</svg>").count();
    assert_eq!(open_tags, 1, "Should have exactly one <svg> opening tag");
    assert_eq!(close_tags, 1, "Should have exactly one </svg> closing tag");

    // Self-closing rect tag
    assert!(svg.contains("/>"), "Rect should be self-closing");
}

#[test]
fn test_svg_coordinate_system_top_left_origin() {
    let renderer = SvgRenderer::new(400.0, 300.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // viewBox starts at 0,0 (top-left origin)
    assert!(svg.contains("viewBox=\"0 0 400 300\""));
    // Page rect starts at x=0, y=0
    assert!(svg.contains("x=\"0\""));
    assert!(svg.contains("y=\"0\""));
}

#[test]
fn test_svg_small_page() {
    let renderer = SvgRenderer::new(50.0, 50.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("viewBox=\"0 0 50 50\""));
    assert!(svg.contains("width=\"50\""));
    assert!(svg.contains("height=\"50\""));
}

// --- US-068 tests: DrawStyle ---

#[test]
fn test_draw_style_default() {
    let style = DrawStyle::default();
    assert!(style.fill.is_none());
    assert_eq!(style.stroke.as_deref(), Some("black"));
    assert!((style.stroke_width - 0.5).abs() < f64::EPSILON);
    assert!((style.opacity - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_draw_style_chars_default() {
    let style = DrawStyle::chars_default();
    assert!(style.fill.is_none());
    assert_eq!(style.stroke.as_deref(), Some("blue"));
}

#[test]
fn test_draw_style_lines_default() {
    let style = DrawStyle::lines_default();
    assert_eq!(style.stroke.as_deref(), Some("red"));
}

#[test]
fn test_draw_style_rects_default() {
    let style = DrawStyle::rects_default();
    assert_eq!(style.stroke.as_deref(), Some("green"));
}

#[test]
fn test_draw_style_tables_default() {
    let style = DrawStyle::tables_default();
    assert_eq!(style.fill.as_deref(), Some("lightblue"));
}

#[test]
fn test_draw_style_to_svg_style_full() {
    let style = DrawStyle {
        fill: Some("red".to_string()),
        stroke: Some("blue".to_string()),
        stroke_width: 2.0,
        opacity: 0.5,
    };
    let s = style.to_svg_style();
    assert!(s.contains("fill:red"));
    assert!(s.contains("stroke:blue"));
    assert!(s.contains("stroke-width:2"));
    assert!(s.contains("opacity:0.5"));
}

#[test]
fn test_draw_style_to_svg_style_no_fill() {
    let style = DrawStyle {
        fill: None,
        stroke: Some("black".to_string()),
        stroke_width: 1.0,
        opacity: 1.0,
    };
    let s = style.to_svg_style();
    assert!(s.contains("fill:none"));
    assert!(s.contains("stroke:black"));
    // opacity=1.0 should be omitted
    assert!(!s.contains("opacity"));
}

// --- US-068 tests: draw_chars ---

fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
    Char {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        fontname: "Helvetica".to_string(),
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

#[test]
fn test_draw_chars_adds_rect_elements() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let chars = vec![
        make_char("A", 10.0, 20.0, 18.0, 32.0),
        make_char("B", 20.0, 20.0, 28.0, 32.0),
    ];
    renderer.draw_chars(&chars, &DrawStyle::chars_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    // Should contain rect elements for each char (page boundary + 2 char rects)
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 3, "1 page boundary + 2 char bboxes");
    assert!(svg.contains("stroke:blue"));
}

#[test]
fn test_draw_chars_correct_coordinates() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let chars = vec![make_char("X", 10.0, 20.0, 25.0, 35.0)];
    renderer.draw_chars(&chars, &DrawStyle::chars_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    // Check the char rect has correct position
    assert!(svg.contains("x=\"10\""));
    assert!(svg.contains("y=\"20\""));
    assert!(svg.contains("width=\"15\"")); // 25 - 10
    assert!(svg.contains("height=\"15\"")); // 35 - 20
}

// --- US-068 tests: draw_rects ---

fn make_rect(x0: f64, top: f64, x1: f64, bottom: f64) -> Rect {
    Rect {
        x0,
        top,
        x1,
        bottom,
        line_width: 1.0,
        stroke: true,
        fill: false,
        stroke_color: Color::Gray(0.0),
        fill_color: Color::Gray(1.0),
    }
}

#[test]
fn test_draw_rects_adds_rect_elements() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let rects = vec![make_rect(50.0, 50.0, 150.0, 100.0)];
    renderer.draw_rects(&rects, &DrawStyle::rects_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 2, "1 page boundary + 1 rect overlay");
    assert!(svg.contains("stroke:green"));
}

// --- US-068 tests: draw_lines ---

fn make_line(x0: f64, top: f64, x1: f64, bottom: f64) -> Line {
    Line {
        x0,
        top,
        x1,
        bottom,
        line_width: 1.0,
        stroke_color: Color::Gray(0.0),
        orientation: Orientation::Horizontal,
    }
}

#[test]
fn test_draw_lines_adds_line_elements() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let lines = vec![
        make_line(10.0, 50.0, 190.0, 50.0),
        make_line(100.0, 10.0, 100.0, 190.0),
    ];
    renderer.draw_lines(&lines, &DrawStyle::lines_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    let line_count = svg.matches("<line").count();
    assert_eq!(line_count, 2);
    assert!(svg.contains("stroke:red"));
}

#[test]
fn test_draw_lines_correct_coordinates() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let lines = vec![make_line(10.0, 50.0, 190.0, 50.0)];
    renderer.draw_lines(&lines, &DrawStyle::lines_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("x1=\"10\""));
    assert!(svg.contains("y1=\"50\""));
    assert!(svg.contains("x2=\"190\""));
    assert!(svg.contains("y2=\"50\""));
}

// --- US-068 tests: draw_edges ---

fn make_edge(x0: f64, top: f64, x1: f64, bottom: f64) -> Edge {
    Edge {
        x0,
        top,
        x1,
        bottom,
        orientation: Orientation::Horizontal,
        source: EdgeSource::Line,
    }
}

#[test]
fn test_draw_edges_adds_line_elements() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let edges = vec![make_edge(0.0, 100.0, 200.0, 100.0)];
    renderer.draw_edges(&edges, &DrawStyle::edges_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    let line_count = svg.matches("<line").count();
    assert_eq!(line_count, 1);
    assert!(svg.contains("stroke:orange"));
}

// --- US-068 tests: draw_tables ---

fn make_table() -> Table {
    Table {
        bbox: BBox::new(10.0, 10.0, 200.0, 100.0),
        cells: vec![
            Cell {
                bbox: BBox::new(10.0, 10.0, 100.0, 50.0),
                text: Some("A".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 10.0, 200.0, 50.0),
                text: Some("B".to_string()),
            },
            Cell {
                bbox: BBox::new(10.0, 50.0, 100.0, 100.0),
                text: Some("C".to_string()),
            },
            Cell {
                bbox: BBox::new(100.0, 50.0, 200.0, 100.0),
                text: Some("D".to_string()),
            },
        ],
        rows: vec![],
        columns: vec![],
    }
}

#[test]
fn test_draw_tables_adds_cell_rects() {
    let mut renderer = SvgRenderer::new(300.0, 200.0);
    let tables = vec![make_table()];
    renderer.draw_tables(&tables, &DrawStyle::tables_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    // 1 page boundary + 4 cell rects
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 5);
    assert!(svg.contains("fill:lightblue"));
}

// --- US-068 tests: mixed overlays ---

#[test]
fn test_svg_with_mixed_objects() {
    let mut renderer = SvgRenderer::new(400.0, 400.0);

    let chars = vec![make_char("H", 10.0, 10.0, 20.0, 22.0)];
    let lines = vec![make_line(0.0, 100.0, 400.0, 100.0)];
    let rects = vec![make_rect(50.0, 50.0, 150.0, 80.0)];

    renderer.draw_chars(&chars, &DrawStyle::chars_default());
    renderer.draw_lines(&lines, &DrawStyle::lines_default());
    renderer.draw_rects(&rects, &DrawStyle::rects_default());

    let svg = renderer.to_svg(&SvgOptions::default());

    // Verify all object types present
    assert!(svg.contains("stroke:blue"), "chars overlay");
    assert!(svg.contains("stroke:red"), "lines overlay");
    assert!(svg.contains("stroke:green"), "rects overlay");
    // 1 page boundary rect + 1 char rect + 1 rect overlay = 3 rects, 1 line
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 3);
    let line_count = svg.matches("<line").count();
    assert_eq!(line_count, 1);
}

#[test]
fn test_style_customization() {
    let mut renderer = SvgRenderer::new(100.0, 100.0);
    let chars = vec![make_char("Z", 5.0, 5.0, 15.0, 17.0)];
    let custom_style = DrawStyle {
        fill: Some("yellow".to_string()),
        stroke: Some("purple".to_string()),
        stroke_width: 3.0,
        opacity: 0.5,
    };
    renderer.draw_chars(&chars, &custom_style);
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("fill:yellow"));
    assert!(svg.contains("stroke:purple"));
    assert!(svg.contains("stroke-width:3"));
    assert!(svg.contains("opacity:0.5"));
}

#[test]
fn test_empty_draw_no_overlays() {
    let renderer = SvgRenderer::new(100.0, 100.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // Only the page boundary rect
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 1);
    let line_count = svg.matches("<line").count();
    assert_eq!(line_count, 0);
}

#[test]
fn test_draw_chars_empty_slice() {
    let mut renderer = SvgRenderer::new(100.0, 100.0);
    renderer.draw_chars(&[], &DrawStyle::chars_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    // Only the page boundary rect
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 1);
}

// --- US-069 tests: SvgDebugOptions ---

#[test]
fn test_svg_debug_options_default() {
    let opts = SvgDebugOptions::default();
    assert!(opts.show_edges);
    assert!(opts.show_intersections);
    assert!(opts.show_cells);
    assert!(opts.show_tables);
}

#[test]
fn test_svg_debug_options_selective() {
    let opts = SvgDebugOptions {
        show_edges: true,
        show_intersections: false,
        show_cells: false,
        show_tables: true,
    };
    assert!(opts.show_edges);
    assert!(!opts.show_intersections);
    assert!(!opts.show_cells);
    assert!(opts.show_tables);
}

// --- US-069 tests: DrawStyle defaults for debug ---

#[test]
fn test_draw_style_intersections_default() {
    let style = DrawStyle::intersections_default();
    // Intersections should be filled circles
    assert!(style.fill.is_some());
    assert!(style.stroke.is_some());
}

#[test]
fn test_draw_style_cells_default() {
    let style = DrawStyle::cells_default();
    // Cells should have dashed stroke
    assert!(style.stroke.is_some());
}

// --- US-069 tests: draw_intersections ---

#[test]
fn test_draw_intersections_adds_circle_elements() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let intersections = vec![
        Intersection { x: 50.0, y: 50.0 },
        Intersection { x: 100.0, y: 50.0 },
        Intersection { x: 50.0, y: 100.0 },
    ];
    renderer.draw_intersections(&intersections, &DrawStyle::intersections_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    let circle_count = svg.matches("<circle").count();
    assert_eq!(circle_count, 3, "Should have 3 intersection circles");
}

#[test]
fn test_draw_intersections_correct_coordinates() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let intersections = vec![Intersection { x: 75.0, y: 125.0 }];
    renderer.draw_intersections(&intersections, &DrawStyle::intersections_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("cx=\"75\""));
    assert!(svg.contains("cy=\"125\""));
}

#[test]
fn test_draw_intersections_empty_slice() {
    let mut renderer = SvgRenderer::new(100.0, 100.0);
    renderer.draw_intersections(&[], &DrawStyle::intersections_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    let circle_count = svg.matches("<circle").count();
    assert_eq!(circle_count, 0);
}

// --- US-069 tests: draw_cells (dashed lines) ---

#[test]
fn test_draw_cells_adds_rect_elements_with_dashed_style() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let cells = vec![
        Cell {
            bbox: BBox::new(10.0, 10.0, 100.0, 50.0),
            text: None,
        },
        Cell {
            bbox: BBox::new(100.0, 10.0, 200.0, 50.0),
            text: None,
        },
    ];
    renderer.draw_cells(&cells, &DrawStyle::cells_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    // 1 page boundary + 2 cell rects
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 3);
    // Cell rects should have dashed stroke
    assert!(svg.contains("stroke-dasharray"));
}

#[test]
fn test_draw_cells_correct_coordinates() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);
    let cells = vec![Cell {
        bbox: BBox::new(20.0, 30.0, 80.0, 90.0),
        text: None,
    }];
    renderer.draw_cells(&cells, &DrawStyle::cells_default());
    let svg = renderer.to_svg(&SvgOptions::default());

    assert!(svg.contains("x=\"20\""));
    assert!(svg.contains("y=\"30\""));
    assert!(svg.contains("width=\"60\"")); // 80 - 20
    assert!(svg.contains("height=\"60\"")); // 90 - 30
}

// --- US-069 tests: debug_tablefinder_svg via SvgRenderer ---

#[test]
fn test_debug_tablefinder_svg_with_table() {
    let mut renderer = SvgRenderer::new(300.0, 200.0);

    // Simulate table detection pipeline outputs
    let edges = vec![
        make_edge(10.0, 10.0, 200.0, 10.0),
        make_edge(10.0, 50.0, 200.0, 50.0),
        make_edge(10.0, 100.0, 200.0, 100.0),
    ];
    let intersections = vec![
        Intersection { x: 10.0, y: 10.0 },
        Intersection { x: 200.0, y: 10.0 },
        Intersection { x: 10.0, y: 50.0 },
        Intersection { x: 200.0, y: 50.0 },
    ];
    let cells = vec![Cell {
        bbox: BBox::new(10.0, 10.0, 200.0, 50.0),
        text: None,
    }];
    let tables = vec![Table {
        bbox: BBox::new(10.0, 10.0, 200.0, 100.0),
        cells: cells.clone(),
        rows: vec![],
        columns: vec![],
    }];

    let debug_opts = SvgDebugOptions::default();

    if debug_opts.show_edges {
        renderer.draw_edges(&edges, &DrawStyle::edges_default());
    }
    if debug_opts.show_intersections {
        renderer.draw_intersections(&intersections, &DrawStyle::intersections_default());
    }
    if debug_opts.show_cells {
        renderer.draw_cells(&cells, &DrawStyle::cells_default());
    }
    if debug_opts.show_tables {
        renderer.draw_tables(&tables, &DrawStyle::tables_default());
    }

    let svg = renderer.to_svg(&SvgOptions::default());

    // Edges rendered as lines (red)
    assert!(svg.contains("<line"), "Should contain edge lines");
    // Intersections rendered as circles
    assert!(
        svg.contains("<circle"),
        "Should contain intersection circles"
    );
    // Cells rendered as dashed rects
    assert!(
        svg.contains("stroke-dasharray"),
        "Should contain dashed cell boundaries"
    );
    // Tables rendered as filled rects
    assert!(svg.contains("fill:lightblue"), "Should contain table fill");
}

#[test]
fn test_debug_tablefinder_svg_no_tables() {
    let renderer = SvgRenderer::new(300.0, 200.0);
    let svg = renderer.to_svg(&SvgOptions::default());

    // No edges, intersections, cells, or tables - just the page boundary
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    let rect_count = svg.matches("<rect").count();
    assert_eq!(rect_count, 1, "Only page boundary rect");
}

#[test]
fn test_debug_tablefinder_svg_selective_show() {
    let mut renderer = SvgRenderer::new(200.0, 200.0);

    let edges = vec![make_edge(10.0, 10.0, 200.0, 10.0)];
    let intersections = vec![Intersection { x: 10.0, y: 10.0 }];

    let debug_opts = SvgDebugOptions {
        show_edges: true,
        show_intersections: false,
        show_cells: false,
        show_tables: false,
    };

    if debug_opts.show_edges {
        renderer.draw_edges(&edges, &DrawStyle::edges_default());
    }
    if debug_opts.show_intersections {
        renderer.draw_intersections(&intersections, &DrawStyle::intersections_default());
    }

    let svg = renderer.to_svg(&SvgOptions::default());

    // Edges should be present
    assert!(svg.contains("<line"), "Edges should be shown");
    // Intersections should NOT be present
    assert!(!svg.contains("<circle"), "Intersections should be hidden");
}
