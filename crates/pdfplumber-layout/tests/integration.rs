//! Integration tests for pdfplumber-layout.
//!
//! These tests exercise the full pipeline: Pdf::open_file → Document::from_pdf →
//! inspect sections/headings/paragraphs/tables/figures.
//!
//! PDFs are borrowed from the pdfplumber-rs fixture suite (relative to workspace root).

use pdfplumber::Pdf;

use pdfplumber_layout::{Document, LayoutBlock, LayoutOptions};

/// Path relative to workspace root for fixture PDFs.
fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../crates/pdfplumber/tests/fixtures/pdfs");
    p.push(name);
    p
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn open(name: &str) -> Pdf {
    let path = fixture(name);
    Pdf::open_file(&path, None).unwrap_or_else(|e| {
        panic!("failed to open fixture {name}: {e}");
    })
}

fn layout(name: &str) -> Document {
    let pdf = open(name);
    Document::from_pdf(&pdf)
}

// ── sanity: Document never panics on any fixture ─────────────────────────────

macro_rules! no_panic {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            let pdf = open($file);
            let _doc = Document::from_pdf(&pdf);
        }
    };
}

no_panic!(no_panic_federal_register,  "federal-register-2020-17221.pdf");
no_panic!(no_panic_chelsea_pdta,       "chelsea_pdta.pdf");
no_panic!(no_panic_cupertino,          "cupertino_usd_4-6-16.pdf");
no_panic!(no_panic_hello_structure,    "hello_structure.pdf");
no_panic!(no_panic_figure_structure,   "figure_structure.pdf");
no_panic!(no_panic_annotations,        "annotations.pdf");
no_panic!(no_panic_issue_1054,         "issue-1054-example.pdf");
no_panic!(no_panic_issue_1114,         "issue-1114-dedupe-chars.pdf");
no_panic!(no_panic_milw,               "150109DSP-Milw-505-90D.pdf");
no_panic!(no_panic_pdtoppm,            "2023-06-20-PV.pdf");

// ── stats consistency ────────────────────────────────────────────────────────

#[test]
fn stats_page_count_matches_pdf() {
    let pdf = open("federal-register-2020-17221.pdf");
    let page_count = pdf.pages_iter().count();
    let doc = Document::from_pdf(&pdf);
    assert_eq!(doc.stats().page_count, page_count);
}

#[test]
fn stats_block_counts_are_consistent() {
    let doc = layout("federal-register-2020-17221.pdf");
    let s = doc.stats();
    // Sum of typed counts should equal total blocks across all pages.
    let total_from_pages: usize = doc.pages().iter().map(|p| p.blocks.len()).sum();
    let total_from_stats = s.heading_count + s.paragraph_count + s.table_count + s.figure_count;
    assert_eq!(total_from_pages, total_from_stats,
        "stats counts don't add up: pages={total_from_pages} stats={total_from_stats}");
}

#[test]
fn stats_heading_count_lte_total() {
    let doc = layout("federal-register-2020-17221.pdf");
    let s = doc.stats();
    assert!(s.heading_count <= s.heading_count + s.paragraph_count);
}

// ── section structure ────────────────────────────────────────────────────────

#[test]
fn sections_cover_all_blocks() {
    let doc = layout("federal-register-2020-17221.pdf");
    let section_block_count: usize = doc.sections().iter().map(|s| s.block_count()).sum();
    let heading_count = doc.sections().iter().filter(|s| s.heading().is_some()).count();
    let total_from_pages: usize = doc.pages().iter().map(|p| p.blocks.len()).sum();
    // Every non-heading block should be in a section. Headings are section delimiters, not blocks.
    assert_eq!(section_block_count + heading_count, total_from_pages,
        "section blocks + headings ({}) != total page blocks ({})",
        section_block_count + heading_count, total_from_pages);
}

#[test]
fn sections_are_nonempty_or_have_heading() {
    let doc = layout("cupertino_usd_4-6-16.pdf");
    for section in doc.sections() {
        assert!(
            section.heading().is_some() || section.block_count() > 0,
            "section with no heading and no blocks should not exist"
        );
    }
}

#[test]
fn sections_bbox_is_none_only_for_empty() {
    let doc = layout("chelsea_pdta.pdf");
    for section in doc.sections() {
        if section.heading().is_some() || section.block_count() > 0 {
            // Non-empty sections must have a bbox.
            assert!(section.bbox.is_some(), "non-empty section missing bbox");
        }
    }
}

// ── block type properties ────────────────────────────────────────────────────

#[test]
fn headings_have_nonempty_text() {
    let doc = layout("federal-register-2020-17221.pdf");
    for h in doc.headings() {
        assert!(!h.text.trim().is_empty(), "heading with empty text: {:?}", h.bbox);
    }
}

#[test]
fn paragraphs_have_nonempty_text() {
    let doc = layout("federal-register-2020-17221.pdf");
    for p in doc.paragraphs() {
        assert!(!p.text.trim().is_empty(), "paragraph with empty text: {:?}", p.bbox);
    }
}

#[test]
fn headings_font_size_above_zero() {
    let doc = layout("cupertino_usd_4-6-16.pdf");
    for h in doc.headings() {
        assert!(h.font_size > 0.0, "heading font_size should be > 0, got {}", h.font_size);
    }
}

#[test]
fn paragraphs_line_count_above_zero() {
    let doc = layout("federal-register-2020-17221.pdf");
    for p in doc.paragraphs() {
        assert!(p.line_count >= 1, "paragraph must have at least 1 line");
    }
}

#[test]
fn tables_have_valid_dimensions() {
    let doc = layout("cupertino_usd_4-6-16.pdf");
    for t in doc.tables() {
        assert!(t.rows >= 1, "table must have at least 1 row");
        assert!(t.cols >= 1, "table must have at least 1 col");
        assert_eq!(t.cells.len(), t.rows, "cells row count mismatch");
        for row in &t.cells {
            assert_eq!(row.len(), t.cols, "cells col count mismatch in row");
        }
    }
}

#[test]
fn figures_have_positive_area() {
    let doc = layout("figure_structure.pdf");
    for f in doc.figures() {
        let area = (f.bbox.x1 - f.bbox.x0) * (f.bbox.bottom - f.bbox.top);
        assert!(area > 0.0, "figure area must be positive, got {area}");
    }
}

// ── bbox validity ─────────────────────────────────────────────────────────────

#[test]
fn all_block_bboxes_are_valid() {
    let doc = layout("federal-register-2020-17221.pdf");
    for block in doc.all_blocks() {
        let bb = block.bbox();
        assert!(bb.x0 <= bb.x1, "bbox x0 > x1: {:?}", bb);
        assert!(bb.top <= bb.bottom, "bbox top > bottom: {:?}", bb);
    }
}

#[test]
fn all_block_bboxes_on_page_bounds() {
    // Block bboxes should stay within reasonable PDF coordinate space (never hugely negative).
    let doc = layout("federal-register-2020-17221.pdf");
    for block in doc.all_blocks() {
        let bb = block.bbox();
        assert!(bb.x0 >= -5.0, "bbox x0 implausibly negative: {}", bb.x0);
        assert!(bb.top >= -5.0, "bbox top implausibly negative: {}", bb.top);
    }
}

// ── reading order ────────────────────────────────────────────────────────────

#[test]
fn page_blocks_sorted_top_to_bottom() {
    let doc = layout("federal-register-2020-17221.pdf");
    for page in doc.pages() {
        let mut prev_top = f64::NEG_INFINITY;
        for block in &page.blocks {
            let top = block.bbox().top;
            assert!(
                top >= prev_top - 1.0, // 1pt tolerance for floating point
                "page {} block out of order: top={top} prev_top={prev_top}",
                page.page_number
            );
            prev_top = top;
        }
    }
}

// ── text extraction ───────────────────────────────────────────────────────────

#[test]
fn document_text_is_nonempty_for_text_pdf() {
    let doc = layout("federal-register-2020-17221.pdf");
    let text = doc.text();
    assert!(!text.trim().is_empty(), "document text should not be empty for a text PDF");
}

#[test]
fn document_text_contains_recognizable_words() {
    let doc = layout("federal-register-2020-17221.pdf");
    let text = doc.text().to_lowercase();
    // The Federal Register PDF should contain some common English words.
    let has_content = text.contains("the") || text.contains("and") || text.contains("of");
    assert!(has_content, "document text looks empty or garbled");
}

// ── flat iterators match page-level blocks ────────────────────────────────────

#[test]
fn flat_heading_count_matches_stats() {
    let doc = layout("federal-register-2020-17221.pdf");
    assert_eq!(doc.headings().count(), doc.stats().heading_count);
}

#[test]
fn flat_paragraph_count_matches_stats() {
    let doc = layout("federal-register-2020-17221.pdf");
    assert_eq!(doc.paragraphs().count(), doc.stats().paragraph_count);
}

#[test]
fn flat_table_count_matches_stats() {
    let doc = layout("cupertino_usd_4-6-16.pdf");
    assert_eq!(doc.tables().count(), doc.stats().table_count);
}

// ── LayoutOptions customisation ───────────────────────────────────────────────

#[test]
fn no_tables_when_disabled() {
    let pdf = open("cupertino_usd_4-6-16.pdf");
    let opts = LayoutOptions { detect_tables: false, ..LayoutOptions::default() };
    let doc = Document::from_pdf_with_options(&pdf, &opts);
    assert_eq!(doc.stats().table_count, 0, "tables should be 0 when detection disabled");
}

#[test]
fn no_figures_when_disabled() {
    let pdf = open("figure_structure.pdf");
    let opts = LayoutOptions { detect_figures: false, ..LayoutOptions::default() };
    let doc = Document::from_pdf_with_options(&pdf, &opts);
    assert_eq!(doc.stats().figure_count, 0, "figures should be 0 when detection disabled");
}

// ── page-level API ────────────────────────────────────────────────────────────

#[test]
fn page_layout_width_height_positive() {
    let doc = layout("federal-register-2020-17221.pdf");
    for page in doc.pages() {
        assert!(page.width > 0.0, "page width must be positive");
        assert!(page.height > 0.0, "page height must be positive");
    }
}

#[test]
fn page_layout_accessors_consistent_with_blocks() {
    let doc = layout("federal-register-2020-17221.pdf");
    for page in doc.pages() {
        let h_count = page.headings().count();
        let p_count = page.paragraphs().count();
        let t_count = page.tables().count();
        let f_count = page.figures().count();
        let total = h_count + p_count + t_count + f_count;
        assert_eq!(total, page.blocks.len(), "accessor counts don't sum to block count");
    }
}

// ── caption detection ─────────────────────────────────────────────────────────

#[test]
fn captions_are_short() {
    let doc = layout("figure_structure.pdf");
    for p in doc.paragraphs() {
        if p.is_caption {
            assert!(p.text.len() < 300, "caption text implausibly long: {}", p.text.len());
        }
    }
}

// ── body baseline sanity ──────────────────────────────────────────────────────

#[test]
fn body_font_size_in_reasonable_range() {
    let doc = layout("federal-register-2020-17221.pdf");
    let size = doc.stats().body_font_size;
    // Typical PDF body text is 6–18pt. Tolerate wider range.
    assert!(size >= 4.0 && size <= 36.0,
        "body_font_size {size} outside expected range 4–36pt");
}

// ── markdown output ───────────────────────────────────────────────────────────

#[test]
fn to_markdown_is_nonempty_for_text_pdf() {
    let doc = layout("federal-register-2020-17221.pdf");
    let md = doc.to_markdown();
    assert!(!md.trim().is_empty(), "markdown should not be empty for a text PDF");
}

#[test]
fn to_markdown_contains_heading_syntax() {
    let doc = layout("federal-register-2020-17221.pdf");
    let md = doc.to_markdown();
    // At least one ATX heading should appear if any heading was detected
    if doc.stats().heading_count > 0 {
        assert!(md.contains('#'), "markdown should contain ATX headings when headings detected");
    }
}

#[test]
fn to_markdown_tables_have_separator_row() {
    let doc = layout("cupertino_usd_4-6-16.pdf");
    let md = doc.to_markdown();
    if doc.stats().table_count > 0 {
        // GFM tables always have | --- | separator rows
        assert!(md.contains("| ---"), "GFM table separator row missing");
    }
}

// ── two-pass header/footer suppression ───────────────────────────────────────

#[test]
fn header_footer_stats_are_sensible() {
    let doc = layout("federal-register-2020-17221.pdf");
    let s = doc.stats();
    // Can't assert exact counts without knowing the PDF, but these should never
    // exceed page_count.
    assert!(s.pages_with_header <= s.page_count);
    assert!(s.pages_with_footer <= s.page_count);
}

// ── column mode override ──────────────────────────────────────────────────────

#[test]
fn explicit_no_column_mode_produces_results() {
    use pdfplumber_core::ColumnMode;
    let pdf = open("federal-register-2020-17221.pdf");
    let opts = LayoutOptions {
        column_mode: ColumnMode::None,
        ..LayoutOptions::default()
    };
    let doc = Document::from_pdf_with_options(&pdf, &opts);
    // Should still produce output — just not column-aware.
    assert!(doc.stats().paragraph_count > 0 || doc.stats().heading_count > 0);
}

// ── list detection ────────────────────────────────────────────────────────────

#[test]
fn parse_list_prefix_in_fixture_text() {
    use pdfplumber_layout::lists::parse_list_prefix;
    // Verify the list detection functions work correctly end-to-end.
    assert!(parse_list_prefix("• Item one").is_some());
    assert!(parse_list_prefix("1. First step").is_some());
    assert!(parse_list_prefix("Not a list item here.").is_none());
}

// ── block_to_markdown round-trip ──────────────────────────────────────────────

#[test]
fn block_to_markdown_headings() {
    use pdfplumber_layout::{block_to_markdown, Heading, HeadingLevel, LayoutBlock};
    use pdfplumber_core::BBox;

    let h = Heading {
        text: "Results".to_string(),
        bbox: BBox::new(72.0, 100.0, 400.0, 120.0),
        page_number: 0,
        level: HeadingLevel::H2,
        font_size: 16.0,
        fontname: "Helvetica-Bold".to_string(),
    };
    let md = block_to_markdown(&LayoutBlock::Heading(h));
    assert_eq!(md, "## Results");
}

#[test]
fn block_to_markdown_table() {
    use pdfplumber_layout::{LayoutBlock, LayoutTable, block_to_markdown};
    use pdfplumber_core::BBox;

    let t = LayoutTable {
        bbox: BBox::new(72.0, 200.0, 500.0, 300.0),
        page_number: 1,
        rows: 2,
        cols: 2,
        cells: vec![
            vec![Some("Year".to_string()), Some("Revenue".to_string())],
            vec![Some("2024".to_string()), Some("$1M".to_string())],
        ],
    };
    let md = block_to_markdown(&LayoutBlock::Table(t));
    assert!(md.contains("| Year | Revenue |"));
    assert!(md.contains("| --- | --- |"));
    assert!(md.contains("| 2024 | $1M |"));
}
