//! Integration tests for the `info` subcommand (US-056).

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
fn info_shows_page_count() {
    let pdf_bytes = pdf_with_pages(&["Hello", "World"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pages: 2"));
}

#[test]
fn info_shows_page_dimensions() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("612.00 x 792.00"));
}

#[test]
fn info_shows_rotation() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Rotation:"));
}

#[test]
fn info_shows_object_counts() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hi) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Chars:"))
        .stdout(predicate::str::contains("Lines:"))
        .stdout(predicate::str::contains("Rects:"))
        .stdout(predicate::str::contains("Curves:"))
        .stdout(predicate::str::contains("Images:"));
}

#[test]
fn info_shows_summary() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (ABC) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Summary:"))
        .stdout(predicate::str::contains("Total chars:"))
        .stdout(predicate::str::contains("Total tables:"));
}

#[test]
fn info_multi_page_shows_each_page() {
    let pdf_bytes = pdf_with_pages(&["A", "B", "C"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Page 1:"))
        .stdout(predicate::str::contains("Page 2:"))
        .stdout(predicate::str::contains("Page 3:"));
}

// --- Page filtering tests ---

#[test]
fn info_pages_option_filters_pages() {
    let pdf_bytes = pdf_with_pages(&["A", "B", "C"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap(), "--pages", "1,3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Page 1:"))
        .stdout(predicate::str::contains("Page 3:"))
        .stdout(predicate::str::contains("Page 2:").not());
}

// --- JSON output tests ---

#[test]
fn info_json_format_outputs_valid_json() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["info", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should be valid JSON
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v.is_object());
}

#[test]
fn info_json_has_required_fields() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["info", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Top-level fields
    assert!(v.get("pages").is_some(), "missing 'pages' field");
    assert!(v.get("page_info").is_some(), "missing 'page_info' field");
    assert!(v.get("summary").is_some(), "missing 'summary' field");
}

#[test]
fn info_json_page_info_has_all_fields() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["info", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let page_info = v["page_info"].as_array().unwrap();
    assert_eq!(page_info.len(), 1);

    let page = &page_info[0];
    assert!(page.get("page").is_some(), "missing 'page'");
    assert!(page.get("width").is_some(), "missing 'width'");
    assert!(page.get("height").is_some(), "missing 'height'");
    assert!(page.get("rotation").is_some(), "missing 'rotation'");
    assert!(page.get("chars").is_some(), "missing 'chars'");
    assert!(page.get("lines").is_some(), "missing 'lines'");
    assert!(page.get("rects").is_some(), "missing 'rects'");
    assert!(page.get("curves").is_some(), "missing 'curves'");
    assert!(page.get("images").is_some(), "missing 'images'");
}

#[test]
fn info_json_summary_has_totals() {
    let pdf_bytes = pdf_with_pages(&["Hello", "World"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["info", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let summary = &v["summary"];
    assert!(
        summary.get("total_chars").is_some(),
        "missing 'total_chars'"
    );
    assert!(
        summary.get("total_tables").is_some(),
        "missing 'total_tables'"
    );
}

#[test]
fn info_json_page_dimensions_are_correct() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (X) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["info", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let page = &v["page_info"][0];
    assert_eq!(page["width"].as_f64().unwrap(), 612.0);
    assert_eq!(page["height"].as_f64().unwrap(), 792.0);
    assert_eq!(page["page"].as_u64().unwrap(), 1);
}

#[test]
fn info_json_pages_filter_works() {
    let pdf_bytes = pdf_with_pages(&["A", "B", "C"]);
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args([
            "info",
            f.path().to_str().unwrap(),
            "--format",
            "json",
            "--pages",
            "2",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let page_info = v["page_info"].as_array().unwrap();
    assert_eq!(page_info.len(), 1);
    assert_eq!(page_info[0]["page"].as_u64().unwrap(), 2);
}

// --- Error handling tests ---

#[test]
fn info_file_not_found_error() {
    cmd()
        .args(["info", "nonexistent_file.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No such file")));
}

#[test]
fn info_invalid_pdf_error() {
    let mut f = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
    f.write_all(b"this is not a pdf").unwrap();
    f.flush().unwrap();

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn info_invalid_page_range_error() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap(), "--pages", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn info_exit_code_zero_on_success() {
    let pdf_bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET");
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["info", f.path().to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}
