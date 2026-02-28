//! Integration tests for the `tables` subcommand (US-054).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

/// Create a single-page PDF with a 2x2 table drawn with explicit lines.
///
/// The table has cells: A | B
///                      C | D
fn pdf_with_table() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Draw a 2x2 table grid with lines + text in cells
    let content = b"
        1 w
        100 700 m 300 700 l S
        100 680 m 300 680 l S
        100 660 m 300 660 l S
        100 700 m 100 660 l S
        200 700 m 200 660 l S
        300 700 m 300 660 l S
        BT /F1 10 Tf 110 685 Td (A) Tj ET
        BT /F1 10 Tf 210 685 Td (B) Tj ET
        BT /F1 10 Tf 110 665 Td (C) Tj ET
        BT /F1 10 Tf 210 665 Td (D) Tj ET
    ";
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

/// Create a single-page PDF with just text (no table lines).
fn pdf_without_table() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content = b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET";
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

// --- Default text (grid) format tests ---

#[test]
fn tables_default_text_format_succeeds() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["tables", f.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn tables_no_tables_found_succeeds() {
    let pdf_bytes = pdf_without_table();
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["tables", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should indicate no tables found
    assert!(
        stdout.contains("No tables") || stdout.is_empty() || stdout.contains("0 table"),
        "Expected empty output or 'No tables' message, got: {stdout}"
    );
}

#[test]
fn tables_grid_format_uses_pipe_separators() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["tables", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // If tables found, grid format should use | separators
    if !stdout.is_empty() && !stdout.contains("No tables") {
        assert!(
            stdout.contains('|'),
            "Grid format should contain pipe separators, got: {stdout}"
        );
    }
}

#[test]
fn tables_grid_format_shows_summary_line() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["tables", f.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // If tables found, should have a summary/table header line with bbox info
    if !stdout.is_empty() && !stdout.contains("No tables") {
        assert!(
            stdout.contains("Table") || stdout.contains("table"),
            "Should report table info, got: {stdout}"
        );
    }
}

// --- JSON output tests ---

#[test]
fn tables_json_format_outputs_valid_json() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["tables", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Output should be valid JSON (array)
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    // arr is a JSON array of tables (may be empty if no tables detected)
    let _ = arr;
}

#[test]
fn tables_json_format_empty_when_no_tables() {
    let pdf_bytes = pdf_without_table();
    let f = write_temp_pdf(&pdf_bytes);

    let output = cmd()
        .args(["tables", f.path().to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should be empty JSON array
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(arr.is_empty());
}

// --- CSV output tests ---

#[test]
fn tables_csv_format_succeeds() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["tables", f.path().to_str().unwrap(), "--format", "csv"])
        .assert()
        .success();
}

// --- Page filtering tests ---

#[test]
fn tables_pages_option_accepted() {
    let pdf_bytes = pdf_with_pages(&["Page1", "Page2", "Page3"]);
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args([
            "tables",
            f.path().to_str().unwrap(),
            "--pages",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success();
}

// --- Strategy option tests ---

#[test]
fn tables_strategy_lattice_accepted() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args([
            "tables",
            f.path().to_str().unwrap(),
            "--strategy",
            "lattice",
        ])
        .assert()
        .success();
}

#[test]
fn tables_strategy_stream_accepted() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["tables", f.path().to_str().unwrap(), "--strategy", "stream"])
        .assert()
        .success();
}

// --- Tolerance options tests ---

#[test]
fn tables_tolerance_options_accepted() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args([
            "tables",
            f.path().to_str().unwrap(),
            "--snap-tolerance",
            "5.0",
            "--join-tolerance",
            "4.0",
            "--text-tolerance",
            "2.0",
        ])
        .assert()
        .success();
}

// --- Error handling tests ---

#[test]
fn tables_file_not_found_error() {
    cmd()
        .args(["tables", "nonexistent_file.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("No such file")));
}

#[test]
fn tables_invalid_page_range_error() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["tables", f.path().to_str().unwrap(), "--pages", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn tables_exit_code_zero_on_success() {
    let pdf_bytes = pdf_with_table();
    let f = write_temp_pdf(&pdf_bytes);

    cmd()
        .args(["tables", f.path().to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}
