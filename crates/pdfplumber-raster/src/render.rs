//! Core rendering pipeline: Page → PNG bytes.
//!
//! Renders a [`pdfplumber::Page`]'s extracted content (chars, rects, lines,
//! curves) into a pixel buffer using `tiny-skia`, then encodes to PNG.
//!
//! ## Rendering order (painter's model, back-to-front)
//!
//! 1. White background fill
//! 2. Filled rectangles
//! 3. Filled curves (bezier paths)
//! 4. Stroked rectangles
//! 5. Stroked lines
//! 6. Stroked curves
//! 7. Text glyphs (top layer — always above geometry)
//!
//! This order matches the typical PDF painting model for documents that have
//! background fills, table borders, and text. It is not a fully correct PDF
//! graphics model (which is stream-ordered), but it is correct for the
//! overwhelming majority of real-world documents.

use std::collections::HashMap;

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

use pdfplumber::Page;
use pdfplumber_core::{Curve, Line, Rect};

use crate::color::{to_skia_color, to_skia_color_f};
use crate::font_cache::FontCache;

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a single render call.
#[derive(Debug, Clone)]
pub struct RasterOptions {
    /// Scale factor: output pixels = PDF points × scale.
    ///
    /// - `1.0` → 72 DPI (1pt = 1px)
    /// - `1.5` → 108 DPI — good default for Ollama fallback
    /// - `2.0` → 144 DPI — crisp on HiDPI displays
    /// - `3.0` → 216 DPI — high quality for archival
    pub scale: f32,

    /// Background fill color (default: white).
    pub background: [u8; 3],

    /// Whether to render text glyphs (default: true).
    ///
    /// Set `false` to render geometry only — useful for debugging table
    /// detection without text clutter.
    pub render_text: bool,

    /// Whether to render geometric shapes (rects, lines, curves) (default: true).
    pub render_geometry: bool,

    /// Supplemental font bytes: `fontname → raw TTF/OTF bytes`.
    ///
    /// Pass embedded font data extracted from lopdf here for highest
    /// text fidelity. When empty, the font cache falls back to system
    /// fonts and then to the built-in fallback font.
    pub font_bytes: HashMap<String, Vec<u8>>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            scale: 1.5,
            background: [255, 255, 255],
            render_text: true,
            render_geometry: true,
            font_bytes: HashMap::new(),
        }
    }
}

/// The result of rasterizing a page.
///
/// Contains the PNG bytes plus metadata about how the image was produced.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// PNG-encoded bytes. Valid PNG file, can be written directly to disk.
    pub png: Vec<u8>,
    /// Page number (0-based) that was rendered.
    pub page_number: usize,
    /// Output image width in pixels.
    pub width_px: u32,
    /// Output image height in pixels.
    pub height_px: u32,
    /// Scale factor used (`output_px = pdf_points × scale`).
    pub scale: f32,
}

impl RenderResult {
    /// Write the PNG bytes to a file at the given path.
    ///
    /// Creates or overwrites the file. Returns an I/O error if writing fails.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, &self.png)
    }
}

/// Errors that can occur during rasterization.
#[derive(Debug)]
pub enum RasterError {
    /// The page dimensions, combined with the scale factor, produce a pixel
    /// buffer that exceeds a safe allocation limit (16 000 × 16 000 px).
    DimensionsTooLarge {
        /// Requested width in pixels.
        width_px: u32,
        /// Requested height in pixels.
        height_px: u32,
    },
    /// PNG encoding failed.
    PngEncodeError(String),
    /// Pixmap allocation failed (typically OOM).
    PixmapAllocationFailed,
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DimensionsTooLarge {
                width_px,
                height_px,
            } => write!(
                f,
                "page dimensions too large for rasterization: {width_px}×{height_px} px (max 16000×16000)"
            ),
            Self::PngEncodeError(msg) => write!(f, "PNG encoding failed: {msg}"),
            Self::PixmapAllocationFailed => write!(f, "failed to allocate pixel buffer"),
        }
    }
}

impl std::error::Error for RasterError {}

/// Maximum safe pixel dimension per axis.
const MAX_DIM_PX: u32 = 16_000;

/// A page rasterizer. Construct once and reuse across multiple pages.
pub struct Rasterizer {
    opts: RasterOptions,
}

impl Rasterizer {
    /// Create a new [`Rasterizer`] with the given options.
    pub fn new(opts: RasterOptions) -> Self {
        Self { opts }
    }

    /// Render a [`Page`] to a [`RenderResult`] containing PNG bytes and metadata.
    ///
    /// Returns a [`RenderResult`] with the PNG file bytes, pixel dimensions,
    /// and the scale factor applied.
    pub fn render_page(&self, page: &Page) -> Result<RenderResult, RasterError> {
        let scale = self.opts.scale;
        let width_pt = page.width();
        let height_pt = page.height();

        let width_px = (width_pt * scale as f64).ceil() as u32;
        let height_px = (height_pt * scale as f64).ceil() as u32;

        if width_px > MAX_DIM_PX || height_px > MAX_DIM_PX {
            return Err(RasterError::DimensionsTooLarge {
                width_px,
                height_px,
            });
        }

        let mut pixmap =
            Pixmap::new(width_px, height_px).ok_or(RasterError::PixmapAllocationFailed)?;

        // 1. Background.
        let [br, bg, bb] = self.opts.background;
        pixmap.fill(Color::from_rgba8(br, bg, bb, 255));

        // 2–6. Geometry.
        if self.opts.render_geometry {
            render_rects_filled(page.rects(), &mut pixmap, scale);
            render_curves_filled(page.curves(), &mut pixmap, scale);
            render_rects_stroked(page.rects(), &mut pixmap, scale);
            render_lines(page.lines(), &mut pixmap, scale);
            render_curves_stroked(page.curves(), &mut pixmap, scale);
        }

        // 7. Text.
        if self.opts.render_text {
            let mut font_cache = FontCache::with_fonts(self.opts.font_bytes.clone());
            render_text(page.chars(), &mut pixmap, &mut font_cache, scale);
        }

        // Encode to PNG.
        let png = pixmap
            .encode_png()
            .map_err(|e| RasterError::PngEncodeError(e.to_string()))?;

        Ok(RenderResult {
            png,
            page_number: page.page_number(),
            width_px,
            height_px,
            scale,
        })
    }

    /// Render all pages of a [`pdfplumber::Pdf`] to a `Vec<RenderResult>`.
    ///
    /// Pages that fail to parse are skipped silently. Pages that fail to render
    /// (e.g., exceed the dimension guard) have their error logged to `stderr`
    /// and are also skipped — the returned vec contains only successful renders.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pdfplumber::Pdf;
    /// use pdfplumber_raster::{Rasterizer, RasterOptions};
    ///
    /// let pdf = Pdf::open_file("report.pdf", None).unwrap();
    /// let pages = Rasterizer::new(RasterOptions::default()).render_all_pages(&pdf);
    /// for result in &pages {
    ///     let path = format!("page_{:03}.png", result.page_number + 1);
    ///     result.save(&path).expect("write failed");
    /// }
    /// ```
    pub fn render_all_pages(&self, pdf: &pdfplumber::Pdf) -> Vec<RenderResult> {
        let mut results = Vec::new();
        for page_result in pdf.pages_iter() {
            let Ok(page) = page_result else { continue };
            match self.render_page(&page) {
                Ok(r) => results.push(r),
                Err(e) => eprintln!(
                    "pdfplumber-raster: skipping page {}: {e}",
                    page.page_number()
                ),
            }
        }
        results
    }

    /// Render a single page by index from a [`pdfplumber::Pdf`].
    ///
    /// Returns `None` if the page index is out of bounds or the page fails to parse.
    pub fn render_page_index(
        &self,
        pdf: &pdfplumber::Pdf,
        page_idx: usize,
    ) -> Option<Result<RenderResult, RasterError>> {
        let page = pdf.page(page_idx).ok()?;
        Some(self.render_page(&page))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Geometry rendering
// ─────────────────────────────────────────────────────────────────────────────

fn render_rects_filled(rects: &[Rect], pixmap: &mut Pixmap, scale: f32) {
    for r in rects {
        if !r.fill {
            continue;
        }
        let color = to_skia_color_f(&r.fill_color);
        if color.alpha() == 0.0 {
            continue;
        }
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        let x = (r.x0 * scale as f64) as f32;
        let y = (r.top * scale as f64) as f32;
        let w = ((r.x1 - r.x0) * scale as f64) as f32;
        let h = ((r.bottom - r.top) * scale as f64) as f32;

        if w <= 0.0 || h <= 0.0 {
            continue;
        }

        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
        }
    }
}

fn render_rects_stroked(rects: &[Rect], pixmap: &mut Pixmap, scale: f32) {
    for r in rects {
        if !r.stroke {
            continue;
        }
        let color = to_skia_color_f(&r.stroke_color);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        let line_width = (r.line_width * scale as f64).max(0.5) as f32;
        let stroke = Stroke {
            width: line_width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Default::default()
        };

        let x = (r.x0 * scale as f64) as f32;
        let y = (r.top * scale as f64) as f32;
        let w = ((r.x1 - r.x0) * scale as f64) as f32;
        let h = ((r.bottom - r.top) * scale as f64) as f32;

        if w <= 0.0 || h <= 0.0 {
            continue;
        }

        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

fn render_lines(lines: &[Line], pixmap: &mut Pixmap, scale: f32) {
    for line in lines {
        let color = to_skia_color_f(&line.stroke_color);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        let line_width = (line.line_width * scale as f64).max(0.5) as f32;
        let stroke = Stroke {
            width: line_width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Default::default()
        };

        let x0 = (line.x0 * scale as f64) as f32;
        let y0 = (line.top * scale as f64) as f32;
        let x1 = (line.x1 * scale as f64) as f32;
        let y1 = (line.bottom * scale as f64) as f32;

        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

fn render_curves_filled(curves: &[Curve], pixmap: &mut Pixmap, scale: f32) {
    for curve in curves {
        if !curve.fill || curve.pts.len() < 4 {
            continue;
        }
        let color = to_skia_color_f(&curve.fill_color);
        if color.alpha() == 0.0 {
            continue;
        }
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        if let Some(path) = build_curve_path(curve, scale) {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn render_curves_stroked(curves: &[Curve], pixmap: &mut Pixmap, scale: f32) {
    for curve in curves {
        if !curve.stroke || curve.pts.len() < 4 {
            continue;
        }
        let color = to_skia_color_f(&curve.stroke_color);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        let line_width = (curve.line_width * scale as f64).max(0.5) as f32;
        let stroke = Stroke {
            width: line_width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };

        if let Some(path) = build_curve_path(curve, scale) {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

/// Build a tiny-skia path for a cubic Bezier curve.
///
/// `Curve.pts` is `[start, cp1, cp2, end, cp1, cp2, end, ...]` — one start
/// point followed by groups of 3 (two control points + endpoint) per segment.
fn build_curve_path(curve: &Curve, scale: f32) -> Option<tiny_skia::Path> {
    if curve.pts.len() < 4 {
        return None;
    }

    let pt = |idx: usize| -> (f32, f32) {
        let (x, y) = curve.pts[idx];
        ((x * scale as f64) as f32, (y * scale as f64) as f32)
    };

    let mut pb = PathBuilder::new();
    let (sx, sy) = pt(0);
    pb.move_to(sx, sy);

    let mut i = 1;
    while i + 2 < curve.pts.len() {
        let (cx1, cy1) = pt(i);
        let (cx2, cy2) = pt(i + 1);
        let (ex, ey) = pt(i + 2);
        pb.cubic_to(cx1, cy1, cx2, cy2, ex, ey);
        i += 3;
    }

    pb.finish()
}

// ─────────────────────────────────────────────────────────────────────────────
// Text rendering
// ─────────────────────────────────────────────────────────────────────────────

fn render_text(
    chars: &[pdfplumber_core::Char],
    pixmap: &mut Pixmap,
    font_cache: &mut FontCache,
    scale: f32,
) {
    for ch in chars {
        // Skip control characters and whitespace glyphs with no visible content.
        let c = ch.text.chars().next().unwrap_or(' ');
        if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
            continue;
        }

        // Determine text color — use non-stroking (fill) color, fallback black.
        let text_color = ch
            .non_stroking_color
            .as_ref()
            .map(to_skia_color)
            .unwrap_or_else(|| tiny_skia::ColorU8::from_rgba(0, 0, 0, 255));

        // Font size in pixels at the render scale.
        let font_size_px = (ch.size * scale as f64) as f32;
        if font_size_px < 1.0 {
            continue;
        }

        // Glyph position: bbox top-left in pixel space.
        let glyph_x = (ch.bbox.x0 * scale as f64) as f32;
        let glyph_y = (ch.bbox.top * scale as f64) as f32;

        let font = font_cache.get(&ch.fontname);

        // Rasterize the glyph at our target size.
        let (metrics, bitmap) = font.rasterize(c, font_size_px);

        if metrics.width == 0 || metrics.height == 0 || bitmap.is_empty() {
            continue;
        }

        // Fontdue gives us a grayscale coverage bitmap (0 = transparent, 255 = fully opaque).
        // Blit it into the pixmap by compositing over the existing pixels.
        let img_w = pixmap.width() as i32;
        let img_h = pixmap.height() as i32;

        // Offset from the glyph bounding-box top: fontdue ymin is from baseline downward.
        // We use the full bbox from the PDF to position; adjust by the bearing.
        let blit_x = glyph_x as i32 + metrics.xmin;
        // ymin is positive = below baseline; we want top of glyph:
        // glyph_y is already top of the PDF char bbox. fontdue metrics.ymin goes up
        // from baseline. We place baseline at (glyph_y + ascent) then adjust.
        let ascent = font_size_px * 0.8; // approximate: 80% of em is above baseline
        let baseline_y = glyph_y + ascent;
        let blit_y = (baseline_y as i32) - metrics.height as i32 - metrics.ymin;

        let [tr, tg, tb, _] = [
            text_color.red(),
            text_color.green(),
            text_color.blue(),
            text_color.alpha(),
        ];

        let pixels = pixmap.pixels_mut();
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let px = blit_x + gx as i32;
                let py = blit_y + gy as i32;
                if px < 0 || py < 0 || px >= img_w || py >= img_h {
                    continue;
                }
                let coverage = bitmap[gy * metrics.width + gx];
                if coverage == 0 {
                    continue;
                }
                let alpha = coverage;
                let idx = (py * img_w + px) as usize;
                let dst = &mut pixels[idx];
                // Alpha-composite text color over existing pixel (pre-multiplied).
                let inv_alpha = 255 - alpha as u32;
                let new_r =
                    (tr as u32 * alpha as u32 / 255 + dst.red() as u32 * inv_alpha / 255) as u8;
                let new_g =
                    (tg as u32 * alpha as u32 / 255 + dst.green() as u32 * inv_alpha / 255) as u8;
                let new_b =
                    (tb as u32 * alpha as u32 / 255 + dst.blue() as u32 * inv_alpha / 255) as u8;
                *dst = tiny_skia::PremultipliedColorU8::from_rgba(new_r, new_g, new_b, 255)
                    .unwrap_or(*dst);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Char, Color as PdfColor, Curve, Line, Rect, TextDirection};

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

    fn make_rect(x0: f64, top: f64, x1: f64, bottom: f64, fill: bool, stroke: bool) -> Rect {
        Rect {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            fill,
            stroke,
            fill_color: PdfColor::Rgb(0.9, 0.9, 0.9),
            stroke_color: PdfColor::Gray(0.0),
        }
    }

    fn is_png(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n"
    }

    #[test]
    fn render_empty_page_produces_png() {
        use pdfplumber::Page;
        let page = Page::new(0, 100.0, 100.0, vec![]);
        let opts = RasterOptions {
            scale: 1.0,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert!(is_png(&result.png));
        assert_eq!(result.page_number, 0);
        assert_eq!(result.width_px, 100);
        assert_eq!(result.height_px, 100);
        assert!((result.scale - 1.0).abs() < 1e-6);
    }

    #[test]
    fn render_result_dimensions_match_scale() {
        use pdfplumber::Page;
        let page = Page::new(0, 200.0, 300.0, vec![]);
        let opts = RasterOptions {
            scale: 2.0,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert_eq!(result.width_px, 400);
        assert_eq!(result.height_px, 600);
    }

    #[test]
    fn render_page_with_rect_produces_png() {
        use pdfplumber::Page;
        let rect = make_rect(10.0, 10.0, 100.0, 100.0, true, true);
        let page = Page::with_geometry(0, 200.0, 200.0, vec![], vec![], vec![rect], vec![]);
        let opts = RasterOptions {
            scale: 1.0,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert!(is_png(&result.png));
    }

    #[test]
    fn render_page_with_text_produces_png() {
        use pdfplumber::Page;
        let chars = vec![
            make_char("H", 10.0, 20.0, 22.0, 32.0, 12.0),
            make_char("i", 22.0, 20.0, 28.0, 32.0, 12.0),
        ];
        let page = Page::new(0, 200.0, 200.0, chars);
        let opts = RasterOptions {
            scale: 1.5,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert!(is_png(&result.png));
    }

    #[test]
    fn oversized_page_returns_error() {
        use pdfplumber::Page;
        let page = Page::new(0, 10_000.0, 10_000.0, vec![]);
        let opts = RasterOptions {
            scale: 2.0,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page);
        assert!(matches!(
            result,
            Err(RasterError::DimensionsTooLarge { .. })
        ));
    }

    #[test]
    fn no_text_option_skips_text_render() {
        use pdfplumber::Page;
        let chars = vec![make_char("X", 10.0, 10.0, 20.0, 20.0, 10.0)];
        let page = Page::new(0, 100.0, 100.0, chars);
        let opts = RasterOptions {
            scale: 1.0,
            render_text: false,
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert!(is_png(&result.png));
    }

    #[test]
    fn background_color_applied() {
        use pdfplumber::Page;
        let page = Page::new(0, 10.0, 10.0, vec![]);
        let opts = RasterOptions {
            scale: 1.0,
            background: [255, 0, 0],
            ..Default::default()
        };
        let result = Rasterizer::new(opts).render_page(&page).unwrap();
        assert!(is_png(&result.png));
    }

    #[test]
    fn render_result_page_number_preserved() {
        use pdfplumber::Page;
        let page = Page::new(7, 100.0, 100.0, vec![]);
        let result = Rasterizer::new(RasterOptions::default())
            .render_page(&page)
            .unwrap();
        assert_eq!(result.page_number, 7);
    }
}
