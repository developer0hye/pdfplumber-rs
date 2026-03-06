//! Document-level layout extraction: the top-level entry point.
//!
//! [`Document`] is built by running layout inference over all pages of a
//! [`Pdf`]. It provides:
//!
//! - [`Document::sections()`] — the document partitioned into heading-delimited [`Section`]s
//! - [`Document::pages()`] — per-page [`PageLayout`]s
//! - [`Document::stats()`] — summary statistics
//! - [`Document::to_markdown()`] — GFM markdown rendering of the full document
//! - Flat iterators for headings, paragraphs, tables, figures
//!
//! ## Two-pass architecture
//!
//! Pass 1: collect raw page text from the top and bottom margins of every page
//!         → run `detect_page_regions` to identify repeating headers/footers.
//! Pass 2: extract layout from each page, suppressing blocks that fall in the
//!         detected header/footer zones.
//!
//! This produces correct body text even on documents with page numbers, chapter
//! titles, and other running headers.

use pdfplumber::Pdf;
use pdfplumber_core::{PageRegionOptions, detect_page_regions};

use crate::extractor::{LayoutOptions, PageLayout, extract_page_layout};
use crate::markdown::sections_to_markdown;
use crate::sections::{Section, partition_into_sections};
use crate::{Heading, LayoutBlock, Paragraph};
use crate::figures::Figure;
use crate::LayoutTable;

/// Document-wide layout statistics.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentStats {
    /// Total page count.
    pub page_count: usize,
    /// Total headings detected.
    pub heading_count: usize,
    /// Total paragraph blocks detected.
    pub paragraph_count: usize,
    /// Total tables detected.
    pub table_count: usize,
    /// Total figures detected.
    pub figure_count: usize,
    /// Total character count across all pages.
    pub char_count: usize,
    /// Estimated body baseline font size (median across pages, 0.0 if no chars).
    pub body_font_size: f64,
    /// Number of pages where headers were suppressed.
    pub pages_with_header: usize,
    /// Number of pages where footers were suppressed.
    pub pages_with_footer: usize,
}

/// The result of running semantic layout inference over a full [`Pdf`].
///
/// # Examples
///
/// ```no_run
/// use pdfplumber::Pdf;
/// use pdfplumber_layout::Document;
///
/// let pdf = Pdf::open_file("report.pdf", None).unwrap();
/// let doc = Document::from_pdf(&pdf);
///
/// println!("{}", doc.to_markdown());
/// println!("Sections: {}", doc.sections().len());
/// println!("Body font: {:.1}pt", doc.stats().body_font_size);
/// ```
#[derive(Debug, Clone)]
pub struct Document {
    pages: Vec<PageLayout>,
    sections: Vec<Section>,
    stats: DocumentStats,
}

impl Document {
    /// Run layout inference over all pages with default options.
    ///
    /// This uses [`LayoutOptions::default()`] with `ColumnMode::Auto` and
    /// automatic header/footer detection.
    pub fn from_pdf(pdf: &Pdf) -> Self {
        Self::from_pdf_with_options(pdf, &LayoutOptions::default())
    }

    /// Run layout inference with custom [`LayoutOptions`].
    ///
    /// Two-pass:
    /// 1. Collect all pages, extract top/bottom margin text per page.
    /// 2. Detect repeating header/footer patterns across pages.
    /// 3. Re-extract each page with header/footer zones set in options.
    pub fn from_pdf_with_options(pdf: &Pdf, opts: &LayoutOptions) -> Self {
        // ── Pass 1: collect pages and build page-region data ────────────────
        let mut raw_pages: Vec<pdfplumber::Page> = Vec::new();
        let mut char_count = 0usize;
        let mut body_sizes: Vec<f64> = Vec::new();

        for page_result in pdf.pages_iter() {
            let page = match page_result {
                Ok(p) => p,
                Err(_) => continue,
            };
            char_count += page.chars().len();
            let baseline = crate::classifier::compute_body_baseline(page.chars());
            if baseline > 0.0 {
                body_sizes.push(baseline);
            }
            raw_pages.push(page);
        }

        // Build the (header_text, footer_text, width, height) tuples for region detection.
        // Header text = text in top 10% of page; footer text = text in bottom 10%.
        let page_data: Vec<(String, String, f64, f64)> = raw_pages
            .iter()
            .map(|page| {
                let h = page.height();
                let w = page.width();
                let header_text = extract_margin_text(page, 0.0, h * 0.10);
                let footer_text = extract_margin_text(page, h * 0.90, h);
                (header_text, footer_text, w, h)
            })
            .collect();

        // ── Detect regions ──────────────────────────────────────────────────
        let regions = detect_page_regions(&page_data, &PageRegionOptions::default());

        let pages_with_header = regions.iter().filter(|r| r.header.is_some()).count();
        let pages_with_footer = regions.iter().filter(|r| r.footer.is_some()).count();

        // ── Pass 2: extract layout per page with zone suppression ────────────
        let mut pages: Vec<PageLayout> = Vec::new();
        let mut all_blocks: Vec<LayoutBlock> = Vec::new();

        for (i, page) in raw_pages.iter().enumerate() {
            let region = regions.get(i);
            let page_opts = LayoutOptions {
                header_zone_bottom: region.and_then(|r| r.header.map(|h| h.bottom)),
                footer_zone_top: region.and_then(|r| r.footer.map(|f| f.top)),
                ..opts.clone()
            };
            let layout = extract_page_layout(page, &page_opts);
            all_blocks.extend(layout.blocks.clone());
            pages.push(layout);
        }

        // ── Section partitioning ─────────────────────────────────────────────
        let sections = partition_into_sections(all_blocks.clone());

        // ── Stats ────────────────────────────────────────────────────────────
        let heading_count = all_blocks.iter().filter(|b| matches!(b, LayoutBlock::Heading(_))).count();
        let paragraph_count = all_blocks.iter().filter(|b| matches!(b, LayoutBlock::Paragraph(_))).count();
        let table_count = all_blocks.iter().filter(|b| matches!(b, LayoutBlock::Table(_))).count();
        let figure_count = all_blocks.iter().filter(|b| matches!(b, LayoutBlock::Figure(_))).count();

        body_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let body_font_size = if body_sizes.is_empty() {
            0.0
        } else {
            body_sizes[body_sizes.len() / 2]
        };

        let stats = DocumentStats {
            page_count: pages.len(),
            heading_count,
            paragraph_count,
            table_count,
            figure_count,
            char_count,
            body_font_size,
            pages_with_header,
            pages_with_footer,
        };

        Document { pages, sections, stats }
    }

    /// Per-page layouts, in page order.
    pub fn pages(&self) -> &[PageLayout] {
        &self.pages
    }

    /// Document sections (heading-delimited partitions of all blocks).
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Summary statistics for this document.
    pub fn stats(&self) -> &DocumentStats {
        &self.stats
    }

    /// Flat iterator over all headings in document order.
    pub fn headings(&self) -> impl Iterator<Item = &Heading> {
        self.pages.iter().flat_map(|p| p.headings())
    }

    /// Flat iterator over all paragraphs in document order.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> {
        self.pages.iter().flat_map(|p| p.paragraphs())
    }

    /// Flat iterator over all tables in document order.
    pub fn tables(&self) -> impl Iterator<Item = &LayoutTable> {
        self.pages.iter().flat_map(|p| p.tables())
    }

    /// Flat iterator over all figures in document order.
    pub fn figures(&self) -> impl Iterator<Item = &Figure> {
        self.pages.iter().flat_map(|p| p.figures())
    }

    /// Iterator over all blocks across all pages in document order.
    pub fn all_blocks(&self) -> impl Iterator<Item = &LayoutBlock> {
        self.pages.iter().flat_map(|p| p.blocks.iter())
    }

    /// Render the document as GitHub-Flavored Markdown.
    ///
    /// Headings → ATX `#` style. Tables → GFM pipe tables.
    /// Captions → *italic*. Figures → image placeholders.
    /// Sections separated by `---` horizontal rules.
    ///
    /// This is the primary output for LLM context building and RAG indexing.
    pub fn to_markdown(&self) -> String {
        sections_to_markdown(&self.sections)
    }

    /// Extract all document text in reading order.
    ///
    /// Headings and paragraphs only — tables and figures are excluded.
    /// Pages are separated by double newlines.
    pub fn text(&self) -> String {
        self.pages
            .iter()
            .map(|p| {
                p.blocks
                    .iter()
                    .filter_map(|b| match b {
                        LayoutBlock::Heading(h) => Some(h.text.as_str()),
                        LayoutBlock::Paragraph(para) => Some(para.text.as_str()),
                        LayoutBlock::Table(_) | LayoutBlock::Figure(_) => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the text of all characters whose vertical midpoint falls within
/// [top_y, bottom_y] on the given page, as a single space-joined string.
fn extract_margin_text(page: &pdfplumber::Page, top_y: f64, bottom_y: f64) -> String {
    let chars = page.chars();
    let mut words: Vec<(f64, &str)> = chars
        .iter()
        .filter(|c| {
            let cy = (c.bbox.top + c.bbox.bottom) / 2.0;
            cy >= top_y && cy <= bottom_y
        })
        .map(|c| (c.bbox.x0, c.text.as_str()))
        .collect();
    words.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    words.iter().map(|(_, t)| *t).collect::<Vec<_>>().join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractor::LayoutOptions;

    #[test]
    fn document_stats_default() {
        let s = DocumentStats::default();
        assert_eq!(s.page_count, 0);
        assert_eq!(s.heading_count, 0);
        assert_eq!(s.body_font_size, 0.0);
        assert_eq!(s.pages_with_header, 0);
        assert_eq!(s.pages_with_footer, 0);
    }

    #[test]
    fn layout_options_default_values() {
        let opts = LayoutOptions::default();
        assert!(opts.detect_tables);
        assert!(opts.detect_figures);
        assert!((opts.y_tolerance - 3.0).abs() < 1e-9);
        assert!((opts.y_density - 12.0).abs() < 1e-9);
        assert!(opts.header_zone_bottom.is_none());
        assert!(opts.footer_zone_top.is_none());
    }
}
