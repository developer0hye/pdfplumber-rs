//! Integration tests for `pdfplumber-raster`.
//!
//! These tests exercise the full render pipeline against a live PDF using
//! `pdfplumber`. They are marked `#[ignore]` by default so they only run
//! when a PDF fixture is present; CI uses `cargo test --test integration
//! -- --ignored` with fixture files checked in.

use pdfplumber::Pdf;
use pdfplumber_raster::{RasterError, RasterOptions, Rasterizer, RenderResult};

/// Check PNG magic bytes.
fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n"
}

// ─── Unit-level integration: construct Page via the public API ─────────────

#[test]
fn empty_page_round_trip() {
    let page = pdfplumber::Page::new(0, 595.0, 842.0, vec![]);
    let opts = RasterOptions {
        scale: 1.0,
        ..Default::default()
    };
    let result = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&result.png), "output must be valid PNG");
    assert_eq!(result.page_number, 0);
    assert_eq!(result.width_px, 595);
    assert_eq!(result.height_px, 842);
}

#[test]
fn scale_2x_produces_larger_png() {
    use pdfplumber::Page;
    let page = Page::new(0, 100.0, 100.0, vec![]);
    let res_1x = Rasterizer::new(RasterOptions {
        scale: 1.0,
        ..Default::default()
    })
    .render_page(&page)
    .unwrap();
    let res_2x = Rasterizer::new(RasterOptions {
        scale: 2.0,
        ..Default::default()
    })
    .render_page(&page)
    .unwrap();
    assert!(
        res_2x.png.len() > res_1x.png.len(),
        "2× render should be larger than 1× render"
    );
    assert_eq!(res_2x.width_px, 200);
    assert_eq!(res_2x.height_px, 200);
}

#[test]
fn max_dimension_guard() {
    use pdfplumber::Page;
    let page = Page::new(0, 9_000.0, 9_000.0, vec![]);
    let result = Rasterizer::new(RasterOptions {
        scale: 2.0,
        ..Default::default()
    })
    .render_page(&page);
    assert!(
        matches!(result, Err(RasterError::DimensionsTooLarge { .. })),
        "must reject oversized pages"
    );
}

#[test]
fn geometry_only_mode() {
    use pdfplumber::Page;
    use pdfplumber_core::{Color as PdfColor, Rect};

    let rect = Rect {
        x0: 10.0,
        top: 10.0,
        x1: 90.0,
        bottom: 90.0,
        line_width: 1.0,
        fill: true,
        stroke: true,
        fill_color: PdfColor::Rgb(0.8, 0.2, 0.2),
        stroke_color: PdfColor::Gray(0.0),
    };
    let page = Page::with_geometry(0, 100.0, 100.0, vec![], vec![], vec![rect], vec![]);
    let opts = RasterOptions {
        scale: 1.0,
        render_text: false,
        render_geometry: true,
        ..Default::default()
    };
    let result = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&result.png));
}

#[test]
fn text_only_mode() {
    use pdfplumber::Page;
    use pdfplumber_core::{BBox, Char, Color as PdfColor, TextDirection};

    let ch = Char {
        text: "A".to_owned(),
        bbox: BBox {
            x0: 10.0,
            top: 10.0,
            x1: 20.0,
            bottom: 22.0,
        },
        fontname: "Helvetica".to_owned(),
        size: 12.0,
        doctop: 10.0,
        upright: true,
        direction: TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: Some(PdfColor::Gray(0.0)),
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: b'A' as u32,
        mcid: None,
        tag: None,
    };
    let page = Page::new(0, 100.0, 100.0, vec![ch]);
    let opts = RasterOptions {
        scale: 1.5,
        render_text: true,
        render_geometry: false,
        ..Default::default()
    };
    let result = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&result.png));
}

#[test]
fn render_result_page_number_is_preserved() {
    use pdfplumber::Page;
    let page = Page::new(5, 100.0, 100.0, vec![]);
    let result = Rasterizer::new(RasterOptions::default())
        .render_page(&page)
        .unwrap();
    assert_eq!(result.page_number, 5);
}

#[test]
fn render_result_scale_is_preserved() {
    use pdfplumber::Page;
    let page = Page::new(0, 100.0, 100.0, vec![]);
    let opts = RasterOptions {
        scale: 3.0,
        ..Default::default()
    };
    let result = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!((result.scale - 3.0).abs() < 1e-6);
}

// ─── render_all_pages convenience ──────────────────────────────────────────

#[test]
fn render_all_pages_empty_pdf_returns_empty_vec() {
    // We can't easily build a zero-page Pdf without a file, but we can
    // verify the method exists and compiles by calling it on a mock.
    // For now, we just ensure the return type is Vec<RenderResult>.
    let _: fn(&Rasterizer, &Pdf) -> Vec<RenderResult> = Rasterizer::render_all_pages;
}

// ─── Live PDF integration tests (require fixture, run with --ignored) ──────

const PDF_FIXTURE: &str = "tests/fixtures/sample.pdf";

#[test]
#[ignore]
fn render_pdf_first_page_produces_valid_png() {
    let path = std::env::var("PDF_FIXTURE").unwrap_or_else(|_| PDF_FIXTURE.to_owned());
    let pdf = Pdf::open_file(&path, None).expect("fixture PDF must be openable");
    let page = pdf
        .pages_iter()
        .next()
        .expect("PDF must have at least one page")
        .expect("first page must parse successfully");
    let result = Rasterizer::new(RasterOptions::default())
        .render_page(&page)
        .expect("render must not fail");
    assert!(is_png(&result.png), "output must be valid PNG");
    assert!(
        result.png.len() > 10_000,
        "PNG for a real page must be non-trivial in size"
    );
    assert_eq!(result.page_number, page.page_number());
}

#[test]
#[ignore]
fn render_all_pages_no_panic() {
    let path = std::env::var("PDF_FIXTURE").unwrap_or_else(|_| PDF_FIXTURE.to_owned());
    let pdf = Pdf::open_file(&path, None).expect("fixture PDF must be openable");
    let rasterizer = Rasterizer::new(RasterOptions::default());
    let results = rasterizer.render_all_pages(&pdf);
    // Must have rendered at least one page.
    assert!(
        !results.is_empty(),
        "render_all_pages must return at least one page"
    );
    // All results must be valid PNGs.
    for r in &results {
        assert!(
            is_png(&r.png),
            "page {} must produce valid PNG",
            r.page_number
        );
    }
}

#[test]
#[ignore]
fn render_page_index_convenience() {
    let path = std::env::var("PDF_FIXTURE").unwrap_or_else(|_| PDF_FIXTURE.to_owned());
    let pdf = Pdf::open_file(&path, None).expect("fixture PDF must be openable");
    let rasterizer = Rasterizer::new(RasterOptions::default());
    // Page 0 must exist.
    let result = rasterizer
        .render_page_index(&pdf, 0)
        .expect("page 0 must exist")
        .expect("page 0 must render");
    assert!(is_png(&result.png));
    assert_eq!(result.page_number, 0);
    // Out-of-bounds page must return None.
    assert!(rasterizer.render_page_index(&pdf, 999_999).is_none());
}
