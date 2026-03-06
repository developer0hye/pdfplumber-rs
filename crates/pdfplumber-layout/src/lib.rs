//! Semantic document structure inference for PDF documents.
//!
//! This crate analyses the geometric and typographic signals already present
//! in extracted `pdfplumber` output (chars, words, lines, tables) and infers
//! higher-level document structure: headings, paragraphs, sections, figures,
//! and tables.
//!
//! **No ML, no external models.** Everything is rule-based geometry.
//!
//! # Quick Start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_layout::Document;
//!
//! let pdf = Pdf::open_file("report.pdf", None).unwrap();
//! let doc = Document::from_pdf(&pdf).unwrap();
//!
//! for section in doc.sections() {
//!     if let Some(h) = section.heading() {
//!         println!("## {}", h.text());
//!     }
//!     for para in section.paragraphs() {
//!         println!("  {}", para.text());
//!     }
//! }
//! ```
//!
//! # Algorithm
//!
//! Layout inference runs in three passes:
//!
//! **Pass 1 — Block classification**: Each [`TextBlock`] from `pdfplumber`'s
//! line clustering is examined in isolation. Blocks are classified as
//! [`BlockKind::Heading`], [`BlockKind::Paragraph`], [`BlockKind::Caption`],
//! or [`BlockKind::Other`] based on:
//! - Font size relative to the page median
//! - Bold weight (fontname contains "Bold" / "Heavy" / "Black")
//! - Block height (single line = potential heading)
//! - Vertical whitespace above (larger gap → more likely heading)
//! - Left margin offset (indented = paragraph continuation)
//!
//! **Pass 2 — Section segmentation**: Heading blocks delimit sections.
//! A new [`Section`] begins at each heading. Content blocks between headings
//! belong to the preceding section. Top-of-page content before any heading
//! forms an implicit preamble section.
//!
//! **Pass 3 — Figure detection**: Page regions containing no text chars but
//! with significant path/rect/image content are marked as [`Figure`]s.
//!
//! # Accuracy Note
//!
//! This is geometric heuristics — it will not be perfect on every PDF.
//! Multi-column layouts, sidebars, headers/footers, and highly stylised
//! documents may confuse the classifier. The output is best-effort structure
//! for search indexing, summary generation, and RAG chunking — not archival
//! quality structural analysis.

mod block_classifier;
mod figure_detector;
mod section_builder;

pub use block_classifier::{BlockClassification, BlockKind, FontStats};
pub use figure_detector::Figure;
pub use section_builder::{Paragraph, Section};

use pdfplumber::{
    Pdf, TableSettings, TextOptions, WordOptions,
    cluster_lines_into_blocks, cluster_words_into_lines,
};
use pdfplumber_core::{BBox, Table, TextBlock};

/// Error type for layout inference failures.
#[derive(Debug)]
pub enum LayoutError {
    /// PDF page extraction failed.
    PageError(String),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::PageError(msg) => write!(f, "page error: {msg}"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A semantically structured document inferred from a [`Pdf`].
///
/// Contains the full list of [`Section`]s (each with headings and paragraphs)
/// and [`Figure`]s detected across all pages.
#[derive(Debug, Clone)]
pub struct Document {
    sections: Vec<Section>,
    figures: Vec<Figure>,
    page_count: usize,
}

impl Document {
    /// Analyse the given PDF and infer document structure.
    ///
    /// Returns a [`Document`] containing all inferred sections and figures.
    /// Individual page errors are skipped (logged to stderr in debug builds);
    /// only a complete failure to open any page returns `Err`.
    pub fn from_pdf(pdf: &Pdf) -> Result<Self, LayoutError> {
        let page_count = pdf.page_count();
        let text_opts = TextOptions::default();

        let mut all_blocks: Vec<(usize, TextBlock)> = Vec::new();
        let mut all_tables: Vec<(usize, Table, BBox)> = Vec::new();
        let mut all_figures: Vec<Figure> = Vec::new();
        let mut font_sizes: Vec<f64> = Vec::new();

        // Pass 0: collect raw blocks, tables, font sizes across all pages
        for page_idx in 0..page_count {
            let page = match pdf.page(page_idx) {
                Some(p) => p,
                None => continue,
            };

            // Extract words → cluster into lines → cluster into blocks
            let word_opts = WordOptions::default();
            let words = page.extract_words(&word_opts);
            let lines = cluster_words_into_lines(&words, text_opts.y_tolerance);
            let blocks: Vec<TextBlock> = cluster_lines_into_blocks(lines, text_opts.y_density);

            for block in &blocks {
                for line in &block.lines {
                    for word in &line.words {
                        for ch in &word.chars {
                            if ch.size > 0.0 {
                                font_sizes.push(ch.size);
                            }
                        }
                    }
                }
            }
            for block in blocks {
                all_blocks.push((page_idx, block));
            }

            let tables = page.find_tables(&TableSettings::default());
            for table in tables {
                let bbox = table.bbox;
                all_tables.push((page_idx, table, bbox));
            }

            // Figure detection: regions with paths/rects but no chars
            let figs = figure_detector::detect_figures(&page, page_idx);
            all_figures.extend(figs);
        }

        // Global font-size statistics (median = normal body text size)
        let stats = FontStats::from_sizes(&font_sizes);

        // Pass 1: classify each block
        let classified: Vec<BlockClassification> = all_blocks
            .iter()
            .map(|(page_idx, block)| {
                block_classifier::classify(block, *page_idx, &stats)
            })
            .collect();

        // Pass 2: segment into sections
        let sections = section_builder::build_sections(&classified, &all_blocks, &all_tables);

        Ok(Document {
            sections,
            figures: all_figures,
            page_count,
        })
    }

    /// All inferred sections in document order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// All detected figures (image-only or path-only regions).
    pub fn figures(&self) -> &[Figure] {
        &self.figures
    }

    /// Number of pages in the source PDF.
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Flatten all body text into a single string, section by section.
    ///
    /// Headings are prefixed with `## `. Paragraphs follow directly.
    /// Useful for dumping the document to plain text in reading order.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for section in &self.sections {
            if let Some(h) = section.heading() {
                out.push_str("## ");
                out.push_str(h.text());
                out.push('\n');
            }
            for para in section.paragraphs() {
                out.push_str(para.text());
                out.push('\n');
            }
            out.push('\n');
        }
        out
    }
}
