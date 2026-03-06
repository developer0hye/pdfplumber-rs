//! Semantically-aware PDF chunking for LLM/RAG pipelines.
//!
//! This crate splits a PDF document into text chunks that carry full spatial
//! provenance — page number, bounding box, inferred section heading, and chunk
//! type — so downstream retrieval is spatially-aware rather than just token-count
//! splitting.
//!
//! # Why this exists
//!
//! The standard approach to feeding PDFs into LLMs dumps the whole document to a
//! flat string and splits on token count. That loses all structure. You can no longer
//! answer "find the revenue table on page 12" or "what does section 4.2 say" because
//! the page and section context is gone.
//!
//! This crate preserves that structure. Every chunk knows where it came from.
//!
//! # Quick start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_chunk::{Chunker, ChunkSettings};
//!
//! let pdf = Pdf::open_file("document.pdf", None).unwrap();
//! let chunker = Chunker::new(ChunkSettings::default());
//! let chunks = chunker.chunk(&pdf).unwrap();
//!
//! for chunk in &chunks {
//!     println!("page {}, {:?}: {}", chunk.page, chunk.chunk_type, &chunk.text[..50.min(chunk.text.len())]);
//! }
//! ```
//!
//! # Heading detection
//!
//! Heading inference is delegated to [`pdfplumber_layout`], which uses a
//! column-aware rule-based classifier calibrated against the page's modal font
//! size — no ML, no trained model. `pdfplumber_layout` also performs two-pass
//! header/footer suppression so running headers and page numbers never pollute
//! the body text chunks. Use [`Chunker::chunk_document`] for the highest-quality
//! path (builds a layout [`pdfplumber_layout::Document`] first), or
//! [`Chunker::chunk`] for direct per-page extraction.
//!
//! # Tables
//!
//! Tables are always emitted as a single [`ChunkType::Table`] chunk using a
//! pipe-delimited text representation of their rows. They are never split across
//! chunk boundaries, regardless of `max_tokens`.
//!
//! # Token counting
//!
//! Token count is approximated as `ceil(whitespace_word_count * 1.3)`. This is
//! within ±20% of actual BPE token counts for English text (measured against GPT-4
//! tokenizer) and requires no external dependency. The factor 1.3 accounts for
//! sub-word tokenization of compound words and punctuation.
//!
//! # Utilities
//!
//! - [`heading`]: Low-level heading heuristics for [`pdfplumber_core::TextBlock`]
//!   sequences (useful if you're building a custom pipeline on top of raw blocks).
//! - [`table_render`]: Render a [`pdfplumber_core::Table`] to pipe-delimited text.
//!   Useful when you have a `Table` from a direct call to `Page::find_tables()`.

#![deny(missing_docs)]

mod chunk;
mod chunker;
/// Low-level heading heuristics for [`pdfplumber_core::TextBlock`] sequences.
///
/// These functions are used internally and are exposed for callers building
/// custom pipelines on top of raw text blocks. For normal use, prefer
/// [`Chunker`] which delegates to the higher-level `pdfplumber_layout` crate.
pub mod heading;
/// Render a [`pdfplumber_core::Table`] to pipe-delimited text.
///
/// Useful when you have a `Table` from a direct call to
/// [`pdfplumber::Page::find_tables`](pdfplumber::Page::find_tables) and want
/// the same text representation the chunker uses for [`ChunkType::Table`] chunks.
pub mod table_render;
mod token;

pub use chunk::{Chunk, ChunkType};
pub use chunker::{ChunkError, ChunkSettings, Chunker};

/// Estimate the number of BPE tokens in `text`.
///
/// Exported for use in test assertions and external token budget checking.
/// Uses the same algorithm as [`Chunk::token_count`].
pub fn token_estimate(text: &str) -> usize {
    token::estimate(text)
}
