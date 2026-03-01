//! Tests for US-186-1: Error recovery for unrecognized content stream operators.
//!
//! Verifies that the content stream interpreter gracefully handles unrecognized
//! operators by skipping them instead of aborting.

use pdfplumber_parse::tokenizer::tokenize;

/// Verify that tokenizing a content stream with an unknown operator does not
/// produce an error — the tokenizer should parse it as a normal operator.
#[test]
fn tokenizer_accepts_unknown_operators() {
    // "q" (save), "XYZ" (unknown), "Q" (restore)
    let stream = b"q XYZ Q";
    let ops = tokenize(stream).expect("tokenizer should not fail on unknown operators");
    let names: Vec<&str> = ops.iter().map(|op| op.name.as_str()).collect();
    assert!(names.contains(&"XYZ"), "unknown operator should be parsed");
    assert!(names.contains(&"q"));
    assert!(names.contains(&"Q"));
}

/// Verify that a content stream with recognized operators surrounding an
/// unrecognized one still tokenizes correctly.
#[test]
fn unknown_operator_does_not_disrupt_parsing() {
    let stream = b"BT /F1 12 Tf 72 720 Td (A) Tj fakeop 144 720 Td (B) Tj ET";
    let ops = tokenize(stream).expect("should tokenize");
    let names: Vec<&str> = ops.iter().map(|op| op.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["BT", "Tf", "Td", "Tj", "fakeop", "Td", "Tj", "ET"]
    );
}
