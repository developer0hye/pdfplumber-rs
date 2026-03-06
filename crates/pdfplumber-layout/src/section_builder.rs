//! Section segmentation: groups classified blocks into [`Section`]s delimited
//! by heading blocks.

use crate::block_classifier::{BlockClassification, BlockKind, block_text};
use pdfplumber_core::{BBox, Table, TextBlock};

/// A paragraph of body text.
#[derive(Debug, Clone)]
pub struct Paragraph {
    /// The paragraph text.
    text: String,
    /// Page index (0-based) where this paragraph starts.
    pub page: usize,
    /// Bounding box of the paragraph on its page.
    pub bbox: BBox,
    /// True if this paragraph is a list item.
    pub is_list_item: bool,
    /// True if this paragraph is a caption.
    pub is_caption: bool,
}

impl Paragraph {
    /// The paragraph text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A heading block — the title of a section.
#[derive(Debug, Clone)]
pub struct Heading {
    text: String,
    /// Page index where this heading appears.
    pub page: usize,
    /// Bounding box of the heading.
    pub bbox: BBox,
    /// Font size of the heading.
    pub font_size: f64,
    /// True if the heading font is bold.
    pub is_bold: bool,
}

impl Heading {
    /// The heading text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A table found within a section.
#[derive(Debug, Clone)]
pub struct SectionTable {
    /// The extracted table data.
    pub table: Table,
    /// Page where the table appears.
    pub page: usize,
    /// Bounding box of the table.
    pub bbox: BBox,
}

/// A contiguous section of document content delimited by a heading.
///
/// The first section in a document may have no heading (preamble content
/// before any heading block).
#[derive(Debug, Clone)]
pub struct Section {
    heading: Option<Heading>,
    paragraphs: Vec<Paragraph>,
    tables: Vec<SectionTable>,
    /// Page index where this section starts.
    pub start_page: usize,
    /// Page index where this section ends (inclusive).
    pub end_page: usize,
}

impl Section {
    /// The section heading, if one was detected.
    pub fn heading(&self) -> Option<&Heading> {
        self.heading.as_ref()
    }

    /// All body paragraphs in this section (in document order).
    pub fn paragraphs(&self) -> &[Paragraph] {
        &self.paragraphs
    }

    /// All tables in this section (in document order).
    pub fn tables(&self) -> &[SectionTable] {
        &self.tables
    }

    /// Flatten all paragraphs in this section into a single string.
    pub fn text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// True if this section has no heading (preamble or unstructured content).
    pub fn is_preamble(&self) -> bool {
        self.heading.is_none()
    }
}

/// Build sections from classified blocks and extracted tables.
///
/// Algorithm:
/// 1. Walk blocks in order (page order, top-to-bottom within each page).
/// 2. Each heading block starts a new section.
/// 3. Tables are attributed to the section that spatially contains them
///    (or the nearest preceding heading section).
pub fn build_sections(
    classified: &[BlockClassification],
    raw_blocks: &[(usize, TextBlock)],
    all_tables: &[(usize, Table, BBox)],
) -> Vec<Section> {
    if classified.is_empty() {
        return Vec::new();
    }

    // --- Build initial section list from heading segmentation ---
    let mut sections: Vec<Section> = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut current_paras: Vec<Paragraph> = Vec::new();
    let mut current_start_page = classified[0].page;
    let mut current_end_page = classified[0].page;

    for cls in classified {
        current_end_page = current_end_page.max(cls.page);

        match cls.kind {
            BlockKind::Heading => {
                // Flush accumulated paragraphs into a section
                if !current_paras.is_empty() || current_heading.is_some() {
                    sections.push(Section {
                        heading: current_heading.take(),
                        paragraphs: std::mem::take(&mut current_paras),
                        tables: Vec::new(),
                        start_page: current_start_page,
                        end_page: cls.page.saturating_sub(1),
                    });
                    current_start_page = cls.page;
                }
                current_heading = Some(Heading {
                    text: cls.text.clone(),
                    page: cls.page,
                    bbox: cls.bbox,
                    font_size: cls.font_size,
                    is_bold: cls.is_bold,
                });
            }
            BlockKind::Paragraph => {
                current_paras.push(Paragraph {
                    text: cls.text.clone(),
                    page: cls.page,
                    bbox: cls.bbox,
                    is_list_item: false,
                    is_caption: false,
                });
            }
            BlockKind::ListItem => {
                current_paras.push(Paragraph {
                    text: cls.text.clone(),
                    page: cls.page,
                    bbox: cls.bbox,
                    is_list_item: true,
                    is_caption: false,
                });
            }
            BlockKind::Caption => {
                current_paras.push(Paragraph {
                    text: cls.text.clone(),
                    page: cls.page,
                    bbox: cls.bbox,
                    is_list_item: false,
                    is_caption: true,
                });
            }
            BlockKind::Other => {
                // Skip headers, footers, page numbers etc.
            }
        }
    }

    // Flush final section
    if !current_paras.is_empty() || current_heading.is_some() {
        sections.push(Section {
            heading: current_heading,
            paragraphs: current_paras,
            tables: Vec::new(),
            start_page: current_start_page,
            end_page: current_end_page,
        });
    }

    // --- Attribute tables to sections ---
    // For each table, find the section whose page range covers the table's page.
    // If multiple sections cover the same page, attribute to the last one whose
    // heading appears before (above) the table's bbox.top.
    for (table_page, table, table_bbox) in all_tables {
        let section_idx = best_section_for_table(*table_page, table_bbox, &sections);
        if let Some(idx) = section_idx {
            sections[idx].tables.push(SectionTable {
                table: table.clone(),
                page: *table_page,
                bbox: *table_bbox,
            });
        }
    }

    sections
}

/// Find the best section index to attribute a table on `page` at `bbox`.
fn best_section_for_table(page: usize, bbox: &BBox, sections: &[Section]) -> Option<usize> {
    // Find all sections that cover this page
    let candidates: Vec<usize> = sections
        .iter()
        .enumerate()
        .filter(|(_, s)| s.start_page <= page && s.end_page >= page)
        .map(|(i, _)| i)
        .collect();

    if candidates.is_empty() {
        // Fallback: last section
        return if sections.is_empty() {
            None
        } else {
            Some(sections.len() - 1)
        };
    }

    if candidates.len() == 1 {
        return Some(candidates[0]);
    }

    // Multiple sections share this page: prefer the one whose heading is on the
    // same page above the table's bbox.top, or the last one before the table.
    let mut best = candidates[0];
    for &idx in &candidates {
        let section = &sections[idx];
        if let Some(h) = &section.heading {
            if h.page == page && h.bbox.bottom <= bbox.top {
                best = idx;
            }
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_classifier::{BlockClassification, BlockKind};
    use pdfplumber_core::BBox;

    fn dummy_bbox() -> BBox {
        BBox::new(72.0, 100.0, 540.0, 120.0)
    }

    fn make_cls(kind: BlockKind, text: &str, page: usize) -> BlockClassification {
        BlockClassification {
            kind,
            page,
            font_size: 12.0,
            is_bold: false,
            is_italic: false,
            line_count: 1,
            left_margin: 72.0,
            bbox: dummy_bbox(),
            text: text.to_string(),
        }
    }

    #[test]
    fn empty_classified_returns_empty_sections() {
        let result = build_sections(&[], &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn single_paragraph_with_no_heading_is_preamble() {
        let cls = vec![make_cls(
            BlockKind::Paragraph,
            "Preamble text here.",
            0,
        )];
        let result = build_sections(&cls, &[], &[]);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_preamble());
        assert_eq!(result[0].paragraphs().len(), 1);
    }

    #[test]
    fn heading_starts_new_section() {
        let cls = vec![
            make_cls(BlockKind::Heading, "Introduction", 0),
            make_cls(BlockKind::Paragraph, "Body text here.", 0),
        ];
        let result = build_sections(&cls, &[], &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].heading().unwrap().text(), "Introduction");
        assert_eq!(result[0].paragraphs().len(), 1);
    }

    #[test]
    fn two_headings_produce_two_sections() {
        let cls = vec![
            make_cls(BlockKind::Heading, "Chapter 1", 0),
            make_cls(BlockKind::Paragraph, "First chapter content.", 0),
            make_cls(BlockKind::Heading, "Chapter 2", 1),
            make_cls(BlockKind::Paragraph, "Second chapter content.", 1),
        ];
        let result = build_sections(&cls, &[], &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].heading().unwrap().text(), "Chapter 1");
        assert_eq!(result[1].heading().unwrap().text(), "Chapter 2");
    }

    #[test]
    fn preamble_then_sections() {
        let cls = vec![
            make_cls(BlockKind::Paragraph, "Abstract text before any heading.", 0),
            make_cls(BlockKind::Heading, "1. Introduction", 1),
            make_cls(BlockKind::Paragraph, "Intro body.", 1),
        ];
        let result = build_sections(&cls, &[], &[]);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_preamble());
        assert_eq!(result[1].heading().unwrap().text(), "1. Introduction");
    }

    #[test]
    fn list_items_go_into_paragraphs_as_list_items() {
        let cls = vec![
            make_cls(BlockKind::Heading, "Features", 0),
            make_cls(BlockKind::ListItem, "• Fast", 0),
            make_cls(BlockKind::ListItem, "• Correct", 0),
        ];
        let result = build_sections(&cls, &[], &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].paragraphs().len(), 2);
        assert!(result[0].paragraphs()[0].is_list_item);
    }

    #[test]
    fn section_text_joins_paragraphs() {
        let cls = vec![
            make_cls(BlockKind::Heading, "Section", 0),
            make_cls(BlockKind::Paragraph, "First para.", 0),
            make_cls(BlockKind::Paragraph, "Second para.", 0),
        ];
        let result = build_sections(&cls, &[], &[]);
        let text = result[0].text();
        assert!(text.contains("First para."));
        assert!(text.contains("Second para."));
    }
}
