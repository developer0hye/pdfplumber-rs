//! Parity tests for the reported character size.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [(c["fontname"], c["size"]) for c in page.chars]
//! ```
//!
//! A PDF is free to select a font at size 1 and scale the text matrix instead,
//! which several real documents do. pdfminer reports the size the glyph is
//! actually drawn at — the height of its box — rather than the number passed to
//! the `Tf` operator.

use std::path::PathBuf;

use pdfplumber::{Char, Pdf};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(kind)
        .join(name)
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// The first character of each distinct font, in page order.
fn first_char_per_font(chars: &[Char]) -> Vec<(String, String, f64)> {
    let mut seen: Vec<String> = Vec::new();
    let mut sampled = Vec::new();

    for ch in chars {
        if seen.contains(&ch.fontname) {
            continue;
        }
        seen.push(ch.fontname.clone());
        sampled.push((ch.text.clone(), ch.fontname.clone(), round4(ch.size)));
    }

    sampled
}

fn sizes(kind: &str, name: &str) -> Vec<(String, String, f64)> {
    let pdf = Pdf::open_file(fixture(kind, name), None).unwrap();
    let page = pdf.page(0).unwrap();
    first_char_per_font(page.chars())
}

#[test]
fn a_scaled_text_matrix_reports_the_rendered_size() {
    // nics-firearm-checks.pdf selects each font at 1pt and scales the text
    // matrix, so the `Tf` operand says nothing about how large the text is.
    // pdfplumber reports 6.96, 5.76, 5.76, 5.76.
    assert_eq!(
        sizes("downloaded", "nics-firearm-checks.pdf"),
        vec![
            ("S".to_string(), "Helvetica-Bold".to_string(), 6.96),
            ("P".to_string(), "ArialMT".to_string(), 5.76),
            ("N".to_string(), "Times-Bold".to_string(), 5.76),
            ("*".to_string(), "TimesNewRomanPSMT".to_string(), 5.76),
        ]
    );
}

#[test]
fn scotus_transcript_sizes_match_python_pdfplumber() {
    assert_eq!(
        sizes("downloaded", "scotus-transcript-p1.pdf"),
        vec![
            (" ".to_string(), "CourierNewPSMT".to_string(), 12.0),
            ("O".to_string(), "TimesNewRomanPSMT".to_string(), 9.96),
            ("\n".to_string(), "Times-Roman".to_string(), 12.0),
        ]
    );
}

#[test]
fn an_unscaled_text_matrix_still_reports_the_font_size() {
    assert_eq!(
        sizes("generated", "multi_font.pdf"),
        vec![
            ("D".to_string(), "Helvetica-Bold".to_string(), 24.0),
            ("A".to_string(), "Helvetica-Oblique".to_string(), 14.0),
            ("T".to_string(), "Helvetica".to_string(), 12.0),
            ("d".to_string(), "Courier".to_string(), 10.0),
        ]
    );
}

#[test]
fn size_equals_the_height_of_the_character_box() {
    // The relationship pdfminer defines, checked over a whole page rather than
    // a sample: for horizontal text, size *is* the box height.
    for (kind, name) in [
        ("generated", "basic_text.pdf"),
        ("generated", "multi_font.pdf"),
        ("downloaded", "nics-firearm-checks.pdf"),
    ] {
        let pdf = Pdf::open_file(fixture(kind, name), None).unwrap();
        let page = pdf.page(0).unwrap();

        for ch in page.chars() {
            assert!(
                (ch.size - (ch.bbox.bottom - ch.bbox.top)).abs() < 0.001,
                "{name}: char {:?} size {} != height {}",
                ch.text,
                ch.size,
                ch.bbox.bottom - ch.bbox.top
            );
        }
    }
}
