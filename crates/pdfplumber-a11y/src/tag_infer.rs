//! Structure tag inference for untagged PDFs.
//!
//! When a PDF has no structure tree, this module attempts to infer one from
//! geometric and typographic signals — the same signals used by `pdfplumber-layout`.
//!
//! The output is a `Vec<InferredTag>` that can be used to:
//! 1. Generate a PDF/UA-compliant structure tree (written back via Lane 10's
//!    incremental update API).
//! 2. Feed into the [`A11yReport`](crate::A11yReport) to assess what a
//!    tagged version would look like.
//!
//! # Algorithm
//!
//! For each page, in reading order (top-to-bottom, left-to-right):
//!
//! 1. Group chars into text blocks using vertical gap analysis.
//! 2. Classify each block:
//!    - Font size ≥ 1.4× median → `H1`
//!    - Font size ≥ 1.2× median → `H2`  
//!    - Font size ≥ 1.1× median → `H3`
//!    - Otherwise → `P`
//! 3. Images → `Figure` (needs manual alt text — flagged as TODO)
//! 4. Detected tables → `Table` with inferred `TR`/`TH`/`TD` children
//! 5. Page header/footer regions (top/bottom 8% of page height) → `Artifact`

use pdfplumber::Page;
use pdfplumber_core::{BBox, TableSettings};

/// A single inferred structure tag for a region of a page.
#[derive(Debug, Clone)]
pub struct InferredTag {
    /// PDF structure type name (e.g. "H1", "P", "Figure", "Table").
    pub role: String,
    /// Bounding box of the tagged region on the page.
    pub bbox: BBox,
    /// Page number (0-based).
    pub page: usize,
    /// Text content (for text elements). Empty for Figure/Artifact.
    pub text: String,
    /// Child tags (for Table → TR → TD nesting).
    pub children: Vec<InferredTag>,
    /// Whether this tag needs manual review (e.g., Figure needs alt text).
    pub needs_review: bool,
    /// Reason for manual review flag, if any.
    pub review_reason: Option<String>,
}

impl InferredTag {
    fn text_tag(role: &str, bbox: BBox, page: usize, text: String) -> Self {
        Self {
            role: role.to_owned(),
            bbox,
            page,
            text,
            children: vec![],
            needs_review: false,
            review_reason: None,
        }
    }

    fn figure(bbox: BBox, page: usize) -> Self {
        Self {
            role: "Figure".to_owned(),
            bbox,
            page,
            text: String::new(),
            children: vec![],
            needs_review: true,
            review_reason: Some("Figure requires /Alt text — describe the image content".to_owned()),
        }
    }

    fn artifact(bbox: BBox, page: usize, reason: &str) -> Self {
        Self {
            role: "Artifact".to_owned(),
            bbox,
            page,
            text: String::new(),
            children: vec![],
            needs_review: false,
            review_reason: Some(reason.to_owned()),
        }
    }
}

/// Infers structure tags for an untagged PDF.
pub struct TagInferrer {
    /// Fraction of page height treated as header/footer artifact zone.
    /// Default: 0.08 (top 8% and bottom 8%).
    pub artifact_zone: f64,
    /// Font size ratio for H1 detection (default: 1.4).
    pub h1_ratio: f64,
    /// Font size ratio for H2 detection (default: 1.2).
    pub h2_ratio: f64,
    /// Font size ratio for H3 detection (default: 1.1).
    pub h3_ratio: f64,
    /// Minimum vertical gap (in points) to split text blocks (default: 4.0).
    pub block_gap: f64,
}

impl Default for TagInferrer {
    fn default() -> Self {
        Self {
            artifact_zone: 0.08,
            h1_ratio: 1.4,
            h2_ratio: 1.2,
            h3_ratio: 1.1,
            block_gap: 4.0,
        }
    }
}

impl TagInferrer {
    /// Create a new [`TagInferrer`] with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Infer structure tags for a single page.
    pub fn infer_page(&self, page: &Page, page_idx: usize) -> Vec<InferredTag> {
        let mut tags = Vec::new();
        let page_height = page.height();
        let artifact_top = page_height * self.artifact_zone;
        let artifact_bottom = page_height * (1.0 - self.artifact_zone);

        // Compute median font size for heading detection
        let median_size = median_font_size(page);

        // Figures first (always behind text in painter model)
        for img in page.images() {
            tags.push(InferredTag::figure(img.bbox(), page_idx));
        }

        // Tables (detected via pdfplumber's table algorithm)
        let settings = TableSettings::default();
        for table in page.find_tables(&settings) {
            let tag = infer_table_tag(&table, page_idx);
            tags.push(tag);
        }

        // Text blocks: group chars by vertical proximity
        let chars = page.chars();
        if chars.is_empty() {
            return tags;
        }

        let blocks = group_chars_into_blocks(chars, self.block_gap);
        for block in blocks {
            let bbox = block_bbox(&block);
            let text: String = block.iter().map(|c| c.text.as_str()).collect();
            let text = text.trim().to_owned();
            if text.is_empty() {
                continue;
            }

            // Artifact zone detection (header/footer)
            let mid_y = (bbox.top + bbox.bottom) / 2.0;
            if mid_y < artifact_top || mid_y > artifact_bottom {
                tags.push(InferredTag::artifact(bbox, page_idx, "Possible header/footer region"));
                continue;
            }

            // Heading classification
            let block_size = block_median_size(&block);
            let role = if block_size >= median_size * self.h1_ratio {
                "H1"
            } else if block_size >= median_size * self.h2_ratio {
                "H2"
            } else if block_size >= median_size * self.h3_ratio {
                "H3"
            } else {
                "P"
            };

            tags.push(InferredTag::text_tag(role, bbox, page_idx, text));
        }

        // Sort by reading order: top-to-bottom, then left-to-right
        tags.sort_by(|a, b| {
            a.bbox.top.partial_cmp(&b.bbox.top).unwrap_or(std::cmp::Ordering::Equal)
                .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(std::cmp::Ordering::Equal))
        });

        tags
    }

    /// Infer structure tags for all pages of a document.
    pub fn infer_document(&self, pdf: &pdfplumber::Pdf) -> Vec<InferredTag> {
        let mut tags = Vec::new();
        for page_result in pdf.pages_iter() {
            let Ok(page) = page_result else { continue };
            let idx = page.page_number();
            tags.extend(self.infer_page(&page, idx));
        }
        tags
    }

    /// Count how many inferred tags need manual review.
    pub fn review_count(&self, tags: &[InferredTag]) -> usize {
        fn count_recursive(tags: &[InferredTag]) -> usize {
            tags.iter().map(|t| {
                (if t.needs_review { 1 } else { 0 }) + count_recursive(&t.children)
            }).sum()
        }
        count_recursive(tags)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn median_font_size(page: &Page) -> f64 {
    let mut sizes: Vec<f64> = page.chars().iter()
        .filter(|c| c.size > 0.0)
        .map(|c| c.size)
        .collect();
    if sizes.is_empty() {
        return 12.0; // fallback
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sizes.len() / 2;
    if sizes.len() % 2 == 0 {
        (sizes[mid - 1] + sizes[mid]) / 2.0
    } else {
        sizes[mid]
    }
}

fn block_median_size(chars: &[&pdfplumber_core::Char]) -> f64 {
    let mut sizes: Vec<f64> = chars.iter()
        .filter(|c| c.size > 0.0)
        .map(|c| c.size)
        .collect();
    if sizes.is_empty() {
        return 12.0;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sizes[sizes.len() / 2]
}

fn group_chars_into_blocks<'a>(
    chars: &'a [pdfplumber_core::Char],
    gap: f64,
) -> Vec<Vec<&'a pdfplumber_core::Char>> {
    if chars.is_empty() {
        return vec![];
    }
    // Sort chars by doctop (y position in document space) then x0
    let mut sorted: Vec<&pdfplumber_core::Char> = chars.iter().collect();
    sorted.sort_by(|a, b| {
        a.doctop.partial_cmp(&b.doctop).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut blocks: Vec<Vec<&pdfplumber_core::Char>> = Vec::new();
    let mut current: Vec<&pdfplumber_core::Char> = vec![sorted[0]];
    let mut prev_bottom = sorted[0].bbox.bottom;

    for ch in &sorted[1..] {
        let vertical_gap = ch.bbox.top - prev_bottom;
        if vertical_gap > gap {
            if !current.is_empty() {
                blocks.push(current.clone());
                current.clear();
            }
        }
        current.push(ch);
        prev_bottom = ch.bbox.bottom.max(prev_bottom);
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn block_bbox(chars: &[&pdfplumber_core::Char]) -> BBox {
    let x0 = chars.iter().map(|c| c.bbox.x0).fold(f64::MAX, f64::min);
    let x1 = chars.iter().map(|c| c.bbox.x1).fold(f64::MIN, f64::max);
    let top = chars.iter().map(|c| c.bbox.top).fold(f64::MAX, f64::min);
    let bottom = chars.iter().map(|c| c.bbox.bottom).fold(f64::MIN, f64::max);
    BBox { x0, top, x1, bottom }
}

fn infer_table_tag(table: &pdfplumber_core::Table, page_idx: usize) -> InferredTag {
    let mut children = Vec::new();
    for (row_idx, row) in table.rows.iter().enumerate() {
        let mut row_children = Vec::new();
        for cell in row.iter() {
            let cell_role = if row_idx == 0 { "TH" } else { "TD" };
            let cell_text = cell.text.clone().unwrap_or_default();
            row_children.push(InferredTag {
                role: cell_role.to_owned(),
                bbox: cell.bbox,
                page: page_idx,
                text: cell_text,
                children: vec![],
                needs_review: row_idx == 0, // header row needs scope attribute review
                review_reason: if row_idx == 0 {
                    Some("TH cells should have /Scope (Column/Row/Both) attribute".to_owned())
                } else {
                    None
                },
            });
        }
        // Row bbox = union of cell bboxes
        let row_bbox = if row_children.is_empty() {
            table.bbox
        } else {
            BBox {
                x0: row_children.iter().map(|c| c.bbox.x0).fold(f64::MAX, f64::min),
                x1: row_children.iter().map(|c| c.bbox.x1).fold(f64::MIN, f64::max),
                top: row_children.iter().map(|c| c.bbox.top).fold(f64::MAX, f64::min),
                bottom: row_children.iter().map(|c| c.bbox.bottom).fold(f64::MIN, f64::max),
            }
        };
        children.push(InferredTag {
            role: "TR".to_owned(),
            bbox: row_bbox,
            page: page_idx,
            text: String::new(),
            children: row_children,
            needs_review: false,
            review_reason: None,
        });
    }

    InferredTag {
        role: "Table".to_owned(),
        bbox: table.bbox,
        page: page_idx,
        text: String::new(),
        children,
        needs_review: false,
        review_reason: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Char, Color, TextDirection};

    fn make_char(text: &str, x0: f64, top: f64, x1: f64, bottom: f64, size: f64) -> Char {
        Char {
            text: text.to_owned(),
            bbox: BBox { x0, top, x1, bottom },
            fontname: "Helvetica".to_owned(),
            size,
            doctop: top,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: Some(Color::Gray(0.0)),
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: text.chars().next().unwrap_or('?') as u32,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn median_font_size_single() {
        let page = pdfplumber::Page::new(0, 200.0, 200.0, vec![
            make_char("A", 0.0, 0.0, 10.0, 12.0, 12.0),
            make_char("B", 10.0, 0.0, 20.0, 12.0, 12.0),
        ]);
        assert!((median_font_size(&page) - 12.0).abs() < 0.01);
    }

    #[test]
    fn group_chars_into_blocks_gap() {
        let chars = vec![
            make_char("A", 0.0, 0.0, 10.0, 12.0, 12.0),
            make_char("B", 10.0, 0.0, 20.0, 12.0, 12.0),
            // 20 pt gap — should split
            make_char("C", 0.0, 32.0, 10.0, 44.0, 12.0),
        ];
        let blocks = group_chars_into_blocks(&chars, 4.0);
        assert_eq!(blocks.len(), 2, "gap of 20pt should split into 2 blocks");
    }

    #[test]
    fn group_chars_no_gap() {
        let chars = vec![
            make_char("A", 0.0, 0.0, 10.0, 12.0, 12.0),
            make_char("B", 10.0, 0.0, 20.0, 12.0, 12.0),
            make_char("C", 20.0, 0.0, 30.0, 12.0, 12.0),
        ];
        let blocks = group_chars_into_blocks(&chars, 4.0);
        assert_eq!(blocks.len(), 1, "adjacent chars should form one block");
    }

    #[test]
    fn inferred_tag_fields() {
        let tag = InferredTag::text_tag("P", BBox { x0: 0.0, top: 0.0, x1: 100.0, bottom: 12.0 }, 0, "Hello".to_owned());
        assert_eq!(tag.role, "P");
        assert!(!tag.needs_review);
        assert_eq!(tag.text, "Hello");
    }

    #[test]
    fn figure_tag_needs_review() {
        let tag = InferredTag::figure(BBox { x0: 0.0, top: 0.0, x1: 100.0, bottom: 100.0 }, 0);
        assert_eq!(tag.role, "Figure");
        assert!(tag.needs_review);
        assert!(tag.review_reason.is_some());
    }

    #[test]
    fn artifact_tag_no_review() {
        let tag = InferredTag::artifact(BBox { x0: 0.0, top: 0.0, x1: 100.0, bottom: 10.0 }, 0, "header");
        assert_eq!(tag.role, "Artifact");
        assert!(!tag.needs_review);
    }

    #[test]
    fn inferrer_default_settings() {
        let inf = TagInferrer::new();
        assert!((inf.h1_ratio - 1.4).abs() < 0.01);
        assert!((inf.artifact_zone - 0.08).abs() < 0.001);
    }

    #[test]
    fn review_count_recursive() {
        let tags = vec![
            InferredTag { role: "P".to_owned(), bbox: BBox { x0:0.0,top:0.0,x1:100.0,bottom:12.0 }, page: 0, text: "t".to_owned(), children: vec![
                InferredTag::figure(BBox { x0:0.0,top:0.0,x1:50.0,bottom:50.0 }, 0),
            ], needs_review: false, review_reason: None },
            InferredTag::figure(BBox { x0:0.0,top:0.0,x1:50.0,bottom:50.0 }, 0),
        ];
        assert_eq!(TagInferrer::new().review_count(&tags), 2);
    }
}
