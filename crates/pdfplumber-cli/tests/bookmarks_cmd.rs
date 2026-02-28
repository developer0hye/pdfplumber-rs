//! Integration tests for the `bookmarks` subcommand (US-062).

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

fn cmd() -> Command {
    Command::cargo_bin("pdfplumber").unwrap()
}

/// Create a PDF with multi-level bookmarks.
fn pdf_with_bookmarks() -> Vec<u8> {
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
    for text in &["Chapter 1", "Chapter 2"] {
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
        "Count" => Object::Integer(2),
    };
    let pages_id = doc.add_object(pages_dict);

    for &pid in &page_ids {
        if let Ok(page_obj) = doc.get_object_mut(pid) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }
    }

    // Create outline: Chapter 1 → page 1, Chapter 2 → page 2
    let ch1_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Chapter 1"),
        "Dest" => vec![Object::Reference(page_ids[0]), Object::Name(b"Fit".to_vec())],
    });
    let ch2_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Chapter 2"),
        "Dest" => vec![Object::Reference(page_ids[1]), Object::Name(b"Fit".to_vec())],
    });

    if let Ok(obj) = doc.get_object_mut(ch1_id) {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Next", Object::Reference(ch2_id));
        }
    }
    if let Ok(obj) = doc.get_object_mut(ch2_id) {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Prev", Object::Reference(ch1_id));
        }
    }

    let outlines_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => Object::Reference(ch1_id),
        "Last" => Object::Reference(ch2_id),
        "Count" => Object::Integer(2),
    });

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
        "Outlines" => Object::Reference(outlines_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

#[test]
fn bookmarks_text_format_shows_entries() {
    let bytes = pdf_with_bookmarks();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args(["bookmarks", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Chapter 1"))
        .stdout(predicate::str::contains("Chapter 2"));
}

#[test]
fn bookmarks_json_format_shows_entries() {
    let bytes = pdf_with_bookmarks();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&bytes).unwrap();

    cmd()
        .args([
            "bookmarks",
            tmp.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"title\":\"Chapter 1\""))
        .stdout(predicate::str::contains("\"title\":\"Chapter 2\""));
}

#[test]
fn bookmarks_no_outlines() {
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

    cmd()
        .args(["bookmarks", tmp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No bookmarks found."));
}
