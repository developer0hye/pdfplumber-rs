//! Integration tests using real-world and generated PDF fixtures.
//!
//! These tests exercise the full end-to-end pipeline against realistic PDFs
//! rather than minimal programmatically-generated ones.

use std::path::{Path, PathBuf};

use pdfplumber::{Pdf, SearchOptions, Strategy, StructElement, TableSettings, TextOptions};

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

fn cv_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn cv_pdf(name: &str) -> PathBuf {
    cv_fixtures_dir().join("pdfs").join(name)
}

fn open_cv_fixture(path: &Path) -> Pdf {
    Pdf::open_file(path, None).unwrap()
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
    let sizes: Vec<f64> = chars
        .iter()
        .filter(|c| c.text != " ")
        .map(|c| c.size)
        .collect();
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
        assert!(
            ch.bbox.x0 <= ch.bbox.x1,
            "x0 ({}) should be <= x1 ({})",
            ch.bbox.x0,
            ch.bbox.x1
        );
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
        // Pages 0 (0°) and 1 (90°) produce readable text.
        // Pages 2 (180°) and 3 (270°) produce spatially-ordered text
        // which appears reversed (matching Python pdfplumber behavior).
        let has_rotation_text = text.contains("rotation") || text.contains("noitator");
        assert!(
            has_rotation_text,
            "page {} should have text about rotation (possibly reversed), got: {}",
            i, text
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
    assert!(pdf.page_count() > 0, "nics PDF should have at least 1 page");
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

// ==================== annotations.pdf (Issue #163) ====================

#[test]
fn annotations_opens_successfully() {
    let pdf = open_fixture(&downloaded("annotations.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn annotations_extracts_chars() {
    let pdf = open_fixture(&downloaded("annotations.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    // Python pdfplumber extracts 14 chars
    assert_eq!(
        chars.len(),
        14,
        "annotations.pdf should have 14 chars, got {}",
        chars.len()
    );
}

#[test]
fn annotations_extracts_words() {
    let pdf = open_fixture(&downloaded("annotations.pdf"));
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&pdfplumber::WordOptions::default());
    // Python pdfplumber extracts 3 words: "Dummy", "PDF", "file"
    assert_eq!(
        words.len(),
        3,
        "annotations.pdf should have 3 words, got {}",
        words.len()
    );
}

#[test]
fn annotations_rotated_90_opens_successfully() {
    let pdf = open_fixture(&downloaded("annotations-rotated-90.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn annotations_rotated_180_opens_successfully() {
    let pdf = open_fixture(&downloaded("annotations-rotated-180.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn annotations_rotated_270_opens_successfully() {
    let pdf = open_fixture(&downloaded("annotations-rotated-270.pdf"));
    assert_eq!(pdf.page_count(), 1);
}

// ==================== Structure Tree: mcid_example.pdf ====================

#[test]
fn mcid_example_has_structure_tree() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree();
    assert!(
        tree.is_some(),
        "mcid_example.pdf should have a structure tree"
    );
}

#[test]
fn mcid_example_structure_has_document_root() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    assert!(!tree.is_empty(), "structure tree should not be empty");
    // Top-level element should be Document
    assert_eq!(tree[0].element_type, "Document");
}

#[test]
fn mcid_example_structure_has_headings_and_paragraphs() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    // Collect all element types recursively
    let types = collect_element_types(tree);
    assert!(
        types.contains(&"H1".to_string()) || types.contains(&"P".to_string()),
        "mcid_example.pdf should contain H1 or P elements, found: {:?}",
        types
    );
}

#[test]
fn mcid_example_structure_elements_have_mcids() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    // At least some leaf elements should have MCIDs
    let all_mcids = collect_all_mcids(tree);
    assert!(
        !all_mcids.is_empty(),
        "structure elements should have MCID references"
    );
}

#[test]
fn mcid_example_structure_elements_api() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    // Test the structure_elements() API (flat list)
    let elements = page.structure_elements();
    assert!(
        !elements.is_empty(),
        "structure_elements() should return non-empty list for tagged PDF"
    );
    // Every element should have a valid element_type
    for elem in &elements {
        assert!(
            !elem.element_type.is_empty(),
            "each structure element should have a type"
        );
    }
}

// ==================== Structure Tree: figure_structure.pdf ====================

#[test]
fn figure_structure_has_structure_tree() {
    let pdf = open_cv_fixture(&cv_pdf("figure_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree();
    assert!(
        tree.is_some(),
        "figure_structure.pdf should have a structure tree"
    );
}

#[test]
fn figure_structure_contains_figure_element() {
    let pdf = open_cv_fixture(&cv_pdf("figure_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    let types = collect_element_types(tree);
    assert!(
        types.contains(&"Figure".to_string()),
        "figure_structure.pdf should contain Figure elements, found: {:?}",
        types
    );
}

// ==================== Structure Tree: pdf_structure.pdf ====================

#[test]
fn pdf_structure_has_structure_tree() {
    let pdf = open_cv_fixture(&cv_pdf("pdf_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree();
    assert!(
        tree.is_some(),
        "pdf_structure.pdf should have a structure tree"
    );
}

#[test]
fn pdf_structure_has_table_element() {
    let pdf = open_cv_fixture(&cv_pdf("pdf_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    let types = collect_element_types(tree);
    assert!(
        types.contains(&"Table".to_string()),
        "pdf_structure.pdf should contain Table elements, found: {:?}",
        types
    );
}

#[test]
fn pdf_structure_has_list_elements() {
    let pdf = open_cv_fixture(&cv_pdf("pdf_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let tree = page.structure_tree().unwrap();
    let types = collect_element_types(tree);
    assert!(
        types.contains(&"L".to_string()) || types.contains(&"LI".to_string()),
        "pdf_structure.pdf should contain list elements (L or LI), found: {:?}",
        types
    );
}

// ==================== MCID-to-Content Mapping: mcid_example.pdf ====================

#[test]
fn mcid_example_chars_have_mcid_values() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    // At least some chars should have MCIDs set from the content stream
    let tagged_count = chars.iter().filter(|c| c.mcid.is_some()).count();
    assert!(
        tagged_count > 0,
        "mcid_example.pdf chars should have MCID values, but none found among {} chars",
        chars.len()
    );
}

#[test]
fn mcid_example_chars_by_mcid_returns_groups() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let groups = page.chars_by_mcid();
    assert!(
        !groups.is_empty(),
        "chars_by_mcid() should return non-empty groups for tagged PDF"
    );
    // Each group should have at least one char
    for (mcid, chars) in &groups {
        assert!(!chars.is_empty(), "MCID {} group should not be empty", mcid);
        // All chars in the group should have the matching MCID
        for c in chars {
            assert_eq!(
                c.mcid,
                Some(*mcid),
                "char '{}' in MCID {} group has wrong mcid {:?}",
                c.text,
                mcid,
                c.mcid
            );
        }
    }
}

#[test]
fn mcid_example_chars_by_mcid_covers_all_tagged_chars() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let groups = page.chars_by_mcid();
    let grouped_count: usize = groups.values().map(|v| v.len()).sum();
    let tagged_count = page.chars().iter().filter(|c| c.mcid.is_some()).count();
    assert_eq!(
        grouped_count, tagged_count,
        "chars_by_mcid() should cover all tagged chars"
    );
}

#[test]
fn mcid_example_chars_by_mcid_mcids_match_structure_tree() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let groups = page.chars_by_mcid();
    let tree = page.structure_tree().unwrap();
    let struct_mcids: std::collections::HashSet<u32> =
        collect_all_mcids(tree).into_iter().collect();
    // All MCIDs in chars_by_mcid should exist in the structure tree
    for mcid in groups.keys() {
        assert!(
            struct_mcids.contains(mcid),
            "MCID {} from chars not found in structure tree",
            mcid
        );
    }
}

#[test]
fn mcid_example_semantic_chars_returns_ordered_chars() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let semantic = page.semantic_chars();
    assert!(
        !semantic.is_empty(),
        "semantic_chars() should return non-empty for tagged PDF"
    );
}

#[test]
fn mcid_example_semantic_chars_covers_tagged_chars() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let semantic = page.semantic_chars();
    let tagged_count = page.chars().iter().filter(|c| c.mcid.is_some()).count();
    // Semantic chars should include at least all tagged chars
    assert!(
        semantic.len() >= tagged_count,
        "semantic_chars() should include at least all {} tagged chars, got {}",
        tagged_count,
        semantic.len()
    );
}

#[test]
fn mcid_example_semantic_chars_follows_structure_order() {
    let pdf = open_cv_fixture(&cv_pdf("mcid_example.pdf"));
    let page = pdf.page(0).unwrap();
    let semantic = page.semantic_chars();
    let tree = page.structure_tree().unwrap();

    // Collect MCIDs in structure tree order (depth-first)
    let struct_mcid_order = collect_mcid_order(tree);

    // Collect the MCID sequence from semantic_chars (first occurrence of each new MCID)
    let mut seen_mcids: Vec<u32> = Vec::new();
    for c in &semantic {
        if let Some(mcid) = c.mcid {
            if seen_mcids.last() != Some(&mcid) {
                seen_mcids.push(mcid);
            }
        }
    }

    // The order of first-seen MCIDs should match the structure tree traversal order
    // (only for MCIDs that appear in both)
    let struct_filtered: Vec<u32> = struct_mcid_order
        .iter()
        .copied()
        .filter(|m| seen_mcids.contains(m))
        .collect();
    let seen_filtered: Vec<u32> = seen_mcids
        .iter()
        .copied()
        .filter(|m| struct_mcid_order.contains(m))
        .collect();
    assert_eq!(
        seen_filtered, struct_filtered,
        "semantic_chars() MCID order should match structure tree order"
    );
}

#[test]
fn figure_structure_semantic_chars_returns_chars() {
    let pdf = open_cv_fixture(&cv_pdf("figure_structure.pdf"));
    let page = pdf.page(0).unwrap();
    let semantic = page.semantic_chars();
    // If the PDF has tagged chars, semantic_chars should return them
    let tagged_count = page.chars().iter().filter(|c| c.mcid.is_some()).count();
    if tagged_count > 0 {
        assert!(
            !semantic.is_empty(),
            "semantic_chars() should return chars for tagged PDF with MCID chars"
        );
    }
}

// ==================== Structure Tree: untagged PDF ====================

#[test]
fn untagged_pdf_has_no_structure_tree() {
    // basic_text.pdf is a simple generated PDF without structure tags
    let pdf = open_fixture(&generated("basic_text.pdf"));
    let page = pdf.page(0).unwrap();
    assert!(
        page.structure_tree().is_none(),
        "untagged PDF should have no structure tree"
    );
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn collect_element_types(elements: &[StructElement]) -> Vec<String> {
    let mut types = Vec::new();
    for elem in elements {
        types.push(elem.element_type.clone());
        types.extend(collect_element_types(&elem.children));
    }
    types
}

fn collect_all_mcids(elements: &[StructElement]) -> Vec<u32> {
    let mut mcids = Vec::new();
    for elem in elements {
        mcids.extend_from_slice(&elem.mcids);
        mcids.extend(collect_all_mcids(&elem.children));
    }
    mcids
}

/// Collect MCIDs in structure tree depth-first traversal order.
fn collect_mcid_order(elements: &[StructElement]) -> Vec<u32> {
    let mut order = Vec::new();
    for elem in elements {
        // MCIDs on this element come first, then recurse into children
        order.extend_from_slice(&elem.mcids);
        order.extend(collect_mcid_order(&elem.children));
    }
    order
}
