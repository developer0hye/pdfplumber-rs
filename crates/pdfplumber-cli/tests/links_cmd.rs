//! Integration tests for the `links` subcommand (US-061).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

/// Create a PDF with a Link annotation that has a URI action.
fn pdf_with_uri_link() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Click here) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    let annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => vec![Object::Integer(72), Object::Integer(710), Object::Integer(200), Object::Integer(730)],
        "A" => dictionary! {
            "S" => "URI",
            "URI" => Object::string_literal("https://example.com"),
        },
    });

    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)],
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
        "Annots" => vec![Object::Reference(annot_id)],
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
fn links_text_format_shows_hyperlink() {
    let bytes = pdf_with_uri_link();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args(["links", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://example.com"));
}

#[test]
fn links_json_format_shows_hyperlink() {
    let bytes = pdf_with_uri_link();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args(["links", tmp.path().to_str().unwrap(), "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"uri\":\"https://example.com\""));
}

#[test]
fn links_csv_format_shows_hyperlink() {
    let bytes = pdf_with_uri_link();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args(["links", tmp.path().to_str().unwrap(), "--format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("page,uri,x0,top,x1,bottom"))
        .stdout(predicate::str::contains("https://example.com"));
}

#[test]
fn links_empty_page_no_links() {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);
    let resources = dictionary! { "Font" => dictionary! { "F1" => Object::Reference(font_id) } };
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

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();

    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    // Should only show the header line (no link data lines)
    cmd()
        .args(["links", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("page\turi\tx0\ttop\tx1\tbottom"));
}
