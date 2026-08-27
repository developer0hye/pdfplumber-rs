//! Contracts for the high-level path, byte-buffer, and reader input family.

use std::error::Error as _;
use std::io::{self, Read};
use std::path::PathBuf;

use pdfplumber::{ExtractOptions, Pdf, PdfError, PdfErrorKind, TextOptions};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn valid_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdfs/test-punkt.pdf")
}

fn empty_fixture() -> PathBuf {
    repository_root().join("compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/empty.pdf")
}

struct ReadOnly {
    bytes: Vec<u8>,
    position: usize,
}

impl ReadOnly {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, position: 0 }
    }

    fn starting_at(bytes: Vec<u8>, position: usize) -> Self {
        Self { bytes, position }
    }
}

impl Read for ReadOnly {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let remaining = &self.bytes[self.position..];
        let count = remaining.len().min(output.len()).min(17);
        output[..count].copy_from_slice(&remaining[..count]);
        self.position += count;
        Ok(count)
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _output: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "reader disconnected",
        ))
    }
}

fn first_page_text(pdf: &Pdf) -> String {
    pdf.page(0).unwrap().extract_text(&TextOptions::default())
}

fn assert_input_limit(result: Result<Pdf, PdfError>, limit: usize) {
    match result {
        Err(error) => {
            assert_eq!(error.kind(), PdfErrorKind::ResourceLimit);
            let details = error.resource_limit().unwrap();
            assert_eq!(details.name, "max_input_bytes");
            assert_eq!(details.limit, limit);
            assert!(details.observed > limit);
        }
        Ok(_) => panic!("expected ResourceLimitExceeded, got Ok"),
    }
}

#[test]
fn path_bytes_and_read_only_reader_open_the_same_document() {
    let path_pdf = Pdf::open_path(valid_fixture(), None).unwrap();

    let bytes_pdf = {
        let bytes = std::fs::read(valid_fixture()).unwrap();
        Pdf::open_bytes(&bytes, None).unwrap()
    };

    let reader_pdf = {
        let reader = ReadOnly::new(std::fs::read(valid_fixture()).unwrap());
        Pdf::open_reader(reader, None).unwrap()
    };

    assert_eq!(path_pdf.page_count(), bytes_pdf.page_count());
    assert_eq!(path_pdf.page_count(), reader_pdf.page_count());
    assert_eq!(first_page_text(&path_pdf), first_page_text(&bytes_pdf));
    assert_eq!(first_page_text(&path_pdf), first_page_text(&reader_pdf));
}

#[test]
fn password_variants_use_the_same_three_input_names() {
    let bytes = std::fs::read(valid_fixture()).unwrap();

    let path_pdf = Pdf::open_path_with_password(valid_fixture(), b"unused", None).unwrap();
    let bytes_pdf = Pdf::open_bytes_with_password(&bytes, b"unused", None).unwrap();
    let reader_pdf = Pdf::open_reader_with_password(ReadOnly::new(bytes), b"unused", None).unwrap();

    assert_eq!(path_pdf.page_count(), bytes_pdf.page_count());
    assert_eq!(path_pdf.page_count(), reader_pdf.page_count());
}

#[test]
fn borrowed_reader_starts_at_its_current_position_and_is_not_retained() {
    let prefix = b"not part of the PDF";
    let mut bytes = prefix.to_vec();
    bytes.extend(std::fs::read(valid_fixture()).unwrap());
    let expected_end = bytes.len();
    let mut reader = ReadOnly::starting_at(bytes, prefix.len());

    let pdf = Pdf::open_reader(&mut reader, None).unwrap();

    assert_eq!(reader.position, expected_end);
    assert_eq!(
        pdf.page_count(),
        Pdf::open_path(valid_fixture(), None).unwrap().page_count()
    );
}

#[test]
fn invalid_pdf_data_is_a_parse_error_for_every_input_kind() {
    let empty = Vec::new();

    for result in [
        Pdf::open_path(empty_fixture(), None),
        Pdf::open_bytes(&empty, None),
        Pdf::open_reader(ReadOnly::new(empty), None),
    ] {
        assert_eq!(result.err().unwrap().kind(), PdfErrorKind::Parse);
    }
}

#[test]
fn path_and_reader_io_failures_share_the_io_error_variant() {
    let missing = repository_root().join("this-input-does-not-exist.pdf");

    assert_eq!(
        Pdf::open_path(missing, None).err().unwrap().kind(),
        PdfErrorKind::Io
    );
    match Pdf::open_reader(FailingReader, None) {
        Err(error) => {
            assert_eq!(error.kind(), PdfErrorKind::Io);
            assert_eq!(error.source().unwrap().to_string(), "reader disconnected");
        }
        Ok(_) => panic!("expected reader failure"),
    }
}

#[test]
fn max_input_bytes_is_enforced_for_every_input_kind() {
    let limit = 32;
    let bytes = std::fs::read(valid_fixture()).unwrap();
    let options = || {
        Some(ExtractOptions {
            max_input_bytes: Some(limit),
            ..ExtractOptions::default()
        })
    };

    assert_input_limit(Pdf::open_path(valid_fixture(), options()), limit);
    assert_input_limit(Pdf::open_bytes(&bytes, options()), limit);
    let mut limited_reader = ReadOnly::new(bytes.clone());
    assert_input_limit(Pdf::open_reader(&mut limited_reader, options()), limit);
    assert_eq!(limited_reader.position, limit + 1);
    assert_input_limit(
        Pdf::open_path_with_password(valid_fixture(), b"unused", options()),
        limit,
    );
    assert_input_limit(
        Pdf::open_bytes_with_password(&bytes, b"unused", options()),
        limit,
    );
    assert_input_limit(
        Pdf::open_reader_with_password(ReadOnly::new(bytes), b"unused", options()),
        limit,
    );
}
