use std::collections::HashMap;

use crate::geometry::BBox;
use crate::text::TextDirection;
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
///
/// Uses y-coordinate bucketing for O(n log n) performance instead of O(n²).
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
    // Map from quantized y-bucket to line index. Each line is registered
    // in the bucket corresponding to its current mid_y. When a line's
    // bbox grows (union with a new word), its bucket registration is updated.
    let mut bucket_to_line: HashMap<i64, Vec<usize>> = HashMap::new();

    let bucket_size = if y_tolerance > 0.0 {
        y_tolerance
    } else {
        // For zero tolerance, use a very small bucket size
        1e-9
    };

    for word in sorted {
        let word_mid_y = (word.bbox.top + word.bbox.bottom) / 2.0;
        let word_bucket = (word_mid_y / bucket_size).floor() as i64;

        // Check adjacent buckets (word_bucket - 1, word_bucket, word_bucket + 1)
        // to find a matching line within y_tolerance
        let mut matched_line_idx: Option<usize> = None;
        'outer: for delta in [-1i64, 0, 1] {
            let check_bucket = word_bucket + delta;
            if let Some(line_indices) = bucket_to_line.get(&check_bucket) {
                for &line_idx in line_indices {
                    let line = &lines[line_idx];
                    let line_mid_y = (line.bbox.top + line.bbox.bottom) / 2.0;
                    if (word_mid_y - line_mid_y).abs() <= y_tolerance {
                        matched_line_idx = Some(line_idx);
                        break 'outer;
                    }
                }
            }
        }

        if let Some(idx) = matched_line_idx {
            // Remove old bucket registration for this line
            let old_mid_y = (lines[idx].bbox.top + lines[idx].bbox.bottom) / 2.0;
            let old_bucket = (old_mid_y / bucket_size).floor() as i64;

            // Update the line
            lines[idx].bbox = lines[idx].bbox.union(&word.bbox);
            lines[idx].words.push(word.clone());

            // Re-register in the new bucket if mid_y changed
            let new_mid_y = (lines[idx].bbox.top + lines[idx].bbox.bottom) / 2.0;
            let new_bucket = (new_mid_y / bucket_size).floor() as i64;
            if new_bucket != old_bucket {
                if let Some(indices) = bucket_to_line.get_mut(&old_bucket) {
                    indices.retain(|&i| i != idx);
                }
                bucket_to_line.entry(new_bucket).or_default().push(idx);
            }
        } else {
            let new_idx = lines.len();
            let mid_y = (word.bbox.top + word.bbox.bottom) / 2.0;
            let bucket = (mid_y / bucket_size).floor() as i64;
            lines.push(TextLine {
                words: vec![word.clone()],
                bbox: word.bbox,
            });
            bucket_to_line.entry(bucket).or_default().push(new_idx);
        }
    }

    // Sort words within each line by reading direction.
    // For Rtl lines (e.g., 180° rotated text), sort right-to-left.
    for line in &mut lines {
        let rtl_count = line
            .words
            .iter()
            .filter(|w| w.direction == TextDirection::Rtl)
            .count();
        if rtl_count > line.words.len() / 2 {
            // Majority Rtl: sort by x0 descending (right-to-left)
            line.words
                .sort_by(|a, b| b.bbox.x0.partial_cmp(&a.bbox.x0).unwrap());
        } else {
            // Default Ltr: sort by x0 ascending (left-to-right)
            line.words
                .sort_by(|a, b| a.bbox.x0.partial_cmp(&b.bbox.x0).unwrap());
        }
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
mod tests;
