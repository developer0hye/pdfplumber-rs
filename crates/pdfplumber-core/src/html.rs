//! HTML rendering for PDF page content.
//!
//! Converts extracted text, tables, and structural elements into
//! semantic HTML. Useful for document conversion and web display.

use crate::layout::{
    TextBlock, TextLine, cluster_lines_into_blocks, cluster_words_into_lines,
    sort_blocks_reading_order, split_lines_at_columns,
};
use crate::table::Table;
use crate::text::Char;
use crate::words::{Word, WordExtractor, WordOptions};

/// Options for HTML rendering.
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    /// Vertical tolerance for clustering words into lines (in points).
    pub y_tolerance: f64,
    /// Maximum vertical gap for grouping lines into blocks (in points).
    pub y_density: f64,
    /// Minimum horizontal gap to detect column boundaries (in points).
    pub x_density: f64,
    /// Minimum font size ratio (relative to median) to consider text a heading.
    pub heading_min_ratio: f64,
    /// Whether to detect bullet/numbered lists from text patterns.
    pub detect_lists: bool,
    /// Whether to detect bold/italic from font name analysis.
    pub detect_emphasis: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            y_tolerance: 3.0,
            y_density: 10.0,
            x_density: 10.0,
            heading_min_ratio: 1.2,
            detect_lists: true,
            detect_emphasis: true,
        }
    }
}

/// A content element identified during HTML rendering.
#[derive(Debug, Clone, PartialEq)]
enum HtmlElement {
    /// A heading with level (1-6) and text content.
    Heading { level: u8, text: String },
    /// A paragraph of text (may contain inline HTML for emphasis).
    Paragraph(String),
    /// An HTML table.
    Table(String),
    /// A list item (bullet or numbered).
    ListItem {
        /// Whether it's a numbered (ordered) list item.
        ordered: bool,
        /// The text content.
        text: String,
    },
}

/// Renders PDF page content as semantic HTML.
pub struct HtmlRenderer;

impl HtmlRenderer {
    /// Render characters and tables as HTML.
    ///
    /// This is the main entry point. It:
    /// 1. Extracts words from characters
    /// 2. Groups words into text blocks
    /// 3. Classifies blocks as headings, paragraphs, or lists
    /// 4. Converts tables to HTML table elements
    /// 5. Interleaves text and tables in reading order
    pub fn render(chars: &[Char], tables: &[Table], options: &HtmlOptions) -> String {
        if chars.is_empty() && tables.is_empty() {
            return String::new();
        }

        let words = WordExtractor::extract(
            chars,
            &WordOptions {
                y_tolerance: options.y_tolerance,
                ..WordOptions::default()
            },
        );

        let lines = cluster_words_into_lines(&words, options.y_tolerance);
        let split = split_lines_at_columns(lines, options.x_density);
        let mut blocks = cluster_lines_into_blocks(split, options.y_density);
        sort_blocks_reading_order(&mut blocks, options.x_density);

        let median_size = compute_median_font_size(chars);

        let mut elements = classify_blocks(&blocks, median_size, options);

        // Insert tables
        for table in tables {
            let table_html = table_to_html(table);
            elements.push(HtmlElement::Table(table_html));
        }

        render_elements(&elements)
    }

    /// Render characters as HTML (no tables).
    pub fn render_text(chars: &[Char], options: &HtmlOptions) -> String {
        Self::render(chars, &[], options)
    }

    /// Convert a table to HTML table element.
    pub fn table_to_html(table: &Table) -> String {
        table_to_html(table)
    }

    /// Detect heading level from font size relative to median.
    ///
    /// Returns `Some(level)` (1-6) if the text qualifies as a heading,
    /// or `None` if it's normal text.
    pub fn detect_heading_level(font_size: f64, median_size: f64, min_ratio: f64) -> Option<u8> {
        detect_heading_level(font_size, median_size, min_ratio)
    }
}

/// Compute the median font size from characters.
fn compute_median_font_size(chars: &[Char]) -> f64 {
    if chars.is_empty() {
        return 12.0;
    }

    let mut sizes: Vec<f64> = chars
        .iter()
        .filter(|c| c.size > 0.0 && !c.text.trim().is_empty())
        .map(|c| c.size)
        .collect();

    if sizes.is_empty() {
        return 12.0;
    }

    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sizes.len() / 2;
    if sizes.len() % 2 == 0 {
        (sizes[mid - 1] + sizes[mid]) / 2.0
    } else {
        sizes[mid]
    }
}

/// Detect heading level from font size ratio.
fn detect_heading_level(font_size: f64, median_size: f64, min_ratio: f64) -> Option<u8> {
    if median_size <= 0.0 || font_size <= 0.0 {
        return None;
    }

    let ratio = font_size / median_size;
    if ratio < min_ratio {
        return None;
    }

    if ratio >= 2.0 {
        Some(1)
    } else if ratio >= 1.6 {
        Some(2)
    } else if ratio >= 1.3 {
        Some(3)
    } else {
        Some(4)
    }
}

/// Detect if text is a list item. Returns (ordered, prefix, rest_text).
fn detect_list_item(text: &str) -> Option<(bool, String)> {
    let trimmed = text.trim_start();

    // Bullet patterns
    for prefix in &["- ", "* ", "\u{2022} ", "\u{2013} ", "\u{2014} "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some((false, rest.to_string()));
        }
    }

    // Numbered patterns: "1. ", "2) ", etc.
    let bytes = trimmed.as_bytes();
    if !bytes.is_empty() {
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i > 0 && i + 1 < bytes.len() {
            let sep = bytes[i];
            let space = bytes[i + 1];
            if (sep == b'.' || sep == b')') && space == b' ' {
                let rest = &trimmed[i + 2..];
                return Some((true, rest.to_string()));
            }
        }
    }

    None
}

/// Get the dominant font size in a text block.
fn block_dominant_size(block: &TextBlock) -> f64 {
    let mut sizes: Vec<f64> = Vec::new();
    for line in &block.lines {
        for word in &line.words {
            for ch in &word.chars {
                if ch.size > 0.0 && !ch.text.trim().is_empty() {
                    sizes.push(ch.size);
                }
            }
        }
    }
    if sizes.is_empty() {
        return 0.0;
    }

    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut best_size = sizes[0];
    let mut best_count = 1;
    let mut current_count = 1;
    for i in 1..sizes.len() {
        if (sizes[i] - sizes[i - 1]).abs() < 0.1 {
            current_count += 1;
        } else {
            if current_count > best_count {
                best_count = current_count;
                best_size = sizes[i - 1];
            }
            current_count = 1;
        }
    }
    if current_count > best_count {
        best_size = *sizes.last().unwrap();
    }
    best_size
}

/// Check if a font name indicates bold.
fn is_bold_font(fontname: &str) -> bool {
    let lower = fontname.to_lowercase();
    lower.contains("bold") || lower.contains("heavy") || lower.contains("black")
}

/// Check if a font name indicates italic.
fn is_italic_font(fontname: &str) -> bool {
    let lower = fontname.to_lowercase();
    lower.contains("italic") || lower.contains("oblique")
}

/// Get the dominant font name in a word.
fn word_dominant_font(word: &Word) -> &str {
    word.chars
        .iter()
        .find(|c| !c.text.trim().is_empty())
        .map(|c| c.fontname.as_str())
        .unwrap_or("")
}

/// Escape special HTML characters.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Classify text blocks into HTML content elements.
fn classify_blocks(
    blocks: &[TextBlock],
    median_size: f64,
    options: &HtmlOptions,
) -> Vec<HtmlElement> {
    let mut elements = Vec::new();

    for block in blocks {
        let block_text = block_to_text(block);
        if block_text.trim().is_empty() {
            continue;
        }

        let dominant_size = block_dominant_size(block);

        // Check for heading
        if let Some(level) =
            detect_heading_level(dominant_size, median_size, options.heading_min_ratio)
        {
            let is_short =
                block.lines.len() <= 2 && block.lines.iter().all(|l| l.words.len() <= 15);
            if is_short {
                let text = escape_html(block_text.trim());
                elements.push(HtmlElement::Heading { level, text });
                continue;
            }
        }

        // Check for list items
        if options.detect_lists {
            let line_texts: Vec<String> = block.lines.iter().map(line_to_text).collect();
            let all_list_items = line_texts.iter().all(|t| detect_list_item(t).is_some());
            if all_list_items && !line_texts.is_empty() {
                for text in &line_texts {
                    if let Some((ordered, rest)) = detect_list_item(text) {
                        elements.push(HtmlElement::ListItem {
                            ordered,
                            text: escape_html(&rest),
                        });
                    }
                }
                continue;
            }
        }

        // Apply emphasis if enabled
        let rendered_text = if options.detect_emphasis {
            render_block_with_emphasis(block)
        } else {
            escape_html(&block_text)
        };

        elements.push(HtmlElement::Paragraph(rendered_text.trim().to_string()));
    }

    elements
}

/// Convert a text block to plain text.
fn block_to_text(block: &TextBlock) -> String {
    block
        .lines
        .iter()
        .map(line_to_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert a text line to plain text.
fn line_to_text(line: &TextLine) -> String {
    line.words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render a block with bold/italic emphasis as HTML.
fn render_block_with_emphasis(block: &TextBlock) -> String {
    block
        .lines
        .iter()
        .map(render_line_with_emphasis)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render a line with HTML emphasis tags.
fn render_line_with_emphasis(line: &TextLine) -> String {
    let mut parts: Vec<String> = Vec::new();

    for word in &line.words {
        let font = word_dominant_font(word);
        let bold = is_bold_font(font);
        let italic = is_italic_font(font);
        let text = escape_html(&word.text);

        if bold && italic {
            parts.push(format!("<strong><em>{text}</em></strong>"));
        } else if bold {
            parts.push(format!("<strong>{text}</strong>"));
        } else if italic {
            parts.push(format!("<em>{text}</em>"));
        } else {
            parts.push(text);
        }
    }

    parts.join(" ")
}

/// Convert a Table to an HTML table element.
fn table_to_html(table: &Table) -> String {
    if table.rows.is_empty() {
        return String::new();
    }

    let mut html = String::from("<table>\n");

    for (i, row) in table.rows.iter().enumerate() {
        if i == 0 {
            html.push_str("<thead>\n<tr>");
            for cell in row {
                let text = escape_html(cell.text.as_deref().unwrap_or(""));
                html.push_str(&format!("<th>{text}</th>"));
            }
            html.push_str("</tr>\n</thead>\n<tbody>\n");
        } else {
            html.push_str("<tr>");
            for cell in row {
                let text = escape_html(cell.text.as_deref().unwrap_or(""));
                html.push_str(&format!("<td>{text}</td>"));
            }
            html.push_str("</tr>\n");
        }
    }

    html.push_str("</tbody>\n</table>");
    html
}

/// Render HTML elements into a complete HTML string.
fn render_elements(elements: &[HtmlElement]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;

    while i < elements.len() {
        match &elements[i] {
            HtmlElement::Heading { level, text } => {
                parts.push(format!("<h{level}>{text}</h{level}>"));
                i += 1;
            }
            HtmlElement::Paragraph(text) => {
                parts.push(format!("<p>{text}</p>"));
                i += 1;
            }
            HtmlElement::Table(html) => {
                parts.push(html.clone());
                i += 1;
            }
            HtmlElement::ListItem { ordered, .. } => {
                // Collect consecutive list items of the same type
                let is_ordered = *ordered;
                let tag = if is_ordered { "ol" } else { "ul" };
                let mut items = Vec::new();
                while i < elements.len() {
                    if let HtmlElement::ListItem { ordered, text } = &elements[i] {
                        if *ordered == is_ordered {
                            items.push(format!("<li>{text}</li>"));
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                parts.push(format!("<{tag}>\n{}\n</{tag}>", items.join("\n")));
            }
        }
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::BBox;
    use crate::table::Cell;
    use crate::text::TextDirection;

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64, size: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            fontname: "Helvetica".to_string(),
            size,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 0,
        }
    }

    fn make_word_from_text(
        text: &str,
        x0: f64,
        top: f64,
        x1: f64,
        bottom: f64,
        size: f64,
        fontname: &str,
    ) -> Word {
        let chars: Vec<Char> = text
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let char_width = (x1 - x0) / text.len() as f64;
                let cx0 = x0 + i as f64 * char_width;
                let cx1 = cx0 + char_width;
                Char {
                    text: c.to_string(),
                    bbox: BBox::new(cx0, top, cx1, bottom),
                    fontname: fontname.to_string(),
                    size,
                    doctop: top,
                    upright: true,
                    direction: TextDirection::Ltr,
                    stroking_color: None,
                    non_stroking_color: None,
                    ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    char_code: 0,
                }
            })
            .collect();
        Word {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            doctop: top,
            direction: TextDirection::Ltr,
            chars,
        }
    }

    // --- Heading detection tests ---

    #[test]
    fn test_heading_h1() {
        assert_eq!(detect_heading_level(24.0, 12.0, 1.2), Some(1));
    }

    #[test]
    fn test_heading_h2() {
        assert_eq!(detect_heading_level(20.0, 12.0, 1.2), Some(2));
    }

    #[test]
    fn test_heading_h3() {
        assert_eq!(detect_heading_level(16.0, 12.0, 1.2), Some(3));
    }

    #[test]
    fn test_heading_h4() {
        assert_eq!(detect_heading_level(14.5, 12.0, 1.2), Some(4));
    }

    #[test]
    fn test_no_heading_normal_size() {
        assert_eq!(detect_heading_level(12.0, 12.0, 1.2), None);
    }

    #[test]
    fn test_heading_zero_median() {
        assert_eq!(detect_heading_level(12.0, 0.0, 1.2), None);
    }

    // --- HTML escape tests ---

    #[test]
    fn test_escape_html_ampersand() {
        assert_eq!(escape_html("A & B"), "A &amp; B");
    }

    #[test]
    fn test_escape_html_angle_brackets() {
        assert_eq!(escape_html("<div>"), "&lt;div&gt;");
    }

    #[test]
    fn test_escape_html_quotes() {
        assert_eq!(escape_html("say \"hello\""), "say &quot;hello&quot;");
    }

    #[test]
    fn test_escape_html_combined() {
        assert_eq!(escape_html("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    // --- Table to HTML tests ---

    #[test]
    fn test_table_to_html_simple() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            cells: vec![],
            rows: vec![
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 0.0, 50.0, 25.0),
                        text: Some("Name".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 0.0, 100.0, 25.0),
                        text: Some("Age".to_string()),
                    },
                ],
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 25.0, 50.0, 50.0),
                        text: Some("Alice".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 25.0, 100.0, 50.0),
                        text: Some("30".to_string()),
                    },
                ],
            ],
            columns: vec![],
        };
        let html = table_to_html(&table);
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>Name</th>"));
        assert!(html.contains("<th>Age</th>"));
        assert!(html.contains("</thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>Alice</td>"));
        assert!(html.contains("<td>30</td>"));
        assert!(html.contains("</tbody>"));
        assert!(html.contains("</table>"));
    }

    #[test]
    fn test_table_to_html_with_none_cells() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            cells: vec![],
            rows: vec![
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 0.0, 50.0, 25.0),
                        text: Some("Header".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 0.0, 100.0, 25.0),
                        text: None,
                    },
                ],
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 25.0, 50.0, 50.0),
                        text: None,
                    },
                    Cell {
                        bbox: BBox::new(50.0, 25.0, 100.0, 50.0),
                        text: Some("Data".to_string()),
                    },
                ],
            ],
            columns: vec![],
        };
        let html = table_to_html(&table);
        assert!(html.contains("<th>Header</th>"));
        assert!(html.contains("<th></th>"));
        assert!(html.contains("<td></td>"));
        assert!(html.contains("<td>Data</td>"));
    }

    #[test]
    fn test_table_to_html_empty() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            cells: vec![],
            rows: vec![],
            columns: vec![],
        };
        assert_eq!(table_to_html(&table), "");
    }

    #[test]
    fn test_table_to_html_escapes_html() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            cells: vec![],
            rows: vec![
                vec![Cell {
                    bbox: BBox::new(0.0, 0.0, 100.0, 25.0),
                    text: Some("A<B>".to_string()),
                }],
                vec![Cell {
                    bbox: BBox::new(0.0, 25.0, 100.0, 50.0),
                    text: Some("C&D".to_string()),
                }],
            ],
            columns: vec![],
        };
        let html = table_to_html(&table);
        assert!(html.contains("A&lt;B&gt;"));
        assert!(html.contains("C&amp;D"));
    }

    // --- Paragraph wrapping tests ---

    #[test]
    fn test_render_simple_paragraph() {
        let chars = vec![
            make_char("H", 0.0, 0.0, 8.0, 12.0, 12.0),
            make_char("e", 8.0, 0.0, 16.0, 12.0, 12.0),
            make_char("l", 16.0, 0.0, 24.0, 12.0, 12.0),
            make_char("l", 24.0, 0.0, 32.0, 12.0, 12.0),
            make_char("o", 32.0, 0.0, 40.0, 12.0, 12.0),
            make_char(" ", 40.0, 0.0, 44.0, 12.0, 12.0),
            make_char("W", 44.0, 0.0, 52.0, 12.0, 12.0),
            make_char("o", 52.0, 0.0, 60.0, 12.0, 12.0),
            make_char("r", 60.0, 0.0, 68.0, 12.0, 12.0),
            make_char("l", 68.0, 0.0, 76.0, 12.0, 12.0),
            make_char("d", 76.0, 0.0, 84.0, 12.0, 12.0),
        ];
        let result = HtmlRenderer::render_text(&chars, &HtmlOptions::default());
        assert!(
            result.contains("<p>Hello World</p>"),
            "Expected paragraph wrapping, got: {result}"
        );
    }

    #[test]
    fn test_render_heading_detection() {
        let mut chars = Vec::new();
        // Large heading at 24pt
        for (i, c) in "Title".chars().enumerate() {
            chars.push(make_char(
                &c.to_string(),
                i as f64 * 16.0,
                0.0,
                (i + 1) as f64 * 16.0,
                24.0,
                24.0,
            ));
        }
        // Normal body text (gap > y_density)
        for (i, c) in "Body text here".chars().enumerate() {
            let x0 = i as f64 * 8.0;
            chars.push(make_char(&c.to_string(), x0, 40.0, x0 + 8.0, 52.0, 12.0));
        }
        let result = HtmlRenderer::render_text(&chars, &HtmlOptions::default());
        assert!(
            result.contains("<h1>Title</h1>"),
            "Expected H1 heading, got: {result}"
        );
        assert!(
            result.contains("Body text here"),
            "Expected body text, got: {result}"
        );
    }

    #[test]
    fn test_render_empty_input() {
        let result = HtmlRenderer::render(&[], &[], &HtmlOptions::default());
        assert_eq!(result, "");
    }

    // --- Bold/italic emphasis tests ---

    #[test]
    fn test_bold_font_detection() {
        assert!(is_bold_font("Helvetica-Bold"));
        assert!(is_bold_font("TimesNewRoman-BoldItalic"));
        assert!(!is_bold_font("Helvetica"));
        assert!(!is_bold_font("Times-Roman"));
    }

    #[test]
    fn test_italic_font_detection() {
        assert!(is_italic_font("Helvetica-Oblique"));
        assert!(is_italic_font("Times-Italic"));
        assert!(!is_italic_font("Helvetica"));
        assert!(!is_italic_font("Helvetica-Bold"));
    }

    #[test]
    fn test_render_line_with_emphasis() {
        let line = TextLine {
            words: vec![
                make_word_from_text("normal", 0.0, 0.0, 48.0, 12.0, 12.0, "Helvetica"),
                make_word_from_text("bold", 52.0, 0.0, 88.0, 12.0, 12.0, "Helvetica-Bold"),
                make_word_from_text("italic", 92.0, 0.0, 140.0, 12.0, 12.0, "Helvetica-Oblique"),
            ],
            bbox: BBox::new(0.0, 0.0, 140.0, 12.0),
        };
        let result = render_line_with_emphasis(&line);
        assert_eq!(result, "normal <strong>bold</strong> <em>italic</em>");
    }

    #[test]
    fn test_render_bold_italic_combined() {
        let line = TextLine {
            words: vec![make_word_from_text(
                "emphasis",
                0.0,
                0.0,
                64.0,
                12.0,
                12.0,
                "Helvetica-BoldOblique",
            )],
            bbox: BBox::new(0.0, 0.0, 64.0, 12.0),
        };
        let result = render_line_with_emphasis(&line);
        assert_eq!(result, "<strong><em>emphasis</em></strong>");
    }

    // --- HtmlOptions default tests ---

    #[test]
    fn test_html_options_default() {
        let opts = HtmlOptions::default();
        assert_eq!(opts.y_tolerance, 3.0);
        assert_eq!(opts.y_density, 10.0);
        assert_eq!(opts.x_density, 10.0);
        assert_eq!(opts.heading_min_ratio, 1.2);
        assert!(opts.detect_lists);
        assert!(opts.detect_emphasis);
    }

    // --- List detection tests ---

    #[test]
    fn test_detect_bullet_list() {
        let result = detect_list_item("- item text");
        assert_eq!(result, Some((false, "item text".to_string())));
    }

    #[test]
    fn test_detect_numbered_list() {
        let result = detect_list_item("1. first item");
        assert_eq!(result, Some((true, "first item".to_string())));
    }

    #[test]
    fn test_detect_no_list() {
        assert_eq!(detect_list_item("Just normal text"), None);
    }

    // --- Element rendering tests ---

    #[test]
    fn test_render_heading_and_paragraph() {
        let elements = vec![
            HtmlElement::Heading {
                level: 1,
                text: "My Title".to_string(),
            },
            HtmlElement::Paragraph("Some body text.".to_string()),
        ];
        let result = render_elements(&elements);
        assert_eq!(result, "<h1>My Title</h1>\n<p>Some body text.</p>");
    }

    #[test]
    fn test_render_unordered_list() {
        let elements = vec![
            HtmlElement::ListItem {
                ordered: false,
                text: "first".to_string(),
            },
            HtmlElement::ListItem {
                ordered: false,
                text: "second".to_string(),
            },
        ];
        let result = render_elements(&elements);
        assert_eq!(result, "<ul>\n<li>first</li>\n<li>second</li>\n</ul>");
    }

    #[test]
    fn test_render_ordered_list() {
        let elements = vec![
            HtmlElement::ListItem {
                ordered: true,
                text: "first".to_string(),
            },
            HtmlElement::ListItem {
                ordered: true,
                text: "second".to_string(),
            },
        ];
        let result = render_elements(&elements);
        assert_eq!(result, "<ol>\n<li>first</li>\n<li>second</li>\n</ol>");
    }

    #[test]
    fn test_render_with_table() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            cells: vec![],
            rows: vec![
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 0.0, 50.0, 25.0),
                        text: Some("Col1".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 0.0, 100.0, 25.0),
                        text: Some("Col2".to_string()),
                    },
                ],
                vec![
                    Cell {
                        bbox: BBox::new(0.0, 25.0, 50.0, 50.0),
                        text: Some("A".to_string()),
                    },
                    Cell {
                        bbox: BBox::new(50.0, 25.0, 100.0, 50.0),
                        text: Some("B".to_string()),
                    },
                ],
            ],
            columns: vec![],
        };
        let result = HtmlRenderer::render(&[], &[table], &HtmlOptions::default());
        assert!(result.contains("<table>"));
        assert!(result.contains("<th>Col1</th>"));
        assert!(result.contains("<td>A</td>"));
        assert!(result.contains("</table>"));
    }

    #[test]
    fn test_table_single_row() {
        let table = Table {
            bbox: BBox::new(0.0, 0.0, 100.0, 25.0),
            cells: vec![],
            rows: vec![vec![
                Cell {
                    bbox: BBox::new(0.0, 0.0, 50.0, 25.0),
                    text: Some("Only".to_string()),
                },
                Cell {
                    bbox: BBox::new(50.0, 0.0, 100.0, 25.0),
                    text: Some("Row".to_string()),
                },
            ]],
            columns: vec![],
        };
        let html = table_to_html(&table);
        assert!(html.contains("<th>Only</th>"));
        assert!(html.contains("<th>Row</th>"));
        // Single row: thead only, empty tbody
        assert!(html.contains("<tbody>"));
    }

    #[test]
    fn test_median_font_size_empty() {
        assert_eq!(compute_median_font_size(&[]), 12.0);
    }

    #[test]
    fn test_median_font_size_single() {
        let chars = vec![make_char("A", 0.0, 0.0, 10.0, 12.0, 14.0)];
        assert_eq!(compute_median_font_size(&chars), 14.0);
    }

    #[test]
    fn test_block_dominant_size() {
        let block = TextBlock {
            lines: vec![TextLine {
                words: vec![make_word_from_text(
                    "Hello",
                    0.0,
                    0.0,
                    40.0,
                    12.0,
                    14.0,
                    "Helvetica",
                )],
                bbox: BBox::new(0.0, 0.0, 40.0, 12.0),
            }],
            bbox: BBox::new(0.0, 0.0, 40.0, 12.0),
        };
        assert_eq!(block_dominant_size(&block), 14.0);
    }

    // --- End-to-end rendering tests ---

    #[test]
    fn test_render_list_items_as_html() {
        let mut chars = Vec::new();
        for (i, c) in "- first item".chars().enumerate() {
            let x0 = i as f64 * 8.0;
            chars.push(make_char(&c.to_string(), x0, 0.0, x0 + 8.0, 12.0, 12.0));
        }
        for (i, c) in "- second item".chars().enumerate() {
            let x0 = i as f64 * 8.0;
            chars.push(make_char(&c.to_string(), x0, 15.0, x0 + 8.0, 27.0, 12.0));
        }
        let result = HtmlRenderer::render_text(&chars, &HtmlOptions::default());
        assert!(
            result.contains("<ul>"),
            "Expected unordered list, got: {result}"
        );
        assert!(
            result.contains("<li>first item</li>"),
            "Expected first list item, got: {result}"
        );
        assert!(
            result.contains("<li>second item</li>"),
            "Expected second list item, got: {result}"
        );
        assert!(
            result.contains("</ul>"),
            "Expected closing ul tag, got: {result}"
        );
    }

    #[test]
    fn test_heading_html_escapes_content() {
        let elements = vec![HtmlElement::Heading {
            level: 2,
            text: "A &amp; B".to_string(),
        }];
        let result = render_elements(&elements);
        assert_eq!(result, "<h2>A &amp; B</h2>");
    }

    #[test]
    fn test_paragraph_html_wrapping() {
        let elements = vec![HtmlElement::Paragraph("Hello world".to_string())];
        let result = render_elements(&elements);
        assert_eq!(result, "<p>Hello world</p>");
    }
}
