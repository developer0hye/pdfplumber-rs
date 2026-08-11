//! Parity tests for the word-grouping options.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [w["text"] for w in page.extract_words(split_at_punctuation=True)]
//! ```
//!
//! The tolerance ratios are covered in
//! `pdfplumber-core/tests/word_tolerance_ratio.rs`, where the character
//! geometry can be chosen to actually exercise them.

use std::path::PathBuf;

use pdfplumber::{DEFAULT_SPLIT_PUNCTUATION, Pdf, WordOptions};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(kind)
        .join(name)
}

fn words(kind: &str, name: &str, options: &WordOptions) -> Vec<String> {
    let pdf = Pdf::open_file(fixture(kind, name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_words(options)
        .into_iter()
        .map(|word| word.text)
        .collect()
}

#[test]
fn punctuation_splitting_matches_python_pdfplumber() {
    let options = WordOptions {
        split_at_punctuation: Some(DEFAULT_SPLIT_PUNCTUATION.to_string()),
        ..WordOptions::default()
    };

    assert_eq!(
        words("generated", "basic_text.pdf", &options)[..20],
        [
            "The",
            "quick",
            "brown",
            "fox",
            "jumps",
            "over",
            "the",
            "lazy",
            "dog",
            ".",
            "Special",
            "chars",
            ":",
            "\"",
            "quotes",
            "\"",
            ",",
            "copyright",
            "©",
            ",",
        ]
    );
}

#[test]
fn a_custom_punctuation_set_splits_only_those_characters() {
    let options = WordOptions {
        split_at_punctuation: Some(".,".to_string()),
        ..WordOptions::default()
    };

    // The colon and quotes stay attached; only "." and "," break a word.
    assert_eq!(
        words("generated", "basic_text.pdf", &options)[..16],
        [
            "The",
            "quick",
            "brown",
            "fox",
            "jumps",
            "over",
            "the",
            "lazy",
            "dog",
            ".",
            "Special",
            "chars:",
            "\"quotes\"",
            ",",
            "copyright",
            "©",
        ]
    );
}

#[test]
fn punctuation_splitting_is_off_by_default() {
    let default = words("generated", "basic_text.pdf", &WordOptions::default());

    assert_eq!(
        &default[..10],
        [
            "The", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog.", "Special",
        ]
    );
}

#[test]
fn punctuation_splitting_word_counts_match_python_pdfplumber() {
    let options = WordOptions {
        split_at_punctuation: Some(DEFAULT_SPLIT_PUNCTUATION.to_string()),
        ..WordOptions::default()
    };

    for (kind, name, expected) in [
        ("generated", "multi_font.pdf", 55),
        ("generated", "table_lattice.pdf", 62),
        ("downloaded", "scotus-transcript-p1.pdf", 186),
    ] {
        assert_eq!(words(kind, name, &options).len(), expected, "{name}");
    }
}

#[test]
fn word_grouping_is_unchanged_when_no_ratio_is_given() {
    let baseline = words(
        "downloaded",
        "nics-firearm-checks.pdf",
        &WordOptions::default(),
    );

    assert_eq!(baseline.len(), 1499);
    assert_eq!(&baseline[4..7], ["November", "-", "2015"]);
}
