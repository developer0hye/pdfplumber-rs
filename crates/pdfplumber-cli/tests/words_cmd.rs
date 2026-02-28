//! Integration tests for the `words` subcommand (US-053).

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
fn words_default_text_format_outputs_tab_separated() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should contain words
    assert!(stdout.contains("Hello"));
    assert!(stdout.contains("World"));
    // Should contain tab characters (TSV format)
    assert!(stdout.contains('\t'));
}

#[test]
fn words_text_format_has_header_line() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap()])
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
    assert!(first_line.contains("x1"));
    assert!(first_line.contains("bottom"));
}

// --- JSON output tests ---

#[test]
fn words_json_format_outputs_valid_json_with_all_fields() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Output should be valid JSON (array of word objects)
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(!arr.is_empty());

    // Check required fields on first word object
    let word = &arr[0];
    assert!(word.get("page").is_some(), "missing 'page' field");
    assert!(word.get("text").is_some(), "missing 'text' field");
    assert!(word.get("x0").is_some(), "missing 'x0' field");
    assert!(word.get("top").is_some(), "missing 'top' field");
    assert!(word.get("x1").is_some(), "missing 'x1' field");
    assert!(word.get("bottom").is_some(), "missing 'bottom' field");
    assert!(word.get("doctop").is_some(), "missing 'doctop' field");
    assert!(word.get("direction").is_some(), "missing 'direction' field");
}

#[test]
fn words_json_contains_correct_text_values() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();

    let texts: Vec<&str> = arr.iter().map(|w| w["text"].as_str().unwrap()).collect();
    assert!(texts.contains(&"Hello"));
    assert!(texts.contains(&"World"));
}

// --- CSV output tests ---

#[test]
fn words_csv_format_outputs_csv_with_header() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap(), "--format", "csv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    // First line is CSV header
    assert!(lines.len() >= 2);
    let header = lines[0];
    assert_eq!(header, "page,text,x0,top,x1,bottom");

    // Data line should contain "Test"
    assert!(lines[1].contains("Test"));
}

#[test]
fn words_csv_format_columns_match_header() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["words", f.path().to_str().unwrap(), "--format", "csv"])
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
fn words_pages_option_filters_pages() {
    let pdf_bytes = pdf_with_pages(&["First", "Second", "Third"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args([
            "words",
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

    // Should only have words from pages 1 and 3
    let pages: std::collections::HashSet<i64> =
        arr.iter().map(|w| w["page"].as_i64().unwrap()).collect();
    assert!(pages.contains(&1));
    assert!(pages.contains(&3));
    assert!(!pages.contains(&2));
}

// --- Tolerance options tests ---

#[test]
fn words_tolerance_options_accepted() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    // Should not error with tolerance options
    cmd()
        .args([
            "words",
            f.path().to_str().unwrap(),
            "--x-tolerance",
            "5.0",
            "--y-tolerance",
            "2.0",
        ])
        .assert()
        .success();
}

// --- Error handling tests ---

#[test]
fn words_file_not_found_error() {
    cmd()
        .args(["words", "nonexistent_file.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No such file")));
}

#[test]
fn words_invalid_page_range_error() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (X) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["words", f.path().to_str().unwrap(), "--pages", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn words_exit_code_zero_on_success() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["words", f.path().to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}
