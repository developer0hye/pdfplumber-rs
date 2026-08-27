//! Compile-time and runtime contracts for thread-safe Rust extraction.

use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use pdfplumber::{
    CroppedPage, ExtractOptions, Page, Pages, PagesIter, Pdf, PdfErrorKind, TextOptions,
};

fn valid_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdfs/test-punkt.pdf")
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_document_and_page_values_are_send_and_sync() {
    assert_send_sync::<Pdf>();
    assert_send_sync::<Pages<'static>>();
    assert_send_sync::<PagesIter<'static>>();
    assert_send_sync::<Page>();
    assert_send_sync::<CroppedPage>();
}

#[test]
fn shared_document_supports_concurrent_immutable_page_reads() {
    let pdf = Arc::new(Pdf::open_path(valid_fixture(), None).unwrap());
    let expected: Vec<_> = (0..pdf.page_count())
        .map(|index| {
            pdf.page(index)
                .unwrap()
                .extract_text(&TextOptions::default())
        })
        .collect();

    let handles: Vec<_> = (0..pdf.page_count())
        .map(|index| {
            let pdf = Arc::clone(&pdf);
            std::thread::spawn(move || {
                (
                    index,
                    pdf.page(index)
                        .unwrap()
                        .extract_text(&TextOptions::default()),
                )
            })
        })
        .collect();

    let mut actual = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    actual.sort_by_key(|(index, _)| *index);

    assert_eq!(
        actual.into_iter().map(|(_, text)| text).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn owned_page_and_crop_support_concurrent_read_only_queries() {
    let pdf = Pdf::open_path(valid_fixture(), None).unwrap();
    let page = Arc::new(pdf.page(0).unwrap());
    let crop = Arc::new(page.crop(page.bbox()));
    let expected_page_text = page.extract_text(&TextOptions::default());
    let expected_crop_text = crop.extract_text(&TextOptions::default());

    let page_handle = {
        let page = Arc::clone(&page);
        std::thread::spawn(move || page.extract_text(&TextOptions::default()))
    };
    let crop_handle = {
        let crop = Arc::clone(&crop);
        std::thread::spawn(move || crop.extract_text(&TextOptions::default()))
    };

    assert_eq!(page_handle.join().unwrap(), expected_page_text);
    assert_eq!(crop_handle.join().unwrap(), expected_crop_text);
}

#[test]
fn repeated_concurrent_extraction_shares_one_document_budget() {
    let probe = Pdf::open_path(valid_fixture(), None).unwrap();
    let probe_page = probe.page(0).unwrap();
    let page_object_count = probe_page.chars().len()
        + probe_page.lines().len()
        + probe_page.rects().len()
        + probe_page.curves().len()
        + probe_page.images().len();
    assert!(page_object_count > 0);

    let pdf = Arc::new(
        Pdf::open_path(
            valid_fixture(),
            Some(ExtractOptions {
                max_total_objects: Some(page_object_count),
                ..ExtractOptions::default()
            }),
        )
        .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let pdf = Arc::clone(&pdf);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                pdf.page(0)
            })
        })
        .collect();
    barrier.wait();

    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);

    let errors = results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), PdfErrorKind::ResourceLimit);
    let details = errors[0].resource_limit().unwrap();
    assert_eq!(details.name, "max_total_objects");
    assert_eq!(details.limit, page_object_count);
    assert_eq!(details.observed, page_object_count * 2);
}

#[cfg(feature = "parallel")]
#[test]
fn rayon_results_match_sequential_page_index_order() {
    let sequential_pdf = Pdf::open_path(valid_fixture(), None).unwrap();
    let expected = (0..sequential_pdf.page_count())
        .map(|index| {
            let page = sequential_pdf.page(index).unwrap();
            (
                page.page_number(),
                page.extract_text(&TextOptions::default()),
            )
        })
        .collect::<Vec<_>>();

    let parallel_pdf = Pdf::open_path(valid_fixture(), None).unwrap();
    let actual = parallel_pdf
        .pages_parallel()
        .into_iter()
        .map(|result| {
            let page = result.unwrap();
            (
                page.page_number(),
                page.extract_text(&TextOptions::default()),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}
