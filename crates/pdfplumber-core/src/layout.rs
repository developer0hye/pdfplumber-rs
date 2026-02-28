use crate::geometry::BBox;
use crate::words::Word;

/// Column detection mode for multi-column layout reading order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColumnMode {
    /// No column detection (current default behavior).
    /// Blocks are sorted top-to-bottom, left-to-right.
    None,
    /// Automatically detect columns by clustering word x-coordinates
    /// and finding gaps wider than `min_column_gap`.
    Auto,
    /// Use explicit column boundary x-coordinates.
    /// Each value is an x-coordinate that separates adjacent columns.
    Explicit(Vec<f64>),
}

/// A text line: a sequence of words on the same y-level.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextLine {
    /// Words in this line, sorted left-to-right.
    pub words: Vec<Word>,
    /// Bounding box of this line.
    pub bbox: BBox,
}

/// A text block: a group of lines forming a coherent paragraph or section.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextBlock {
    /// Lines in this block, sorted top-to-bottom.
    pub lines: Vec<TextLine>,
    /// Bounding box of this block.
    pub bbox: BBox,
}

/// Options for layout-aware text extraction.
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// If true, use layout-aware extraction (detect blocks and reading order).
    /// If false, simple concatenation of words by spatial order.
    pub layout: bool,
    /// Vertical tolerance for clustering words into the same line (in points).
    pub y_tolerance: f64,
    /// Maximum vertical gap between lines to group into the same block (in points).
    pub y_density: f64,
    /// Minimum horizontal gap to detect column boundaries (in points).
    pub x_density: f64,
    /// If true, expand common Latin ligatures (U+FB00–U+FB06) to their multi-character equivalents.
    pub expand_ligatures: bool,
    /// Column detection mode. Default: `ColumnMode::None`.
    pub column_mode: ColumnMode,
    /// Minimum horizontal gap (in points) to detect as a column separator.
    /// Only used when `column_mode` is `Auto`. Default: 20.0.
    pub min_column_gap: f64,
    /// Maximum number of columns to detect.
    /// Only used when `column_mode` is `Auto`. Default: 6.
    pub max_columns: usize,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            layout: false,
            y_tolerance: 3.0,
            y_density: 10.0,
            x_density: 10.0,
            expand_ligatures: true,
            column_mode: ColumnMode::None,
            min_column_gap: 20.0,
            max_columns: 6,
        }
    }
}

/// Cluster words into text lines based on y-proximity.
///
/// Words whose vertical midpoints are within `y_tolerance` of a line's
/// vertical midpoint are grouped into the same line. Words within each
/// line are sorted left-to-right.
pub fn cluster_words_into_lines(words: &[Word], y_tolerance: f64) -> Vec<TextLine> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<&Word> = words.iter().collect();
    sorted.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });

    let mut lines: Vec<TextLine> = Vec::new();

    for word in sorted {
        let word_mid_y = (word.bbox.top + word.bbox.bottom) / 2.0;

        // Try to find an existing line this word belongs to
        let mut found = false;
        for line in &mut lines {
            let line_mid_y = (line.bbox.top + line.bbox.bottom) / 2.0;
            if (word_mid_y - line_mid_y).abs() <= y_tolerance {
                line.bbox = line.bbox.union(&word.bbox);
                line.words.push(word.clone());
                found = true;
                break;
            }
        }

        if !found {
            lines.push(TextLine {
                words: vec![word.clone()],
                bbox: word.bbox,
            });
        }
    }

    // Sort words within each line left-to-right
    for line in &mut lines {
        line.words
            .sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap());
    }

    // Sort lines top-to-bottom
    lines.sort_by(|a, b| a.bbox.top.partial_cmp(&b.bbox.top).unwrap());

    lines
}

/// Split text lines at large horizontal gaps to detect column boundaries.
///
/// Within each line, if consecutive words have a gap larger than `x_density`,
/// the line is split into separate line segments (one per column).
pub fn split_lines_at_columns(lines: Vec<TextLine>, x_density: f64) -> Vec<TextLine> {
    let mut result = Vec::new();
    for line in lines {
        if line.words.len() <= 1 {
            result.push(line);
            continue;
        }

        let mut current_words = vec![line.words[0].clone()];
        let mut current_bbox = line.words[0].bbox;

        for word in line.words.iter().skip(1) {
            let gap = word.bbox.x0 - current_bbox.x1;
            if gap > x_density {
                result.push(TextLine {
                    words: current_words,
                    bbox: current_bbox,
                });
                current_words = vec![word.clone()];
                current_bbox = word.bbox;
            } else {
                current_bbox = current_bbox.union(&word.bbox);
                current_words.push(word.clone());
            }
        }

        result.push(TextLine {
            words: current_words,
            bbox: current_bbox,
        });
    }

    // Re-sort by (top, x0) after splitting
    result.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });

    result
}

/// Cluster text line segments into text blocks based on x-overlap and vertical proximity.
///
/// Line segments that vertically follow each other (gap <= `y_density`) and
/// have overlapping x-ranges are grouped into the same block.
pub fn cluster_lines_into_blocks(lines: Vec<TextLine>, y_density: f64) -> Vec<TextBlock> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut blocks: Vec<TextBlock> = Vec::new();

    for line in lines {
        // Find the best matching block: closest vertically, with x-overlap
        let mut best_block: Option<usize> = None;
        let mut best_gap = f64::INFINITY;

        for (i, block) in blocks.iter().enumerate() {
            let gap = line.bbox.top - block.bbox.bottom;
            if gap >= 0.0
                && gap <= y_density
                && has_x_overlap(&line.bbox, &block.bbox)
                && gap < best_gap
            {
                best_gap = gap;
                best_block = Some(i);
            }
        }

        if let Some(idx) = best_block {
            blocks[idx].bbox = blocks[idx].bbox.union(&line.bbox);
            blocks[idx].lines.push(line);
        } else {
            blocks.push(TextBlock {
                bbox: line.bbox,
                lines: vec![line],
            });
        }
    }

    // Sort lines within each block top-to-bottom
    for block in &mut blocks {
        block
            .lines
            .sort_by(|a, b| a.bbox.top.partial_cmp(&b.bbox.top).unwrap());
    }

    blocks
}

/// Check if two bounding boxes overlap horizontally.
fn has_x_overlap(a: &BBox, b: &BBox) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1
}

/// Sort text blocks in natural reading order.
///
/// Sorts blocks by top position first, then by x0 within the same vertical band.
/// This produces left-to-right, top-to-bottom reading order.
pub fn sort_blocks_reading_order(blocks: &mut [TextBlock], _x_density: f64) {
    blocks.sort_by(|a, b| {
        a.bbox
            .top
            .partial_cmp(&b.bbox.top)
            .unwrap()
            .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
    });
}

/// Detect column boundaries from word x-coordinates.
///
/// Clusters word x-positions to find consistent vertical gaps that indicate
/// column separators. Returns a sorted list of x-coordinate boundaries.
///
/// # Arguments
/// * `words` — All words on the page
/// * `min_column_gap` — Minimum horizontal gap (in points) to detect as a column separator
/// * `max_columns` — Upper limit on the number of columns to detect
pub fn detect_columns(words: &[Word], min_column_gap: f64, max_columns: usize) -> Vec<f64> {
    if words.is_empty() || max_columns <= 1 {
        return Vec::new();
    }

    // Collect all inter-word gaps within each line
    // A column gap should appear consistently across multiple lines
    let mut gap_positions: Vec<(f64, f64)> = Vec::new(); // (gap_start_x, gap_end_x)

    // Group words into lines by y-proximity
    let lines = cluster_words_into_lines(words, 3.0);

    for line in &lines {
        if line.words.len() < 2 {
            continue;
        }
        for pair in line.words.windows(2) {
            let gap_start = pair[0].bbox.x1;
            let gap_end = pair[1].bbox.x0;
            let gap_width = gap_end - gap_start;
            if gap_width >= min_column_gap {
                gap_positions.push((gap_start, gap_end));
            }
        }
    }

    if gap_positions.is_empty() {
        return Vec::new();
    }

    // Cluster gap positions by their midpoint x-coordinate
    gap_positions.sort_by(|a, b| {
        let mid_a = (a.0 + a.1) / 2.0;
        let mid_b = (b.0 + b.1) / 2.0;
        mid_a.partial_cmp(&mid_b).unwrap()
    });

    // Merge gap positions that are close together into column boundaries
    let mut boundaries: Vec<f64> = Vec::new();
    let mut cluster_sum = (gap_positions[0].0 + gap_positions[0].1) / 2.0;
    let mut cluster_count = 1usize;
    let merge_tolerance = min_column_gap;

    for gap in gap_positions.iter().skip(1) {
        let mid = (gap.0 + gap.1) / 2.0;
        let cluster_mid = cluster_sum / cluster_count as f64;
        if (mid - cluster_mid).abs() <= merge_tolerance {
            cluster_sum += mid;
            cluster_count += 1;
        } else {
            // Emit previous cluster
            boundaries.push(cluster_sum / cluster_count as f64);
            cluster_sum = mid;
            cluster_count = 1;
        }
    }
    // Emit last cluster
    boundaries.push(cluster_sum / cluster_count as f64);

    // Limit to max_columns - 1 boundaries
    if boundaries.len() >= max_columns {
        boundaries.truncate(max_columns - 1);
    }

    boundaries
}

/// Sort text blocks in column-aware reading order.
///
/// Detects which blocks are in multi-column regions (blocks that have vertical
/// overlap with blocks in other columns) vs. standalone blocks that act as
/// section separators. Multi-column blocks are sorted by column first, then
/// top-to-bottom within each column. Standalone blocks maintain their natural
/// vertical position relative to multi-column sections.
///
/// # Arguments
/// * `blocks` — Text blocks to sort
/// * `column_boundaries` — Sorted x-coordinates that separate columns
pub fn sort_blocks_column_order(blocks: &mut [TextBlock], column_boundaries: &[f64]) {
    if blocks.is_empty() || column_boundaries.is_empty() {
        // Fall back to default reading order
        blocks.sort_by(|a, b| {
            a.bbox
                .top
                .partial_cmp(&b.bbox.top)
                .unwrap()
                .then(a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap())
        });
        return;
    }

    // Assign each block a column index
    let col_assignments: Vec<usize> = blocks
        .iter()
        .map(|block| column_index(block.bbox.x0, column_boundaries))
        .collect();

    // Determine which blocks are in multi-column regions.
    // A block is in a multi-column region if some block in a different column
    // has vertical overlap with it.
    let n = blocks.len();
    let mut in_multicolumn = vec![false; n];
    for i in 0..n {
        for j in (i + 1)..n {
            if col_assignments[i] != col_assignments[j]
                && blocks[i].bbox.top < blocks[j].bbox.bottom
                && blocks[j].bbox.top < blocks[i].bbox.bottom
            {
                in_multicolumn[i] = true;
                in_multicolumn[j] = true;
            }
        }
    }

    // Sort indices by vertical position to establish scan order
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        blocks[a]
            .bbox
            .top
            .partial_cmp(&blocks[b].bbox.top)
            .unwrap()
            .then(blocks[a].bbox.x0.partial_cmp(&blocks[b].bbox.x0).unwrap())
    });

    // Walk blocks in vertical order and group into sections.
    // Multi-column blocks form contiguous sections; standalone blocks are
    // each their own section.
    let mut sections: Vec<Vec<usize>> = Vec::new();
    let mut current_section: Vec<usize> = Vec::new();
    let mut current_is_multi = false;

    for &idx in &indices {
        if current_section.is_empty() {
            current_section.push(idx);
            current_is_multi = in_multicolumn[idx];
        } else if in_multicolumn[idx] && current_is_multi {
            // Continue multi-column section
            current_section.push(idx);
        } else if !in_multicolumn[idx] && !current_is_multi {
            // Each standalone block is its own section
            sections.push(current_section);
            current_section = vec![idx];
        } else {
            // Type changed — start new section
            sections.push(current_section);
            current_section = vec![idx];
            current_is_multi = in_multicolumn[idx];
        }
    }
    if !current_section.is_empty() {
        sections.push(current_section);
    }

    // Within multi-column sections, sort by (column, top)
    for section in &mut sections {
        if section.len() > 1 && section.iter().any(|&i| in_multicolumn[i]) {
            section.sort_by(|&a, &b| {
                col_assignments[a]
                    .cmp(&col_assignments[b])
                    .then(blocks[a].bbox.top.partial_cmp(&blocks[b].bbox.top).unwrap())
            });
        }
    }

    // Flatten sections into final order
    let final_order: Vec<usize> = sections.into_iter().flatten().collect();

    // Reorder blocks
    let original: Vec<TextBlock> = blocks.to_vec();
    for (dest, &src) in final_order.iter().enumerate() {
        blocks[dest] = original[src].clone();
    }
}

/// Determine which column a given x-coordinate falls into.
fn column_index(x: f64, boundaries: &[f64]) -> usize {
    for (i, &boundary) in boundaries.iter().enumerate() {
        if x < boundary {
            return i;
        }
    }
    boundaries.len()
}

/// Convert text blocks into a string.
///
/// Words within a line are joined by spaces.
/// Lines within a block are joined by newlines.
/// Blocks are separated by double newlines.
pub fn blocks_to_text(blocks: &[TextBlock]) -> String {
    blocks
        .iter()
        .map(|block| {
            block
                .lines
                .iter()
                .map(|line| {
                    line.words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Simple (non-layout) text extraction from words.
///
/// Clusters words into lines by y-proximity, then joins with spaces/newlines.
pub fn words_to_text(words: &[Word], y_tolerance: f64) -> String {
    let lines = cluster_words_into_lines(words, y_tolerance);
    lines
        .iter()
        .map(|line| {
            line.words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Char;

    fn make_word(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Word {
        Word {
            text: text.to_string(),
            bbox: BBox::new(x0, top, x1, bottom),
            doctop: top,
            direction: crate::text::TextDirection::Ltr,
            chars: vec![Char {
                text: text.to_string(),
                bbox: BBox::new(x0, top, x1, bottom),
                fontname: "TestFont".to_string(),
                size: 12.0,
                doctop: top,
                upright: true,
                direction: crate::text::TextDirection::Ltr,
                stroking_color: None,
                non_stroking_color: None,
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                char_code: 0,
                mcid: None,
                tag: None,
            }],
        }
    }

    // --- TextOptions ---

    #[test]
    fn test_text_options_default() {
        let opts = TextOptions::default();
        assert!(!opts.layout);
        assert_eq!(opts.y_tolerance, 3.0);
        assert_eq!(opts.y_density, 10.0);
        assert_eq!(opts.x_density, 10.0);
    }

    #[test]
    fn test_text_options_layout_true() {
        let opts = TextOptions {
            layout: true,
            ..TextOptions::default()
        };
        assert!(opts.layout);
    }

    // --- cluster_words_into_lines ---

    #[test]
    fn test_cluster_empty_words() {
        let lines = cluster_words_into_lines(&[], 3.0);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_cluster_single_word() {
        let words = vec![make_word("Hello", 10.0, 100.0, 50.0, 112.0)];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].words.len(), 1);
        assert_eq!(lines[0].words[0].text, "Hello");
        assert_eq!(lines[0].bbox, BBox::new(10.0, 100.0, 50.0, 112.0));
    }

    #[test]
    fn test_cluster_words_same_line() {
        let words = vec![
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
            make_word("World", 55.0, 100.0, 95.0, 112.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[0].text, "Hello");
        assert_eq!(lines[0].words[1].text, "World");
    }

    #[test]
    fn test_cluster_words_different_lines() {
        let words = vec![
            make_word("Line1", 10.0, 100.0, 50.0, 112.0),
            make_word("Line2", 10.0, 120.0, 50.0, 132.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words[0].text, "Line1");
        assert_eq!(lines[1].words[0].text, "Line2");
    }

    #[test]
    fn test_cluster_words_slight_y_variation() {
        // Words on "same line" but slightly different y positions (within tolerance)
        let words = vec![
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
            make_word("World", 55.0, 101.0, 95.0, 113.0), // 1pt y-offset
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].words.len(), 2);
    }

    #[test]
    fn test_cluster_words_sorted_left_to_right_within_line() {
        // Words given in reverse x-order
        let words = vec![
            make_word("World", 55.0, 100.0, 95.0, 112.0),
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines[0].words[0].text, "Hello");
        assert_eq!(lines[0].words[1].text, "World");
    }

    #[test]
    fn test_cluster_three_lines() {
        let words = vec![
            make_word("First", 10.0, 100.0, 50.0, 112.0),
            make_word("line", 55.0, 100.0, 85.0, 112.0),
            make_word("Second", 10.0, 120.0, 60.0, 132.0),
            make_word("line", 65.0, 120.0, 95.0, 132.0),
            make_word("Third", 10.0, 140.0, 50.0, 152.0),
            make_word("line", 55.0, 140.0, 85.0, 152.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[1].words.len(), 2);
        assert_eq!(lines[2].words.len(), 2);
    }

    #[test]
    fn test_cluster_line_bbox_is_union() {
        let words = vec![
            make_word("A", 10.0, 98.0, 20.0, 112.0),
            make_word("B", 25.0, 100.0, 35.0, 110.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines[0].bbox, BBox::new(10.0, 98.0, 35.0, 112.0));
    }

    // --- cluster_lines_into_blocks ---

    #[test]
    fn test_cluster_lines_empty() {
        let blocks = cluster_lines_into_blocks(vec![], 10.0);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_cluster_lines_single_block() {
        let lines = vec![
            TextLine {
                words: vec![make_word("Line1", 10.0, 100.0, 50.0, 112.0)],
                bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
            },
            TextLine {
                words: vec![make_word("Line2", 10.0, 115.0, 50.0, 127.0)],
                bbox: BBox::new(10.0, 115.0, 50.0, 127.0),
            },
        ];
        let blocks = cluster_lines_into_blocks(lines, 10.0);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].lines.len(), 2);
        assert_eq!(blocks[0].bbox, BBox::new(10.0, 100.0, 50.0, 127.0));
    }

    #[test]
    fn test_cluster_lines_two_blocks() {
        let lines = vec![
            TextLine {
                words: vec![make_word("Block1", 10.0, 100.0, 60.0, 112.0)],
                bbox: BBox::new(10.0, 100.0, 60.0, 112.0),
            },
            TextLine {
                words: vec![make_word("Still1", 10.0, 115.0, 60.0, 127.0)],
                bbox: BBox::new(10.0, 115.0, 60.0, 127.0),
            },
            // Large gap (127 to 200 = 73pt gap, >> 10.0)
            TextLine {
                words: vec![make_word("Block2", 10.0, 200.0, 60.0, 212.0)],
                bbox: BBox::new(10.0, 200.0, 60.0, 212.0),
            },
        ];
        let blocks = cluster_lines_into_blocks(lines, 10.0);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].lines.len(), 2);
        assert_eq!(blocks[1].lines.len(), 1);
    }

    #[test]
    fn test_cluster_lines_block_bbox() {
        let lines = vec![
            TextLine {
                words: vec![make_word("Line1", 10.0, 100.0, 80.0, 112.0)],
                bbox: BBox::new(10.0, 100.0, 80.0, 112.0),
            },
            TextLine {
                words: vec![make_word("Line2", 5.0, 115.0, 90.0, 127.0)],
                bbox: BBox::new(5.0, 115.0, 90.0, 127.0),
            },
        ];
        let blocks = cluster_lines_into_blocks(lines, 10.0);
        assert_eq!(blocks[0].bbox, BBox::new(5.0, 100.0, 90.0, 127.0));
    }

    // --- sort_blocks_reading_order ---

    #[test]
    fn test_sort_single_column_top_to_bottom() {
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Second", 10.0, 200.0, 60.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 60.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 60.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("First", 10.0, 100.0, 60.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 60.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 60.0, 112.0),
            },
        ];
        sort_blocks_reading_order(&mut blocks, 10.0);
        assert_eq!(blocks[0].lines[0].words[0].text, "First");
        assert_eq!(blocks[1].lines[0].words[0].text, "Second");
    }

    #[test]
    fn test_sort_two_columns() {
        // Left column at x=10..100, right column at x=200..300
        // Blocks at different y-levels: sorts by (top, x0)
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right1", 200.0, 100.0, 300.0, 112.0)],
                    bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
                }],
                bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left1", 10.0, 100.0, 100.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right2", 200.0, 200.0, 300.0, 212.0)],
                    bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
                }],
                bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left2", 10.0, 200.0, 100.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
            },
        ];
        sort_blocks_reading_order(&mut blocks, 10.0);
        // Reading order: top-to-bottom, left-to-right within same y-level
        assert_eq!(blocks[0].lines[0].words[0].text, "Left1");
        assert_eq!(blocks[1].lines[0].words[0].text, "Right1");
        assert_eq!(blocks[2].lines[0].words[0].text, "Left2");
        assert_eq!(blocks[3].lines[0].words[0].text, "Right2");
    }

    #[test]
    fn test_sort_single_block_unchanged() {
        let mut blocks = vec![TextBlock {
            lines: vec![TextLine {
                words: vec![make_word("Only", 10.0, 100.0, 50.0, 112.0)],
                bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
            }],
            bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
        }];
        sort_blocks_reading_order(&mut blocks, 10.0);
        assert_eq!(blocks[0].lines[0].words[0].text, "Only");
    }

    // --- blocks_to_text ---

    #[test]
    fn test_blocks_to_text_single_block_single_line() {
        let blocks = vec![TextBlock {
            lines: vec![TextLine {
                words: vec![
                    make_word("Hello", 10.0, 100.0, 50.0, 112.0),
                    make_word("World", 55.0, 100.0, 95.0, 112.0),
                ],
                bbox: BBox::new(10.0, 100.0, 95.0, 112.0),
            }],
            bbox: BBox::new(10.0, 100.0, 95.0, 112.0),
        }];
        assert_eq!(blocks_to_text(&blocks), "Hello World");
    }

    #[test]
    fn test_blocks_to_text_single_block_multi_line() {
        let blocks = vec![TextBlock {
            lines: vec![
                TextLine {
                    words: vec![make_word("Line1", 10.0, 100.0, 50.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
                },
                TextLine {
                    words: vec![make_word("Line2", 10.0, 115.0, 50.0, 127.0)],
                    bbox: BBox::new(10.0, 115.0, 50.0, 127.0),
                },
            ],
            bbox: BBox::new(10.0, 100.0, 50.0, 127.0),
        }];
        assert_eq!(blocks_to_text(&blocks), "Line1\nLine2");
    }

    #[test]
    fn test_blocks_to_text_two_blocks() {
        let blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Block1", 10.0, 100.0, 60.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 60.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 60.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Block2", 10.0, 200.0, 60.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 60.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 60.0, 212.0),
            },
        ];
        assert_eq!(blocks_to_text(&blocks), "Block1\n\nBlock2");
    }

    #[test]
    fn test_blocks_to_text_empty() {
        assert_eq!(blocks_to_text(&[]), "");
    }

    // --- words_to_text ---

    #[test]
    fn test_words_to_text_single_line() {
        let words = vec![
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
            make_word("World", 55.0, 100.0, 95.0, 112.0),
        ];
        assert_eq!(words_to_text(&words, 3.0), "Hello World");
    }

    #[test]
    fn test_words_to_text_multi_line() {
        let words = vec![
            make_word("Line1", 10.0, 100.0, 50.0, 112.0),
            make_word("Line2", 10.0, 120.0, 50.0, 132.0),
        ];
        assert_eq!(words_to_text(&words, 3.0), "Line1\nLine2");
    }

    #[test]
    fn test_words_to_text_empty() {
        assert_eq!(words_to_text(&[], 3.0), "");
    }

    // --- split_lines_at_columns ---

    #[test]
    fn test_split_lines_no_columns() {
        let lines = vec![TextLine {
            words: vec![
                make_word("Hello", 10.0, 100.0, 50.0, 112.0),
                make_word("World", 55.0, 100.0, 95.0, 112.0),
            ],
            bbox: BBox::new(10.0, 100.0, 95.0, 112.0),
        }];
        let result = split_lines_at_columns(lines, 50.0);
        assert_eq!(result.len(), 1); // gap=5 < x_density=50
    }

    #[test]
    fn test_split_lines_with_column_gap() {
        let lines = vec![TextLine {
            words: vec![
                make_word("Left", 10.0, 100.0, 50.0, 112.0),
                make_word("Right", 200.0, 100.0, 250.0, 112.0),
            ],
            bbox: BBox::new(10.0, 100.0, 250.0, 112.0),
        }];
        let result = split_lines_at_columns(lines, 10.0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].words[0].text, "Left");
        assert_eq!(result[1].words[0].text, "Right");
    }

    #[test]
    fn test_split_lines_single_word_line() {
        let lines = vec![TextLine {
            words: vec![make_word("Only", 10.0, 100.0, 50.0, 112.0)],
            bbox: BBox::new(10.0, 100.0, 50.0, 112.0),
        }];
        let result = split_lines_at_columns(lines, 10.0);
        assert_eq!(result.len(), 1);
    }

    // --- End-to-end layout tests ---

    #[test]
    fn test_end_to_end_single_column() {
        // Two paragraphs in a single column
        let words = vec![
            make_word("Para1", 10.0, 100.0, 50.0, 112.0),
            make_word("line1", 55.0, 100.0, 90.0, 112.0),
            make_word("Para1", 10.0, 115.0, 50.0, 127.0),
            make_word("line2", 55.0, 115.0, 90.0, 127.0),
            // Large gap
            make_word("Para2", 10.0, 200.0, 50.0, 212.0),
            make_word("line1", 55.0, 200.0, 90.0, 212.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_reading_order(&mut blocks, 10.0);
        let text = blocks_to_text(&blocks);

        assert_eq!(text, "Para1 line1\nPara1 line2\n\nPara2 line1");
    }

    #[test]
    fn test_end_to_end_two_column_layout() {
        // Left column at x=10..60, right column at x=200..260
        // Each column has 2 lines
        let words = vec![
            // Left column
            make_word("Left", 10.0, 100.0, 40.0, 112.0),
            make_word("L1", 45.0, 100.0, 60.0, 112.0),
            make_word("Left", 10.0, 115.0, 40.0, 127.0),
            make_word("L2", 45.0, 115.0, 60.0, 127.0),
            // Right column
            make_word("Right", 200.0, 100.0, 240.0, 112.0),
            make_word("R1", 245.0, 100.0, 260.0, 112.0),
            make_word("Right", 200.0, 115.0, 240.0, 127.0),
            make_word("R2", 245.0, 115.0, 260.0, 127.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_reading_order(&mut blocks, 10.0);
        let text = blocks_to_text(&blocks);

        // Left column block first (top=100), then right column block (top=100)
        // Both start at same y, sorted left-to-right
        assert_eq!(text, "Left L1\nLeft L2\n\nRight R1\nRight R2");
    }

    #[test]
    fn test_end_to_end_mixed_blocks() {
        // Full-width header, then two columns, then full-width footer
        let words = vec![
            // Header (full width)
            make_word("Header", 10.0, 50.0, 100.0, 62.0),
            // Left column
            make_word("Left", 10.0, 100.0, 50.0, 112.0),
            // Right column
            make_word("Right", 200.0, 100.0, 250.0, 112.0),
            // Footer (full width)
            make_word("Footer", 10.0, 250.0, 100.0, 262.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_reading_order(&mut blocks, 10.0);
        let text = blocks_to_text(&blocks);

        // Header, Left, Right, Footer
        assert_eq!(text, "Header\n\nLeft\n\nRight\n\nFooter");
    }

    #[test]
    fn test_reading_order_top_to_bottom_left_to_right() {
        // Verify blocks are in proper reading order
        let words = vec![
            make_word("C", 10.0, 300.0, 50.0, 312.0),
            make_word("A", 10.0, 100.0, 50.0, 112.0),
            make_word("B", 10.0, 200.0, 50.0, 212.0),
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_reading_order(&mut blocks, 10.0);
        let text = blocks_to_text(&blocks);

        assert_eq!(text, "A\n\nB\n\nC");
    }

    // --- ColumnMode and TextOptions column fields ---

    #[test]
    fn test_text_options_default_column_mode() {
        let opts = TextOptions::default();
        assert_eq!(opts.column_mode, ColumnMode::None);
        assert_eq!(opts.min_column_gap, 20.0);
        assert_eq!(opts.max_columns, 6);
    }

    #[test]
    fn test_column_mode_auto() {
        let opts = TextOptions {
            column_mode: ColumnMode::Auto,
            ..TextOptions::default()
        };
        assert_eq!(opts.column_mode, ColumnMode::Auto);
    }

    #[test]
    fn test_column_mode_explicit() {
        let opts = TextOptions {
            column_mode: ColumnMode::Explicit(vec![300.0]),
            ..TextOptions::default()
        };
        match &opts.column_mode {
            ColumnMode::Explicit(boundaries) => {
                assert_eq!(boundaries, &[300.0]);
            }
            _ => panic!("expected Explicit"),
        }
    }

    // --- detect_columns ---

    #[test]
    fn test_detect_columns_empty_words() {
        let boundaries = detect_columns(&[], 20.0, 6);
        assert!(boundaries.is_empty());
    }

    #[test]
    fn test_detect_columns_single_column() {
        // All words in one column — no large gaps
        let words = vec![
            make_word("Hello", 10.0, 100.0, 50.0, 112.0),
            make_word("World", 55.0, 100.0, 95.0, 112.0),
            make_word("Foo", 10.0, 120.0, 40.0, 132.0),
            make_word("Bar", 45.0, 120.0, 80.0, 132.0),
        ];
        let boundaries = detect_columns(&words, 20.0, 6);
        assert!(
            boundaries.is_empty(),
            "single column should have no boundaries"
        );
    }

    #[test]
    fn test_detect_columns_two_columns() {
        // Two columns with a large gap at x~130
        let words = vec![
            // Left column: x=10..100
            make_word("Left1", 10.0, 100.0, 50.0, 112.0),
            make_word("word1", 55.0, 100.0, 100.0, 112.0),
            make_word("Left2", 10.0, 120.0, 50.0, 132.0),
            make_word("word2", 55.0, 120.0, 100.0, 132.0),
            make_word("Left3", 10.0, 140.0, 50.0, 152.0),
            make_word("word3", 55.0, 140.0, 100.0, 152.0),
            // Right column: x=200..300
            make_word("Right1", 200.0, 100.0, 250.0, 112.0),
            make_word("rword1", 255.0, 100.0, 300.0, 112.0),
            make_word("Right2", 200.0, 120.0, 250.0, 132.0),
            make_word("rword2", 255.0, 120.0, 300.0, 132.0),
            make_word("Right3", 200.0, 140.0, 250.0, 152.0),
            make_word("rword3", 255.0, 140.0, 300.0, 152.0),
        ];
        let boundaries = detect_columns(&words, 20.0, 6);
        assert_eq!(boundaries.len(), 1, "should detect one column boundary");
        // Boundary should be around x=150 (midpoint of gap from 100 to 200)
        assert!(
            boundaries[0] > 100.0 && boundaries[0] < 200.0,
            "boundary {} should be between columns",
            boundaries[0]
        );
    }

    #[test]
    fn test_detect_columns_three_columns() {
        // Three columns
        let words = vec![
            // Column 1: x=10..80
            make_word("A1", 10.0, 100.0, 40.0, 112.0),
            make_word("a1", 45.0, 100.0, 80.0, 112.0),
            make_word("A2", 10.0, 120.0, 40.0, 132.0),
            make_word("a2", 45.0, 120.0, 80.0, 132.0),
            make_word("A3", 10.0, 140.0, 40.0, 152.0),
            make_word("a3", 45.0, 140.0, 80.0, 152.0),
            // Column 2: x=150..220
            make_word("B1", 150.0, 100.0, 180.0, 112.0),
            make_word("b1", 185.0, 100.0, 220.0, 112.0),
            make_word("B2", 150.0, 120.0, 180.0, 132.0),
            make_word("b2", 185.0, 120.0, 220.0, 132.0),
            make_word("B3", 150.0, 140.0, 180.0, 152.0),
            make_word("b3", 185.0, 140.0, 220.0, 152.0),
            // Column 3: x=290..360
            make_word("C1", 290.0, 100.0, 320.0, 112.0),
            make_word("c1", 325.0, 100.0, 360.0, 112.0),
            make_word("C2", 290.0, 120.0, 320.0, 132.0),
            make_word("c2", 325.0, 120.0, 360.0, 132.0),
            make_word("C3", 290.0, 140.0, 320.0, 152.0),
            make_word("c3", 325.0, 140.0, 360.0, 152.0),
        ];
        let boundaries = detect_columns(&words, 20.0, 6);
        assert_eq!(boundaries.len(), 2, "should detect two column boundaries");
        assert!(
            boundaries[0] > 80.0 && boundaries[0] < 150.0,
            "first boundary {} should be between col1 and col2",
            boundaries[0]
        );
        assert!(
            boundaries[1] > 220.0 && boundaries[1] < 290.0,
            "second boundary {} should be between col2 and col3",
            boundaries[1]
        );
    }

    #[test]
    fn test_detect_columns_max_columns_limit() {
        // Three-column layout but max_columns=2 should return at most 1 boundary
        let words = vec![
            make_word("A", 10.0, 100.0, 40.0, 112.0),
            make_word("B", 150.0, 100.0, 180.0, 112.0),
            make_word("C", 290.0, 100.0, 320.0, 112.0),
            make_word("A", 10.0, 120.0, 40.0, 132.0),
            make_word("B", 150.0, 120.0, 180.0, 132.0),
            make_word("C", 290.0, 120.0, 320.0, 132.0),
            make_word("A", 10.0, 140.0, 40.0, 152.0),
            make_word("B", 150.0, 140.0, 180.0, 152.0),
            make_word("C", 290.0, 140.0, 320.0, 152.0),
        ];
        let boundaries = detect_columns(&words, 20.0, 2);
        assert!(
            boundaries.len() <= 1,
            "max_columns=2 should produce at most 1 boundary"
        );
    }

    #[test]
    fn test_detect_columns_max_columns_one_returns_empty() {
        let words = vec![
            make_word("A", 10.0, 100.0, 40.0, 112.0),
            make_word("B", 200.0, 100.0, 240.0, 112.0),
        ];
        let boundaries = detect_columns(&words, 20.0, 1);
        assert!(
            boundaries.is_empty(),
            "max_columns=1 should return no boundaries"
        );
    }

    // --- sort_blocks_column_order ---

    #[test]
    fn test_column_order_two_columns() {
        // Two-column layout: blocks at Left and Right at same y-levels
        // With column-aware sort, all Left blocks come before all Right blocks
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left1", 10.0, 100.0, 100.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right1", 200.0, 100.0, 300.0, 112.0)],
                    bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
                }],
                bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left2", 10.0, 200.0, 100.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right2", 200.0, 200.0, 300.0, 212.0)],
                    bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
                }],
                bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
            },
        ];

        let boundaries = vec![150.0]; // column boundary at x=150
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Column-aware: Left1, Left2, Right1, Right2
        assert_eq!(text, "Left1\n\nLeft2\n\nRight1\n\nRight2");
    }

    #[test]
    fn test_column_order_three_columns() {
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("A1", 10.0, 100.0, 80.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 80.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 80.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("B1", 150.0, 100.0, 220.0, 112.0)],
                    bbox: BBox::new(150.0, 100.0, 220.0, 112.0),
                }],
                bbox: BBox::new(150.0, 100.0, 220.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("C1", 290.0, 100.0, 360.0, 112.0)],
                    bbox: BBox::new(290.0, 100.0, 360.0, 112.0),
                }],
                bbox: BBox::new(290.0, 100.0, 360.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("A2", 10.0, 200.0, 80.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 80.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 80.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("B2", 150.0, 200.0, 220.0, 212.0)],
                    bbox: BBox::new(150.0, 200.0, 220.0, 212.0),
                }],
                bbox: BBox::new(150.0, 200.0, 220.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("C2", 290.0, 200.0, 360.0, 212.0)],
                    bbox: BBox::new(290.0, 200.0, 360.0, 212.0),
                }],
                bbox: BBox::new(290.0, 200.0, 360.0, 212.0),
            },
        ];

        let boundaries = vec![120.0, 260.0];
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Column order: A1, A2, B1, B2, C1, C2
        assert_eq!(text, "A1\n\nA2\n\nB1\n\nB2\n\nC1\n\nC2");
    }

    #[test]
    fn test_column_order_full_width_heading_not_split() {
        // Full-width heading spans both columns — should not be split
        // It should appear first, then left column content, then right column content
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Full-Width Heading", 10.0, 50.0, 300.0, 62.0)],
                    bbox: BBox::new(10.0, 50.0, 300.0, 62.0),
                }],
                bbox: BBox::new(10.0, 50.0, 300.0, 62.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left1", 10.0, 100.0, 100.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right1", 200.0, 100.0, 300.0, 112.0)],
                    bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
                }],
                bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Left2", 10.0, 200.0, 100.0, 212.0)],
                    bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
                }],
                bbox: BBox::new(10.0, 200.0, 100.0, 212.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("Right2", 200.0, 200.0, 300.0, 212.0)],
                    bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
                }],
                bbox: BBox::new(200.0, 200.0, 300.0, 212.0),
            },
        ];

        let boundaries = vec![150.0];
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Heading first, then Left column, then Right column
        assert_eq!(
            text,
            "Full-Width Heading\n\nLeft1\n\nLeft2\n\nRight1\n\nRight2"
        );
    }

    #[test]
    fn test_column_order_no_boundaries_falls_back() {
        // When no boundaries provided, should fall back to default order
        let mut blocks = vec![
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("B", 200.0, 100.0, 300.0, 112.0)],
                    bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
                }],
                bbox: BBox::new(200.0, 100.0, 300.0, 112.0),
            },
            TextBlock {
                lines: vec![TextLine {
                    words: vec![make_word("A", 10.0, 100.0, 100.0, 112.0)],
                    bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
                }],
                bbox: BBox::new(10.0, 100.0, 100.0, 112.0),
            },
        ];

        sort_blocks_column_order(&mut blocks, &[]);
        // Falls back to (top, x0) order
        assert_eq!(blocks[0].lines[0].words[0].text, "A");
        assert_eq!(blocks[1].lines[0].words[0].text, "B");
    }

    // --- End-to-end column-aware layout tests ---

    #[test]
    fn test_end_to_end_two_column_auto_detection() {
        // Two-column layout with auto detection
        let words = vec![
            // Left column: x=10..100
            make_word("Left", 10.0, 100.0, 50.0, 112.0),
            make_word("L1", 55.0, 100.0, 100.0, 112.0),
            make_word("Left", 10.0, 120.0, 50.0, 132.0),
            make_word("L2", 55.0, 120.0, 100.0, 132.0),
            make_word("Left", 10.0, 140.0, 50.0, 152.0),
            make_word("L3", 55.0, 140.0, 100.0, 152.0),
            // Right column: x=200..300
            make_word("Right", 200.0, 100.0, 250.0, 112.0),
            make_word("R1", 255.0, 100.0, 300.0, 112.0),
            make_word("Right", 200.0, 120.0, 250.0, 132.0),
            make_word("R2", 255.0, 120.0, 300.0, 132.0),
            make_word("Right", 200.0, 140.0, 250.0, 152.0),
            make_word("R3", 255.0, 140.0, 300.0, 152.0),
        ];

        let boundaries = detect_columns(&words, 20.0, 6);
        assert_eq!(boundaries.len(), 1, "should detect one column boundary");

        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Column-aware: all Left content, then all Right content
        assert_eq!(
            text,
            "Left L1\nLeft L2\nLeft L3\n\nRight R1\nRight R2\nRight R3"
        );
    }

    #[test]
    fn test_end_to_end_three_column_auto_detection() {
        let words = vec![
            // Column 1: x=10..80
            make_word("A1", 10.0, 100.0, 40.0, 112.0),
            make_word("a1", 45.0, 100.0, 80.0, 112.0),
            make_word("A2", 10.0, 120.0, 40.0, 132.0),
            make_word("a2", 45.0, 120.0, 80.0, 132.0),
            make_word("A3", 10.0, 140.0, 40.0, 152.0),
            make_word("a3", 45.0, 140.0, 80.0, 152.0),
            // Column 2: x=150..220
            make_word("B1", 150.0, 100.0, 180.0, 112.0),
            make_word("b1", 185.0, 100.0, 220.0, 112.0),
            make_word("B2", 150.0, 120.0, 180.0, 132.0),
            make_word("b2", 185.0, 120.0, 220.0, 132.0),
            make_word("B3", 150.0, 140.0, 180.0, 152.0),
            make_word("b3", 185.0, 140.0, 220.0, 152.0),
            // Column 3: x=290..360
            make_word("C1", 290.0, 100.0, 320.0, 112.0),
            make_word("c1", 325.0, 100.0, 360.0, 112.0),
            make_word("C2", 290.0, 120.0, 320.0, 132.0),
            make_word("c2", 325.0, 120.0, 360.0, 132.0),
            make_word("C3", 290.0, 140.0, 320.0, 152.0),
            make_word("c3", 325.0, 140.0, 360.0, 152.0),
        ];

        let boundaries = detect_columns(&words, 20.0, 6);
        assert_eq!(boundaries.len(), 2, "should detect two column boundaries");

        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Column-aware: A content, B content, C content
        assert_eq!(
            text,
            "A1 a1\nA2 a2\nA3 a3\n\nB1 b1\nB2 b2\nB3 b3\n\nC1 c1\nC2 c2\nC3 c3"
        );
    }

    #[test]
    fn test_end_to_end_full_width_heading_with_columns() {
        // Full-width heading, then two columns, then full-width footer
        let words = vec![
            // Full-width heading
            make_word("Document", 10.0, 50.0, 80.0, 62.0),
            make_word("Title", 85.0, 50.0, 130.0, 62.0),
            // Left column: x=10..100
            make_word("Left", 10.0, 100.0, 50.0, 112.0),
            make_word("L1", 55.0, 100.0, 100.0, 112.0),
            make_word("Left", 10.0, 120.0, 50.0, 132.0),
            make_word("L2", 55.0, 120.0, 100.0, 132.0),
            make_word("Left", 10.0, 140.0, 50.0, 152.0),
            make_word("L3", 55.0, 140.0, 100.0, 152.0),
            // Right column: x=200..300
            make_word("Right", 200.0, 100.0, 250.0, 112.0),
            make_word("R1", 255.0, 100.0, 300.0, 112.0),
            make_word("Right", 200.0, 120.0, 250.0, 132.0),
            make_word("R2", 255.0, 120.0, 300.0, 132.0),
            make_word("Right", 200.0, 140.0, 250.0, 152.0),
            make_word("R3", 255.0, 140.0, 300.0, 152.0),
            // Full-width footer
            make_word("Footer", 10.0, 250.0, 80.0, 262.0),
            make_word("Text", 85.0, 250.0, 130.0, 262.0),
        ];

        let boundaries = detect_columns(&words, 20.0, 6);
        assert!(!boundaries.is_empty(), "should detect column boundary");

        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        // Full-width heading, then Left column, then Right column, then footer
        assert_eq!(
            text,
            "Document Title\n\nLeft L1\nLeft L2\nLeft L3\n\nRight R1\nRight R2\nRight R3\n\nFooter Text"
        );
    }

    #[test]
    fn test_column_order_explicit_boundaries() {
        // Use explicit column boundaries
        let words = vec![
            // Left column
            make_word("Left1", 10.0, 100.0, 100.0, 112.0),
            make_word("Left2", 10.0, 120.0, 100.0, 132.0),
            // Right column
            make_word("Right1", 200.0, 100.0, 300.0, 112.0),
            make_word("Right2", 200.0, 120.0, 300.0, 132.0),
        ];

        let boundaries = vec![150.0]; // Explicit boundary

        let lines = cluster_words_into_lines(&words, 3.0);
        let split = split_lines_at_columns(lines, 10.0);
        let mut blocks = cluster_lines_into_blocks(split, 10.0);
        sort_blocks_column_order(&mut blocks, &boundaries);
        let text = blocks_to_text(&blocks);

        assert_eq!(text, "Left1\nLeft2\n\nRight1\nRight2");
    }
}
