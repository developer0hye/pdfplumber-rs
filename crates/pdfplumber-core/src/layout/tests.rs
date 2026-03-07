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
