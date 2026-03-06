//! Core chunker: [`Chunker`], [`ChunkSettings`], [`ChunkError`].
//!
//! ## Design
//!
//! The chunker delegates semantic block detection to [`pdfplumber_layout::extract_page_layout`],
//! which provides:
//! - Column-aware reading order (detects 1-/2-column layouts automatically)
//! - Heading inference calibrated against the page's modal font size
//! - Table detection via pdfplumber's lattice + stream finder
//! - Figure region detection
//! - Header/footer suppression (use [`Chunker::chunk_document`] for that)
//!
//! The chunker's job is purely to take those semantic blocks and produce
//! token-budgeted [`Chunk`]s with overlap windows.
//!
//! ## Chunking semantics
//!
//! - **Heading** → flush any pending prose accumulation, emit as a single Heading chunk,
//!   update `current_section`. No overlap into or out of headings.
//! - **Table** → flush pending prose, emit as a single Table chunk (never split),
//!   rendered pipe-delimited. No overlap into tables.
//! - **Figure** → no text content, skip.
//! - **Paragraph** → accumulate. When token count exceeds `max_tokens`, split at word
//!   boundary, emit, carry `overlap_tokens` tail text into next chunk.

use pdfplumber::Pdf;
use pdfplumber_core::BBox;
use pdfplumber_layout::{Document, LayoutBlock, LayoutOptions, extract_page_layout};

use crate::chunk::{Chunk, ChunkType};
use crate::token;

/// Configuration for the PDF chunker.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChunkSettings {
    /// Maximum estimated token count per prose chunk. Tables are never split.
    /// Default: 512.
    pub max_tokens: usize,

    /// Tokens to overlap between adjacent prose chunks (sliding window for RAG).
    /// Set to 0 to disable. Default: 64.
    pub overlap_tokens: usize,

    /// If true (default), tables emit as a single [`ChunkType::Table`] chunk.
    /// If false, table text merges into the prose accumulator.
    pub preserve_tables: bool,

    /// If true (default), [`Chunk::bbox`] is populated from source block bboxes.
    pub include_bbox: bool,

    /// Layout options passed to the layout engine (column mode, tolerances, etc.).
    /// Default: [`LayoutOptions::default()`].
    pub layout_opts: LayoutOptions,
}

impl Default for ChunkSettings {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            overlap_tokens: 64,
            preserve_tables: true,
            include_bbox: true,
            layout_opts: LayoutOptions::default(),
        }
    }
}

/// Errors that can occur during chunking.
#[derive(Debug)]
pub enum ChunkError {
    /// PDF parsing error from the underlying `pdfplumber` crate.
    PdfError(pdfplumber_core::PdfError),
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkError::PdfError(e) => write!(f, "PDF error: {e}"),
        }
    }
}

impl std::error::Error for ChunkError {}

impl From<pdfplumber_core::PdfError> for ChunkError {
    fn from(e: pdfplumber_core::PdfError) -> Self {
        ChunkError::PdfError(e)
    }
}

/// The main PDF chunker.
///
/// Construct once, call [`Chunker::chunk`] on any [`Pdf`].
///
/// ```no_run
/// use pdfplumber::Pdf;
/// use pdfplumber_chunk::{Chunker, ChunkSettings};
///
/// let pdf = Pdf::open_file("document.pdf", None).unwrap();
/// let chunker = Chunker::new(ChunkSettings::default());
/// let chunks = chunker.chunk(&pdf).unwrap();
/// println!("{} chunks", chunks.len());
/// ```
pub struct Chunker {
    settings: ChunkSettings,
}

impl Chunker {
    /// Create a new chunker with the given settings.
    ///
    /// If `overlap_tokens >= max_tokens`, overlap is silently clamped to 0 to
    /// prevent infinite splitting loops (overlap would exceed the budget on
    /// every split, causing unbounded memory growth).
    pub fn new(mut settings: ChunkSettings) -> Self {
        if settings.overlap_tokens >= settings.max_tokens {
            settings.overlap_tokens = 0;
        }
        Self { settings }
    }

    /// Chunk all pages of `pdf` in reading order.
    ///
    /// Uses `extract_page_layout` per page. For header/footer suppression across
    /// pages use [`Chunker::chunk_document`] instead.
    pub fn chunk(&self, pdf: &Pdf) -> Result<Vec<Chunk>, ChunkError> {
        let mut all_chunks = Vec::new();
        for page_idx in 0..pdf.page_count() {
            let page = pdf.page(page_idx)?;
            let layout = extract_page_layout(&page, &self.settings.layout_opts);
            let page_chunks = self.chunk_from_layout(page_idx, &layout.blocks);
            all_chunks.extend(page_chunks);
        }
        Ok(all_chunks)
    }

    /// Chunk using a pre-built [`Document`] (which has header/footer suppression).
    ///
    /// This is the highest-quality path: the Document's two-pass analysis suppresses
    /// repeating headers and footers before chunking, so page numbers and chapter
    /// titles don't pollute the body text chunks.
    pub fn chunk_document(&self, doc: &Document) -> Vec<Chunk> {
        let mut all_chunks = Vec::new();
        for page_layout in doc.pages() {
            let page_chunks = self.chunk_from_layout(page_layout.page_number, &page_layout.blocks);
            all_chunks.extend(page_chunks);
        }
        all_chunks
    }

    /// Chunk a pre-computed sequence of [`LayoutBlock`]s from a single page.
    ///
    /// This is the core chunking algorithm. Exposed so callers can supply
    /// their own layout analysis results.
    #[allow(unused_assignments)] // flush_prose! macro sets overlap_prefix/accum_bbox at every call site; final expansion has no reader
    pub fn chunk_from_layout(&self, page_idx: usize, blocks: &[LayoutBlock]) -> Vec<Chunk> {
        let mut chunks: Vec<Chunk> = Vec::new();
        let mut current_section: Option<String> = None;

        // Prose accumulator state.
        let mut accum_text = String::new();
        let mut accum_bbox: Option<BBox> = None;
        let mut overlap_prefix = String::new();

        macro_rules! flush_prose {
            () => {{
                if !accum_text.trim().is_empty() {
                    let bbox = accum_bbox.take().unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));
                    let text = std::mem::take(&mut accum_text);
                    let new_overlap = if self.settings.overlap_tokens > 0 {
                        token::extract_overlap(&text, self.settings.overlap_tokens).to_string()
                    } else {
                        String::new()
                    };
                    chunks.push(Chunk::new(
                        text,
                        page_idx,
                        bbox,
                        current_section.clone(),
                        ChunkType::Paragraph,
                    ));
                    overlap_prefix = new_overlap;
                } else {
                    accum_text.clear();
                    accum_bbox = None;
                }
            }};
        }

        for block in blocks {
            match block {
                LayoutBlock::Heading(h) => {
                    flush_prose!();
                    overlap_prefix.clear(); // no overlap into headings

                    let heading_text = h.text.trim().to_string();
                    if heading_text.is_empty() {
                        continue;
                    }
                    let bbox = if self.settings.include_bbox {
                        h.bbox
                    } else {
                        BBox::new(0.0, 0.0, 0.0, 0.0)
                    };
                    current_section = Some(heading_text.clone());
                    chunks.push(Chunk::new(
                        heading_text,
                        page_idx,
                        bbox,
                        current_section.clone(),
                        ChunkType::Heading,
                    ));
                }

                LayoutBlock::Paragraph(p) => {
                    let block_text = p.text.trim().to_string();
                    if block_text.is_empty() {
                        continue;
                    }

                    // Prepend overlap prefix if starting fresh accumulation.
                    let incoming = if !overlap_prefix.is_empty() && accum_text.is_empty() {
                        format!("{} {}", overlap_prefix.trim(), block_text)
                    } else if accum_text.is_empty() {
                        block_text
                    } else {
                        format!("{} {}", accum_text.trim(), block_text)
                    };

                    // Split loop: keep emitting until remainder fits in budget.
                    //
                    // Overlap is tracked separately and prepended to the NEXT
                    // chunk's text at emission time — it is NOT re-injected into
                    // the splitting remainder. Re-injecting overlap into the
                    // remainder causes it to grow (or not shrink) each iteration,
                    // leading to infinite loops and OOM when overlap ≈ max_tokens.
                    let mut remainder = incoming;
                    let mut pending_overlap = String::new();
                    loop {
                        if token::estimate(&remainder) <= self.settings.max_tokens {
                            break;
                        }
                        let (head, tail) =
                            token::split_at_token_boundary(&remainder, self.settings.max_tokens);
                        if head.is_empty() {
                            break; // safety: no progress possible
                        }

                        // Emit: prepend any pending overlap from the previous split.
                        let emit_text = if pending_overlap.is_empty() {
                            head.to_string()
                        } else {
                            format!("{} {}", pending_overlap, head)
                        };

                        let bbox = accum_bbox
                            .take()
                            .map(|b| {
                                if self.settings.include_bbox {
                                    b.union(&p.bbox)
                                } else {
                                    BBox::new(0.0, 0.0, 0.0, 0.0)
                                }
                            })
                            .unwrap_or(if self.settings.include_bbox {
                                p.bbox
                            } else {
                                BBox::new(0.0, 0.0, 0.0, 0.0)
                            });
                        chunks.push(Chunk::new(
                            emit_text,
                            page_idx,
                            bbox,
                            current_section.clone(),
                            ChunkType::Paragraph,
                        ));

                        // Compute overlap for the next chunk from the raw head
                        // (not the emit_text which includes previous overlap).
                        pending_overlap = if self.settings.overlap_tokens > 0 {
                            token::extract_overlap(head, self.settings.overlap_tokens).to_string()
                        } else {
                            String::new()
                        };

                        // Remainder is purely the unconsumed tail — no overlap injected.
                        remainder = tail.to_string();
                    }

                    // If there's pending overlap from the last split, prepend it
                    // to the remainder so the final accumulated chunk has continuity.
                    if !pending_overlap.is_empty() && !remainder.trim().is_empty() {
                        remainder = format!("{} {}", pending_overlap, remainder.trim());
                    }

                    // Update accumulator.
                    let bbox_update = if self.settings.include_bbox {
                        p.bbox
                    } else {
                        BBox::new(0.0, 0.0, 0.0, 0.0)
                    };
                    accum_bbox = Some(match accum_bbox {
                        Some(b) => b.union(&bbox_update),
                        None => bbox_update,
                    });
                    accum_text = remainder;
                    overlap_prefix.clear();
                }

                LayoutBlock::Table(t) => {
                    flush_prose!();
                    overlap_prefix.clear(); // no overlap into tables

                    if self.settings.preserve_tables {
                        let text = render_layout_table_cells(&t.cells);
                        if !text.trim().is_empty() {
                            let bbox = if self.settings.include_bbox {
                                t.bbox
                            } else {
                                BBox::new(0.0, 0.0, 0.0, 0.0)
                            };
                            chunks.push(Chunk::new(
                                text,
                                page_idx,
                                bbox,
                                current_section.clone(),
                                ChunkType::Table,
                            ));
                        }
                    } else {
                        // Merge table text into prose accumulator.
                        let text = render_layout_table_cells(&t.cells);
                        if !text.trim().is_empty() {
                            if !accum_text.is_empty() {
                                accum_text.push('\n');
                            }
                            accum_text.push_str(text.trim());
                            let bbox_update = if self.settings.include_bbox {
                                t.bbox
                            } else {
                                BBox::new(0.0, 0.0, 0.0, 0.0)
                            };
                            accum_bbox = Some(match accum_bbox {
                                Some(b) => b.union(&bbox_update),
                                None => bbox_update,
                            });
                        }
                    }
                }

                LayoutBlock::Figure(_) => {
                    // Figures have no text content — no chunk emitted.
                    // Future: emit a ChunkType::Figure with alt-text if available.
                }
            }
        }

        // Final flush of remaining prose.
        flush_prose!();
        chunks
    }
}

/// Render 2D cell data to pipe-delimited text (one row per line).
///
/// Empty rows (all cells None or empty) are omitted.
fn render_layout_table_cells(cells: &[Vec<Option<String>>]) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(cells.len());
    for row in cells {
        let cols: Vec<&str> = row
            .iter()
            .map(|c| c.as_deref().unwrap_or("").trim())
            .collect();
        if cols.iter().all(|c| c.is_empty()) {
            continue;
        }
        lines.push(cols.join(" | "));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::BBox;
    use pdfplumber_layout::{Heading, HeadingLevel, LayoutTable, Paragraph};

    fn heading(text: &str, page: usize, top: f64) -> LayoutBlock {
        LayoutBlock::Heading(Heading {
            text: text.to_string(),
            bbox: BBox::new(72.0, top, 400.0, top + 20.0),
            page_number: page,
            level: HeadingLevel::H1,
            font_size: 18.0,
            fontname: "Helvetica-Bold".to_string(),
        })
    }

    fn para(text: &str, page: usize, top: f64) -> LayoutBlock {
        LayoutBlock::Paragraph(Paragraph {
            text: text.to_string(),
            bbox: BBox::new(72.0, top, 500.0, top + 30.0),
            page_number: page,
            line_count: 2,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption: false,
            is_list_item: false,
        })
    }

    fn table(page: usize, top: f64) -> LayoutBlock {
        LayoutBlock::Table(LayoutTable {
            bbox: BBox::new(72.0, top, 500.0, top + 100.0),
            page_number: page,
            rows: 2,
            cols: 2,
            cells: vec![
                vec![Some("Header A".into()), Some("Header B".into())],
                vec![Some("Value 1".into()), Some("Value 2".into())],
            ],
        })
    }

    #[test]
    fn empty_blocks_give_no_chunks() {
        let chunker = Chunker::new(ChunkSettings::default());
        assert!(chunker.chunk_from_layout(0, &[]).is_empty());
    }

    #[test]
    fn single_paragraph_yields_one_chunk() {
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks = chunker.chunk_from_layout(0, &[para("Hello world.", 0, 50.0)]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::Paragraph);
        assert!(chunks[0].text.contains("Hello world"));
    }

    #[test]
    fn heading_chunk_updates_section() {
        let blocks = vec![
            heading("Introduction", 0, 50.0),
            para("Body text.", 0, 80.0),
        ];
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks = chunker.chunk_from_layout(0, &blocks);
        let h_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Heading)
            .collect();
        let p_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == ChunkType::Paragraph)
            .collect();
        assert_eq!(h_chunks.len(), 1);
        assert_eq!(p_chunks[0].section.as_deref(), Some("Introduction"));
    }

    #[test]
    fn table_emits_as_table_chunk() {
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks = chunker.chunk_from_layout(0, &[table(0, 200.0)]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::Table);
        assert!(chunks[0].text.contains("Header A | Header B"));
    }

    #[test]
    fn table_never_split_at_tiny_max_tokens() {
        let settings = ChunkSettings {
            max_tokens: 2,
            preserve_tables: true,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        let chunks = chunker.chunk_from_layout(0, &[table(0, 200.0)]);
        assert_eq!(
            chunks
                .iter()
                .filter(|c| c.chunk_type == ChunkType::Table)
                .count(),
            1
        );
    }

    #[test]
    fn long_paragraph_splits_at_token_boundary() {
        let long_text: String = (0..300).map(|i| format!("word{i} ")).collect();
        let blocks = vec![para(&long_text, 0, 100.0)];
        let settings = ChunkSettings {
            max_tokens: 100,
            overlap_tokens: 0,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        let chunks = chunker.chunk_from_layout(0, &blocks);
        assert!(chunks.len() > 1, "should split into multiple chunks");
        for chunk in &chunks {
            assert!(
                chunk.token_count <= 130,
                "chunk token_count {} > budget+10%",
                chunk.token_count
            );
        }
    }

    #[test]
    fn chunk_carries_page_idx() {
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks = chunker.chunk_from_layout(7, &[para("text", 7, 50.0)]);
        assert!(chunks.iter().all(|c| c.page == 7));
    }

    #[test]
    fn include_bbox_false_zeros_bboxes() {
        let settings = ChunkSettings {
            include_bbox: false,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        let chunks = chunker.chunk_from_layout(0, &[para("text", 0, 50.0)]);
        for c in &chunks {
            assert_eq!(c.bbox.x0, 0.0);
            assert_eq!(c.bbox.x1, 0.0);
        }
    }

    #[test]
    fn section_resets_across_headings() {
        let blocks = vec![
            heading("Section A", 0, 50.0),
            para("Para in A.", 0, 80.0),
            heading("Section B", 0, 200.0),
            para("Para in B.", 0, 230.0),
        ];
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks = chunker.chunk_from_layout(0, &blocks);
        let a = chunks
            .iter()
            .find(|c| c.text.contains("Para in A"))
            .unwrap();
        let b = chunks
            .iter()
            .find(|c| c.text.contains("Para in B"))
            .unwrap();
        assert_eq!(a.section.as_deref(), Some("Section A"));
        assert_eq!(b.section.as_deref(), Some("Section B"));
    }

    #[test]
    fn token_count_matches_estimate() {
        let chunker = Chunker::new(ChunkSettings::default());
        let chunks =
            chunker.chunk_from_layout(0, &[para("Alpha beta gamma delta epsilon.", 0, 50.0)]);
        for chunk in &chunks {
            assert_eq!(chunk.token_count, crate::token::estimate(&chunk.text));
        }
    }

    #[test]
    fn preserve_tables_false_merges_into_prose() {
        let blocks = vec![
            para("Before.", 0, 50.0),
            table(0, 100.0),
            para("After.", 0, 220.0),
        ];
        let settings = ChunkSettings {
            preserve_tables: false,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        let chunks = chunker.chunk_from_layout(0, &blocks);
        assert!(
            chunks.iter().all(|c| c.chunk_type != ChunkType::Table),
            "no table chunks when preserve=false"
        );
    }

    #[test]
    fn default_settings_sensible() {
        let s = ChunkSettings::default();
        assert_eq!(s.max_tokens, 512);
        assert_eq!(s.overlap_tokens, 64);
        assert!(s.preserve_tables);
        assert!(s.include_bbox);
    }

    #[test]
    fn overlap_clamped_when_exceeds_max_tokens() {
        // overlap >= max_tokens would cause infinite splitting → OOM.
        // Chunker::new must clamp to 0.
        let settings = ChunkSettings {
            max_tokens: 10,
            overlap_tokens: 64,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        assert_eq!(chunker.settings.overlap_tokens, 0);
    }

    #[test]
    fn overlap_equal_to_max_tokens_clamped() {
        let settings = ChunkSettings {
            max_tokens: 50,
            overlap_tokens: 50,
            ..Default::default()
        };
        let chunker = Chunker::new(settings);
        assert_eq!(chunker.settings.overlap_tokens, 0);
    }
}
