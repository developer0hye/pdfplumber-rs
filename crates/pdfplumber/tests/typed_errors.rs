//! Public contracts for actionable, source-preserving errors (DX-007).

use std::error::Error as _;
use std::io::{self, Read};

use lopdf::{Document, Object, ObjectId, dictionary};
use pdfplumber::{ExtractOptions, Pdf, PdfError, PdfErrorKind};

const SENSITIVE_DETAIL: &str = "private-reader-payload-4f4d9f";

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _output: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            SENSITIVE_DETAIL,
        ))
    }
}

fn valid_fixture() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdfs/test-punkt.pdf")
}

fn pdf_without_media_box() -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id: ObjectId = document.new_object_id();
    let page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1_i64,
        }),
    );
    let catalog_id = document.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn assert_send_sync<T: Send + Sync>() {}

fn expect_error<T>(result: Result<T, PdfError>) -> PdfError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected the operation to fail"),
    }
}

#[test]
fn io_error_preserves_the_concrete_source_without_default_disclosure() {
    assert_send_sync::<PdfError>();
    let error = expect_error(Pdf::open_reader(FailingReader, None));

    assert_eq!(error.kind(), PdfErrorKind::Io);
    assert_eq!(error.kind().code(), "IO");
    assert_eq!(error.context().operation, Some("read input"));

    let source = error.source().expect("I/O cause must be preserved");
    let io_error = source
        .downcast_ref::<io::Error>()
        .expect("the original std::io::Error must remain downcastable");
    assert_eq!(io_error.kind(), io::ErrorKind::ConnectionReset);
    assert_eq!(io_error.to_string(), SENSITIVE_DETAIL);

    let display = error.to_string();
    let debug = format!("{error:?}");
    assert!(display.contains("check that the source is available and readable"));
    assert!(!display.contains(SENSITIVE_DETAIL));
    assert!(!debug.contains(SENSITIVE_DETAIL));
}

#[test]
fn parse_error_is_typed_actionable_and_does_not_echo_input_bytes() {
    let sensitive_input = b"%PDF-1.7\nprivate-document-fragment-43b7d0\n";
    let error = expect_error(Pdf::open_bytes(sensitive_input, None));

    assert_eq!(error.kind(), PdfErrorKind::Parse);
    assert_eq!(error.context().operation, Some("open bytes"));
    assert!(error.source().is_some());
    assert!(error.to_string().contains("valid, supported PDF"));
    assert!(!error.to_string().contains("private-document-fragment"));
    assert!(!format!("{error:?}").contains("private-document-fragment"));
}

#[test]
fn resource_limit_metadata_remains_machine_readable() {
    let options = ExtractOptions {
        max_input_bytes: Some(32),
        ..ExtractOptions::default()
    };
    let error = expect_error(Pdf::open_bytes(&[0_u8; 33], Some(options)));

    assert_eq!(error.kind(), PdfErrorKind::ResourceLimit);
    let limit = error
        .resource_limit()
        .expect("resource limit details must be typed");
    assert_eq!(limit.name, "max_input_bytes");
    assert_eq!(limit.limit, 32);
    assert_eq!(limit.observed, 33);
    assert!(error.to_string().contains("use a smaller input"));
    assert!(error.source().is_none());
}

#[test]
fn unavailable_page_reports_the_requested_zero_based_index() {
    let pdf = Pdf::open_path(valid_fixture(), None).unwrap();
    let error = expect_error(pdf.page(usize::MAX));

    assert_eq!(error.kind(), PdfErrorKind::Parse);
    assert_eq!(error.context().operation, Some("load page"));
    assert_eq!(error.context().page_index, Some(usize::MAX));
    assert_eq!(error.context().object_id, None);
}

#[test]
fn malformed_page_reports_both_page_and_indirect_object_context() {
    let error = expect_error(Pdf::open_bytes(&pdf_without_media_box(), None));

    assert_eq!(error.kind(), PdfErrorKind::Parse);
    assert_eq!(error.context().operation, Some("read page media box"));
    assert_eq!(error.context().page_index, Some(0));
    let object = error
        .context()
        .object_id
        .expect("known page object ID must be attached");
    assert!(object.number > 0);
    assert_eq!(object.generation, 0);
    assert!(error.to_string().contains("page index 0"));
    assert!(error.to_string().contains("object"));
}
