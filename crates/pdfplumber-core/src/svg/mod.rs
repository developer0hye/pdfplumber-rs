//! SVG rendering for visual debugging of PDF pages.
//!
//! Generates SVG representations of PDF pages showing page boundaries,
//! coordinate system, and overlaid extracted objects (chars, lines, rects,
//! edges, tables). This is pdfplumber's visual debugging system — Python
//! pdfplumber's most unique feature.

use crate::edges::Edge;
use crate::geometry::BBox;
use crate::shapes::{Line, Rect};
use crate::table::{Cell, Intersection, Table};
use crate::text::Char;

/// Style options for drawing overlays on the SVG page.
#[derive(Debug, Clone)]
pub struct DrawStyle {
    /// Fill color (CSS color string). `None` means no fill.
    pub fill: Option<String>,
    /// Stroke color (CSS color string). `None` means no stroke.
    pub stroke: Option<String>,
    /// Stroke width in points.
    pub stroke_width: f64,
    /// Opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f64,
}

impl Default for DrawStyle {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: Some("black".to_string()),
            stroke_width: 0.5,
            opacity: 1.0,
        }
    }
}

impl DrawStyle {
    /// Default style for character bounding boxes (blue outline).
    pub fn chars_default() -> Self {
        Self {
            fill: None,
            stroke: Some("blue".to_string()),
            stroke_width: 0.3,
            opacity: 0.7,
        }
    }

    /// Default style for lines (red).
    pub fn lines_default() -> Self {
        Self {
            fill: None,
            stroke: Some("red".to_string()),
            stroke_width: 1.0,
            opacity: 0.8,
        }
    }

    /// Default style for rectangles (green outline).
    pub fn rects_default() -> Self {
        Self {
            fill: None,
            stroke: Some("green".to_string()),
            stroke_width: 0.5,
            opacity: 0.8,
        }
    }

    /// Default style for edges (orange).
    pub fn edges_default() -> Self {
        Self {
            fill: None,
            stroke: Some("orange".to_string()),
            stroke_width: 0.5,
            opacity: 0.8,
        }
    }

    /// Default style for table cell boundaries (lightblue fill).
    pub fn tables_default() -> Self {
        Self {
            fill: Some("lightblue".to_string()),
            stroke: Some("steelblue".to_string()),
            stroke_width: 0.5,
            opacity: 0.3,
        }
    }

    /// Default style for intersection points (red filled circles).
    pub fn intersections_default() -> Self {
        Self {
            fill: Some("red".to_string()),
            stroke: Some("darkred".to_string()),
            stroke_width: 0.5,
            opacity: 0.9,
        }
    }

    /// Default style for cell boundaries (dashed pink outline).
    pub fn cells_default() -> Self {
        Self {
            fill: None,
            stroke: Some("magenta".to_string()),
            stroke_width: 0.5,
            opacity: 0.6,
        }
    }

    /// Build the SVG style attribute string.
    fn to_svg_style(&self) -> String {
        let mut parts = Vec::new();
        match &self.fill {
            Some(color) => parts.push(format!("fill:{color}")),
            None => parts.push("fill:none".to_string()),
        }
        if let Some(color) = &self.stroke {
            parts.push(format!("stroke:{color}"));
            parts.push(format!("stroke-width:{}", self.stroke_width));
        } else {
            parts.push("stroke:none".to_string());
        }
        if (self.opacity - 1.0).abs() > f64::EPSILON {
            parts.push(format!("opacity:{}", self.opacity));
        }
        parts.join(";")
    }
}

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

/// Options for the debug_tablefinder SVG output.
///
/// Controls which pipeline stages are rendered in the debug SVG.
/// All flags default to `true`.
#[derive(Debug, Clone)]
pub struct SvgDebugOptions {
    /// Show detected edges (red lines).
    pub show_edges: bool,
    /// Show intersection points (small circles).
    pub show_intersections: bool,
    /// Show cell boundaries (dashed lines).
    pub show_cells: bool,
    /// Show table bounding boxes (light blue rectangles).
    pub show_tables: bool,
}

impl Default for SvgDebugOptions {
    fn default() -> Self {
        Self {
            show_edges: true,
            show_intersections: true,
            show_cells: true,
            show_tables: true,
        }
    }
}

/// Renders PDF page content as SVG markup for visual debugging.
///
/// `SvgRenderer` takes page dimensions and produces valid SVG 1.1 markup.
/// The SVG coordinate system matches pdfplumber's top-left origin system.
///
/// Use the `draw_*` methods to add overlay elements, then call `to_svg()`
/// to produce the final SVG string.
pub struct SvgRenderer {
    /// Page width in points.
    page_width: f64,
    /// Page height in points.
    page_height: f64,
    /// Bounding box of the page.
    page_bbox: BBox,
    /// Accumulated SVG overlay elements.
    elements: Vec<String>,
}

impl SvgRenderer {
    /// Create a new `SvgRenderer` for a page with the given dimensions.
    pub fn new(page_width: f64, page_height: f64) -> Self {
        let page_bbox = BBox::new(0.0, 0.0, page_width, page_height);
        Self {
            page_width,
            page_height,
            page_bbox,
            elements: Vec::new(),
        }
    }

    /// Draw character bounding boxes onto the SVG.
    pub fn draw_chars(&mut self, chars: &[Char], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for ch in chars {
            self.elements.push(format!(
                "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"{style_attr}\"/>\n",
                ch.bbox.x0,
                ch.bbox.top,
                ch.bbox.width(),
                ch.bbox.height(),
            ));
        }
    }

    /// Draw rectangle outlines/fills onto the SVG.
    pub fn draw_rects(&mut self, rects: &[Rect], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for r in rects {
            self.elements.push(format!(
                "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"{style_attr}\"/>\n",
                r.x0,
                r.top,
                r.x1 - r.x0,
                r.bottom - r.top,
            ));
        }
    }

    /// Draw line segments onto the SVG.
    pub fn draw_lines(&mut self, lines: &[Line], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for l in lines {
            self.elements.push(format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"{style_attr}\"/>\n",
                l.x0, l.top, l.x1, l.bottom,
            ));
        }
    }

    /// Draw detected edges onto the SVG.
    pub fn draw_edges(&mut self, edges: &[Edge], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for e in edges {
            self.elements.push(format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"{style_attr}\"/>\n",
                e.x0, e.top, e.x1, e.bottom,
            ));
        }
    }

    /// Draw intersection points as small circles onto the SVG.
    pub fn draw_intersections(&mut self, intersections: &[Intersection], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        let radius = 3.0;
        for pt in intersections {
            self.elements.push(format!(
                "  <circle cx=\"{}\" cy=\"{}\" r=\"{radius}\" style=\"{style_attr}\"/>\n",
                pt.x, pt.y,
            ));
        }
    }

    /// Draw cell boundaries as dashed rectangles onto the SVG.
    pub fn draw_cells(&mut self, cells: &[Cell], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for cell in cells {
            self.elements.push(format!(
                "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"{style_attr}\" stroke-dasharray=\"4,2\"/>\n",
                cell.bbox.x0,
                cell.bbox.top,
                cell.bbox.width(),
                cell.bbox.height(),
            ));
        }
    }

    /// Draw table cell boundaries onto the SVG.
    pub fn draw_tables(&mut self, tables: &[Table], style: &DrawStyle) {
        let style_attr = style.to_svg_style();
        for table in tables {
            // Draw each cell
            for cell in &table.cells {
                self.elements.push(format!(
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"{style_attr}\"/>\n",
                    cell.bbox.x0,
                    cell.bbox.top,
                    cell.bbox.width(),
                    cell.bbox.height(),
                ));
            }
        }
    }

    /// Generate SVG markup for the page.
    ///
    /// The output is a complete, valid SVG 1.1 document including:
    /// - Proper `viewBox` matching page dimensions
    /// - Page boundary rectangle
    /// - All overlay elements added via `draw_*` methods
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

        // Overlay elements
        for element in &self.elements {
            svg.push_str(element);
        }

        // Close SVG
        svg.push_str("</svg>\n");

        svg
    }
}


#[cfg(test)]
mod tests;
