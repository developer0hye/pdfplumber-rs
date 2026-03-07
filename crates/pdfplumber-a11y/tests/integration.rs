//! Integration tests for `pdfplumber-a11y`.
//!
//! Tests the `A11yAnalyzer` and `TagInferrer` against:
//! 1. Synthetic `Page` instances (no PDF file required — always run)
//! 2. Real PDF fixtures (marked `#[ignore]`, run in CI with fixture files)

use pdfplumber::Page;
use pdfplumber_a11y::{A11yAnalyzer, InferredTag, TagInferrer};
use pdfplumber_core::{BBox, Char, Color as PdfColor, TextDirection};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64, size: f64) -> Char {
    Char {
        text: text.to_owned(),
        bbox: BBox {
            x0,
            top,
            x1,
            bottom,
        },
        fontname: "Helvetica".to_owned(),
        size,
        doctop: top,
        upright: true,
        direction: TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: Some(PdfColor::Gray(0.0)),
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: text.chars().next().unwrap_or('?') as u32,
        mcid: None,
        tag: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TagInferrer — no PDF required
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn infer_empty_page_returns_empty_tags() {
    let page = Page::new(0, 595.0, 842.0, vec![]);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 0);
    // No chars → no text tags. May have empty vec.
    assert!(
        tags.iter().all(|t| t.role != "P" && t.role != "H1"),
        "empty page should have no text tags"
    );
}

#[test]
fn infer_body_text_tagged_as_p() {
    // 10pt body text on a 200pt page — below heading threshold.
    let chars: Vec<Char> = "Hello world"
        .chars()
        .enumerate()
        .map(|(i, c)| {
            make_char(
                &c.to_string(),
                10.0 + i as f64 * 6.0,
                100.0,
                16.0 + i as f64 * 6.0,
                112.0,
                10.0,
            )
        })
        .collect();
    let page = Page::new(0, 595.0, 842.0, chars);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 0);
    let p_tags: Vec<_> = tags.iter().filter(|t| t.role == "P").collect();
    assert!(
        !p_tags.is_empty(),
        "body text should be tagged as P; got: {:?}",
        tags.iter().map(|t| &t.role).collect::<Vec<_>>()
    );
}

#[test]
fn infer_large_text_tagged_as_heading() {
    // Mix: small body chars (10pt) + large heading chars (20pt).
    // body chars (10pt)
    let mut chars: Vec<Char> = "Body text here"
        .chars()
        .enumerate()
        .map(|(i, c)| {
            make_char(
                &c.to_string(),
                10.0 + i as f64 * 6.0,
                400.0,
                16.0 + i as f64 * 6.0,
                412.0,
                10.0,
            )
        })
        .collect();
    // heading chars (20pt — 2× body) — well above h1_ratio=1.4×
    // Placed at y=100-120 to stay below the artifact zone (top 8% of 842pt ≈ 67pt).
    let heading_text = "Title";
    for (i, c) in heading_text.chars().enumerate() {
        chars.push(make_char(
            &c.to_string(),
            10.0 + i as f64 * 12.0,
            100.0,
            22.0 + i as f64 * 12.0,
            120.0,
            20.0,
        ));
    }
    let page = Page::new(0, 595.0, 842.0, chars);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 0);
    let heading_tags: Vec<_> = tags
        .iter()
        .filter(|t| t.role == "H1" || t.role == "H2" || t.role == "H3")
        .collect();
    assert!(
        !heading_tags.is_empty(),
        "large text should be classified as a heading; tags: {:?}",
        tags.iter().map(|t| (&t.role, &t.text)).collect::<Vec<_>>()
    );
}

#[test]
fn infer_header_zone_text_tagged_as_artifact() {
    // Text in top 8% of page (top 8% of 842pt = top 67pt).
    let chars: Vec<Char> = "Header text"
        .chars()
        .enumerate()
        .map(|(i, c)| {
            make_char(
                &c.to_string(),
                10.0 + i as f64 * 6.0,
                5.0, // near top of page
                16.0 + i as f64 * 6.0,
                15.0,
                10.0,
            )
        })
        .collect();
    let page = Page::new(0, 595.0, 842.0, chars);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 0);
    let artifact_tags: Vec<_> = tags.iter().filter(|t| t.role == "Artifact").collect();
    assert!(
        !artifact_tags.is_empty(),
        "top-of-page text should be tagged as Artifact"
    );
    assert!(
        artifact_tags.iter().all(|t| !t.needs_review),
        "artifacts should not require review"
    );
}

#[test]
fn infer_page_number_preserved() {
    let chars = vec![make_char("A", 10.0, 200.0, 20.0, 212.0, 10.0)];
    let page = Page::new(0, 595.0, 842.0, chars);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 3); // explicit page_idx=3
    for tag in &tags {
        assert_eq!(tag.page, 3, "all tags should carry page index 3");
    }
}

#[test]
fn review_count_counts_figures_recursively() {
    let inferrer = TagInferrer::new();
    let tags = vec![InferredTag {
        role: "P".to_owned(),
        bbox: BBox {
            x0: 0.0,
            top: 0.0,
            x1: 100.0,
            bottom: 12.0,
        },
        page: 0,
        text: "text".to_owned(),
        children: vec![InferredTag {
            role: "Figure".to_owned(),
            bbox: BBox {
                x0: 0.0,
                top: 0.0,
                x1: 50.0,
                bottom: 50.0,
            },
            page: 0,
            text: String::new(),
            children: vec![],
            needs_review: true,
            review_reason: Some("needs alt".to_owned()),
        }],
        needs_review: false,
        review_reason: None,
    }];
    assert_eq!(inferrer.review_count(&tags), 1);
}

#[test]
fn infer_tags_sorted_top_to_bottom() {
    // Two text blocks at different vertical positions.
    let mut chars = vec![];
    // Block at y=500 (lower on page)
    for (i, c) in "Lower".chars().enumerate() {
        chars.push(make_char(
            &c.to_string(),
            10.0 + i as f64 * 6.0,
            500.0,
            16.0 + i as f64 * 6.0,
            512.0,
            10.0,
        ));
    }
    // Block at y=100 (higher on page)
    for (i, c) in "Upper".chars().enumerate() {
        chars.push(make_char(
            &c.to_string(),
            10.0 + i as f64 * 6.0,
            100.0,
            16.0 + i as f64 * 6.0,
            112.0,
            10.0,
        ));
    }
    let page = Page::new(0, 595.0, 842.0, chars);
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_page(&page, 0);
    // Filter to only text tags (not artifacts)
    let text_tags: Vec<_> = tags
        .iter()
        .filter(|t| t.role == "P" || t.role.starts_with('H'))
        .collect();
    // Should be sorted top-to-bottom (lower bbox.top first)
    for window in text_tags.windows(2) {
        assert!(
            window[0].bbox.top <= window[1].bbox.top,
            "tags should be sorted top-to-bottom: {:?} before {:?}",
            window[0].bbox.top,
            window[1].bbox.top
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// A11yAnalyzer — untagged PDF behavior with synthetic pages
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn analyzer_constructed_with_defaults() {
    let analyzer = A11yAnalyzer::new();
    assert!(!analyzer.emit_info, "emit_info should default to false");
}

// ─────────────────────────────────────────────────────────────────────────────
// Live PDF fixture tests (require real PDF files, run with --ignored)
// ─────────────────────────────────────────────────────────────────────────────

const FIXTURE_DIR: &str = "../../tests/fixtures/generated";

fn fixture(name: &str) -> String {
    format!("{}/{}", FIXTURE_DIR, name)
}

#[test]
#[ignore]
fn analyze_basic_text_no_panic() {
    let pdf = pdfplumber::Pdf::open_file(&fixture("basic_text.pdf"), None)
        .expect("fixture must be openable");
    let report = A11yAnalyzer::new().analyze(&pdf);
    // An untagged PDF will have violations but must not panic.
    let _ = report.summary();
    // page count must match
    assert!(report.page_count() >= 1);
}

#[test]
#[ignore]
fn analyze_untagged_pdf_has_ua001() {
    let pdf = pdfplumber::Pdf::open_file(&fixture("basic_text.pdf"), None)
        .expect("fixture must be openable");
    let report = A11yAnalyzer::new().analyze(&pdf);
    // Generated fixtures are typically untagged → UA-001 should fire.
    let has_ua001 = report.violations().iter().any(|v| v.rule_id() == "UA-001");
    assert!(has_ua001, "untagged PDF should trigger UA-001");
}

#[test]
#[ignore]
fn analyze_pdf_with_images_no_panic() {
    let path = format!(
        "{}/{}",
        "../../tests/fixtures/real-world/images", "annotated_figures.pdf"
    );
    let Ok(pdf) = pdfplumber::Pdf::open_file(&path, None) else {
        return;
    };
    let report = A11yAnalyzer::new().analyze(&pdf);
    let _ = report.summary();
}

#[test]
#[ignore]
fn infer_document_all_pages_no_panic() {
    let pdf = pdfplumber::Pdf::open_file(&fixture("long_document.pdf"), None)
        .expect("fixture must be openable");
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_document(&pdf);
    // Every page should produce some tags.
    assert!(
        !tags.is_empty(),
        "long document should produce some inferred tags"
    );
}

#[test]
#[ignore]
fn infer_multi_font_sizes_detects_headings() {
    let path = format!(
        "{}/{}",
        "../../tests/fixtures/real-world/layout", "multi-font-sizes.pdf"
    );
    let pdf =
        pdfplumber::Pdf::open_file(&path, None).expect("multi-font-sizes fixture must be openable");
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_document(&pdf);
    let heading_tags: Vec<_> = tags.iter().filter(|t| t.role.starts_with('H')).collect();
    assert!(
        !heading_tags.is_empty(),
        "multi-font-sizes PDF should produce at least one heading tag"
    );
}

#[test]
#[ignore]
fn infer_tags_all_have_valid_bboxes() {
    let pdf = pdfplumber::Pdf::open_file(&fixture("basic_text.pdf"), None)
        .expect("fixture must be openable");
    let inferrer = TagInferrer::new();
    let tags = inferrer.infer_document(&pdf);
    for tag in &tags {
        let b = &tag.bbox;
        assert!(b.x1 >= b.x0, "bbox x1 must be >= x0: {:?}", b);
        assert!(b.bottom >= b.top, "bbox bottom must be >= top: {:?}", b);
    }
}

#[test]
#[ignore]
fn analyze_report_summary_is_non_empty() {
    let pdf = pdfplumber::Pdf::open_file(&fixture("basic_text.pdf"), None)
        .expect("fixture must be openable");
    let report = A11yAnalyzer::new().analyze(&pdf);
    let summary = report.summary();
    assert!(!summary.is_empty(), "summary must not be empty");
    assert!(
        summary.contains("PASS") || summary.contains("FAIL"),
        "summary must state PASS or FAIL"
    );
}
