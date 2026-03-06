//! Core chunk types: [`Chunk`] and [`ChunkType`].

use pdfplumber_core::BBox;

/// Classification of a chunk's semantic role in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChunkType {
    /// A narrative paragraph or body text block.
    Paragraph,
    /// An inferred section or chapter heading.
    Heading,
    /// A complete table, rendered as pipe-delimited text. Never split.
    Table,
    /// A figure caption or image label.
    Caption,
}

/// A semantically-chunked piece of a PDF document with full spatial provenance.
///
/// Every chunk carries:
/// - The extracted text (UTF-8, whitespace-normalised).
/// - The 0-based page index it came from.
/// - The bounding box in PDF coordinate space (top-left origin, y increases down).
/// - An inferred section heading if one was detected above this chunk on the same page.
/// - The semantic type of this chunk.
/// - An estimated token count.
///
/// For [`ChunkType::Table`] chunks the text is a pipe-delimited rendering of the table
/// rows (see [`crate::table_render`]). For all other types it is plain prose.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Chunk {
    /// UTF-8 text content of this chunk.
    pub text: String,

    /// 0-based page index this chunk originates from.
    pub page: usize,

    /// Bounding box in page coordinate space (top-left origin).
    ///
    /// For merged chunks that span multiple source blocks the bbox is the union of
    /// all constituent block bboxes.
    pub bbox: BBox,

    /// Section heading in scope when this chunk was created.
    ///
    /// This is the text of the nearest [`ChunkType::Heading`] block that precedes
    /// this chunk on the same page (reading order). `None` if no heading was detected
    /// before this chunk on the current page.
    pub section: Option<String>,

    /// Semantic classification of this chunk.
    pub chunk_type: ChunkType,

    /// Estimated token count.
    ///
    /// Computed as `ceil(whitespace_word_count * 1.3)`. Within ±20% of GPT-4
    /// BPE token counts for English prose. See [`crate::token`].
    pub token_count: usize,
}

impl Chunk {
    /// Create a new chunk.
    pub fn new(
        text: String,
        page: usize,
        bbox: BBox,
        section: Option<String>,
        chunk_type: ChunkType,
    ) -> Self {
        let token_count = crate::token::estimate(&text);
        Self { text, page, bbox, section, chunk_type, token_count }
    }
}
