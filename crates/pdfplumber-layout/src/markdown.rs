//! Markdown rendering for [`Document`] and individual layout blocks.
//!
//! Converts the semantic layout tree to GitHub-Flavored Markdown (GFM).
//! Tables use the GFM pipe syntax. Figures become image placeholders.
//! Headings use ATX style (`#` through `####`).
//!
//! This is the primary output format for LLM context building, RAG indexing,
//! and human-readable document export.

use crate::figures::Figure;
use crate::headings::HeadingLevel;
use crate::{Heading, LayoutBlock, LayoutTable, Paragraph, Section};

/// Render a [`Heading`] to an ATX markdown heading line.
pub fn heading_to_markdown(h: &Heading) -> String {
    let pounds = match h.level {
        HeadingLevel::H1 => "#",
        HeadingLevel::H2 => "##",
        HeadingLevel::H3 => "###",
        HeadingLevel::H4 => "####",
    };
    format!("{pounds} {}", h.text.trim())
}

/// Render a [`Paragraph`] to a markdown paragraph.
///
/// - Captions are wrapped in `*italic*` to visually distinguish them.
/// - List items are preserved as-is (the text already contains the bullet/ordinal prefix
///   from the PDF, which is valid markdown for unordered/ordered lists).
/// - Regular body text is emitted verbatim (trimmed).
pub fn paragraph_to_markdown(p: &Paragraph) -> String {
    let text = p.text.trim();
    if p.is_caption {
        format!("*{text}*")
    } else if p.is_list_item {
        // Text already starts with the bullet/ordinal from the PDF.
        // Normalise the prefix to a standard GFM form.
        use crate::lists::{ListKind, parse_list_prefix};
        if let Some((_, rest, kind)) = parse_list_prefix(text) {
            match kind {
                ListKind::Unordered => format!("- {rest}"),
                ListKind::Ordered => {
                    // Extract the ordinal prefix verbatim from text for GFM ordered list.
                    let prefix_end = text.len() - rest.len();
                    let raw_prefix = text[..prefix_end].trim_end();
                    // GFM ordered lists need `N. ` format; re-normalise.
                    format!("{raw_prefix} {rest}")
                }
            }
        } else {
            text.to_string()
        }
    } else {
        text.to_string()
    }
}

/// Render a [`LayoutTable`] to a GFM pipe table.
///
/// The first row is treated as the header row. If the table has no rows,
/// returns an empty string. Cells with `None` content render as empty.
pub fn table_to_markdown(t: &LayoutTable) -> String {
    if t.cells.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let cols = t.cols.max(1);

    for (row_idx, row) in t.cells.iter().enumerate() {
        // Pad or truncate row to `cols` cells
        let cells: Vec<String> = (0..cols)
            .map(|ci| {
                row.get(ci)
                    .and_then(|c| c.as_deref())
                    .unwrap_or("")
                    .replace('|', "\\|")
                    .replace('\n', " ")
                    .trim()
                    .to_string()
            })
            .collect();

        out.push_str("| ");
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");

        // After header row, emit separator
        if row_idx == 0 {
            out.push_str("| ");
            out.push_str(&vec!["---"; cols].join(" | "));
            out.push_str(" |\n");
        }
    }
    out
}

/// Render a [`Figure`] to a markdown image placeholder.
///
/// Since we don't have a URI for the figure content, this produces a
/// descriptive placeholder that downstream tools can key on.
pub fn figure_to_markdown(f: &Figure) -> String {
    format!(
        "![Figure (p.{}; {:.0},{:.0}–{:.0},{:.0})](figure)",
        f.page_number + 1,
        f.bbox.x0,
        f.bbox.top,
        f.bbox.x1,
        f.bbox.bottom,
    )
}

/// Render a [`LayoutBlock`] to markdown.
pub fn block_to_markdown(block: &LayoutBlock) -> String {
    match block {
        LayoutBlock::Heading(h) => heading_to_markdown(h),
        LayoutBlock::Paragraph(p) => paragraph_to_markdown(p),
        LayoutBlock::Table(t) => table_to_markdown(t),
        LayoutBlock::Figure(f) => figure_to_markdown(f),
    }
}

/// Render a [`Section`] to markdown.
///
/// The section heading (if any) is rendered first, then all blocks.
pub fn section_to_markdown(section: &Section) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(h) = section.heading() {
        parts.push(heading_to_markdown(h));
    }
    for block in &section.blocks {
        let md = block_to_markdown(block);
        if !md.trim().is_empty() {
            parts.push(md);
        }
    }
    parts.join("\n\n")
}

/// Render all sections to a full markdown document.
pub fn sections_to_markdown(sections: &[Section]) -> String {
    sections
        .iter()
        .map(section_to_markdown)
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headings::HeadingLevel;
    use pdfplumber_core::BBox;

    fn make_heading(text: &str, level: HeadingLevel) -> Heading {
        Heading {
            text: text.to_string(),
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            page_number: 0,
            level,
            font_size: 18.0,
            fontname: "Helvetica-Bold".to_string(),
        }
    }

    fn make_para(text: &str, is_caption: bool) -> Paragraph {
        Paragraph {
            text: text.to_string(),
            bbox: BBox::new(0.0, 0.0, 100.0, 40.0),
            page_number: 0,
            line_count: 1,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption,
            is_list_item: false,
        }
    }

    #[test]
    fn h1_renders_as_single_pound() {
        let h = make_heading("Introduction", HeadingLevel::H1);
        assert_eq!(heading_to_markdown(&h), "# Introduction");
    }

    #[test]
    fn h4_renders_as_four_pounds() {
        let h = make_heading("Sub-subsection", HeadingLevel::H4);
        assert_eq!(heading_to_markdown(&h), "#### Sub-subsection");
    }

    #[test]
    fn paragraph_plain_text() {
        let p = make_para("Hello world.", false);
        assert_eq!(paragraph_to_markdown(&p), "Hello world.");
    }

    #[test]
    fn caption_is_italicised() {
        let p = make_para("Figure 1. A chart.", true);
        assert_eq!(paragraph_to_markdown(&p), "*Figure 1. A chart.*");
    }

    #[test]
    fn bullet_list_item_renders_gfm() {
        let p = Paragraph {
            text: "• First item".to_string(),
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            page_number: 0,
            line_count: 1,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption: false,
            is_list_item: true,
        };
        assert_eq!(paragraph_to_markdown(&p), "- First item");
    }

    #[test]
    fn ordered_list_item_renders_gfm() {
        let p = Paragraph {
            text: "1. First step".to_string(),
            bbox: BBox::new(0.0, 0.0, 100.0, 20.0),
            page_number: 0,
            line_count: 1,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption: false,
            is_list_item: true,
        };
        let md = paragraph_to_markdown(&p);
        assert!(md.contains("First step"), "should contain item text");
    }

    #[test]
    fn empty_table_renders_empty() {
        let t = LayoutTable {
            bbox: BBox::new(0.0, 0.0, 100.0, 50.0),
            page_number: 0,
            rows: 0,
            cols: 0,
            cells: vec![],
        };
        assert_eq!(table_to_markdown(&t), "");
    }

    #[test]
    fn two_row_table_has_separator() {
        let t = LayoutTable {
            bbox: BBox::new(0.0, 0.0, 200.0, 100.0),
            page_number: 0,
            rows: 2,
            cols: 2,
            cells: vec![
                vec![Some("Name".to_string()), Some("Value".to_string())],
                vec![Some("Alpha".to_string()), Some("42".to_string())],
            ],
        };
        let md = table_to_markdown(&t);
        assert!(md.contains("| Name | Value |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alpha | 42 |"));
    }

    #[test]
    fn table_pipe_in_cell_is_escaped() {
        let t = LayoutTable {
            bbox: BBox::new(0.0, 0.0, 200.0, 50.0),
            page_number: 0,
            rows: 1,
            cols: 1,
            cells: vec![vec![Some("A | B".to_string())]],
        };
        let md = table_to_markdown(&t);
        assert!(md.contains("A \\| B"));
    }

    #[test]
    fn figure_renders_placeholder() {
        use crate::figures::{Figure, FigureKind};
        let f = Figure {
            bbox: BBox::new(72.0, 100.0, 400.0, 350.0),
            page_number: 2,
            kind: FigureKind::Image,
        };
        let md = figure_to_markdown(&f);
        assert!(md.starts_with("![Figure"));
        assert!(md.contains("p.3"));
    }

    #[test]
    fn sections_to_markdown_joins_with_hr() {
        use crate::sections::Section;
        let sections = vec![
            Section {
                heading: Some(make_heading("Sec 1", HeadingLevel::H1)),
                blocks: vec![LayoutBlock::Paragraph(make_para("Body 1.", false))],
                bbox: Some(BBox::new(0.0, 0.0, 100.0, 100.0)),
                start_page: 0,
            },
            Section {
                heading: Some(make_heading("Sec 2", HeadingLevel::H1)),
                blocks: vec![],
                bbox: Some(BBox::new(0.0, 100.0, 100.0, 200.0)),
                start_page: 0,
            },
        ];
        let md = sections_to_markdown(&sections);
        assert!(md.contains("---"));
        assert!(md.contains("# Sec 1"));
        assert!(md.contains("# Sec 2"));
    }
}
