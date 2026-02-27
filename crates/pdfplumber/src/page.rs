//! Page type for accessing extracted content from a PDF page.

use pdfplumber_core::{
    Char, Curve, Edge, Line, Rect, Word, WordExtractor, WordOptions, derive_edges,
};

/// A single page from a PDF document.
///
/// Provides access to characters, words, lines, rects, curves, and edges
/// extracted from the page. Constructed internally by the PDF parsing pipeline.
pub struct Page {
    /// Page index (0-based).
    page_number: usize,
    /// Page width in points.
    width: f64,
    /// Page height in points.
    height: f64,
    /// Characters extracted from this page.
    chars: Vec<Char>,
    /// Lines extracted from painted paths.
    lines: Vec<Line>,
    /// Rectangles extracted from painted paths.
    rects: Vec<Rect>,
    /// Curves extracted from painted paths.
    curves: Vec<Curve>,
}

impl Page {
    /// Create a new page with the given metadata and characters.
    pub fn new(page_number: usize, width: f64, height: f64, chars: Vec<Char>) -> Self {
        Self {
            page_number,
            width,
            height,
            chars,
            lines: Vec::new(),
            rects: Vec::new(),
            curves: Vec::new(),
        }
    }

    /// Create a new page with characters and geometry.
    pub fn with_geometry(
        page_number: usize,
        width: f64,
        height: f64,
        chars: Vec<Char>,
        lines: Vec<Line>,
        rects: Vec<Rect>,
        curves: Vec<Curve>,
    ) -> Self {
        Self {
            page_number,
            width,
            height,
            chars,
            lines,
            rects,
            curves,
        }
    }

    /// Returns the page index (0-based).
    pub fn page_number(&self) -> usize {
        self.page_number
    }

    /// Returns the page width in points.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Returns the page height in points.
    pub fn height(&self) -> f64 {
        self.height
    }

    /// Returns the characters extracted from this page.
    pub fn chars(&self) -> &[Char] {
        &self.chars
    }

    /// Returns the lines extracted from this page.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Returns the rectangles extracted from this page.
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Returns the curves extracted from this page.
    pub fn curves(&self) -> &[Curve] {
        &self.curves
    }

    /// Compute edges from all geometric primitives (lines, rects, curves).
    ///
    /// Edges are line segments derived from all geometric objects on the page,
    /// suitable for table detection. Each edge tracks its source (Line, Rect side, Curve chord).
    pub fn edges(&self) -> Vec<Edge> {
        derive_edges(&self.lines, &self.rects, &self.curves)
    }

    /// Extract words from this page using the specified options.
    ///
    /// Groups characters into words based on spatial proximity using
    /// `x_tolerance` and `y_tolerance` from the options.
    pub fn extract_words(&self, options: &WordOptions) -> Vec<Word> {
        WordExtractor::extract(&self.chars, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Color, EdgeSource, LineOrientation};

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "TestFont".to_string(),
            size: 12.0,
        }
    }

    fn make_line(x0: f64, top: f64, x1: f64, bottom: f64, orient: LineOrientation) -> Line {
        Line {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            stroke_color: Color::black(),
            orientation: orient,
        }
    }

    fn make_rect(x0: f64, top: f64, x1: f64, bottom: f64) -> Rect {
        Rect {
            x0,
            top,
            x1,
            bottom,
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        }
    }

    fn make_curve(pts: Vec<(f64, f64)>) -> Curve {
        let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        Curve {
            x0: xs.iter().cloned().fold(f64::INFINITY, f64::min),
            top: ys.iter().cloned().fold(f64::INFINITY, f64::min),
            x1: xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            bottom: ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            pts,
            line_width: 1.0,
            stroke: true,
            fill: false,
            stroke_color: Color::black(),
            fill_color: Color::black(),
        }
    }

    #[test]
    fn test_page_creation() {
        let page = Page::new(0, 612.0, 792.0, vec![]);
        assert_eq!(page.page_number(), 0);
        assert_eq!(page.width(), 612.0);
        assert_eq!(page.height(), 792.0);
        assert!(page.chars().is_empty());
    }

    #[test]
    fn test_page_with_chars() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("i", 20.0, 100.0, 30.0, 112.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars);
        assert_eq!(page.chars().len(), 2);
        assert_eq!(page.chars()[0].text, "H");
        assert_eq!(page.chars()[1].text, "i");
    }

    #[test]
    fn test_extract_words_default_options() {
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("e", 20.0, 100.0, 30.0, 112.0),
            make_char("l", 30.0, 100.0, 35.0, 112.0),
            make_char("l", 35.0, 100.0, 40.0, 112.0),
            make_char("o", 40.0, 100.0, 50.0, 112.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars);
        let words = page.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[0].bbox, BBox::new(10.0, 100.0, 50.0, 112.0));
        assert_eq!(words[0].chars.len(), 5);
    }

    #[test]
    fn test_extract_words_text_concatenation() {
        // "The quick fox" with spaces separating words
        let chars = vec![
            make_char("T", 10.0, 100.0, 20.0, 112.0),
            make_char("h", 20.0, 100.0, 28.0, 112.0),
            make_char("e", 28.0, 100.0, 36.0, 112.0),
            make_char(" ", 36.0, 100.0, 40.0, 112.0),
            make_char("q", 40.0, 100.0, 48.0, 112.0),
            make_char("u", 48.0, 100.0, 56.0, 112.0),
            make_char("i", 56.0, 100.0, 60.0, 112.0),
            make_char("c", 60.0, 100.0, 68.0, 112.0),
            make_char("k", 68.0, 100.0, 76.0, 112.0),
            make_char(" ", 76.0, 100.0, 80.0, 112.0),
            make_char("f", 80.0, 100.0, 88.0, 112.0),
            make_char("o", 88.0, 100.0, 96.0, 112.0),
            make_char("x", 96.0, 100.0, 104.0, 112.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars);
        let words = page.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "The");
        assert_eq!(words[1].text, "quick");
        assert_eq!(words[2].text, "fox");
    }

    #[test]
    fn test_extract_words_bbox_calculation() {
        // Characters with varying heights — verify the word bbox is the union
        let chars = vec![
            make_char("A", 10.0, 98.0, 20.0, 112.0),
            make_char("b", 20.0, 100.0, 28.0, 110.0),
            make_char("C", 28.0, 97.0, 38.0, 113.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars);
        let words = page.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 1);
        // Union: x0=10, top=97, x1=38, bottom=113
        assert_eq!(words[0].bbox, BBox::new(10.0, 97.0, 38.0, 113.0));
    }

    #[test]
    fn test_extract_words_multiline() {
        // Two lines of text
        let chars = vec![
            make_char("H", 10.0, 100.0, 20.0, 112.0),
            make_char("i", 20.0, 100.0, 30.0, 112.0),
            make_char("L", 10.0, 120.0, 20.0, 132.0),
            make_char("o", 20.0, 120.0, 30.0, 132.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars);
        let words = page.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hi");
        assert_eq!(words[1].text, "Lo");
    }

    #[test]
    fn test_extract_words_custom_options() {
        // Two chars with gap=10, default tolerance=3 splits them, custom tolerance=15 groups them
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 30.0, 100.0, 40.0, 112.0), // gap = 10
        ];
        let page = Page::new(0, 612.0, 792.0, chars);

        let default_words = page.extract_words(&WordOptions::default());
        assert_eq!(default_words.len(), 2);

        let custom_opts = WordOptions {
            x_tolerance: 15.0,
            ..WordOptions::default()
        };
        let custom_words = page.extract_words(&custom_opts);
        assert_eq!(custom_words.len(), 1);
        assert_eq!(custom_words[0].text, "AB");
    }

    #[test]
    fn test_extract_words_empty_page() {
        let page = Page::new(0, 612.0, 792.0, vec![]);
        let words = page.extract_words(&WordOptions::default());
        assert!(words.is_empty());
    }

    #[test]
    fn test_extract_words_constituent_chars() {
        // Verify that words contain their constituent chars
        let chars = vec![
            make_char("A", 10.0, 100.0, 20.0, 112.0),
            make_char("B", 20.0, 100.0, 30.0, 112.0),
        ];
        let page = Page::new(0, 612.0, 792.0, chars.clone());
        let words = page.extract_words(&WordOptions::default());

        assert_eq!(words.len(), 1);
        assert_eq!(words[0].chars.len(), 2);
        assert_eq!(words[0].chars[0].text, "A");
        assert_eq!(words[0].chars[1].text, "B");
        assert_eq!(words[0].chars[0].bbox, BBox::new(10.0, 100.0, 20.0, 112.0));
        assert_eq!(words[0].chars[1].bbox, BBox::new(20.0, 100.0, 30.0, 112.0));
    }

    // --- Geometry accessors ---

    #[test]
    fn test_page_new_has_empty_geometry() {
        let page = Page::new(0, 612.0, 792.0, vec![]);
        assert!(page.lines().is_empty());
        assert!(page.rects().is_empty());
        assert!(page.curves().is_empty());
        assert!(page.edges().is_empty());
    }

    #[test]
    fn test_page_with_geometry() {
        let lines = vec![make_line(
            0.0,
            50.0,
            100.0,
            50.0,
            LineOrientation::Horizontal,
        )];
        let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
        let curves = vec![make_curve(vec![
            (0.0, 100.0),
            (10.0, 50.0),
            (90.0, 50.0),
            (100.0, 100.0),
        ])];
        let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, rects, curves);

        assert_eq!(page.lines().len(), 1);
        assert_eq!(page.rects().len(), 1);
        assert_eq!(page.curves().len(), 1);
    }

    #[test]
    fn test_page_edges_from_lines() {
        let lines = vec![
            make_line(0.0, 50.0, 100.0, 50.0, LineOrientation::Horizontal),
            make_line(50.0, 0.0, 50.0, 100.0, LineOrientation::Vertical),
        ];
        let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, vec![], vec![]);
        let edges = page.edges();

        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].source, EdgeSource::Line);
        assert_eq!(edges[1].source, EdgeSource::Line);
    }

    #[test]
    fn test_page_edges_from_rects() {
        let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
        let page = Page::with_geometry(0, 612.0, 792.0, vec![], vec![], rects, vec![]);
        let edges = page.edges();

        assert_eq!(edges.len(), 4);
        assert_eq!(edges[0].source, EdgeSource::RectTop);
        assert_eq!(edges[1].source, EdgeSource::RectBottom);
        assert_eq!(edges[2].source, EdgeSource::RectLeft);
        assert_eq!(edges[3].source, EdgeSource::RectRight);
    }

    #[test]
    fn test_page_edges_combined() {
        let lines = vec![make_line(
            0.0,
            50.0,
            100.0,
            50.0,
            LineOrientation::Horizontal,
        )];
        let rects = vec![make_rect(10.0, 20.0, 110.0, 70.0)];
        let curves = vec![make_curve(vec![
            (0.0, 100.0),
            (10.0, 50.0),
            (90.0, 50.0),
            (100.0, 100.0),
        ])];
        let page = Page::with_geometry(0, 612.0, 792.0, vec![], lines, rects, curves);
        let edges = page.edges();

        // 1 from line + 4 from rect + 1 from curve = 6
        assert_eq!(edges.len(), 6);
        assert_eq!(edges[0].source, EdgeSource::Line);
        assert_eq!(edges[5].source, EdgeSource::Curve);
    }
}
