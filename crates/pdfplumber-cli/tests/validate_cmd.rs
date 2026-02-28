//! Integration tests for the `validate` subcommand (US-082).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

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

/// Create a PDF with catalog missing /Type key.
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

fn write_temp_pdf(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(bytes).unwrap();
    tmp.flush().unwrap();
    tmp
}

#[test]
fn validate_text_format_valid_pdf() {
    let bytes = valid_pdf();
    let tmp = write_temp_pdf(&bytes);

    cmd()
        .args(["validate", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn validate_text_format_with_issues() {
    let bytes = pdf_missing_catalog_type();
    let tmp = write_temp_pdf(&bytes);

    cmd()
        .args(["validate", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("MISSING_TYPE"))
        .stdout(predicate::str::contains("Summary:"));
}

#[test]
fn validate_json_format() {
    let bytes = pdf_missing_catalog_type();
    let tmp = write_temp_pdf(&bytes);

    let output = cmd()
        .args(["validate", tmp.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(json["issues"].is_array());
    assert!(json["summary"]["errors"].is_number());
    assert!(json["summary"]["warnings"].is_number());
}

#[test]
fn validate_json_format_valid_pdf() {
    let bytes = valid_pdf();
    let tmp = write_temp_pdf(&bytes);

    let output = cmd()
        .args(["validate", tmp.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["issues"].as_array().unwrap().len(), 0);
    assert_eq!(json["summary"]["errors"], 0);
    assert_eq!(json["summary"]["warnings"], 0);
}

#[test]
fn validate_file_not_found() {
    cmd()
        .args(["validate", "/nonexistent/file.pdf"])
        .assert()
        .failure();
}
