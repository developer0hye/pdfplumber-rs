//! Extraction-semantic regressions exercised under every supported feature combination.

use std::fmt::Write as _;

use pdfplumber::{Pdf, TableSettings, TextOptions, WordOptions};

const BASIC_TEXT_BYTES: &[u8] = include_bytes!("../../../tests/fixtures/generated/basic_text.pdf");
const TABLE_LATTICE_BYTES: &[u8] =
    include_bytes!("../../../tests/fixtures/generated/table_lattice.pdf");
#[cfg(feature = "parallel")]
const ROTATED_PAGES_BYTES: &[u8] =
    include_bytes!("../../../tests/fixtures/generated/rotated_pages.pdf");

fn semantic_snapshot(pdf: &Pdf) -> String {
    let mut snapshot = String::new();
    writeln!(&mut snapshot, "page_count={}", pdf.page_count()).unwrap();

    for page_result in pdf.pages() {
        let page = page_result.unwrap();
        writeln!(
            &mut snapshot,
            "page={} rotation={} bbox={:?}",
            page.page_number(),
            page.rotation(),
            page.bbox()
        )
        .unwrap();
        writeln!(
            &mut snapshot,
            "text={:?}",
            page.extract_text(&TextOptions::default())
        )
        .unwrap();
        writeln!(&mut snapshot, "chars={:?}", page.chars()).unwrap();
        writeln!(
            &mut snapshot,
            "words={:?}",
            page.extract_words(&WordOptions::default())
        )
        .unwrap();
        writeln!(&mut snapshot, "lines={:?}", page.lines()).unwrap();
        writeln!(&mut snapshot, "rects={:?}", page.rects()).unwrap();
        writeln!(&mut snapshot, "curves={:?}", page.curves()).unwrap();
        writeln!(&mut snapshot, "images={:?}", page.images()).unwrap();
        writeln!(
            &mut snapshot,
            "tables={:?}",
            page.find_tables(&TableSettings::default())
        )
        .unwrap();
    }

    snapshot
}

fn semantic_digest(bytes: &[u8]) -> String {
    let pdf = Pdf::open_bytes(bytes, None).unwrap();
    format!("{:x}", md5::compute(semantic_snapshot(&pdf)))
}

#[test]
fn text_and_geometry_semantics_are_feature_invariant() {
    assert_eq!(
        semantic_digest(BASIC_TEXT_BYTES),
        "d69e6ef96faf203fe017fb87a57f3ec9"
    );
}

#[test]
fn table_semantics_are_feature_invariant() {
    assert_eq!(
        semantic_digest(TABLE_LATTICE_BYTES),
        "98809e85eef8724a3e8fa2f0b8ca1368"
    );
}

#[cfg(feature = "std")]
#[test]
fn default_path_input_matches_the_feature_independent_byte_path() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/generated/basic_text.pdf"
    );
    let from_path = Pdf::open_path(path, None).unwrap();
    let from_bytes = Pdf::open_bytes(BASIC_TEXT_BYTES, None).unwrap();
    assert_eq!(
        semantic_snapshot(&from_path),
        semantic_snapshot(&from_bytes)
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_adds_serialization_without_changing_extraction() {
    let pdf = Pdf::open_bytes(BASIC_TEXT_BYTES, None).unwrap();
    let page = pdf.pages().get(0).unwrap();
    let value = serde_json::to_value(page.chars()).unwrap();
    let chars = value.as_array().unwrap();
    assert_eq!(chars.len(), page.chars().len());
    assert_eq!(chars[0]["text"], page.chars()[0].text);
    assert_eq!(
        semantic_digest(BASIC_TEXT_BYTES),
        "d69e6ef96faf203fe017fb87a57f3ec9"
    );
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_adds_ordered_processing_without_changing_pages() {
    let sequential_pdf = Pdf::open_bytes(ROTATED_PAGES_BYTES, None).unwrap();
    let sequential = sequential_pdf
        .pages()
        .into_iter()
        .map(|page| page.unwrap().extract_text(&TextOptions::default()))
        .collect::<Vec<_>>();

    let parallel_pdf = Pdf::open_bytes(ROTATED_PAGES_BYTES, None).unwrap();
    let parallel = parallel_pdf
        .pages_parallel()
        .into_iter()
        .map(|page| page.unwrap().extract_text(&TextOptions::default()))
        .collect::<Vec<_>>();

    assert_eq!(parallel, sequential);
}
