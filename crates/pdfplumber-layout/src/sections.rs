//! Section grouping: partitions layout blocks into sections keyed by headings.

use crate::{Figure, Heading, LayoutBlock, LayoutTable, Paragraph};
use pdfplumber_core::BBox;

/// A document section: one heading (optional for the preamble) followed by
/// paragraphs, tables, and figures until the next heading of equal or higher level.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Section {
    /// The heading that opens this section, or `None` for the preamble
    /// (content before the first heading on the first page).
    pub heading: Option<Heading>,
    /// All content blocks in this section in reading order.
    pub blocks: Vec<LayoutBlock>,
    /// Bounding box spanning all blocks in this section (union).
    pub bbox: Option<BBox>,
    /// Page number where the section starts (0-based).
    pub start_page: usize,
}

impl Section {
    /// Return the section heading, if any.
    pub fn heading(&self) -> Option<&Heading> {
        self.heading.as_ref()
    }

    /// Return all paragraphs in this section.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Paragraph(p) = b {
                Some(p)
            } else {
                None
            }
        })
    }

    /// Return all tables in this section.
    pub fn tables(&self) -> impl Iterator<Item = &LayoutTable> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Table(t) = b {
                Some(t)
            } else {
                None
            }
        })
    }

    /// Return all figures in this section.
    pub fn figures(&self) -> impl Iterator<Item = &Figure> {
        self.blocks.iter().filter_map(|b| {
            if let LayoutBlock::Figure(f) = b {
                Some(f)
            } else {
                None
            }
        })
    }

    /// Return all text (headings + paragraphs) in this section concatenated.
    pub fn text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if let Some(h) = &self.heading {
            parts.push(h.text.as_str());
        }
        for b in &self.blocks {
            if let LayoutBlock::Paragraph(p) = b {
                parts.push(p.text.as_str());
            }
        }
        parts.join("\n\n")
    }

    /// Count of non-heading blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Partition a flat sequence of layout blocks into sections.
///
/// Every heading block starts a new section. Blocks before the first heading
/// belong to the preamble section (heading = None). Nested headings (e.g., H3
/// inside an H2 section) are NOT nested in this model — sections are flat,
/// matching the WINTERSTRATEN spec of `Section` → `Heading + Paragraph + Table + Figure`.
pub fn partition_into_sections(blocks: Vec<LayoutBlock>) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut current_blocks: Vec<LayoutBlock> = Vec::new();
    let mut current_start_page: usize = 0;

    for block in blocks {
        if let LayoutBlock::Heading(h) = block {
            // Flush current section
            let bbox = compute_section_bbox(current_heading.as_ref(), &current_blocks);
            sections.push(Section {
                start_page: current_start_page,
                heading: current_heading,
                blocks: current_blocks,
                bbox,
            });
            current_start_page = h.page_number;
            current_heading = Some(h);
            current_blocks = Vec::new();
        } else {
            if current_blocks.is_empty() && current_heading.is_none() {
                current_start_page = block.page_number();
            }
            current_blocks.push(block);
        }
    }

    // Flush final section
    let bbox = compute_section_bbox(current_heading.as_ref(), &current_blocks);
    sections.push(Section {
        start_page: current_start_page,
        heading: current_heading,
        blocks: current_blocks,
        bbox,
    });

    // Remove empty preamble if there's nothing in it and no heading
    sections.retain(|s| s.heading.is_some() || !s.blocks.is_empty());

    sections
}

/// Compute the union bbox of a section from its heading and blocks.
fn compute_section_bbox(heading: Option<&Heading>, blocks: &[LayoutBlock]) -> Option<BBox> {
    let mut bbox: Option<BBox> = heading.map(|h| h.bbox);
    for block in blocks {
        let bb = block.bbox();
        bbox = Some(match bbox {
            None => bb,
            Some(existing) => existing.union(&bb),
        });
    }
    bbox
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headings::HeadingLevel;

    fn make_heading(text: &str, level: HeadingLevel, page: usize, top: f64) -> LayoutBlock {
        LayoutBlock::Heading(Heading {
            text: text.to_string(),
            bbox: BBox::new(72.0, top, 400.0, top + 20.0),
            page_number: page,
            level,
            font_size: 18.0,
            fontname: "Helvetica-Bold".to_string(),
        })
    }

    fn make_para(text: &str, page: usize, top: f64) -> LayoutBlock {
        LayoutBlock::Paragraph(Paragraph {
            text: text.to_string(),
            bbox: BBox::new(72.0, top, 500.0, top + 40.0),
            page_number: page,
            line_count: 2,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption: false,
            is_list_item: false,
        })
    }

    #[test]
    fn partition_empty() {
        let sections = partition_into_sections(vec![]);
        assert!(sections.is_empty());
    }

    #[test]
    fn partition_only_paragraphs_is_preamble() {
        let blocks = vec![
            make_para("First paragraph.", 0, 100.0),
            make_para("Second paragraph.", 0, 150.0),
        ];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].heading.is_none());
        assert_eq!(sections[0].blocks.len(), 2);
    }

    #[test]
    fn partition_heading_then_paragraphs() {
        let blocks = vec![
            make_heading("Introduction", HeadingLevel::H1, 0, 50.0),
            make_para("First paragraph.", 0, 80.0),
            make_para("Second paragraph.", 0, 130.0),
        ];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading.as_ref().unwrap().text, "Introduction");
        assert_eq!(sections[0].blocks.len(), 2);
    }

    #[test]
    fn partition_multiple_sections() {
        let blocks = vec![
            make_heading("Section 1", HeadingLevel::H1, 0, 50.0),
            make_para("Para 1a.", 0, 80.0),
            make_heading("Section 2", HeadingLevel::H1, 0, 300.0),
            make_para("Para 2a.", 0, 330.0),
            make_para("Para 2b.", 0, 380.0),
        ];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading.as_ref().unwrap().text, "Section 1");
        assert_eq!(sections[0].blocks.len(), 1);
        assert_eq!(sections[1].heading.as_ref().unwrap().text, "Section 2");
        assert_eq!(sections[1].blocks.len(), 2);
    }

    #[test]
    fn partition_preamble_then_sections() {
        let blocks = vec![
            make_para("Preamble text.", 0, 30.0),
            make_heading("Main Section", HeadingLevel::H1, 0, 80.0),
            make_para("Body text.", 0, 110.0),
        ];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections.len(), 2);
        assert!(sections[0].heading.is_none()); // preamble
        assert_eq!(sections[1].heading.as_ref().unwrap().text, "Main Section");
    }

    #[test]
    fn section_text_concatenation() {
        let blocks = vec![
            make_heading("My Section", HeadingLevel::H2, 0, 50.0),
            make_para("First paragraph.", 0, 80.0),
            make_para("Second paragraph.", 0, 130.0),
        ];
        let sections = partition_into_sections(blocks);
        let text = sections[0].text();
        assert!(text.contains("My Section"));
        assert!(text.contains("First paragraph."));
        assert!(text.contains("Second paragraph."));
    }

    #[test]
    fn section_accessors() {
        let blocks = vec![
            make_heading("Title", HeadingLevel::H1, 0, 50.0),
            make_para("Some text.", 0, 80.0),
        ];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections[0].paragraphs().count(), 1);
        assert_eq!(sections[0].tables().count(), 0);
        assert_eq!(sections[0].figures().count(), 0);
    }

    #[test]
    fn section_start_page_from_heading() {
        let blocks = vec![make_heading("Chapter 2", HeadingLevel::H1, 5, 50.0)];
        let sections = partition_into_sections(blocks);
        assert_eq!(sections[0].start_page, 5);
    }

    #[test]
    fn section_bbox_is_union() {
        let blocks = vec![
            make_heading("Title", HeadingLevel::H1, 0, 50.0),
            make_para("Text.", 0, 100.0),
        ];
        let sections = partition_into_sections(blocks);
        let bbox = sections[0].bbox.unwrap();
        // Should span from heading top (50) to para bottom (140)
        assert!(bbox.top <= 50.0 + 1.0);
        assert!(bbox.bottom >= 140.0 - 1.0);
    }
}
