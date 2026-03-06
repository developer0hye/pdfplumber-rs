//! Heuristic heading detection from [`TextBlock`] geometry and font metrics.
//!
//! ## The signal
//!
//! A block is classified as a heading when **all three** conditions hold:
//!
//! 1. **Font size ratio**: the block's dominant (median) font size is ≥
//!    [`HEADING_FONT_SIZE_RATIO`] × the page's median font size.
//!
//! 2. **Block length**: the block contains ≤ [`HEADING_MAX_WORDS`] words. Long
//!    blocks are prose even if their font is slightly larger.
//!
//! 3. **Vertical position OR gap**: the block either starts in the top
//!    [`HEADING_TOP_FRACTION`] of the page, OR follows a vertical gap ≥
//!    [`HEADING_GAP_THRESHOLD`] points from the previous block.
//!
//! These three signals together catch section headings in annual reports, legal
//! documents, academic papers, and government PDFs without false-positives on
//! large-font pull-quotes or decorative text.
//!
//! ## Bold detection
//!
//! As a secondary signal, font names containing "Bold", "Heavy", or "Black"
//! (case-insensitive) add [`BOLD_FONT_BOOST`] to the effective font-size ratio,
//! allowing bold body-text to qualify as a heading at a lower size threshold.

use pdfplumber_core::TextBlock;

/// A block must have a dominant font size at least this many times larger than the
/// page median to qualify as a heading.
pub const HEADING_FONT_SIZE_RATIO: f64 = 1.15;

/// Blocks with more words than this cannot be headings.
pub const HEADING_MAX_WORDS: usize = 20;

/// Blocks whose top edge is within this fraction of the page height from the top
/// qualify regardless of gap.
pub const HEADING_TOP_FRACTION: f64 = 0.40;

/// A vertical gap of at least this many points between consecutive blocks is
/// considered a section break, making the following block eligible for heading
/// classification.
pub const HEADING_GAP_THRESHOLD: f64 = 18.0;

/// Bold font names contribute this many extra ratio points.
pub const BOLD_FONT_BOOST: f64 = 0.05;

/// Return the dominant (median) font size of all chars in `block`.
///
/// Returns `None` if the block contains no chars.
pub fn dominant_font_size(block: &TextBlock) -> Option<f64> {
    let mut sizes: Vec<f64> = block
        .lines
        .iter()
        .flat_map(|line| line.words.iter())
        .flat_map(|word| word.chars.iter())
        .map(|ch| ch.size)
        .filter(|&s| s > 0.0)
        .collect();
    if sizes.is_empty() {
        return None;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(sizes[sizes.len() / 2])
}

/// Return the dominant font name of all chars in `block` (most frequent).
///
/// Returns empty string if no chars.
pub fn dominant_font_name(block: &TextBlock) -> String {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for word in block.lines.iter().flat_map(|l| l.words.iter()) {
        for ch in &word.chars {
            *counts.entry(ch.fontname.as_str()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.to_string())
        .unwrap_or_default()
}

/// Count whitespace-separated words in all text lines of `block`.
pub fn block_word_count(block: &TextBlock) -> usize {
    block
        .lines
        .iter()
        .flat_map(|line| line.words.iter())
        .count()
}

/// Compute the median font size across all blocks on a page.
///
/// Used as the denominator for the heading size ratio.
/// Returns `12.0` as a reasonable fallback if no chars are present.
pub fn page_median_font_size(blocks: &[TextBlock]) -> f64 {
    let mut sizes: Vec<f64> = blocks
        .iter()
        .flat_map(|b| b.lines.iter())
        .flat_map(|l| l.words.iter())
        .flat_map(|w| w.chars.iter())
        .map(|ch| ch.size)
        .filter(|&s| s > 0.0)
        .collect();
    if sizes.is_empty() {
        return 12.0;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sizes[sizes.len() / 2]
}

/// Decide whether `block` is a heading.
///
/// # Parameters
/// - `block`: the candidate block
/// - `page_height`: height of the page in points (for top-fraction test)
/// - `median_font_size`: precomputed page median (from [`page_median_font_size`])
/// - `prev_block_bottom`: `Some(y)` where y is the bottom of the previous block,
///   or `None` if this is the first block on the page
pub fn is_heading(
    block: &TextBlock,
    page_height: f64,
    median_font_size: f64,
    prev_block_bottom: Option<f64>,
) -> bool {
    // Word count gate: long blocks are never headings.
    if block_word_count(block) > HEADING_MAX_WORDS {
        return false;
    }

    // Font size gate.
    let Some(block_size) = dominant_font_size(block) else {
        return false;
    };

    // Bold boost.
    let font_name = dominant_font_name(block);
    let is_bold = font_name.to_ascii_lowercase().contains("bold")
        || font_name.to_ascii_lowercase().contains("heavy")
        || font_name.to_ascii_lowercase().contains("black");
    let effective_ratio = HEADING_FONT_SIZE_RATIO - if is_bold { BOLD_FONT_BOOST } else { 0.0 };

    if block_size < median_font_size * effective_ratio {
        return false;
    }

    // Positional gate: top-fraction OR gap.
    let in_top_fraction =
        page_height > 0.0 && (block.bbox.top / page_height) < HEADING_TOP_FRACTION;
    let after_large_gap = prev_block_bottom
        .map(|bottom| block.bbox.top - bottom >= HEADING_GAP_THRESHOLD)
        .unwrap_or(true); // First block on page: gap is implicitly large.

    in_top_fraction || after_large_gap
}

/// Classify a sequence of blocks on a single page, returning `true` at each
/// index where the block is a heading.
pub fn classify_blocks(blocks: &[TextBlock], page_height: f64, median_font_size: f64) -> Vec<bool> {
    let mut result = Vec::with_capacity(blocks.len());
    let mut prev_bottom: Option<f64> = None;
    for block in blocks {
        let heading = is_heading(block, page_height, median_font_size, prev_bottom);
        result.push(heading);
        prev_bottom = Some(block.bbox.bottom);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Char, TextBlock, TextDirection, TextLine, Word};

    fn make_char(size: f64, fontname: &str) -> Char {
        Char {
            text: "A".to_string(),
            bbox: BBox::new(0.0, 0.0, size * 0.6, size),
            fontname: fontname.to_string(),
            size,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            upright: true,
            doctop: 0.0,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: 65,
            mcid: None,
            tag: None,
        }
    }

    fn make_block(chars: Vec<Char>, top: f64, bottom: f64) -> TextBlock {
        let bbox = BBox::new(0.0, top, 200.0, bottom);
        let word = Word {
            text: chars.iter().map(|c| c.text.clone()).collect(),
            bbox,
            doctop: top,
            direction: TextDirection::Ltr,
            chars,
        };
        let line = TextLine {
            words: vec![word],
            bbox,
        };
        TextBlock {
            lines: vec![line],
            bbox,
        }
    }

    #[test]
    fn large_font_at_top_is_heading() {
        let block = make_block(vec![make_char(24.0, "Arial")], 10.0, 34.0);
        // page_height=800, median=12, top=10 -> in top 40% (10/800=0.0125)
        assert!(is_heading(&block, 800.0, 12.0, None));
    }

    #[test]
    fn body_font_is_not_heading() {
        let block = make_block(vec![make_char(11.0, "Arial")], 100.0, 120.0);
        // 11 < 12 * 1.15 = 13.8
        assert!(!is_heading(&block, 800.0, 12.0, Some(80.0)));
    }

    /// Build a block with `n` separate single-char words — exercises word count gate.
    fn make_multi_word_block(
        n: usize,
        size: f64,
        fontname: &str,
        top: f64,
        bottom: f64,
    ) -> TextBlock {
        let bbox = BBox::new(0.0, top, 200.0, bottom);
        let words: Vec<Word> = (0..n)
            .map(|i| {
                let ch = make_char(size, fontname);
                Word {
                    text: ch.text.clone(),
                    bbox: BBox::new(i as f64 * 8.0, top, i as f64 * 8.0 + 7.0, bottom),
                    doctop: top,
                    direction: TextDirection::Ltr,
                    chars: vec![ch],
                }
            })
            .collect();
        let line = TextLine { words, bbox };
        TextBlock {
            lines: vec![line],
            bbox,
        }
    }

    #[test]
    fn long_block_is_not_heading_even_if_large() {
        // 25 separate words, each large font — word count gate fires first
        let block = make_multi_word_block(25, 20.0, "Arial", 10.0, 30.0);
        // block_word_count = 25 > HEADING_MAX_WORDS (20) → not heading regardless of font
        assert!(!is_heading(&block, 800.0, 12.0, None));
    }

    #[test]
    fn bold_font_qualifies_at_lower_ratio() {
        // 13.8 normal threshold (12*1.15), bold threshold 12*1.10=13.2
        let block = make_block(vec![make_char(13.5, "Arial-BoldMT")], 400.0, 420.0);
        // Without bold: 13.5 < 13.8 -> not heading; with bold: 13.5 >= 13.2 -> heading
        // But must also pass positional test: 400/800=0.5 > 0.4, need gap
        assert!(!is_heading(&block, 800.0, 12.0, Some(399.0))); // small gap
        assert!(is_heading(&block, 800.0, 12.0, Some(380.0))); // gap=20 >= 18
    }

    #[test]
    fn page_median_font_size_returns_12_for_empty() {
        assert_eq!(page_median_font_size(&[]), 12.0);
    }

    #[test]
    fn classify_blocks_marks_first_large() {
        let small = make_block(vec![make_char(11.0, "Arial")], 50.0, 70.0);
        let large = make_block(vec![make_char(20.0, "Arial-Bold")], 80.0, 100.0);
        let another = make_block(vec![make_char(11.0, "Arial")], 120.0, 140.0);
        let flags = classify_blocks(&[small, large, another], 800.0, 12.0);
        assert_eq!(flags.len(), 3);
        // first is in top fraction -> heading
        // large bold after small gap -> heading (gap=10 < 18, but in top fraction? 80/800=0.10 yes)
        assert!(flags[1], "large bold block in top 40% should be heading");
        assert!(!flags[2], "body text should not be heading");
    }
}
