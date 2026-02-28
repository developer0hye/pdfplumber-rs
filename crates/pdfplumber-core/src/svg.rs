//! SVG rendering for visual debugging of PDF pages.
//!
//! Generates SVG representations of PDF pages showing page boundaries
//! and coordinate system. This is the foundation for the visual debugging
//! system — Python pdfplumber's most unique feature.

use crate::geometry::BBox;

/// Options for SVG generation.
#[derive(Debug, Clone)]
pub struct SvgOptions {
    /// Optional fixed width for the SVG output. If `None`, uses the page width.
    pub width: Option<f64>,
    /// Optional fixed height for the SVG output. If `None`, uses the page height.
    pub height: Option<f64>,
    /// Scale factor for the SVG output. Default is `1.0`.
    pub scale: f64,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            scale: 1.0,
        }
    }
}

/// Renders PDF page content as SVG markup for visual debugging.
///
/// `SvgRenderer` takes page dimensions and produces valid SVG 1.1 markup.
/// The SVG coordinate system matches pdfplumber's top-left origin system.
pub struct SvgRenderer {
    /// Page width in points.
    page_width: f64,
    /// Page height in points.
    page_height: f64,
    /// Bounding box of the page.
    page_bbox: BBox,
}

impl SvgRenderer {
    /// Create a new `SvgRenderer` for a page with the given dimensions.
    pub fn new(page_width: f64, page_height: f64) -> Self {
        let page_bbox = BBox::new(0.0, 0.0, page_width, page_height);
        Self {
            page_width,
            page_height,
            page_bbox,
        }
    }

    /// Generate SVG markup for the page.
    ///
    /// The output is a complete, valid SVG 1.1 document including:
    /// - Proper `viewBox` matching page dimensions
    /// - Page boundary rectangle
    /// - SVG coordinate system matching top-left origin
    pub fn to_svg(&self, options: &SvgOptions) -> String {
        let view_width = self.page_width;
        let view_height = self.page_height;

        let svg_width = options.width.unwrap_or(self.page_width * options.scale);
        let svg_height = options.height.unwrap_or(self.page_height * options.scale);

        let mut svg = String::new();

        // SVG header
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" \
             width=\"{svg_width}\" height=\"{svg_height}\" \
             viewBox=\"0 0 {view_width} {view_height}\">\n"
        ));

        // Page boundary rectangle
        svg.push_str(&format!(
            "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             fill=\"white\" stroke=\"black\" stroke-width=\"0.5\"/>\n",
            self.page_bbox.x0,
            self.page_bbox.top,
            self.page_bbox.width(),
            self.page_bbox.height(),
        ));

        // Close SVG
        svg.push_str("</svg>\n");

        svg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
