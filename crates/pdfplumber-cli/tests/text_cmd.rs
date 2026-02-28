//! Integration tests for the `text` subcommand (US-051).

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

// --- Text output tests ---

#[test]
fn text_extracts_from_single_page() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

#[test]
fn text_shows_page_separators_for_multi_page() {
    let pdf_bytes = pdf_with_pages(&["Page One", "Page Two", "Page Three"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("--- Page 1 ---"))
        .stdout(predicate::str::contains("Page One"))
        .stdout(predicate::str::contains("--- Page 2 ---"))
        .stdout(predicate::str::contains("Page Two"))
        .stdout(predicate::str::contains("--- Page 3 ---"))
        .stdout(predicate::str::contains("Page Three"));
}

#[test]
fn text_pages_option_filters_pages() {
    let pdf_bytes = pdf_with_pages(&["First", "Second", "Third"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap(), "--pages", "1,3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("First"))
        .stdout(predicate::str::contains("Third"))
        .stdout(predicate::str::contains("Second").not());
}

#[test]
fn text_pages_range_option() {
    let pdf_bytes = pdf_with_pages(&["A", "B", "C", "D", "E"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap(), "--pages", "2-4"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--- Page 2 ---"))
        .stdout(predicate::str::contains("B"))
        .stdout(predicate::str::contains("--- Page 3 ---"))
        .stdout(predicate::str::contains("C"))
        .stdout(predicate::str::contains("--- Page 4 ---"))
        .stdout(predicate::str::contains("D"))
        .stdout(predicate::str::contains("--- Page 1 ---").not());
}

#[test]
fn text_layout_flag_accepted() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Layout test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap(), "--layout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Layout test"));
}

// --- JSON output tests ---

#[test]
fn text_json_format_outputs_json_lines() {
    let pdf_bytes = pdf_with_pages(&["Hello", "World"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["text", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Each line should be valid JSON with "page" and "text" fields
    for line in stdout.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("page").is_some(), "missing 'page' field");
        assert!(v.get("text").is_some(), "missing 'text' field");
    }
}

#[test]
fn text_json_format_page_numbers() {
    let pdf_bytes = pdf_with_pages(&["First", "Second"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["text", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);

    let v0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v0["page"], 1);
    assert!(v0["text"].as_str().unwrap().contains("First"));

    let v1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(v1["page"], 2);
    assert!(v1["text"].as_str().unwrap().contains("Second"));
}

// --- Error handling tests ---

#[test]
fn text_file_not_found_error() {
    cmd()
        .args(["text", "nonexistent_file.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No such file")));
}

#[test]
fn text_invalid_pdf_error() {
    let mut f = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
    f.write_all(b"this is not a pdf").unwrap();
    f.flush().unwrap();

    cmd()
        .args(["text", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn text_invalid_page_range_error() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    // Page 0 is invalid (1-indexed)
    cmd()
        .args(["text", f.path().to_str().unwrap(), "--pages", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn text_page_out_of_range_error() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    // Only 1 page, but requesting page 99
    cmd()
        .args(["text", f.path().to_str().unwrap(), "--pages", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds"));
}

#[test]
fn text_exit_code_zero_on_success() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["text", f.path().to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}
