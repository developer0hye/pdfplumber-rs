//! Contracts for ergonomic page selection and lazy iteration.

use std::iter::FusedIterator;
use std::path::PathBuf;

use pdfplumber::{ExtractOptions, Pages, Pdf};

fn valid_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdfs/test-punkt.pdf")
}

fn borrowed_pages(pdf: &Pdf) -> Pages<'_> {
    pdf.pages()
}

fn assert_fused<I: FusedIterator>(_iterator: I) {}

#[test]
fn pages_view_reports_count_without_extracting_content() {
    let pdf = Pdf::open_path(
        valid_fixture(),
        Some(ExtractOptions {
            max_total_objects: Some(1),
            ..ExtractOptions::default()
        }),
    )
    .unwrap();

    let pages = borrowed_pages(&pdf);

    assert_eq!(pages.len(), pdf.page_count());
    assert_eq!(pages.len(), 4);
    assert!(!pages.is_empty());
    assert_eq!(pages.iter().len(), 4);
}

#[test]
fn get_selects_one_page_without_processing_earlier_pages() {
    let pdf = Pdf::open_path(
        valid_fixture(),
        Some(ExtractOptions {
            // Page 3 has 15 objects. Page 0 alone has more than this budget,
            // so a sequential nth-page implementation would fail.
            max_total_objects: Some(20),
            ..ExtractOptions::default()
        }),
    )
    .unwrap();

    let selected = pdf.pages().get(3).unwrap();

    assert_eq!(selected.page_number(), 3);
}

#[test]
fn page_iteration_is_double_ended_exact_sized_and_fused() {
    let pdf = Pdf::open_path(valid_fixture(), None).unwrap();
    let pages = pdf.pages();
    assert_fused(pages.iter());

    let mut iterator = pages.iter();
    assert_eq!(iterator.len(), 4);
    assert_eq!(iterator.next_back().unwrap().unwrap().page_number(), 3);
    assert_eq!(iterator.len(), 3);
    assert_eq!(iterator.next().unwrap().unwrap().page_number(), 0);
    assert_eq!(iterator.len(), 2);

    assert_eq!(iterator.next().unwrap().unwrap().page_number(), 1);
    assert_eq!(iterator.next_back().unwrap().unwrap().page_number(), 2);
    assert!(iterator.next().is_none());
    assert!(iterator.next_back().is_none());
}

#[test]
fn pages_view_works_directly_with_for_loops() {
    let pdf = Pdf::open_path(valid_fixture(), None).unwrap();

    let page_numbers: Vec<_> = pdf
        .pages()
        .into_iter()
        .map(|result| result.unwrap().page_number())
        .collect();

    assert_eq!(page_numbers, vec![0, 1, 2, 3]);
}
