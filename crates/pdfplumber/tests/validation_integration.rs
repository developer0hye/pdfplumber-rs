//! Integration tests for PDF validation (US-082).
//!
//! Tests the full Pdf::validate() pipeline with programmatically
//! created PDFs containing various specification violations.

use pdfplumber::{Pdf, Severity, ValidationIssue};

// --- Test PDF creation helpers ---

/// Create a valid single-page PDF with correct structure.
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

/// Create a PDF with the catalog missing /Type key.
fn pdf_missing_catalog_type() -> Vec<u8> {
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

    // Catalog WITHOUT /Type key
    let catalog_id = doc.add_object(dictionary! {
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Create a PDF with a broken object reference in the page.
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

    // Reference a non-existent object (999, 0)
    let broken_ref = Object::Reference((999, 0));

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

/// Create a PDF where content references a font not in resources.
fn pdf_with_missing_font() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Content references /F2 but resources only has /F1
    let content = b"BT /F2 12 Tf 72 720 Td (Hello) Tj ET";
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

// --- Tests ---

#[test]
fn validate_valid_pdf_no_issues() {
    let bytes = valid_pdf();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();
    assert!(
        issues.is_empty(),
        "expected no issues for valid PDF, got: {issues:?}"
    );
}

#[test]
fn validate_pdf_missing_catalog_type() {
    let bytes = pdf_missing_catalog_type();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();

    let type_issues: Vec<&ValidationIssue> = issues
        .iter()
        .filter(|i| i.code == "MISSING_TYPE" && i.message.contains("catalog"))
        .collect();

    assert!(
        !type_issues.is_empty(),
        "expected MISSING_TYPE issue for catalog, got issues: {issues:?}"
    );
    assert_eq!(type_issues[0].severity, Severity::Warning);
}

#[test]
fn validate_pdf_with_broken_reference() {
    let bytes = pdf_with_broken_reference();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();

    let broken_ref_issues: Vec<&ValidationIssue> =
        issues.iter().filter(|i| i.code == "BROKEN_REF").collect();

    assert!(
        !broken_ref_issues.is_empty(),
        "expected BROKEN_REF issue, got issues: {issues:?}"
    );
    // Check it references object 999
    let found = broken_ref_issues.iter().any(|i| i.message.contains("999"));
    assert!(
        found,
        "expected broken reference to mention object 999, got: {broken_ref_issues:?}"
    );
}

#[test]
fn validate_pdf_with_missing_font() {
    let bytes = pdf_with_missing_font();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();

    let font_issues: Vec<&ValidationIssue> =
        issues.iter().filter(|i| i.code == "MISSING_FONT").collect();

    assert!(
        !font_issues.is_empty(),
        "expected MISSING_FONT issue, got issues: {issues:?}"
    );
    assert_eq!(font_issues[0].severity, Severity::Warning);
    assert!(
        font_issues[0].message.contains("F2"),
        "expected message to mention F2, got: {}",
        font_issues[0].message
    );
}

#[test]
fn validate_issue_counts() {
    let bytes = pdf_missing_catalog_type();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();

    let error_count = issues.iter().filter(|i| i.is_error()).count();
    let warning_count = issues.iter().filter(|i| i.is_warning()).count();

    // Should have at least one warning (missing catalog /Type)
    assert!(
        warning_count > 0,
        "expected at least one warning, got {warning_count}"
    );
    // And no errors since the PDF is otherwise valid
    assert_eq!(error_count, 0, "expected no errors, got {error_count}");
}

#[test]
fn validate_issue_has_location() {
    let bytes = pdf_missing_catalog_type();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let issues = pdf.validate().unwrap();

    let type_issue = issues
        .iter()
        .find(|i| i.code == "MISSING_TYPE" && i.message.contains("catalog"))
        .expect("expected MISSING_TYPE issue");

    assert!(
        type_issue.location.is_some(),
        "expected issue to have a location"
    );
}
