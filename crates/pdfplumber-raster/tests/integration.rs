//! Integration tests for `pdfplumber-raster`.
//!
//! These tests exercise the full render pipeline against a live PDF using
//! `pdfplumber`. They are marked `#[ignore]` by default so they only run
//! when a PDF fixture is present; CI uses `cargo test --test integration
//! -- --ignored` with fixture files checked in.

use pdfplumber::Pdf;
use pdfplumber_raster::{RasterError, RasterOptions, Rasterizer};

/// Check PNG magic bytes.
fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n"
}

// ─── Unit-level integration: construct Page via the public API ─────────────

#[test]
fn empty_page_round_trip() {
    // Page::new is available without a real PDF file.
    let page = pdfplumber::Page::new(0, 595.0, 842.0, vec![]);
    let opts = RasterOptions { scale: 1.0, ..Default::default() };
    let png = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&png), "output must be valid PNG");
}

#[test]
fn scale_2x_produces_larger_png() {
    use pdfplumber::Page;
    let page = Page::new(0, 100.0, 100.0, vec![]);
    let png_1x = Rasterizer::new(RasterOptions { scale: 1.0, ..Default::default() })
        .render_page(&page)
        .unwrap();
    let png_2x = Rasterizer::new(RasterOptions { scale: 2.0, ..Default::default() })
        .render_page(&page)
        .unwrap();
    // 2× render must be larger (more pixels → more PNG data).
    assert!(png_2x.len() > png_1x.len(), "2× render should be larger than 1× render");
}

#[test]
fn max_dimension_guard() {
    use pdfplumber::Page;
    // 9000pt × 2.0 scale = 18000px > MAX_DIM_PX(16000)
    let page = Page::new(0, 9_000.0, 9_000.0, vec![]);
    let result = Rasterizer::new(RasterOptions { scale: 2.0, ..Default::default() })
        .render_page(&page);
    assert!(
        matches!(result, Err(RasterError::DimensionsTooLarge { .. })),
        "must reject oversized pages"
    );
}

#[test]
fn geometry_only_mode() {
    use pdfplumber::{Page};
    use pdfplumber_core::{BBox, Rect, Color as PdfColor};

    let rect = Rect {
        x0: 10.0, top: 10.0, x1: 90.0, bottom: 90.0,
        line_width: 1.0,
        fill: true, stroke: true,
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
    let png = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&png));
}

#[test]
fn text_only_mode() {
    use pdfplumber::Page;
    use pdfplumber_core::{BBox, Char, Color as PdfColor, TextDirection};

    let ch = Char {
        text: "A".to_owned(),
        bbox: BBox { x0: 10.0, top: 10.0, x1: 20.0, bottom: 22.0 },
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
    let png = Rasterizer::new(opts).render_page(&page).unwrap();
    assert!(is_png(&png));
}

// ─── Live PDF integration tests (require fixture, run with --ignored) ──────

/// Path to a PDF fixture for integration testing.
/// Set the `PDF_FIXTURE` env var or place a file at this path.
const PDF_FIXTURE: &str = "tests/fixtures/sample.pdf";

#[test]
#[ignore]
fn render_pdf_first_page_produces_valid_png() {
    let path = std::env::var("PDF_FIXTURE").unwrap_or_else(|_| PDF_FIXTURE.to_owned());
    let pdf = Pdf::open_file(&path, None).expect("fixture PDF must be openable");
    let page = pdf.pages_iter().next()
        .expect("PDF must have at least one page")
        .expect("first page must parse successfully");
    let png = Rasterizer::new(RasterOptions::default())
        .render_page(&page)
        .expect("render must not fail");
    assert!(is_png(&png), "output must be valid PNG");
    // Sanity: 1.5× scale of a typical letter page (612×792pt) → ~918×1188px
    // PNG must be larger than a 100×100 trivial image.
    assert!(png.len() > 10_000, "PNG for a real page must be non-trivial in size");
}

#[test]
#[ignore]
fn render_all_pages_no_panic() {
    let path = std::env::var("PDF_FIXTURE").unwrap_or_else(|_| PDF_FIXTURE.to_owned());
    let pdf = Pdf::open_file(&path, None).expect("fixture PDF must be openable");
    let rasterizer = Rasterizer::new(RasterOptions::default());
    for page_result in pdf.pages_iter() {
        let Ok(page) = page_result else { continue };
        let _ = rasterizer.render_page(&page); // must not panic
    }
}
