//! Integration tests for the `search` subcommand (US-063).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

/// Create a PDF with text "Hello World" on page 1.
fn pdf_with_text() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)],
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

#[test]
fn search_text_format_shows_match() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "search",
            tmp.path().to_str().unwrap(),
            "Hello",
            "--no-regex",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("page\ttext\tx0\ttop\tx1\tbottom"))
        .stdout(predicate::str::contains("Hello"));
}

#[test]
fn search_json_format_shows_match() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "search",
            tmp.path().to_str().unwrap(),
            "World",
            "--no-regex",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\":\"World\""));
}

#[test]
fn search_csv_format_shows_match() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "search",
            tmp.path().to_str().unwrap(),
            "Hello",
            "--no-regex",
            "--format",
            "csv",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("page,text,x0,top,x1,bottom"))
        .stdout(predicate::str::contains("Hello"));
}

#[test]
fn search_no_match_shows_only_header() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "search",
            tmp.path().to_str().unwrap(),
            "NONEXISTENT",
            "--no-regex",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("page\ttext\tx0\ttop\tx1\tbottom"))
        .stdout(predicate::str::contains("NONEXISTENT").not());
}

#[test]
fn search_case_insensitive() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "search",
            tmp.path().to_str().unwrap(),
            "hello",
            "--no-regex",
            "--case-insensitive",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello"));
}

#[test]
fn search_regex_pattern() {
    let bytes = pdf_with_text();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args(["search", tmp.path().to_str().unwrap(), "H.llo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello"));
}
