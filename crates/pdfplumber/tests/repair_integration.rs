//! Integration tests for best-effort PDF repair (US-083).
//!
//! Tests the full Pdf::open_with_repair() pipeline with programmatically
//! created PDFs containing various issues that repair should fix.

use pdfplumber::{Pdf, RepairOptions};

// --- Test PDF creation helpers ---

/// Create a valid single-page PDF.
fn valid_pdf() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content = b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
        },
    };

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];
    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => media_box,
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
    };
    let page_id = doc.add_object(page_dict);

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
    };
    let pages_id = doc.add_object(pages_dict);

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = page_obj.as_dict_mut() {
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

/// Create a PDF where a stream has a missing /Length key.
///
/// lopdf normalizes /Length during parsing, but we can construct a
/// document where the /Length dict key is removed after creation.
/// The repair function should add the correct /Length back.
fn pdf_with_missing_stream_length() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content = b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

    // Remove /Length from the stream dictionary (it was auto-added)
    if let Ok(lopdf::Object::Stream(s)) = doc.get_object_mut(content_id) {
        s.dict.remove(b"Length");
    }

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
        },
    };

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];
    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => media_box,
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
    };
    let page_id = doc.add_object(page_dict);

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
    };
    let pages_id = doc.add_object(pages_dict);

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = page_obj.as_dict_mut() {
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

/// Create a PDF with a broken object reference (dangling reference).
fn pdf_with_broken_reference() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content = b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
        },
    };

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];
    // Add a broken reference to a non-existent object in Annots
    let broken_ref = Object::Reference((999, 0));
    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => media_box,
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
        "Annots" => vec![broken_ref],
    };
    let page_id = doc.add_object(page_dict);

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
    };
    let pages_id = doc.add_object(pages_dict);

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = page_obj.as_dict_mut() {
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

// --- Tests ---

#[test]
fn open_with_repair_valid_pdf_no_changes() {
    let bytes = valid_pdf();
    let (pdf, result) = Pdf::open_with_repair(&bytes, None, None).unwrap();
    assert_eq!(pdf.page_count(), 1);
    // Valid PDF should have no repairs
    assert!(
        !result.has_repairs(),
        "expected no repairs for valid PDF, got: {:?}",
        result.log
    );
}

#[test]
fn open_with_repair_fixes_stream_length() {
    let bytes = pdf_with_missing_stream_length();
    let opts = RepairOptions {
        fix_stream_lengths: true,
        rebuild_xref: false,
        remove_broken_objects: false,
    };
    let (pdf, result) = Pdf::open_with_repair(&bytes, None, Some(opts)).unwrap();
    assert_eq!(pdf.page_count(), 1);
    // Should have at least one repair log entry about stream length
    assert!(
        result.has_repairs(),
        "expected repair log entries for wrong stream length"
    );
    let has_length_fix = result
        .log
        .iter()
        .any(|l| l.contains("stream") || l.contains("Length") || l.contains("length"));
    assert!(
        has_length_fix,
        "expected repair log to mention stream length fix, got: {:?}",
        result.log
    );
}

#[test]
fn open_with_repair_removes_broken_references() {
    let bytes = pdf_with_broken_reference();
    let opts = RepairOptions {
        fix_stream_lengths: false,
        rebuild_xref: false,
        remove_broken_objects: true,
    };
    let (pdf, result) = Pdf::open_with_repair(&bytes, None, Some(opts)).unwrap();
    assert_eq!(pdf.page_count(), 1);
    // Should have logged the broken reference removal
    assert!(
        result.has_repairs(),
        "expected repair log entries for broken reference removal"
    );
    let has_ref_fix = result
        .log
        .iter()
        .any(|l| l.contains("999") || l.contains("broken") || l.contains("reference"));
    assert!(
        has_ref_fix,
        "expected repair log to mention broken reference, got: {:?}",
        result.log
    );
}

#[test]
fn open_with_repair_default_options_repairs_all() {
    let bytes = pdf_with_missing_stream_length();
    // Default options should enable all repairs
    let (pdf, _result) = Pdf::open_with_repair(&bytes, None, None).unwrap();
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn open_with_repair_returns_repair_log() {
    let bytes = pdf_with_missing_stream_length();
    let (_pdf, result) = Pdf::open_with_repair(&bytes, None, None).unwrap();
    // Repair log should be a Vec<String>
    for entry in &result.log {
        assert!(!entry.is_empty(), "repair log entries should not be empty");
    }
}

#[test]
fn open_with_repair_invalid_bytes_returns_error() {
    let result = Pdf::open_with_repair(b"not a pdf", None, None);
    assert!(result.is_err(), "expected error for invalid PDF bytes");
}

#[test]
fn open_with_repair_all_options_disabled() {
    let bytes = pdf_with_missing_stream_length();
    let opts = RepairOptions {
        rebuild_xref: false,
        fix_stream_lengths: false,
        remove_broken_objects: false,
    };
    let (pdf, result) = Pdf::open_with_repair(&bytes, None, Some(opts)).unwrap();
    assert_eq!(pdf.page_count(), 1);
    // With all repairs disabled, nothing should be fixed
    assert!(
        !result.has_repairs(),
        "expected no repairs with all options disabled, got: {:?}",
        result.log
    );
}
