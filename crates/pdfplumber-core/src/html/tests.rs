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
            mcid: None,
            tag: None,
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
                    mcid: None,
                    tag: None,
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
