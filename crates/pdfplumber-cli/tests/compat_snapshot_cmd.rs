//! Integration coverage for the compatibility-only page snapshot.

use assert_cmd::Command;
use std::io::Write;

fn cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("pdfplumber")
}

fn write_two_page_pdf() -> tempfile::NamedTempFile {
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
    for text in ["First page", "Second page"] {
        let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let resources = dictionary! {
            "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        };
        page_ids.push(doc.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box.clone(),
            "Contents" => Object::Reference(content_id),
            "Resources" => resources,
        }));
    }

    let kids: Vec<Object> = page_ids.iter().copied().map(Object::Reference).collect();
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => Object::Integer(page_ids.len() as i64),
    });
    for page_id in page_ids {
        doc.get_object_mut(page_id)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Parent", Object::Reference(pages_id));
    }
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let mut file = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn snapshot_reports_text_lines_and_structure_tree_for_every_page() {
    let file = write_two_page_pdf();
    let output = cmd()
        .args(["compat-snapshot", file.path().to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let pages: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let pages = pages.as_array().unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["page"], 1);
    assert_eq!(pages[1]["page"], 2);
    assert!(pages[0]["text_lines"].is_array());
    assert!(pages[0]["structure_tree"].is_array());
    assert_eq!(pages[0]["text_lines"][0]["words"][0]["text"], "First");
    assert_eq!(pages[1]["text_lines"][0]["words"][0]["text"], "Second");
}

#[test]
fn snapshot_transport_stays_hidden_from_public_help() {
    let output = cmd()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(output).unwrap();
    assert!(!help.contains("compat-snapshot"));
}
