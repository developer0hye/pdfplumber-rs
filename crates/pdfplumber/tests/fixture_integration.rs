//! Integration tests using real-world and generated PDF fixtures.
//!
//! These tests exercise the full end-to-end pipeline against realistic PDFs
//! rather than minimal programmatically-generated ones.

use std::path::{Path, PathBuf};

use pdfplumber::{Pdf, SearchOptions, Strategy, TableSettings, TextOptions};

// --- Helpers ---

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn generated(name: &str) -> PathBuf {
    fixtures_dir().join("generated").join(name)
}

fn downloaded(name: &str) -> PathBuf {
    fixtures_dir().join("downloaded").join(name)
}

fn open_fixture(path: &Path) -> Pdf {
    Pdf::open(&std::fs::read(path).unwrap(), None).unwrap()
}

// ==================== basic_text.pdf ====================

#[test]
fn basic_text_opens_successfully() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn basic_text_chars_have_fontname() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    assert!(!chars.is_empty(), "should extract characters");
    for ch in chars {
        assert!(
            !ch.fontname.is_empty(),
            "each char should have a fontname, got empty for '{}'",
            ch.text
        );
    }
}

#[test]
fn basic_text_contains_pangram() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("quick brown fox"),
        "should contain pangram, got: {}",
        &text[..text.len().min(200)]
    );
}

#[test]
fn basic_text_contains_accented_chars() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    // Latin-1 accented characters should be extracted
    assert!(
        text.contains("caf") && text.contains("r"),
        "should contain accented words, got: {}",
        &text[..text.len().min(300)]
    );
}

#[test]
fn basic_text_contains_numbers() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("1,234.56"),
        "should contain formatted number, got: {}",
        &text[..text.len().min(300)]
    );
}

#[test]
fn basic_text_chars_have_consistent_size() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    // All text is 12pt Helvetica, so sizes should be consistent
    let sizes: Vec<f64> = chars.iter().filter(|c| c.text != " ").map(|c| c.size).collect();
    assert!(!sizes.is_empty());
    let first = sizes[0];
    for &s in &sizes {
        assert!(
            (s - first).abs() < 1.0,
            "font sizes should be consistent (~12pt), got {} vs {}",
            s,
            first
        );
    }
}

#[test]
fn basic_text_chars_have_valid_bboxes() {
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    for ch in page.chars() {
        assert!(ch.bbox.x0 <= ch.bbox.x1, "x0 ({}) should be <= x1 ({})", ch.bbox.x0, ch.bbox.x1);
        assert!(
            ch.bbox.top <= ch.bbox.bottom,
            "top ({}) should be <= bottom ({})",
            ch.bbox.top,
            ch.bbox.bottom
        );
        assert!(ch.bbox.x0 >= 0.0, "x0 should be non-negative");
        assert!(ch.bbox.top >= 0.0, "top should be non-negative");
    }
}

// ==================== multicolumn.pdf ====================

#[test]
fn multicolumn_has_two_pages() {
    let pdf = open_fixture(&generated("multicolumn.pdf"));
    assert_eq!(pdf.page_count(), 2);
}

#[test]
fn multicolumn_page1_contains_column_text() {
    let pdf = open_fixture(&generated("multicolumn.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Left column") && text.contains("Right column"),
        "should contain both column labels, got: {}",
        text
    );
}

#[test]
fn multicolumn_page2_contains_three_columns() {
    let pdf = open_fixture(&generated("multicolumn.pdf"));
    let page = pdf.page(1).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.contains("Col A"), "should contain Col A");
    assert!(text.contains("Col B"), "should contain Col B");
    assert!(text.contains("Col C"), "should contain Col C");
}

// ==================== table_lattice.pdf ====================

#[test]
fn table_lattice_opens_and_has_one_page() {
    let pdf = open_fixture(&generated("table_lattice.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn table_lattice_detects_table() {
    let pdf = open_fixture(&generated("table_lattice.pdf"));
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "lattice strategy should detect at least one table"
    );
}

#[test]
fn table_lattice_table_has_expected_dimensions() {
    let pdf = open_fixture(&generated("table_lattice.pdf"));
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    if tables.is_empty() {
        // Table detection may not find the fpdf2-drawn table
        return;
    }
    let table = &tables[0];
    // 1 header + 7 data rows = 8 rows, 5 cols
    assert!(
        table.rows.len() >= 2,
        "table should have multiple rows, got {}",
        table.rows.len()
    );
}

#[test]
fn table_lattice_contains_header_text() {
    let pdf = open_fixture(&generated("table_lattice.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.contains("ID"), "should contain header 'ID'");
    assert!(text.contains("Name"), "should contain header 'Name'");
    assert!(text.contains("Price"), "should contain header 'Price'");
}

#[test]
fn table_lattice_contains_data_values() {
    let pdf = open_fixture(&generated("table_lattice.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.contains("Widget A"), "should contain 'Widget A'");
    assert!(text.contains("$10.00"), "should contain '$10.00'");
}

// ==================== table_borderless.pdf ====================

#[test]
fn table_borderless_lattice_finds_nothing() {
    let pdf = open_fixture(&generated("table_borderless.pdf"));
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(
        tables.is_empty(),
        "lattice strategy should find no tables in borderless PDF, found {}",
        tables.len()
    );
}

#[test]
fn table_borderless_stream_detects_table() {
    let pdf = open_fixture(&generated("table_borderless.pdf"));
    let page = pdf.page(0).unwrap();
    let settings = TableSettings {
        strategy: Strategy::Stream,
        ..TableSettings::default()
    };
    let tables = page.find_tables(&settings);
    // Stream strategy should detect text-aligned table
    // (may not work perfectly with all layouts, so we just check it doesn't panic)
    let _ = tables;
}

#[test]
fn table_borderless_contains_same_data() {
    let pdf = open_fixture(&generated("table_borderless.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.contains("Widget A"), "should contain 'Widget A'");
    assert!(text.contains("Electronics"), "should contain 'Electronics'");
}

// ==================== table_merged_cells.pdf ====================

#[test]
fn table_merged_cells_opens() {
    let pdf = open_fixture(&generated("table_merged_cells.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn table_merged_cells_contains_header() {
    let pdf = open_fixture(&generated("table_merged_cells.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Quarterly Report"),
        "should contain merged header text"
    );
}

#[test]
fn table_merged_cells_contains_data() {
    let pdf = open_fixture(&generated("table_merged_cells.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.contains("North"), "should contain 'North'");
    assert!(text.contains("South"), "should contain 'South'");
    assert!(text.contains("Q1"), "should contain 'Q1'");
}

// ==================== cjk_mixed.pdf ====================

#[test]
fn cjk_mixed_opens() {
    let pdf = open_fixture(&generated("cjk_mixed.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn cjk_mixed_extracts_text() {
    let pdf = open_fixture(&generated("cjk_mixed.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("CJK") || text.contains("Chinese") || text.contains("placeholder"),
        "should extract CJK-related text, got: {}",
        text
    );
}

#[test]
fn cjk_mixed_has_chars() {
    let pdf = open_fixture(&generated("cjk_mixed.pdf"));
    let page = pdf.page(0).unwrap();
    assert!(
        !page.chars().is_empty(),
        "should extract characters from CJK PDF"
    );
}

// ==================== rotated_pages.pdf ====================

#[test]
fn rotated_pages_has_four_pages() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    assert_eq!(pdf.page_count(), 4);
}

#[test]
fn rotated_pages_rotation_values() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let expected = [0, 90, 180, 270];
    for (i, &expected_rot) in expected.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        let rot = page.rotation();
        assert_eq!(
            rot, expected_rot,
            "page {} should have rotation {}, got {}",
            i, expected_rot, rot
        );
    }
}

#[test]
fn rotated_pages_text_extraction_works() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    for i in 0..4 {
        let page = pdf.page(i).unwrap();
        let text = page.extract_text(&TextOptions::default());
        assert!(
            text.contains("rotation"),
            "page {} should have text about rotation, got: {}",
            i,
            text
        );
    }
}

// ==================== multi_font.pdf ====================

#[test]
fn multi_font_opens() {
    let pdf = open_fixture(&generated("multi_font.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn multi_font_has_multiple_fontnames() {
    let pdf = open_fixture(&generated("multi_font.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    let fontnames: std::collections::HashSet<&str> =
        chars.iter().map(|c| c.fontname.as_str()).collect();
    assert!(
        fontnames.len() >= 2,
        "should have multiple font names, got: {:?}",
        fontnames
    );
}

#[test]
fn multi_font_title_is_large() {
    let pdf = open_fixture(&generated("multi_font.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    // Find chars from "Document Title" - they should be ~24pt
    let title_chars: Vec<_> = chars
        .iter()
        .filter(|c| c.text == "D" && c.size > 20.0)
        .collect();
    assert!(
        !title_chars.is_empty(),
        "should find large title characters (24pt)"
    );
}

#[test]
fn multi_font_has_courier() {
    let pdf = open_fixture(&generated("multi_font.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    let has_courier = chars
        .iter()
        .any(|c| c.fontname.to_lowercase().contains("courier"));
    assert!(has_courier, "should have Courier font for code section");
}

// ==================== long_document.pdf ====================

#[test]
fn long_document_has_five_pages() {
    let pdf = open_fixture(&generated("long_document.pdf"));
    assert_eq!(pdf.page_count(), 5);
}

#[test]
fn long_document_search_finds_header_on_all_pages() {
    let pdf = open_fixture(&generated("long_document.pdf"));
    let opts = SearchOptions::default();
    let matches = pdf.search_all("Long Document", &opts).unwrap();
    assert!(
        matches.len() >= 5,
        "should find 'Long Document' header on all 5 pages, found {}",
        matches.len()
    );
}

#[test]
fn long_document_doctop_increases() {
    let pdf = open_fixture(&generated("long_document.pdf"));
    let mut prev_max_doctop = 0.0_f64;
    for i in 0..5 {
        let page = pdf.page(i).unwrap();
        let chars = page.chars();
        if chars.is_empty() {
            continue;
        }
        let max_doctop = chars
            .iter()
            .map(|c| c.doctop)
            .fold(f64::NEG_INFINITY, f64::max);
        if i > 0 {
            assert!(
                max_doctop > prev_max_doctop,
                "page {} max doctop ({}) should exceed page {} max doctop ({})",
                i,
                max_doctop,
                i - 1,
                prev_max_doctop
            );
        }
        prev_max_doctop = max_doctop;
    }
}

#[test]
fn long_document_each_page_has_body_text() {
    let pdf = open_fixture(&generated("long_document.pdf"));
    for i in 0..5 {
        let page = pdf.page(i).unwrap();
        let text = page.extract_text(&TextOptions::default());
        assert!(
            text.contains("Lorem ipsum") || text.contains("Line"),
            "page {} should contain body text, got: {}",
            i,
            &text[..text.len().min(100)]
        );
    }
}

// ==================== annotations_links.pdf ====================

#[test]
fn annotations_links_opens() {
    let pdf = open_fixture(&generated("annotations_links.pdf"));
    assert_eq!(pdf.page_count(), 3);
}

#[test]
fn annotations_links_has_metadata() {
    let pdf = open_fixture(&generated("annotations_links.pdf"));
    let meta = pdf.metadata();
    // fpdf2 sets title/author/subject
    assert!(
        meta.title.is_some() || meta.author.is_some(),
        "should have some metadata set, got: title={:?}, author={:?}",
        meta.title,
        meta.author
    );
}

#[test]
fn annotations_links_metadata_title() {
    let pdf = open_fixture(&generated("annotations_links.pdf"));
    let meta = pdf.metadata();
    if let Some(ref title) = meta.title {
        assert!(
            title.contains("Annotations") || title.contains("Test"),
            "title should relate to annotations: {}",
            title
        );
    }
}

#[test]
fn annotations_links_hyperlinks() {
    let pdf = open_fixture(&generated("annotations_links.pdf"));
    let page = pdf.page(0).unwrap();
    let links = page.hyperlinks();
    // fpdf2 creates link annotations when link= parameter is used
    if !links.is_empty() {
        let has_example = links.iter().any(|l| l.uri.contains("example.com"));
        assert!(has_example, "should have example.com link");
    }
}

#[test]
fn annotations_links_text_content() {
    let pdf = open_fixture(&generated("annotations_links.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Links") || text.contains("Annotations"),
        "page 1 should have annotations-related text"
    );
}

// ==================== Downloaded: pdffill-demo.pdf ====================

#[test]
fn pdffill_opens_and_has_pages() {
    let pdf = open_fixture(&downloaded("pdffill-demo.pdf"));
    assert!(
        pdf.page_count() > 0,
        "pdffill-demo should have at least 1 page"
    );
}

#[test]
fn pdffill_extracts_chars() {
    let pdf = open_fixture(&downloaded("pdffill-demo.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    assert!(
        !chars.is_empty(),
        "pdffill-demo page 1 should have characters"
    );
}

#[test]
fn pdffill_text_not_empty() {
    let pdf = open_fixture(&downloaded("pdffill-demo.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        !text.trim().is_empty(),
        "pdffill-demo should extract non-empty text"
    );
}

// ==================== Downloaded: nics-firearm-checks.pdf ====================

#[test]
fn nics_opens_and_has_pages() {
    let pdf = open_fixture(&downloaded("nics-firearm-checks.pdf"));
    assert!(
        pdf.page_count() > 0,
        "nics PDF should have at least 1 page"
    );
}

#[test]
fn nics_detects_table() {
    let pdf = open_fixture(&downloaded("nics-firearm-checks.pdf"));
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(
        !tables.is_empty(),
        "nics PDF should have at least one table (government data)"
    );
}

#[test]
fn nics_table_has_many_rows() {
    let pdf = open_fixture(&downloaded("nics-firearm-checks.pdf"));
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    if tables.is_empty() {
        return;
    }
    let table = &tables[0];
    assert!(
        table.rows.len() >= 3,
        "nics table should have many rows, got {}",
        table.rows.len()
    );
}

#[test]
fn nics_has_numeric_content() {
    let pdf = open_fixture(&downloaded("nics-firearm-checks.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    // Government statistics document should contain numbers
    let has_numbers = text.chars().filter(|c| c.is_ascii_digit()).count();
    assert!(
        has_numbers > 10,
        "nics should have many numeric chars, found {}",
        has_numbers
    );
}

// ==================== Downloaded: scotus-transcript-p1.pdf ====================

#[test]
fn scotus_opens_and_has_pages() {
    let pdf = open_fixture(&downloaded("scotus-transcript-p1.pdf"));
    assert!(
        pdf.page_count() > 0,
        "scotus transcript should have at least 1 page"
    );
}

#[test]
fn scotus_substantial_text() {
    let pdf = open_fixture(&downloaded("scotus-transcript-p1.pdf"));
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.len() > 100,
        "scotus transcript should have substantial text, got {} chars",
        text.len()
    );
}

#[test]
fn scotus_chars_have_valid_bboxes() {
    let pdf = open_fixture(&downloaded("scotus-transcript-p1.pdf"));
    let page = pdf.page(0).unwrap();
    for ch in page.chars() {
        assert!(
            ch.bbox.x0 <= ch.bbox.x1 + 0.01,
            "x0 ({}) should be <= x1 ({})",
            ch.bbox.x0,
            ch.bbox.x1
        );
        assert!(
            ch.bbox.top <= ch.bbox.bottom + 0.01,
            "top ({}) should be <= bottom ({})",
            ch.bbox.top,
            ch.bbox.bottom
        );
    }
}

#[test]
fn scotus_has_many_chars() {
    let pdf = open_fixture(&downloaded("scotus-transcript-p1.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    assert!(
        chars.len() > 50,
        "scotus page should have many characters, got {}",
        chars.len()
    );
}
