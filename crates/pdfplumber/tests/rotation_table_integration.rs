//! Integration tests for table detection on rotated pages (Issue #154, US-154-2).
//!
//! Verifies that table detection and cell text extraction work correctly
//! for pages with /Rotate set to 0°, 90°, and 180°.
//!
//! Tests use lopdf to programmatically create PDF fixtures with simple
//! bordered tables (grid lines + cell text) and the /Rotate key.

use lopdf::{Document, Object, Stream, dictionary};
use pdfplumber::{Pdf, TableSettings, TextOptions};

// --- Helper: Create a PDF with a bordered table on a page with given rotation ---

/// Build a content stream that draws a 3-column, 3-row bordered table
/// with cell text, positioned near the top of a US Letter page (612×792).
///
/// Table layout (in native PDF bottom-left coordinates):
///   - Table origin: (72, 632) to (372, 722)  → 300pt wide, 90pt tall
///   - 3 rows, each 30pt tall
///   - 3 columns: 100pt each
///   - Row 0 (top):    "Name"   | "Age"  | "City"
///   - Row 1 (middle): "Alice"  | "30"   | "NYC"
///   - Row 2 (bottom): "Bob"    | "25"   | "London"
fn build_table_content_stream() -> Vec<u8> {
    let mut cs = String::new();

    // Line width
    cs.push_str("1 w\n");

    // Table boundaries (in PDF user space, bottom-left origin)
    let x0 = 72.0_f64;
    let y_top = 722.0_f64; // top of table
    let col_w = 100.0_f64;
    let row_h = 30.0_f64;
    let cols = 3;
    let rows = 3;
    let x1 = x0 + col_w * cols as f64;
    let y_bottom = y_top - row_h * rows as f64;

    // Draw horizontal lines (4 lines for 3 rows)
    for r in 0..=rows {
        let y = y_top - row_h * r as f64;
        cs.push_str(&format!("{x0} {y} m {x1} {y} l S\n"));
    }

    // Draw vertical lines (4 lines for 3 columns)
    for c in 0..=cols {
        let x = x0 + col_w * c as f64;
        cs.push_str(&format!("{x} {y_top} m {x} {y_bottom} l S\n"));
    }

    // Cell text: positioned at (col_x + 5, row_y - 20) for each cell
    let cell_data = [
        // (row, col, text)
        (0, 0, "Name"),
        (0, 1, "Age"),
        (0, 2, "City"),
        (1, 0, "Alice"),
        (1, 1, "30"),
        (1, 2, "NYC"),
        (2, 0, "Bob"),
        (2, 1, "25"),
        (2, 2, "London"),
    ];

    for &(row, col, text) in &cell_data {
        let tx = x0 + col_w * col as f64 + 5.0;
        let ty = y_top - row_h * row as f64 - 20.0;
        cs.push_str(&format!("BT /F1 10 Tf {tx} {ty} Td ({text}) Tj ET\n"));
    }

    cs.into_bytes()
}

/// Create a single-page PDF with a bordered table and the given /Rotate value.
fn create_table_pdf_with_rotation(rotation: i64) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content_bytes = build_table_content_stream();
    let stream = Stream::new(dictionary! {}, content_bytes);
    let content_id = doc.add_object(stream);

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Rotate" => rotation,
        "Contents" => content_id,
        "Resources" => dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        },
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

/// Create a multi-page PDF where page 0 has rotation=0 (baseline),
/// page 1 has rotation=90, and page 2 has rotation=180.
/// All pages have the same table content.
fn create_multi_rotation_table_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let rotations = [0i64, 90, 180];
    let mut page_ids = Vec::new();

    for &rot in &rotations {
        let content_bytes = build_table_content_stream();
        let stream = Stream::new(dictionary! {}, content_bytes);
        let content_id = doc.add_object(stream);

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Rotate" => rot,
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
        });
        page_ids.push(page_id);
    }

    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::from(id)).collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => 3i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

// ==================== Baseline: table detection on 0° page ====================

#[test]
fn table_rotation_0_detects_table() {
    let pdf_bytes = create_table_pdf_with_rotation(0);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 0);

    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "0° page with bordered table should detect at least one table"
    );
}

#[test]
fn table_rotation_0_has_expected_rows() {
    let pdf_bytes = create_table_pdf_with_rotation(0);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    if tables.is_empty() {
        panic!("baseline 0° table detection failed — cannot validate row count");
    }

    let table = &tables[0];
    assert!(
        table.rows.len() >= 2,
        "0° table should have at least 2 rows (header + data), got {}",
        table.rows.len()
    );
}

#[test]
fn table_rotation_0_cell_text_contains_expected_values() {
    let pdf_bytes = create_table_pdf_with_rotation(0);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    if tables.is_empty() {
        panic!("baseline 0° table detection failed — cannot validate cell text");
    }

    let table = &tables[0];
    let all_cell_text: Vec<String> = table.cells.iter().filter_map(|c| c.text.clone()).collect();
    let joined = all_cell_text.join(" ");

    assert!(
        joined.contains("Name"),
        "0° table cells should contain 'Name', got: {:?}",
        all_cell_text
    );
    assert!(
        joined.contains("Alice"),
        "0° table cells should contain 'Alice', got: {:?}",
        all_cell_text
    );
}

#[test]
fn table_rotation_0_page_text_contains_table_content() {
    let pdf_bytes = create_table_pdf_with_rotation(0);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("Name"),
        "0° page text should contain 'Name', got: {:?}",
        text.trim()
    );
    assert!(
        text.contains("Alice"),
        "0° page text should contain 'Alice', got: {:?}",
        text.trim()
    );
    assert!(
        text.contains("Bob"),
        "0° page text should contain 'Bob', got: {:?}",
        text.trim()
    );
}

// ==================== Table detection on 90° rotated page ====================

#[test]
fn table_rotation_90_detects_table() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 90);

    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "90° rotated page with bordered table should detect at least one table"
    );
}

#[test]
fn table_rotation_90_has_expected_rows() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(
        !tables.is_empty(),
        "90° table should be detected for row count validation"
    );

    let table = &tables[0];
    assert!(
        table.rows.len() >= 2,
        "90° table should have at least 2 rows, got {}",
        table.rows.len()
    );
}

#[test]
fn table_rotation_90_cell_text_extraction() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(
        !tables.is_empty(),
        "90° table should be detected for cell text validation"
    );

    let table = &tables[0];
    let all_cell_text: Vec<String> = table.cells.iter().filter_map(|c| c.text.clone()).collect();
    let joined = all_cell_text.join(" ");

    assert!(
        joined.contains("Name"),
        "90° table cells should contain 'Name', got: {:?}",
        all_cell_text
    );
    assert!(
        joined.contains("Alice"),
        "90° table cells should contain 'Alice', got: {:?}",
        all_cell_text
    );
}

#[test]
fn table_rotation_90_page_text_is_readable() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    // Text extraction on rotated pages should still produce readable text
    assert!(
        text.contains("Name") || text.contains("Alice") || text.contains("Bob"),
        "90° page should extract some table text, got: {:?}",
        text.trim()
    );
}

#[test]
fn table_rotation_90_edges_exist() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let edges = page.edges();

    assert!(
        !edges.is_empty(),
        "90° page should have edges derived from table grid lines"
    );
}

// ==================== Table detection on 180° rotated page ====================

#[test]
fn table_rotation_180_detects_table() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 180);

    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "180° rotated page with bordered table should detect at least one table"
    );
}

#[test]
fn table_rotation_180_has_expected_rows() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(
        !tables.is_empty(),
        "180° table should be detected for row count validation"
    );

    let table = &tables[0];
    assert!(
        table.rows.len() >= 2,
        "180° table should have at least 2 rows, got {}",
        table.rows.len()
    );
}

#[test]
fn table_rotation_180_cell_text_extraction() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(
        !tables.is_empty(),
        "180° table should be detected for cell text validation"
    );

    let table = &tables[0];
    let all_cell_text: Vec<String> = table.cells.iter().filter_map(|c| c.text.clone()).collect();
    let joined = all_cell_text.join(" ");

    assert!(
        joined.contains("Name"),
        "180° table cells should contain 'Name', got: {:?}",
        all_cell_text
    );
    assert!(
        joined.contains("Alice"),
        "180° table cells should contain 'Alice', got: {:?}",
        all_cell_text
    );
}

#[test]
fn table_rotation_180_page_text_is_readable() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("Name") || text.contains("Alice") || text.contains("Bob"),
        "180° page should extract some table text, got: {:?}",
        text.trim()
    );
}

#[test]
fn table_rotation_180_edges_exist() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let edges = page.edges();

    assert!(
        !edges.is_empty(),
        "180° page should have edges derived from table grid lines"
    );
}

// ==================== Table bounding box in correct coordinate space ====================

#[test]
fn table_rotation_0_bbox_within_page_bounds() {
    let pdf_bytes = create_table_pdf_with_rotation(0);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let (pw, ph) = (page.width(), page.height());
    let tables = page.find_tables(&TableSettings::default());

    for table in &tables {
        assert!(
            table.bbox.x0 >= -1.0 && table.bbox.x1 <= pw + 1.0,
            "0° table bbox x [{:.1}, {:.1}] should be within page width {:.1}",
            table.bbox.x0,
            table.bbox.x1,
            pw
        );
        assert!(
            table.bbox.top >= -1.0 && table.bbox.bottom <= ph + 1.0,
            "0° table bbox y [{:.1}, {:.1}] should be within page height {:.1}",
            table.bbox.top,
            table.bbox.bottom,
            ph
        );
    }
}

#[test]
fn table_rotation_90_bbox_within_page_bounds() {
    let pdf_bytes = create_table_pdf_with_rotation(90);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let (pw, ph) = (page.width(), page.height());
    let tables = page.find_tables(&TableSettings::default());

    for table in &tables {
        assert!(
            table.bbox.x0 >= -1.0 && table.bbox.x1 <= pw + 1.0,
            "90° table bbox x [{:.1}, {:.1}] should be within page width {:.1}",
            table.bbox.x0,
            table.bbox.x1,
            pw
        );
        assert!(
            table.bbox.top >= -1.0 && table.bbox.bottom <= ph + 1.0,
            "90° table bbox y [{:.1}, {:.1}] should be within page height {:.1}",
            table.bbox.top,
            table.bbox.bottom,
            ph
        );
    }
}

#[test]
fn table_rotation_180_bbox_within_page_bounds() {
    let pdf_bytes = create_table_pdf_with_rotation(180);
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();
    let (pw, ph) = (page.width(), page.height());
    let tables = page.find_tables(&TableSettings::default());

    for table in &tables {
        assert!(
            table.bbox.x0 >= -1.0 && table.bbox.x1 <= pw + 1.0,
            "180° table bbox x [{:.1}, {:.1}] should be within page width {:.1}",
            table.bbox.x0,
            table.bbox.x1,
            pw
        );
        assert!(
            table.bbox.top >= -1.0 && table.bbox.bottom <= ph + 1.0,
            "180° table bbox y [{:.1}, {:.1}] should be within page height {:.1}",
            table.bbox.top,
            table.bbox.bottom,
            ph
        );
    }
}

// ==================== Multi-rotation document table detection ====================

#[test]
fn multi_rotation_table_pdf_has_three_pages() {
    let pdf_bytes = create_multi_rotation_table_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    assert_eq!(pdf.page_count(), 3);
}

#[test]
fn multi_rotation_baseline_page_detects_table() {
    let pdf_bytes = create_multi_rotation_table_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 0);

    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "baseline page (0°) in multi-rotation PDF should detect a table"
    );
}

#[test]
fn multi_rotation_all_pages_have_edges() {
    let pdf_bytes = create_multi_rotation_table_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let rotations = [0, 90, 180];

    for (i, &rot) in rotations.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        assert_eq!(page.rotation(), rot);

        let edges = page.edges();
        assert!(
            !edges.is_empty(),
            "page {} ({}°) should have edges from table grid lines",
            i,
            rot
        );
    }
}

#[test]
fn multi_rotation_all_pages_have_text() {
    let pdf_bytes = create_multi_rotation_table_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let rotations = [0, 90, 180];

    for (i, &rot) in rotations.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        let text = page.extract_text(&TextOptions::default());

        assert!(
            !text.trim().is_empty(),
            "page {} ({}°) should extract some text",
            i,
            rot
        );
    }
}
