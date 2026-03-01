use std::collections::HashMap;

use crate::geometry::BBox;
use crate::words::Word;

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
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            layout: false,
            y_tolerance: 3.0,
            y_density: 10.0,
            x_density: 10.0,
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

    // --- Benchmark and edge case tests for US-152-1 ---

    #[test]
    fn test_cluster_all_words_on_same_line() {
        // All words have the same y-coordinate — should produce a single line
        let words: Vec<Word> = (0..100)
            .map(|i| {
                let x0 = i as f64 * 20.0;
                make_word(&format!("w{i}"), x0, 100.0, x0 + 15.0, 112.0)
            })
            .collect();
        let lines = cluster_words_into_lines(&words, 3.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].words.len(), 100);
        // Words should be sorted left-to-right
        for i in 1..lines[0].words.len() {
            assert!(lines[0].words[i].bbox.x0 > lines[0].words[i - 1].bbox.x0);
        }
    }

    #[test]
    fn test_cluster_overlapping_y_ranges() {
        // Words with overlapping y ranges that straddle bucket boundaries
        // Word A: mid_y = 106, Word B: mid_y = 108.5 (diff = 2.5, within tolerance 3.0)
        // Word C: mid_y = 111.5 (diff from B = 3.0, at boundary)
        let words = vec![
            make_word("A", 10.0, 100.0, 50.0, 112.0),   // mid_y = 106
            make_word("B", 60.0, 102.5, 100.0, 114.5),  // mid_y = 108.5
            make_word("C", 110.0, 105.5, 150.0, 117.5), // mid_y = 111.5
        ];
        let lines = cluster_words_into_lines(&words, 3.0);
        // A and B are within tolerance, B and C are exactly at tolerance boundary
        // The original algorithm processes sorted by (top, x0): A first, then B joins A's line,
        // then C checks A's line (line mid_y evolves as union grows).
        // After A+B: line bbox = (10, 100, 100, 114.5), line mid_y = 107.25
        // C mid_y = 111.5, |111.5 - 107.25| = 4.25 > 3.0 → C becomes new line
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[0].text, "A");
        assert_eq!(lines[0].words[1].text, "B");
        assert_eq!(lines[1].words[0].text, "C");
    }

    #[test]
    fn test_cluster_large_y_tolerance() {
        // With a very large tolerance, all words should merge into one line
        let words = vec![
            make_word("Top", 10.0, 100.0, 50.0, 112.0),
            make_word("Mid", 10.0, 150.0, 50.0, 162.0),
            make_word("Bot", 10.0, 200.0, 50.0, 212.0),
        ];
        let lines = cluster_words_into_lines(&words, 200.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].words.len(), 3);
    }

    #[test]
    fn test_cluster_zero_y_tolerance() {
        // With zero tolerance, only words with identical mid_y merge
        let words = vec![
            make_word("A", 10.0, 100.0, 50.0, 112.0),  // mid_y = 106
            make_word("B", 60.0, 100.0, 100.0, 112.0), // mid_y = 106 (same)
            make_word("C", 10.0, 100.1, 50.0, 112.1),  // mid_y = 106.1 (different)
        ];
        let lines = cluster_words_into_lines(&words, 0.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words.len(), 2); // A and B
        assert_eq!(lines[1].words.len(), 1); // C
    }

    #[test]
    fn test_cluster_benchmark_10k_words_many_lines() {
        // Benchmark: 10,000 words across 500 lines (20 words per line)
        // This test verifies correctness and that the function completes
        // in reasonable time (sub-quadratic behavior).
        let words_per_line = 20;
        let num_lines = 500;
        let total_words = words_per_line * num_lines;

        let mut words = Vec::with_capacity(total_words);
        for line_idx in 0..num_lines {
            let top = line_idx as f64 * 20.0;
            let bottom = top + 12.0;
            for word_idx in 0..words_per_line {
                let x0 = word_idx as f64 * 30.0;
                let x1 = x0 + 25.0;
                words.push(make_word(
                    &format!("L{line_idx}W{word_idx}"),
                    x0,
                    top,
                    x1,
                    bottom,
                ));
            }
        }
        assert_eq!(words.len(), total_words);

        let start = std::time::Instant::now();
        let lines = cluster_words_into_lines(&words, 3.0);
        let elapsed = start.elapsed();

        // Correctness checks
        assert_eq!(lines.len(), num_lines);
        for line in &lines {
            assert_eq!(line.words.len(), words_per_line);
        }
        // Lines should be sorted top-to-bottom
        for i in 1..lines.len() {
            assert!(lines[i].bbox.top >= lines[i - 1].bbox.top);
        }
        // Words within each line should be sorted left-to-right
        for line in &lines {
            for i in 1..line.words.len() {
                assert!(line.words[i].bbox.x0 >= line.words[i - 1].bbox.x0);
            }
        }

        // Performance check: should complete well under 1 second for 10k words
        // with O(n) or O(n log n). The old O(n²) would be significantly slower
        // on much larger inputs, but 10k should still be fast enough for both.
        // This serves as a regression guard.
        assert!(
            elapsed.as_millis() < 5000,
            "cluster_words_into_lines took {}ms for {total_words} words — expected sub-quadratic",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_cluster_benchmark_scaling_sub_quadratic() {
        // Verify sub-quadratic scaling by comparing time for N and 4N words.
        // O(n²) would take ~16x longer for 4x the input.
        // O(n log n) would take ~4.5x longer.
        // O(n) would take ~4x longer.
        // We check that 4N takes less than 10x of N (generous margin).
        let build_words = |num_lines: usize, words_per_line: usize| -> Vec<Word> {
            let mut words = Vec::with_capacity(num_lines * words_per_line);
            for line_idx in 0..num_lines {
                let top = line_idx as f64 * 20.0;
                let bottom = top + 12.0;
                for word_idx in 0..words_per_line {
                    let x0 = word_idx as f64 * 30.0;
                    let x1 = x0 + 25.0;
                    words.push(make_word(
                        &format!("L{line_idx}W{word_idx}"),
                        x0,
                        top,
                        x1,
                        bottom,
                    ));
                }
            }
            words
        };

        let small_words = build_words(250, 20); // 5,000 words
        let large_words = build_words(1000, 20); // 20,000 words (4x)

        // Warm up
        let _ = cluster_words_into_lines(&small_words, 3.0);

        let start_small = std::time::Instant::now();
        let lines_small = cluster_words_into_lines(&small_words, 3.0);
        let elapsed_small = start_small.elapsed();

        let start_large = std::time::Instant::now();
        let lines_large = cluster_words_into_lines(&large_words, 3.0);
        let elapsed_large = start_large.elapsed();

        assert_eq!(lines_small.len(), 250);
        assert_eq!(lines_large.len(), 1000);

        // With O(n²), ratio would be ~16x. With O(n log n), ~4.5x. With O(n), ~4x.
        // Use generous threshold of 10x to avoid flaky tests.
        let ratio = if elapsed_small.as_nanos() > 0 {
            elapsed_large.as_nanos() as f64 / elapsed_small.as_nanos() as f64
        } else {
            1.0 // both are negligibly fast
        };

        assert!(
            ratio < 10.0,
            "Scaling ratio is {ratio:.1}x for 4x input — suggests super-linear behavior \
             (small: {}us, large: {}us)",
            elapsed_small.as_micros(),
            elapsed_large.as_micros()
        );
    }
}
