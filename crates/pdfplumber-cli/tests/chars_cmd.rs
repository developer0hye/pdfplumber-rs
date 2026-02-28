//! Integration tests for the `chars` subcommand (US-052).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

/// Create a single-page PDF with the given content stream using lopdf.
fn pdf_with_content(content: &[u8]) -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

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

/// Create a multi-page PDF. Each page has a single line of text.
fn pdf_with_pages(texts: &[&str]) -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    let mut page_ids = Vec::new();
    for text in texts {
        let content_str = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
        let stream = Stream::new(dictionary! {}, content_str.into_bytes());
        let content_id = doc.add_object(stream);

        let resources = dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        };

        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box.clone(),
            "Contents" => Object::Reference(content_id),
            "Resources" => resources,
        };
        page_ids.push(doc.add_object(page_dict));
    }

    let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => Object::Integer(texts.len() as i64),
    };
    let pages_id = doc.add_object(pages_dict);

    for &pid in &page_ids {
        if let Ok(page_obj) = doc.get_object_mut(pid) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
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

/// Write PDF bytes to a temporary file and return the path.
fn write_temp_pdf(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
    f
}

// --- Default text format (tab-separated) tests ---

#[test]
fn chars_default_text_format_outputs_tab_separated() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (AB) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should contain tab-separated fields including char text
    assert!(stdout.contains("A"));
    assert!(stdout.contains("B"));
    // Should contain tab characters (TSV format)
    assert!(stdout.contains('\t'));
}

#[test]
fn chars_text_format_has_header_line() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (X) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();

    // Header should contain column names
    assert!(first_line.contains("page"));
    assert!(first_line.contains("text"));
    assert!(first_line.contains("x0"));
    assert!(first_line.contains("top"));
    assert!(first_line.contains("fontname"));
    assert!(first_line.contains("size"));
}

// --- JSON output tests ---

#[test]
fn chars_json_format_outputs_valid_json_with_all_fields() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hi) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Output should be valid JSON (array of char objects)
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(!arr.is_empty());

    // Check required fields on first char object
    let ch = &arr[0];
    assert!(ch.get("page").is_some(), "missing 'page' field");
    assert!(ch.get("text").is_some(), "missing 'text' field");
    assert!(ch.get("fontname").is_some(), "missing 'fontname' field");
    assert!(ch.get("size").is_some(), "missing 'size' field");
    assert!(ch.get("x0").is_some(), "missing 'x0' field");
    assert!(ch.get("top").is_some(), "missing 'top' field");
    assert!(ch.get("x1").is_some(), "missing 'x1' field");
    assert!(ch.get("bottom").is_some(), "missing 'bottom' field");
    assert!(ch.get("doctop").is_some(), "missing 'doctop' field");
    assert!(ch.get("upright").is_some(), "missing 'upright' field");
    assert!(ch.get("direction").is_some(), "missing 'direction' field");
}

#[test]
fn chars_json_contains_correct_text_values() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (AB) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();

    let texts: Vec<&str> = arr.iter().map(|c| c["text"].as_str().unwrap()).collect();
    assert!(texts.contains(&"A"));
    assert!(texts.contains(&"B"));
}

// --- CSV output tests ---

#[test]
fn chars_csv_format_outputs_csv_with_header() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Z) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap(), "--format", "csv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    // First line is CSV header
    assert!(lines.len() >= 2);
    let header = lines[0];
    assert_eq!(header, "page,text,x0,top,x1,bottom,fontname,size");

    // Data line should contain "Z"
    assert!(lines[1].contains("Z"));
}

#[test]
fn chars_csv_format_columns_match_header() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Q) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["chars", f.path().to_str().unwrap(), "--format", "csv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    let header_cols = lines[0].split(',').count();
    for line in &lines[1..] {
        if line.is_empty() {
            continue;
        }
        assert_eq!(
            line.split(',').count(),
            header_cols,
            "data line column count should match header"
        );
    }
}

// --- Page filtering tests ---

#[test]
fn chars_pages_option_filters_pages() {
    let pdf_bytes = pdf_with_pages(&["First", "Second", "Third"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args([
            "chars",
            f.path().to_str().unwrap(),
            "--format",
            "json",
            "--pages",
            "1,3",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();

    // Should only have chars from pages 1 and 3
    let pages: std::collections::HashSet<i64> =
        arr.iter().map(|c| c["page"].as_i64().unwrap()).collect();
    assert!(pages.contains(&1));
    assert!(pages.contains(&3));
    assert!(!pages.contains(&2));
}

// --- Error handling tests ---

#[test]
fn chars_file_not_found_error() {
    cmd()
        .args(["chars", "nonexistent_file.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No such file")));
}

#[test]
fn chars_invalid_page_range_error() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (X) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["chars", f.path().to_str().unwrap(), "--pages", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn chars_exit_code_zero_on_success() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (T) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["chars", f.path().to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}
