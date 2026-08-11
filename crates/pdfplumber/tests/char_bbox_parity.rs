//! Parity tests for character bounding boxes.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [(c["text"], c["x0"], c["top"], c["x1"], c["bottom"]) for c in page.chars]
//! ```
//!
//! pdfminer builds a glyph box from the baseline: it spans `descent` to
//! `descent + fontsize`, with the descent taken from the font. For the standard
//! 14 fonts it reads that descent from the bundled AFM metrics rather than the
//! PDF's own descriptor, which is what these fixtures exercise — Helvetica's
//! descent is -207/1000, not the -250/1000 fallback.

use std::path::PathBuf;

use pdfplumber::{Char, Pdf};

/// (text, x0, top, x1, bottom)
type CharTuple = (&'static str, f64, f64, f64, f64);

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(kind)
        .join(name)
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

/// The first `count` characters of each distinct font, in page order.
fn first_chars_per_font(chars: &[Char], count: usize) -> Vec<(String, f64, f64, f64, f64)> {
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut sampled = Vec::new();

    for ch in chars {
        let entry = match seen.iter_mut().find(|(font, _)| *font == ch.fontname) {
            Some(entry) => entry,
            None => {
                seen.push((ch.fontname.clone(), 0));
                seen.last_mut().unwrap()
            }
        };
        if entry.1 < count {
            entry.1 += 1;
            sampled.push((
                ch.text.clone(),
                round4(ch.bbox.x0),
                round4(ch.bbox.top),
                round4(ch.bbox.x1),
                round4(ch.bbox.bottom),
            ));
        }
    }

    sampled
}

fn assert_chars_match(kind: &str, name: &str, expected: &[CharTuple]) {
    let pdf = Pdf::open_file(fixture(kind, name), None).unwrap();
    let page = pdf.page(0).unwrap();
    let actual = first_chars_per_font(page.chars(), 3);

    let expected: Vec<(String, f64, f64, f64, f64)> = expected
        .iter()
        .map(|(text, x0, top, x1, bottom)| ((*text).to_string(), *x0, *top, *x1, *bottom))
        .collect();

    assert_eq!(actual, expected, "{name}");
}

#[test]
fn helvetica_char_boxes_match_python_pdfplumber() {
    assert_chars_match(
        "generated",
        "basic_text.pdf",
        &[
            ("T", 31.18, 30.934, 38.512, 42.934),
            ("h", 38.512, 30.934, 45.184, 42.934),
            ("e", 45.184, 30.934, 51.856, 42.934),
        ],
    );
}

#[test]
fn standard_14_char_boxes_match_python_pdfplumber() {
    // Helvetica-Bold, Helvetica-Oblique, Helvetica and Courier: each family has
    // its own AFM descent (-207/1000 for Helvetica, -194/1000 for Courier).
    assert_chars_match(
        "generated",
        "multi_font.pdf",
        &[
            ("D", 31.18, 36.358, 48.508, 60.358),
            ("o", 48.508, 36.358, 63.172, 60.358),
            ("c", 63.172, 36.358, 76.516, 60.358),
            ("A", 31.18, 86.638, 40.518, 100.638),
            (" ", 40.518, 86.638, 44.41, 100.638),
            ("s", 44.41, 86.638, 51.41, 100.638),
            ("T", 31.18, 121.644, 38.512, 133.644),
            ("h", 38.512, 121.644, 45.184, 133.644),
            ("i", 45.184, 121.644, 47.848, 133.644),
            ("d", 31.18, 166.44, 37.18, 176.44),
            ("e", 37.18, 166.44, 43.18, 176.44),
            ("f", 43.18, 166.44, 49.18, 176.44),
        ],
    );
}

#[test]
fn embedded_font_char_boxes_keep_using_the_pdf_descriptor() {
    // The AFM override applies to the standard 14 names only. These fonts are
    // subset-embedded ("WEVZII+ArialMT"), so their own descriptors decide the
    // box, exactly as in pdfminer.
    let pdf = Pdf::open_file(fixture("downloaded", "nics-firearm-checks.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let sampled = first_chars_per_font(page.chars(), 1);

    let boxes: Vec<(String, f64, f64)> = sampled
        .iter()
        .map(|(text, _, top, _, bottom)| (text.clone(), *top, *bottom))
        .collect();

    assert_eq!(
        boxes,
        vec![
            ("S".to_string(), 71.9408, 78.9008),
            ("P".to_string(), 72.7611, 78.5211),
            ("N".to_string(), 497.71, 503.47),
            ("*".to_string(), 491.2742, 497.0342),
        ]
    );
}

#[test]
fn every_horizontal_char_box_is_exactly_one_font_size_tall() {
    // pdfminer's model: the box runs from the descent to the descent plus the
    // font size, so its height is the font size regardless of the font's ascent.
    let pdf = Pdf::open_file(fixture("generated", "multi_font.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();

    for ch in page.chars() {
        let height = ch.bbox.bottom - ch.bbox.top;
        assert!(
            (height - ch.size).abs() < 0.001,
            "char {:?} in {}: height {height} != size {}",
            ch.text,
            ch.fontname,
            ch.size
        );
    }
}
