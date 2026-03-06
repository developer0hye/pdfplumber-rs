//! Semantic layout inference for pdfplumber-rs.
//!
//! Takes extraction output from `pdfplumber` and returns a structured
//! [`Document`] with [`Section`]s, [`Heading`]s, [`Paragraph`]s,
//! [`LayoutTable`]s, and [`Figure`]s.
//!
//! **Rule-based only. No ML. No new mandatory deps.** All inference derives
//! from geometric and typographic signals already present in the extraction
//! layer: font size, font name, bounding boxes, text content, image presence,
//! and path density.
//!
//! # Quick Start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_layout::Document;
//!
//! let pdf = Pdf::open_file("report.pdf", None).unwrap();
//! let doc = Document::from_pdf(&pdf);
//!
//! // Markdown for LLM context
//! println!("{}", doc.to_markdown());
//!
//! // Structured access
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
//! # Column-aware Layout
//!
//! The extractor detects column boundaries automatically using `ColumnMode::Auto`
//! (the default). For academic papers, newspapers, and annual reports with 2-column
//! layouts, this produces correct reading order. Override via [`LayoutOptions`]:
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_core::ColumnMode;
//! use pdfplumber_layout::{Document, LayoutOptions};
//!
//! let pdf = Pdf::open_file("two-col.pdf", None).unwrap();
//! let opts = LayoutOptions {
//!     column_mode: ColumnMode::Explicit(vec![306.0]), // split at page midpoint
//!     ..LayoutOptions::default()
//! };
//! let doc = Document::from_pdf_with_options(&pdf, &opts);
//! ```
//!
//! # Header/Footer Suppression
//!
//! [`Document::from_pdf`] runs a two-pass algorithm: first detecting repeating
//! header/footer patterns across pages, then suppressing those regions during
//! extraction. This means page numbers, chapter titles, and other running
//! headers do not pollute the body text.

#![deny(missing_docs)]

pub(crate) mod classifier;
pub(crate) mod document;
pub mod extractor;
pub(crate) mod figures;
pub(crate) mod headings;
/// List detection utilities: bullet and ordered list item parsing.
pub mod lists;
pub(crate) mod markdown;
pub(crate) mod paragraphs;
pub(crate) mod sections;

pub use document::{Document, DocumentStats};
pub use extractor::{LayoutOptions, PageLayout, extract_page_layout};
pub use figures::{Figure, FigureKind};
pub use headings::{Heading, HeadingLevel};
pub use lists::{List, ListItem, ListKind, extract_lists_from_section};
pub use markdown::{
    block_to_markdown, figure_to_markdown, heading_to_markdown, paragraph_to_markdown,
    section_to_markdown, sections_to_markdown, table_to_markdown,
};
pub use paragraphs::Paragraph;
pub use sections::Section;

/// Semantic block type — the union of all layout elements on a page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LayoutBlock {
    /// A heading at a given level.
    Heading(Heading),
    /// A text paragraph (may be a caption or a list item).
    Paragraph(Paragraph),
    /// A detected table (wraps the pdfplumber Table with page context).
    Table(LayoutTable),
    /// A figure: an image or path-dense region with no meaningful text.
    Figure(Figure),
}

impl LayoutBlock {
    /// Return the bounding box of this block.
    pub fn bbox(&self) -> pdfplumber_core::BBox {
        match self {
            LayoutBlock::Heading(h) => h.bbox,
            LayoutBlock::Paragraph(p) => p.bbox,
            LayoutBlock::Table(t) => t.bbox,
            LayoutBlock::Figure(f) => f.bbox,
        }
    }

    /// Return the page number (0-based) this block came from.
    pub fn page_number(&self) -> usize {
        match self {
            LayoutBlock::Heading(h) => h.page_number,
            LayoutBlock::Paragraph(p) => p.page_number,
            LayoutBlock::Table(t) => t.page_number,
            LayoutBlock::Figure(f) => f.page_number,
        }
    }

    /// Render this block to GitHub-Flavored Markdown.
    pub fn to_markdown(&self) -> String {
        block_to_markdown(self)
    }
}

/// A table as seen from the layout layer — carries page context alongside the
/// detected table geometry and extracted cell data.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutTable {
    /// Bounding box of the table.
    pub bbox: pdfplumber_core::BBox,
    /// Page number (0-based) this table was found on.
    pub page_number: usize,
    /// Row count.
    pub rows: usize,
    /// Column count.
    pub cols: usize,
    /// Extracted cell text as a 2D array (row-major). `None` = empty cell.
    pub cells: Vec<Vec<Option<String>>>,
}
