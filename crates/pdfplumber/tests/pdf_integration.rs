//! Integration tests for the Pdf public API (US-017).
//!
//! These tests exercise the full end-to-end pipeline:
//! PDF bytes → Pdf::open → Page → chars/extract_text.
//!
//! Test PDFs are created programmatically using lopdf.

use pdfplumber::{DocumentMetadata, ExtractOptions, Pdf, TextOptions, WordOptions};

// --- Test PDF creation helpers ---

/// Create a single-page PDF with the given content stream.
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

// --- End-to-end integration tests ---

#[test]
fn end_to_end_single_page_hello_world() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();

    // Document level
    assert_eq!(pdf.page_count(), 1);

    // Page level
    let page = pdf.page(0).unwrap();
    assert_eq!(page.page_number(), 0);
    assert_eq!(page.width(), 612.0);
    assert_eq!(page.height(), 792.0);
    assert_eq!(page.rotation(), 0);

    // Page bbox
    let bbox = page.bbox();
    assert_eq!(bbox.x0, 0.0);
    assert_eq!(bbox.top, 0.0);
    assert_eq!(bbox.x1, 612.0);
    assert_eq!(bbox.bottom, 792.0);

    // Characters
    let chars = page.chars();
    assert_eq!(chars.len(), 11); // "Hello World" = 11 chars

    // Verify character content
    let text_from_chars: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text_from_chars, "Hello World");

    // Characters should have positive-sized bounding boxes
    for ch in chars {
        assert!(ch.bbox.width() > 0.0, "char '{}' has zero width", ch.text);
        assert!(ch.bbox.height() > 0.0, "char '{}' has zero height", ch.text);
    }

    // Words
    let words = page.extract_words(&WordOptions::default());
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].text, "Hello");
    assert_eq!(words[1].text, "World");

    // Text extraction (layout=false)
    let text = page.extract_text(&TextOptions::default());
    assert_eq!(text, "Hello World");
}

#[test]
fn end_to_end_multiline_text() {
    // Three lines of text
    let content =
        b"BT /F1 12 Tf 72 720 Td (Line One) Tj 0 -20 Td (Line Two) Tj 0 -20 Td (Line Three) Tj ET";
    let bytes = pdf_with_content(content);
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // Should produce three separate lines in text
    let text = page.extract_text(&TextOptions::default());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "Line One");
    assert_eq!(lines[1], "Line Two");
    assert_eq!(lines[2], "Line Three");
}

#[test]
fn end_to_end_multi_page_document() {
    let bytes = pdf_with_pages(&["Page One", "Page Two", "Page Three"]);
    let pdf = Pdf::open(&bytes, None).unwrap();

    assert_eq!(pdf.page_count(), 3);

    // Each page should have its text
    for (i, expected) in ["Page One", "Page Two", "Page Three"].iter().enumerate() {
        let page = pdf.page(i).unwrap();
        assert_eq!(page.page_number(), i);
        let text = page.extract_text(&TextOptions::default());
        assert_eq!(text.trim(), *expected);
    }
}

#[test]
fn end_to_end_doctop_across_pages() {
    let bytes = pdf_with_pages(&["Hello", "World"]);
    let pdf = Pdf::open(&bytes, None).unwrap();

    let page0 = pdf.page(0).unwrap();
    let page1 = pdf.page(1).unwrap();

    let char0 = &page0.chars()[0]; // 'H' on page 0
    let char1 = &page1.chars()[0]; // 'W' on page 1

    // Both at same position on their respective pages
    assert!((char0.bbox.top - char1.bbox.top).abs() < 0.01);

    // doctop for page 1 chars should be offset by page 0's height
    let expected_doctop = char1.bbox.top + page0.height();
    assert!(
        (char1.doctop - expected_doctop).abs() < 0.01,
        "doctop ({}) should be bbox.top ({}) + page_height ({})",
        char1.doctop,
        char1.bbox.top,
        page0.height()
    );
}

#[test]
fn end_to_end_character_coordinates_are_reasonable() {
    // Place text at known position: (72, 720) in PDF coords
    // With page height 792, y-flip gives top ≈ 72 in display coords
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (X) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let ch = &page.chars()[0];
    assert_eq!(ch.text, "X");
    assert_eq!(ch.fontname, "Helvetica");
    assert_eq!(ch.size, 12.0);
    assert!(ch.upright);

    // x0 should be at approximately 72 (text position)
    assert!((ch.bbox.x0 - 72.0).abs() < 1.0);
    // top should be near 72 (792 - 720 = 72, minus ascent adjustment)
    assert!(ch.bbox.top > 50.0 && ch.bbox.top < 80.0);
}

#[test]
fn end_to_end_empty_page() {
    let bytes = pdf_with_content(b"");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert!(page.chars().is_empty());
    assert_eq!(page.extract_text(&TextOptions::default()), "");
    assert!(page.extract_words(&WordOptions::default()).is_empty());
}

#[test]
fn end_to_end_page_out_of_range() {
    let bytes = pdf_with_content(b"BT ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    assert!(pdf.page(1).is_err());
    assert!(pdf.page(999).is_err());
}

#[test]
fn end_to_end_invalid_pdf_bytes() {
    assert!(Pdf::open(b"garbage", None).is_err());
    assert!(Pdf::open(b"", None).is_err());
}

#[test]
fn end_to_end_with_custom_options() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Test) Tj ET");
    let opts = ExtractOptions {
        max_recursion_depth: 3,
        max_objects_per_page: 50_000,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    let page = pdf.page(0).unwrap();
    assert_eq!(page.chars().len(), 4); // T, e, s, t
}

#[test]
fn end_to_end_tj_array_kerning() {
    // TJ operator with kerning adjustments
    let content = b"BT /F1 12 Tf [(H) -20 (e) -10 (llo)] TJ ET";
    let bytes = pdf_with_content(content);
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let chars = page.chars();
    assert_eq!(chars.len(), 5);
    let text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "Hello");
}

// --- Metadata tests (US-058) ---

/// Create a PDF with /Info metadata dictionary.
fn pdf_with_metadata(
    title: Option<&str>,
    author: Option<&str>,
    subject: Option<&str>,
    keywords: Option<&str>,
    creator: Option<&str>,
    producer: Option<&str>,
    creation_date: Option<&str>,
    mod_date: Option<&str>,
) -> Vec<u8> {
    use lopdf::{Object, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = lopdf::Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Test) Tj ET".to_vec(),
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

    // Build /Info dictionary
    let mut info_dict = lopdf::Dictionary::new();
    if let Some(v) = title {
        info_dict.set("Title", Object::string_literal(v));
    }
    if let Some(v) = author {
        info_dict.set("Author", Object::string_literal(v));
    }
    if let Some(v) = subject {
        info_dict.set("Subject", Object::string_literal(v));
    }
    if let Some(v) = keywords {
        info_dict.set("Keywords", Object::string_literal(v));
    }
    if let Some(v) = creator {
        info_dict.set("Creator", Object::string_literal(v));
    }
    if let Some(v) = producer {
        info_dict.set("Producer", Object::string_literal(v));
    }
    if let Some(v) = creation_date {
        info_dict.set("CreationDate", Object::string_literal(v));
    }
    if let Some(v) = mod_date {
        info_dict.set("ModDate", Object::string_literal(v));
    }

    let info_id = doc.add_object(Object::Dictionary(info_dict));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

#[test]
fn metadata_full_fields() {
    let bytes = pdf_with_metadata(
        Some("My Document"),
        Some("Jane Smith"),
        Some("A test PDF"),
        Some("rust, pdf, test"),
        Some("Writer"),
        Some("pdfplumber-rs"),
        Some("D:20240101120000+00'00'"),
        Some("D:20240615153000+00'00'"),
    );
    let pdf = Pdf::open(&bytes, None).unwrap();
    let meta = pdf.metadata();

    assert_eq!(meta.title.as_deref(), Some("My Document"));
    assert_eq!(meta.author.as_deref(), Some("Jane Smith"));
    assert_eq!(meta.subject.as_deref(), Some("A test PDF"));
    assert_eq!(meta.keywords.as_deref(), Some("rust, pdf, test"));
    assert_eq!(meta.creator.as_deref(), Some("Writer"));
    assert_eq!(meta.producer.as_deref(), Some("pdfplumber-rs"));
    assert_eq!(
        meta.creation_date.as_deref(),
        Some("D:20240101120000+00'00'")
    );
    assert_eq!(meta.mod_date.as_deref(), Some("D:20240615153000+00'00'"));
    assert!(!meta.is_empty());
}

#[test]
fn metadata_partial_fields() {
    let bytes = pdf_with_metadata(Some("Title Only"), None, None, None, None, None, None, None);
    let pdf = Pdf::open(&bytes, None).unwrap();
    let meta = pdf.metadata();

    assert_eq!(meta.title.as_deref(), Some("Title Only"));
    assert_eq!(meta.author, None);
    assert_eq!(meta.subject, None);
    assert!(!meta.is_empty());
}

#[test]
fn metadata_no_info_dictionary() {
    // Regular PDF without /Info dictionary
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let meta = pdf.metadata();

    assert!(meta.is_empty());
    assert_eq!(*meta, DocumentMetadata::default());
}

// --- Page box variant tests (US-059) ---

/// Create a PDF where the page has all five box types set.
fn pdf_with_all_boxes() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Test) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    let page_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)],
        "CropBox" => vec![Object::Integer(10), Object::Integer(10), Object::Integer(602), Object::Integer(782)],
        "TrimBox" => vec![Object::Integer(20), Object::Integer(20), Object::Integer(592), Object::Integer(772)],
        "BleedBox" => vec![Object::Integer(5), Object::Integer(5), Object::Integer(607), Object::Integer(787)],
        "ArtBox" => vec![Object::Integer(50), Object::Integer(50), Object::Integer(562), Object::Integer(742)],
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

/// Create a PDF with only MediaBox (no optional boxes).
fn pdf_with_only_media_box() -> Vec<u8> {
    pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Test) Tj ET")
}

/// Create a PDF where boxes are inherited from the parent Pages tree node.
fn pdf_with_inherited_boxes() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Test) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    // Page has NO boxes — they come from the parent Pages node
    let page_dict = dictionary! {
        "Type" => "Page",
        "Contents" => Object::Reference(content_id),
        "Resources" => resources,
    };
    let page_id = doc.add_object(page_dict);

    // Parent Pages node has MediaBox and TrimBox (both inheritable)
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![Object::Reference(page_id)],
        "Count" => Object::Integer(1),
        "MediaBox" => vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)],
        "TrimBox" => vec![Object::Integer(25), Object::Integer(25), Object::Integer(587), Object::Integer(767)],
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
fn page_boxes_all_box_types() {
    let bytes = pdf_with_all_boxes();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // MediaBox
    let mb = page.media_box();
    assert_eq!(mb.x0, 0.0);
    assert_eq!(mb.top, 0.0);
    assert_eq!(mb.x1, 612.0);
    assert_eq!(mb.bottom, 792.0);

    // CropBox
    let cb = page.crop_box().expect("CropBox should be set");
    assert_eq!(cb.x0, 10.0);
    assert_eq!(cb.top, 10.0);
    assert_eq!(cb.x1, 602.0);
    assert_eq!(cb.bottom, 782.0);

    // TrimBox
    let tb = page.trim_box().expect("TrimBox should be set");
    assert_eq!(tb.x0, 20.0);
    assert_eq!(tb.top, 20.0);
    assert_eq!(tb.x1, 592.0);
    assert_eq!(tb.bottom, 772.0);

    // BleedBox
    let bb = page.bleed_box().expect("BleedBox should be set");
    assert_eq!(bb.x0, 5.0);
    assert_eq!(bb.top, 5.0);
    assert_eq!(bb.x1, 607.0);
    assert_eq!(bb.bottom, 787.0);

    // ArtBox
    let ab = page.art_box().expect("ArtBox should be set");
    assert_eq!(ab.x0, 50.0);
    assert_eq!(ab.top, 50.0);
    assert_eq!(ab.x1, 562.0);
    assert_eq!(ab.bottom, 742.0);
}

#[test]
fn page_boxes_only_media_box() {
    let bytes = pdf_with_only_media_box();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // MediaBox always present
    let mb = page.media_box();
    assert_eq!(mb.x0, 0.0);
    assert_eq!(mb.top, 0.0);
    assert_eq!(mb.x1, 612.0);
    assert_eq!(mb.bottom, 792.0);

    // All optional boxes should be None
    assert!(page.crop_box().is_none());
    assert!(page.trim_box().is_none());
    assert!(page.bleed_box().is_none());
    assert!(page.art_box().is_none());
}

#[test]
fn page_boxes_inherited_from_parent() {
    let bytes = pdf_with_inherited_boxes();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // MediaBox inherited from parent
    let mb = page.media_box();
    assert_eq!(mb.x0, 0.0);
    assert_eq!(mb.top, 0.0);
    assert_eq!(mb.x1, 612.0);
    assert_eq!(mb.bottom, 792.0);

    // TrimBox inherited from parent
    let tb = page
        .trim_box()
        .expect("TrimBox should be inherited from parent");
    assert_eq!(tb.x0, 25.0);
    assert_eq!(tb.top, 25.0);
    assert_eq!(tb.x1, 587.0);
    assert_eq!(tb.bottom, 767.0);

    // BleedBox and ArtBox not set anywhere
    assert!(page.bleed_box().is_none());
    assert!(page.art_box().is_none());
}
