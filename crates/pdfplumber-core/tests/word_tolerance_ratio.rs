//! Parity tests for font-size-relative word tolerances.
//!
//! Expected values come from Python pdfplumber 0.11.10 run over the same
//! characters:
//!
//! ```python
//! WordExtractor(x_tolerance_ratio=0.1).extract_words(chars)
//! ```
//!
//! The characters are built here rather than read from a fixture because the
//! ratio only bites when a gap sits between the fixed tolerance and the scaled
//! one, which the generated fixtures never produce.

use pdfplumber_core::{BBox, Char, TextDirection, WordExtractor, WordOptions};

/// Four characters on one line: "AB", a 2pt gap, then "CD".
fn chars_with_a_two_point_gap(size: f64) -> Vec<Char> {
    [
        ("A", 10.0, 17.0),
        ("B", 17.0, 24.0),
        ("C", 26.0, 33.0),
        ("D", 33.0, 40.0),
    ]
    .into_iter()
    .map(|(text, x0, x1)| Char {
        text: text.to_string(),
        bbox: BBox::new(x0, 100.0, x1, 100.0 + size),
        fontname: "Helvetica".to_string(),
        size,
        advance: x1 - x0,
        doctop: 100.0,
        upright: true,
        direction: TextDirection::Ltr,
        stroking_color: None,
        non_stroking_color: None,
        ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        char_code: 0,
        mcid: None,
        tag: None,
    })
    .collect()
}

fn word_texts(chars: &[Char], options: &WordOptions) -> Vec<String> {
    WordExtractor::extract(chars, options)
        .into_iter()
        .map(|word| word.text)
        .collect()
}

#[test]
fn the_fixed_tolerance_applies_when_no_ratio_is_set() {
    // Default x_tolerance is 3.0, wider than the 2pt gap, so it is one word.
    let chars = chars_with_a_two_point_gap(10.0);

    assert_eq!(word_texts(&chars, &WordOptions::default()), ["ABCD"]);
}

#[test]
fn a_ratio_below_the_gap_splits_the_word() {
    // 0.1 of a 10pt font is a 1pt tolerance, narrower than the 2pt gap.
    let chars = chars_with_a_two_point_gap(10.0);
    let options = WordOptions {
        x_tolerance_ratio: Some(0.1),
        ..WordOptions::default()
    };

    assert_eq!(word_texts(&chars, &options), ["AB", "CD"]);
}

#[test]
fn a_ratio_above_the_gap_keeps_the_word_together() {
    let chars = chars_with_a_two_point_gap(10.0);
    let options = WordOptions {
        x_tolerance_ratio: Some(0.3),
        ..WordOptions::default()
    };

    assert_eq!(word_texts(&chars, &options), ["ABCD"]);
}

#[test]
fn the_same_ratio_follows_the_font_size() {
    // Identical geometry at 30pt: 0.1 now allows 3pt, so the gap is bridged.
    let chars = chars_with_a_two_point_gap(30.0);
    let options = WordOptions {
        x_tolerance_ratio: Some(0.1),
        ..WordOptions::default()
    };

    assert_eq!(word_texts(&chars, &options), ["ABCD"]);
}

#[test]
fn a_vertical_ratio_does_not_affect_a_horizontal_gap() {
    let chars = chars_with_a_two_point_gap(10.0);
    let options = WordOptions {
        y_tolerance_ratio: Some(0.1),
        ..WordOptions::default()
    };

    assert_eq!(word_texts(&chars, &options), ["ABCD"]);
}
