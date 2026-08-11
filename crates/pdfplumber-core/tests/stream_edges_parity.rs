//! Parity tests for text-alignment ("stream") edge generation.
//!
//! Expected values in this file were produced by running Python pdfplumber
//! 0.11.10 (`pdfplumber.table.words_to_edges_v` / `words_to_edges_h`) on the
//! same word sets. They pin the Rust implementation to pdfplumber's algorithm:
//!
//! * vertical edges cluster words by `x0`, `x1` **and** horizontal center,
//!   using a fixed cluster tolerance of 1.0 point,
//! * clusters are ranked by size, and a cluster whose bounding box overlaps an
//!   already-accepted one is discarded,
//! * every surviving cluster contributes one edge at its `x0`, plus a single
//!   trailing edge at the largest `x1`, and all vertical edges span the same
//!   `top`/`bottom` range,
//! * horizontal edges cluster words by `top` only, and each cluster emits two
//!   edges (one at the cluster's `top`, one at its `bottom`) spanning the same
//!   `x0`/`x1` range.

use pdfplumber_core::{
    BBox, Edge, Orientation, TextDirection, Word, words_to_edges_h, words_to_edges_v,
};

fn word(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Word {
    Word {
        text: text.to_string(),
        bbox: BBox::new(x0, top, x1, bottom),
        doctop: top,
        direction: TextDirection::Ltr,
        chars: Vec::new(),
    }
}

/// A 3x3 grid of words: columns start at x0 = 10, 60, 110 and end at
/// x1 = 50, 100, 150; rows span top/bottom pairs (10, 20), (30, 40), (50, 60).
fn grid_words() -> Vec<Word> {
    let mut words = Vec::new();
    for (row, (top, bottom)) in [(10.0, 20.0), (30.0, 40.0), (50.0, 60.0)]
        .into_iter()
        .enumerate()
    {
        for (col, (x0, x1)) in [(10.0, 50.0), (60.0, 100.0), (110.0, 150.0)]
            .into_iter()
            .enumerate()
        {
            words.push(word(&format!("w{row}{col}"), x0, top, x1, bottom));
        }
    }
    words
}

fn vertical_positions(edges: &[Edge]) -> Vec<(f64, f64, f64)> {
    edges
        .iter()
        .map(|e| {
            assert_eq!(e.orientation, Orientation::Vertical);
            assert_eq!(e.x0, e.x1, "vertical edge must have zero width");
            (e.x0, e.top, e.bottom)
        })
        .collect()
}

fn horizontal_positions(edges: &[Edge]) -> Vec<(f64, f64, f64)> {
    edges
        .iter()
        .map(|e| {
            assert_eq!(e.orientation, Orientation::Horizontal);
            assert_eq!(e.top, e.bottom, "horizontal edge must have zero height");
            (e.top, e.x0, e.x1)
        })
        .collect()
}

#[test]
fn vertical_edges_match_pdfplumber_for_a_three_column_grid() {
    // Python: words_to_edges_v(grid, 3) ->
    //   x0 = 10, 60, 110, 150; every edge spans top=10, bottom=60.
    // The x1 and center clusters are dropped because their bounding boxes
    // overlap the x0 clusters that were accepted first.
    let edges = words_to_edges_v(&grid_words(), 3);

    assert_eq!(
        vertical_positions(&edges),
        vec![
            (10.0, 10.0, 60.0),
            (60.0, 10.0, 60.0),
            (110.0, 10.0, 60.0),
            (150.0, 10.0, 60.0),
        ]
    );
}

#[test]
fn horizontal_edges_match_pdfplumber_for_a_three_row_grid() {
    // Python: words_to_edges_h(grid, 1) -> tops at 10, 20, 30, 40, 50, 60,
    // each spanning x0 = 10 to x1 = 150.
    let edges = words_to_edges_h(&grid_words(), 1);

    assert_eq!(
        horizontal_positions(&edges),
        vec![
            (10.0, 10.0, 150.0),
            (20.0, 10.0, 150.0),
            (30.0, 10.0, 150.0),
            (40.0, 10.0, 150.0),
            (50.0, 10.0, 150.0),
            (60.0, 10.0, 150.0),
        ]
    );
}

#[test]
fn clusters_below_the_word_threshold_produce_no_edges() {
    // Python: words_to_edges_v(two_words, 3) -> [] and
    //         words_to_edges_h(two_words, 3) -> [].
    let words = vec![
        word("a", 10.0, 10.0, 50.0, 20.0),
        word("b", 10.0, 30.0, 50.0, 40.0),
    ];

    assert!(words_to_edges_v(&words, 3).is_empty());
    assert!(words_to_edges_h(&words, 3).is_empty());
}

#[test]
fn horizontal_threshold_of_one_emits_top_and_bottom_per_row() {
    // Python: words_to_edges_h(two_words, 1) -> tops at 10, 20, 30, 40
    // spanning x0 = 10 to x1 = 50.
    let words = vec![
        word("a", 10.0, 10.0, 50.0, 20.0),
        word("b", 10.0, 30.0, 50.0, 40.0),
    ];

    assert_eq!(
        horizontal_positions(&words_to_edges_h(&words, 1)),
        vec![
            (10.0, 10.0, 50.0),
            (20.0, 10.0, 50.0),
            (30.0, 10.0, 50.0),
            (40.0, 10.0, 50.0),
        ]
    );
}

#[test]
fn clustering_chains_neighbours_within_one_point() {
    // x0 values 10.0, 10.6, 11.2 chain into a single cluster because each is
    // within 1.0 of its predecessor, even though the span exceeds 1.0.
    // Python: words_to_edges_v(words, 3) -> x0 = 10.0 and 51 (the max x1).
    let words = vec![
        word("a", 10.0, 10.0, 50.0, 20.0),
        word("b", 10.6, 30.0, 50.5, 40.0),
        word("c", 11.2, 50.0, 51.0, 60.0),
    ];

    assert_eq!(
        vertical_positions(&words_to_edges_v(&words, 3)),
        vec![(10.0, 10.0, 60.0), (51.0, 10.0, 60.0)]
    );
}

#[test]
fn clustering_does_not_chain_gaps_wider_than_one_point() {
    // x0 values 10.0, 11.5, 12.0: the 1.5 gap splits the first word off, so no
    // cluster reaches the threshold of 3 and no vertical edge is produced.
    // (Python pdfplumber uses a fixed tolerance of 1.0 here, independent of any
    // text tolerance setting.)
    let words = vec![
        word("a", 10.0, 10.0, 30.0, 22.0),
        word("b", 11.5, 30.0, 35.0, 42.0),
        word("c", 12.0, 50.0, 40.0, 62.0),
    ];

    assert!(words_to_edges_v(&words, 3).is_empty());
}

#[test]
fn center_alignment_produces_a_vertical_edge() {
    // Centred words share no x0 or x1, but their centers all cluster at 50.
    // Python: words_to_edges_v(words, 3) -> x0 = 40 (cluster bbox left) and 60.
    let words = vec![
        word("a", 40.0, 10.0, 60.0, 20.0),
        word("b", 45.0, 30.0, 55.0, 40.0),
        word("c", 42.0, 50.0, 58.0, 60.0),
    ];

    assert_eq!(
        vertical_positions(&words_to_edges_v(&words, 3)),
        vec![(40.0, 10.0, 60.0), (60.0, 10.0, 60.0)]
    );
}

#[test]
fn empty_input_produces_no_edges() {
    assert!(words_to_edges_v(&[], 3).is_empty());
    assert!(words_to_edges_h(&[], 1).is_empty());
}
