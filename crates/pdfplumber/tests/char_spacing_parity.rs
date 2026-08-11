//! Parity tests for how character spacing (`Tc`) advances the text position.
//!
//! Expected values come from Python pdfplumber 0.11.10 run on the same content
//! streams:
//!
//! ```python
//! [(c["text"], c["x0"]) for c in page.chars]
//! ```
//!
//! `Tc` is inserted *between* the glyphs of one show operation. The last glyph
//! of a `Tj` is not followed by it, and the first glyph of the next `Tj` is not
//! preceded by it — otherwise every show operation would leave the text
//! position one `Tc` too far right, and the error would accumulate along a line.

use std::path::PathBuf;

use pdfplumber::Pdf;

/// Wrap a content stream in a one-page PDF with Helvetica as `/F1`.
fn pdf_with_content(content: &[u8]) -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ],
        "Contents" => Object::Reference(content_id),
        "Resources" => dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        },
    });
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
    });
    if let Ok(page) = doc.get_object_mut(page_id) {
        if let Ok(dict) = page.as_dict_mut() {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

fn char_positions(content: &[u8]) -> Vec<(String, f64)> {
    let bytes = pdf_with_content(content);
    let pdf = Pdf::open(&bytes, None).unwrap();
    pdf.page(0)
        .unwrap()
        .chars()
        .iter()
        .map(|ch| (ch.text.clone(), (ch.bbox.x0 * 100.0).round() / 100.0))
        .collect()
}

#[test]
fn spacing_does_not_trail_a_show_operation() {
    // Two Tj operations under `5 Tc`. Python pdfplumber: A 100, B 111.67,
    // C 118.34, D 130.56 — the gap between B and C is one glyph width, with no
    // spacing on either side of the operation boundary.
    let positions = char_positions(b"BT /F1 10 Tf 5 Tc 100 700 Td (AB) Tj (CD) Tj ET");

    assert_eq!(
        positions,
        vec![
            ("A".to_string(), 100.0),
            ("B".to_string(), 111.67),
            ("C".to_string(), 118.34),
            ("D".to_string(), 130.56),
        ]
    );
}

#[test]
fn spacing_follows_an_adjustment_inside_one_array() {
    // One TJ array: the -100 adjustment moves the position right by 1.0 and the
    // glyph after it is still spaced from the one before.
    // Python pdfplumber: A 100, B 111.67, C 124.34, D 136.56.
    let positions = char_positions(b"BT /F1 10 Tf 5 Tc 100 700 Td [(AB) -100 (CD)] TJ ET");

    assert_eq!(
        positions,
        vec![
            ("A".to_string(), 100.0),
            ("B".to_string(), 111.67),
            ("C".to_string(), 124.34),
            ("D".to_string(), 136.56),
        ]
    );
}

#[test]
fn spacing_error_does_not_accumulate_across_a_real_line() {
    // chelsea_pdta.pdf sets `0.001 Tc` and shows "de" as its own Tj before
    // resetting the spacing. Everything after it shifted by 0.038pt — one Tc at
    // a 38pt scale — and the shift carried along the rest of the line.
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdfs/chelsea_pdta.pdf");
    let pdf = Pdf::open_file(path, None).unwrap();
    let page = pdf.page(0).unwrap();

    let space = &page.chars()[39];
    assert_eq!(space.text, " ");
    assert!(
        (space.bbox.x0 - 105.651).abs() < 0.001,
        "expected pdfplumber's 105.651, got {}",
        space.bbox.x0
    );
}
