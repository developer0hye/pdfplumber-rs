//! Parity tests for `Page::extract_text_lines`.
//!
//! Expected values come from Python pdfplumber 0.11.10:
//!
//! ```python
//! [(l["text"], l["x0"], l["top"], l["x1"], l["bottom"], len(l["chars"]))
//!  for l in page.extract_text_lines()]
//! ```
//!
//! Character counts exclude the spaces between words: those are inserted by the
//! text layout rather than read from the PDF.
//!
//! Text, every coordinate and the character counts are asserted to within
//! [`COORD_TOLERANCE`], which is float noise rather than a modelling allowance.

use std::path::PathBuf;

use pdfplumber::{BBox, Pdf, TextOptions};

/// Largest accepted coordinate deviation, in points.
const COORD_TOLERANCE: f64 = 0.001;

/// One line of pdfplumber output: text, x0, top, x1, bottom, character count.
type LineTuple = (&'static str, f64, f64, f64, f64, usize);

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/generated")
        .join(name)
}

fn text_lines(name: &str) -> Vec<(String, f64, f64, f64, f64, usize)> {
    let pdf = Pdf::open_file(generated(name), None).unwrap();
    let page = pdf.page(0).unwrap();
    page.extract_text_lines(&TextOptions::default())
        .into_iter()
        .map(|line| {
            (
                line.text(),
                line.bbox.x0,
                line.bbox.top,
                line.bbox.x1,
                line.bbox.bottom,
                line.chars().count(),
            )
        })
        .collect()
}

fn assert_matches_pdfplumber(name: &str, expected: &[LineTuple]) {
    let actual = text_lines(name);

    assert_eq!(
        actual.len(),
        expected.len(),
        "{name}: line count; got {:?}",
        actual.iter().map(|l| &l.0).collect::<Vec<_>>()
    );

    for (index, (got, want)) in actual.iter().zip(expected).enumerate() {
        let label = format!("{name} line {index}");
        assert_eq!(got.0, want.0, "{label}: text");
        for (coordinate, got_value, want_value) in [
            ("x0", got.1, want.1),
            ("top", got.2, want.2),
            ("x1", got.3, want.3),
            ("bottom", got.4, want.4),
        ] {
            assert!(
                (got_value - want_value).abs() < COORD_TOLERANCE,
                "{label}: {coordinate} {got_value} != {want_value}"
            );
        }
        assert_eq!(got.5, want.5, "{label}: character count");
    }
}

#[test]
fn basic_text_lines_match_python_pdfplumber() {
    assert_matches_pdfplumber(
        "basic_text.pdf",
        &[
            (
                "The quick brown fox jumps over the lazy dog.",
                31.18,
                30.934,
                271.948,
                42.934,
                36,
            ),
            (
                "Special chars: \"quotes\", copyright ©, registered ®, section §, degree °, plus-minus ±",
                31.18,
                59.284,
                476.044,
                71.284,
                73,
            ),
            (
                "Accented: café, naïve, résumé, über, piñata, à la carte",
                31.18,
                87.624,
                319.336,
                99.624,
                47,
            ),
            (
                "Numbers: 0 1 2 3 4 5 6 7 8 9. Price: $1,234.56. Ratio: 3:1. Percent: 99.9%",
                31.18,
                115.974,
                424.072,
                127.974,
                58,
            ),
        ],
    );
}

#[test]
fn multi_font_text_lines_match_python_pdfplumber() {
    assert_matches_pdfplumber(
        "multi_font.pdf",
        &[
            ("Document Title", 31.18, 36.358, 204.532, 60.358, 13),
            (
                "A subtitle in italic style",
                31.18,
                86.638,
                168.114,
                100.638,
                22,
            ),
            (
                "This is the body text in regular 12pt Helvetica. It contains multiple sentences to provide enough",
                31.18,
                121.644,
                564.103,
                133.644,
                82,
            ),
            (
                "characters for font analysis. The quick brown fox jumps over the lazy dog.",
                31.18,
                138.654,
                422.008,
                150.654,
                62,
            ),
            ("def hello():", 31.18, 166.44, 103.18, 176.44, 11),
            ("print('Hello, World!')", 55.18, 180.61, 187.18, 190.61, 21),
            ("return 42", 55.18, 194.78, 109.18, 204.78, 8),
        ],
    );
}

#[test]
fn side_by_side_columns_are_joined_into_one_line() {
    // pdfplumber groups purely by vertical position, so two columns printed at
    // the same height belong to a single text line.
    assert_matches_pdfplumber(
        "multicolumn.pdf",
        &[
            (
                "Left column line 1 Right column line 1",
                31.18,
                60.27,
                385.5,
                70.27,
                31,
            ),
            (
                "Left column line 2 Right column line 2",
                31.18,
                82.94,
                385.5,
                92.94,
                31,
            ),
            (
                "Left column line 3 Right column line 3",
                31.18,
                105.62,
                385.5,
                115.62,
                31,
            ),
            (
                "Left column line 4 Right column line 4",
                31.18,
                128.3,
                385.5,
                138.3,
                31,
            ),
            (
                "Left column line 5 Right column line 5",
                31.18,
                150.98,
                385.5,
                160.98,
                31,
            ),
        ],
    );
}

#[test]
fn text_lines_are_ordered_top_to_bottom() {
    let lines = text_lines("multi_font.pdf");

    for pair in lines.windows(2) {
        assert!(
            pair[0].2 <= pair[1].2,
            "line {:?} should not start below {:?}",
            pair[0].0,
            pair[1].0
        );
    }
}

#[test]
fn cropping_separates_columns_that_share_a_line() {
    // The full page joins both columns; cropping to the left half yields the
    // left column alone, which is how pdfplumber users read columns separately.
    let pdf = Pdf::open_file(generated("multicolumn.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let left = page.crop(BBox::new(0.0, 0.0, 200.0, page.height()));

    let texts: Vec<String> = left
        .extract_text_lines(&TextOptions::default())
        .iter()
        .map(|line| line.text())
        .collect();

    assert_eq!(
        texts,
        vec![
            "Left column line 1",
            "Left column line 2",
            "Left column line 3",
            "Left column line 4",
            "Left column line 5",
        ]
    );
}

#[test]
fn a_page_without_text_yields_no_lines() {
    let pdf = Pdf::open_file(generated("table_lattice.pdf"), None).unwrap();
    let page = pdf.page(0).unwrap();
    let empty = page.crop(BBox::new(0.0, 0.0, 1.0, 1.0));

    assert!(empty.extract_text_lines(&TextOptions::default()).is_empty());
}
