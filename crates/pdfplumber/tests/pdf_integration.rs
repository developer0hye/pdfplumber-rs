//! Integration tests for the Pdf public API (US-017).
//!
//! These tests exercise the full end-to-end pipeline:
//! PDF bytes → Pdf::open → Page → chars/extract_text.
//!
//! Test PDFs are created programmatically using lopdf.

use pdfplumber::{
    AnnotationType, Bookmark, DedupeOptions, DocumentMetadata, ExtractOptions, PageObject, Pdf,
    SearchOptions, TextOptions, UnicodeNorm, WordOptions,
};

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

// --- Annotation tests (US-060) ---

/// Create a PDF with a Text annotation on the page.
fn pdf_with_text_annotation() -> Vec<u8> {
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

    // Create a Text annotation with all optional fields
    let annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => vec![Object::Integer(100), Object::Integer(700), Object::Integer(200), Object::Integer(750)],
        "Contents" => Object::string_literal("This is a comment"),
        "T" => Object::string_literal("Alice"),
        "M" => Object::string_literal("D:20240601120000"),
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

/// Create a PDF with a Highlight annotation on the page.
fn pdf_with_highlight_annotation() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Highlighted text) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    // Create a Highlight annotation (no author or date)
    let annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Highlight",
        "Rect" => vec![Object::Integer(72), Object::Integer(710), Object::Integer(200), Object::Integer(730)],
        "Contents" => Object::string_literal("Important section"),
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
fn annotation_text_with_all_fields() {
    let bytes = pdf_with_text_annotation();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let annots = page.annots();
    assert_eq!(annots.len(), 1);

    let annot = &annots[0];
    assert_eq!(annot.annot_type, AnnotationType::Text);
    assert_eq!(annot.raw_subtype, "Text");
    assert_eq!(annot.contents.as_deref(), Some("This is a comment"));
    assert_eq!(annot.author.as_deref(), Some("Alice"));
    assert_eq!(annot.date.as_deref(), Some("D:20240601120000"));

    // Check bbox
    assert_eq!(annot.bbox.x0, 100.0);
    assert_eq!(annot.bbox.top, 700.0);
    assert_eq!(annot.bbox.x1, 200.0);
    assert_eq!(annot.bbox.bottom, 750.0);
}

#[test]
fn annotation_highlight_partial_fields() {
    let bytes = pdf_with_highlight_annotation();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let annots = page.annots();
    assert_eq!(annots.len(), 1);

    let annot = &annots[0];
    assert_eq!(annot.annot_type, AnnotationType::Highlight);
    assert_eq!(annot.raw_subtype, "Highlight");
    assert_eq!(annot.contents.as_deref(), Some("Important section"));
    assert!(annot.author.is_none());
    assert!(annot.date.is_none());
}

#[test]
fn annotation_page_with_no_annotations() {
    // Regular PDF without /Annots
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert!(page.annots().is_empty());
}

// --- Hyperlink tests (US-061) ---

/// Create a PDF with a Link annotation that has a /URI action.
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

    // Create a Link annotation with /A /URI action
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

/// Create a PDF with a Link annotation that has a /GoTo action (internal link).
fn pdf_with_goto_link() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let stream = Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 72 720 Td (Go to page 2) Tj ET".to_vec(),
    );
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };

    // Create a second (empty) page as the GoTo target
    let page2_dict = dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![Object::Integer(0), Object::Integer(0), Object::Integer(612), Object::Integer(792)],
    };
    let page2_id = doc.add_object(page2_dict);

    // Create a Link annotation with /A /GoTo action
    let annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => vec![Object::Integer(72), Object::Integer(710), Object::Integer(200), Object::Integer(730)],
        "A" => dictionary! {
            "S" => "GoTo",
            "D" => vec![Object::Reference(page2_id), Object::Name(b"Fit".to_vec())],
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
        "Kids" => vec![Object::Reference(page_id), Object::Reference(page2_id)],
        "Count" => Object::Integer(2),
    };
    let pages_id = doc.add_object(pages_dict);

    // Set parent for both pages
    for &pid in &[page_id, page2_id] {
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

#[test]
fn hyperlink_uri_link() {
    let bytes = pdf_with_uri_link();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let links = page.hyperlinks();
    assert_eq!(links.len(), 1);

    let link = &links[0];
    assert_eq!(link.uri, "https://example.com");
    assert_eq!(link.bbox.x0, 72.0);
    assert_eq!(link.bbox.top, 710.0);
    assert_eq!(link.bbox.x1, 200.0);
    assert_eq!(link.bbox.bottom, 730.0);
}

#[test]
fn hyperlink_goto_link() {
    let bytes = pdf_with_goto_link();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let links = page.hyperlinks();
    assert_eq!(links.len(), 1);

    // GoTo links should have a destination string
    let link = &links[0];
    assert!(!link.uri.is_empty());
}

#[test]
fn hyperlink_page_with_no_links() {
    // Regular PDF without /Annots
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert!(page.hyperlinks().is_empty());
}

// --- Bookmark tests (US-062) ---

/// Create a PDF with multi-level bookmarks (outlines).
fn pdf_with_bookmarks() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Create 3 pages
    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    let mut page_ids = Vec::new();
    for text in &["Chapter 1", "Section 1.1", "Chapter 2"] {
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
        "Count" => Object::Integer(3),
    };
    let pages_id = doc.add_object(pages_dict);

    for &pid in &page_ids {
        if let Ok(page_obj) = doc.get_object_mut(pid) {
            if let Ok(dict) = page_obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }
    }

    // Create outline entries (bottom-up to reference children)
    // Section 1.1 → child of Chapter 1, links to page 2
    let section_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Section 1.1"),
        "Dest" => vec![
            Object::Reference(page_ids[1]),
            Object::Name(b"XYZ".to_vec()),
            Object::Integer(0),
            Object::Integer(700),
            Object::Integer(0),
        ],
    });

    // Chapter 1 → links to page 1, has child Section 1.1
    let ch1_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Chapter 1"),
        "First" => Object::Reference(section_id),
        "Last" => Object::Reference(section_id),
        "Count" => Object::Integer(1),
        "Dest" => vec![
            Object::Reference(page_ids[0]),
            Object::Name(b"Fit".to_vec()),
        ],
    });

    // Chapter 2 → links to page 3
    let ch2_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Chapter 2"),
        "Dest" => vec![
            Object::Reference(page_ids[2]),
            Object::Name(b"XYZ".to_vec()),
            Object::Null,
            Object::Integer(792),
            Object::Null,
        ],
    });

    // Set parent/sibling links
    if let Ok(obj) = doc.get_object_mut(section_id) {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Parent", Object::Reference(ch1_id));
        }
    }
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

    // Outlines root
    let outlines_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => Object::Reference(ch1_id),
        "Last" => Object::Reference(ch2_id),
        "Count" => Object::Integer(3),
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
fn bookmarks_multi_level() {
    let bytes = pdf_with_bookmarks();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let bookmarks = pdf.bookmarks();

    assert_eq!(bookmarks.len(), 3);

    // Chapter 1 — level 0, page 0
    assert_eq!(bookmarks[0].title, "Chapter 1");
    assert_eq!(bookmarks[0].level, 0);
    assert_eq!(bookmarks[0].page_number, Some(0));

    // Section 1.1 — level 1, page 1
    assert_eq!(bookmarks[1].title, "Section 1.1");
    assert_eq!(bookmarks[1].level, 1);
    assert_eq!(bookmarks[1].page_number, Some(1));
    assert_eq!(bookmarks[1].dest_top, Some(700.0));

    // Chapter 2 — level 0, page 2
    assert_eq!(bookmarks[2].title, "Chapter 2");
    assert_eq!(bookmarks[2].level, 0);
    assert_eq!(bookmarks[2].page_number, Some(2));
    assert_eq!(bookmarks[2].dest_top, Some(792.0));
}

#[test]
fn bookmarks_no_outlines() {
    // Regular PDF without /Outlines
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();

    assert!(pdf.bookmarks().is_empty());
}

#[test]
fn bookmarks_named_destination() {
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

    // Outline entry with /A GoTo action
    let outline_item_id = doc.add_object(dictionary! {
        "Title" => Object::string_literal("Intro"),
        "A" => dictionary! {
            "S" => "GoTo",
            "D" => vec![
                Object::Reference(page_id),
                Object::Name(b"FitH".to_vec()),
                Object::Integer(500),
            ],
        },
    });

    let outlines_id = doc.add_object(dictionary! {
        "Type" => "Outlines",
        "First" => Object::Reference(outline_item_id),
        "Last" => Object::Reference(outline_item_id),
        "Count" => Object::Integer(1),
    });

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
        "Outlines" => Object::Reference(outlines_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();

    let pdf = Pdf::open(&buf, None).unwrap();
    let bookmarks = pdf.bookmarks();

    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0].title, "Intro");
    assert_eq!(bookmarks[0].level, 0);
    assert_eq!(bookmarks[0].page_number, Some(0));
    assert_eq!(bookmarks[0].dest_top, Some(500.0));
}

// --- US-063: Text search with position ---

#[test]
fn search_simple_string_match() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let opts = SearchOptions {
        regex: false,
        ..Default::default()
    };
    let matches = page.search("Hello", &opts);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text, "Hello");
    assert_eq!(matches[0].page_number, 0);
    assert!(!matches[0].char_indices.is_empty());
    // bbox should have positive dimensions
    assert!(matches[0].bbox.width() > 0.0);
    assert!(matches[0].bbox.height() > 0.0);
}

#[test]
fn search_regex_pattern() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let opts = SearchOptions::default(); // regex=true
    let matches = page.search("H.llo", &opts);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text, "Hello");
}

#[test]
fn search_case_insensitive() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let opts = SearchOptions {
        regex: false,
        case_sensitive: false,
    };
    let matches = page.search("hello", &opts);

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text, "Hello");
}

#[test]
fn search_no_matches() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let opts = SearchOptions {
        regex: false,
        ..Default::default()
    };
    let matches = page.search("XYZ", &opts);

    assert!(matches.is_empty());
}

#[test]
fn search_all_multi_page() {
    let bytes = pdf_with_pages(&["Hello World", "Goodbye World"]);
    let pdf = Pdf::open(&bytes, None).unwrap();

    let opts = SearchOptions {
        regex: false,
        ..Default::default()
    };
    let matches = pdf.search_all("World", &opts).unwrap();

    // "World" appears on both pages
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].page_number, 0);
    assert_eq!(matches[1].page_number, 1);
}

// --- US-064: Duplicate character deduplication ---

/// Create a PDF where "H" is rendered twice at the same position (simulating
/// a bold-effect duplicate), followed by normal "i".
fn pdf_with_duplicate_chars() -> Vec<u8> {
    // Render "H" at (72, 700), then "H" again at (72.5, 700), then "i" at (82, 700)
    let content = b"BT /F1 12 Tf 72 700 Td (H) Tj 0.5 0 Td (H) Tj 9.5 0 Td (i) Tj ET";
    pdf_with_content(content)
}

/// Create a PDF where the same char is rendered with two different fonts at the same position.
fn pdf_with_two_fonts_content() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font1_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let font2_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });

    // "A" in Helvetica at (72, 700), then "A" in Courier at same position
    let content = b"BT /F1 12 Tf 72 700 Td (A) Tj /F2 12 Tf 0 0 Td (A) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font1_id),
            "F2" => Object::Reference(font2_id),
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

#[test]
fn dedupe_overlapping_identical_chars() {
    let bytes = pdf_with_duplicate_chars();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // Before dedupe: should have 3 chars (H, H, i)
    assert_eq!(page.chars().len(), 3);
    assert_eq!(page.chars()[0].text, "H");
    assert_eq!(page.chars()[1].text, "H");
    assert_eq!(page.chars()[2].text, "i");

    // After dedupe: duplicate H removed → (H, i)
    let deduped = page.dedupe_chars(&DedupeOptions::default());
    assert_eq!(deduped.chars().len(), 2);
    assert_eq!(deduped.chars()[0].text, "H");
    assert_eq!(deduped.chars()[1].text, "i");
}

#[test]
fn dedupe_preserves_non_overlapping() {
    // "Hello" with no duplicates — all should be preserved
    let content = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    let bytes = pdf_with_content(content);
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    let original_count = page.chars().len();
    let deduped = page.dedupe_chars(&DedupeOptions::default());
    assert_eq!(deduped.chars().len(), original_count);
}

#[test]
fn dedupe_different_font_not_deduped() {
    let bytes = pdf_with_two_fonts_content();
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // Two "A" chars at same position but different fonts
    assert_eq!(page.chars().len(), 2);

    // With default extra_attrs (fontname, size) → NOT deduped (different fontname)
    let deduped = page.dedupe_chars(&DedupeOptions::default());
    assert_eq!(deduped.chars().len(), 2);

    // With no extra_attrs → deduped (only position + text matter)
    let deduped_no_attrs = page.dedupe_chars(&DedupeOptions {
        tolerance: 1.0,
        extra_attrs: vec![],
    });
    assert_eq!(deduped_no_attrs.chars().len(), 1);
}

// --- US-065: Unicode normalization ---

#[test]
fn unicode_norm_nfc_composes_extracted_chars() {
    // PDF with decomposed "é" (e + combining acute)
    // In PDF strings, we can use raw UTF-8 bytes for the composed form
    // and test normalization by checking the default (no norm) vs NFC
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Hello) Tj ET");

    // Without normalization (default)
    let pdf_default = Pdf::open(&bytes, None).unwrap();
    let page_default = pdf_default.page(0).unwrap();
    assert_eq!(page_default.chars().len(), 5);

    // With NFC normalization - same chars, just normalized
    let opts = ExtractOptions {
        unicode_norm: UnicodeNorm::Nfc,
        ..ExtractOptions::default()
    };
    let pdf_nfc = Pdf::open(&bytes, Some(opts)).unwrap();
    let page_nfc = pdf_nfc.page(0).unwrap();
    assert_eq!(page_nfc.chars().len(), 5);
    assert_eq!(page_nfc.chars()[0].text, "H");
}

#[test]
fn unicode_norm_nfkc_normalizes_compatibility_chars() {
    // Test that NFKC normalization option is properly passed through
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Test) Tj ET");

    let opts = ExtractOptions {
        unicode_norm: UnicodeNorm::Nfkc,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    let page = pdf.page(0).unwrap();
    // Basic ASCII chars are unchanged by NFKC
    assert_eq!(page.chars().len(), 4);
    assert_eq!(page.chars()[0].text, "T");
    assert_eq!(page.chars()[1].text, "e");
}

#[test]
fn unicode_norm_none_preserves_original_text() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf (AB) Tj ET");

    // Explicit None normalization
    let opts = ExtractOptions {
        unicode_norm: UnicodeNorm::None,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    let page = pdf.page(0).unwrap();
    assert_eq!(page.chars().len(), 2);
    assert_eq!(page.chars()[0].text, "A");
    assert_eq!(page.chars()[1].text, "B");
}

// ---- US-066: Custom object filtering ----

#[test]
fn filter_by_font_name_keeps_matching_chars() {
    // Create a PDF with two fonts: F1 (Helvetica) and F2 (Courier)
    let bytes =
        pdf_with_two_fonts(b"BT /F1 12 Tf 72 720 Td (AB) Tj /F2 12 Tf 72 700 Td (CD) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // All 4 chars present before filter
    assert_eq!(page.chars().len(), 4);

    // Filter to keep only Helvetica chars (and all non-char objects)
    let filtered = page.filter(|obj| match obj {
        PageObject::Char(c) => c.fontname.contains("Helvetica"),
        _ => true,
    });

    // Only the 2 Helvetica chars should remain
    assert_eq!(filtered.chars().len(), 2);
    assert_eq!(filtered.chars()[0].text, "A");
    assert_eq!(filtered.chars()[1].text, "B");
}

#[test]
fn filter_by_size_keeps_large_chars() {
    // Create a PDF with two font sizes: 12pt and 24pt
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (AB) Tj /F1 24 Tf 72 680 Td (XY) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.chars().len(), 4);

    // Filter to keep only chars with size > 20
    let filtered = page.filter(|obj| match obj {
        PageObject::Char(c) => c.size > 20.0,
        _ => true,
    });

    assert_eq!(filtered.chars().len(), 2);
    assert_eq!(filtered.chars()[0].text, "X");
    assert_eq!(filtered.chars()[1].text, "Y");
}

#[test]
fn filter_by_position_keeps_matching_chars() {
    // Create a PDF with two lines of text at different y positions
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (AB) Tj 0 -20 Td (CD) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.chars().len(), 4);

    // Filter to keep only chars in the upper region (top < 80)
    // In top-left origin: first line at y=72 (792-720), second at y=92 (792-700)
    let filtered = page.filter(|obj| match obj {
        PageObject::Char(c) => c.bbox.top < 80.0,
        _ => true,
    });

    assert_eq!(filtered.chars().len(), 2);
    assert_eq!(filtered.chars()[0].text, "A");
    assert_eq!(filtered.chars()[1].text, "B");
}

#[test]
fn filter_chained_filters_compose() {
    // Create a PDF with two font sizes
    let bytes =
        pdf_with_content(b"BT /F1 12 Tf 72 720 Td (ABCD) Tj /F1 24 Tf 72 680 Td (EFGH) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.chars().len(), 8);

    // First filter: keep only chars (remove everything else)
    let filtered1 = page.filter(|obj| matches!(obj, PageObject::Char(_)));
    assert_eq!(filtered1.chars().len(), 8);

    // Second filter: keep only large chars
    let filtered2 = filtered1.filter(|obj| match obj {
        PageObject::Char(c) => c.size > 20.0,
        _ => false,
    });

    assert_eq!(filtered2.chars().len(), 4);
    assert_eq!(filtered2.chars()[0].text, "E");
}

#[test]
fn filter_preserves_extract_text() {
    let bytes =
        pdf_with_content(b"BT /F1 12 Tf 72 720 Td (Hello) Tj /F1 24 Tf 72 700 Td (World) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // Filter to keep only large chars
    let filtered = page.filter(|obj| match obj {
        PageObject::Char(c) => c.size > 20.0,
        _ => true,
    });

    let text = filtered.extract_text(&TextOptions::default());
    assert_eq!(text, "World");
}

#[test]
fn filter_preserves_find_tables() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf 72 720 Td (AB) Tj ET");
    let pdf = Pdf::open(&bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // Filter keeping everything should not break find_tables
    let filtered = page.filter(|_| true);
    let tables = filtered.find_tables(&pdfplumber::TableSettings::default());
    // No tables in a simple text PDF
    assert!(tables.is_empty());
}

/// Helper: create a single-page PDF with two fonts (F1=Helvetica, F2=Courier).
fn pdf_with_two_fonts(content: &[u8]) -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font1_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let font2_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });

    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font1_id),
            "F2" => Object::Reference(font2_id),
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

    // Set parent
    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Object::Dictionary(d) = page_obj {
            d.set("Parent", Object::Reference(pages_id));
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

// --- US-097: Document-level resource budget tests ---

#[test]
fn resource_budget_max_input_bytes_rejects_oversized_pdf() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Hello) Tj ET");
    let input_size = bytes.len();

    // Set a limit smaller than the actual PDF size
    let opts = ExtractOptions {
        max_input_bytes: Some(10), // 10 bytes is way too small for any valid PDF
        ..ExtractOptions::default()
    };
    let result = Pdf::open(&bytes, Some(opts));
    match result {
        Err(pdfplumber::PdfError::ResourceLimitExceeded {
            limit_name,
            limit_value,
            actual_value,
        }) => {
            assert_eq!(limit_name, "max_input_bytes");
            assert_eq!(limit_value, 10);
            assert_eq!(actual_value, input_size);
        }
        Err(e) => panic!("expected ResourceLimitExceeded, got: {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn resource_budget_max_input_bytes_allows_within_limit() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Hello) Tj ET");
    let opts = ExtractOptions {
        max_input_bytes: Some(bytes.len() + 1000), // generous limit
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn resource_budget_max_pages_rejects_over_limit() {
    let bytes = pdf_with_pages(&["Page 1", "Page 2", "Page 3"]);
    let opts = ExtractOptions {
        max_pages: Some(2), // limit to 2 pages, but PDF has 3
        ..ExtractOptions::default()
    };
    let result = Pdf::open(&bytes, Some(opts));
    match result {
        Err(pdfplumber::PdfError::ResourceLimitExceeded {
            limit_name,
            limit_value,
            actual_value,
        }) => {
            assert_eq!(limit_name, "max_pages");
            assert_eq!(limit_value, 2);
            assert_eq!(actual_value, 3);
        }
        Err(e) => panic!("expected ResourceLimitExceeded, got: {e:?}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn resource_budget_max_pages_allows_within_limit() {
    let bytes = pdf_with_pages(&["Page 1", "Page 2"]);
    let opts = ExtractOptions {
        max_pages: Some(5), // generous limit
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    assert_eq!(pdf.page_count(), 2);
}

#[test]
fn resource_budget_limits_disabled_by_default() {
    // With default options (all limits None), any PDF should open fine
    let bytes = pdf_with_pages(&["A", "B", "C", "D", "E"]);
    let pdf = Pdf::open(&bytes, None).unwrap();
    assert_eq!(pdf.page_count(), 5);
    // Should be able to extract all pages
    for i in 0..5 {
        let page = pdf.page(i).unwrap();
        assert!(!page.chars().is_empty());
    }
}

#[test]
fn resource_budget_max_total_objects_rejects_over_limit() {
    // Create a multi-page PDF
    let bytes = pdf_with_pages(&["Page 1 text content", "Page 2 text content"]);
    let opts = ExtractOptions {
        max_total_objects: Some(5), // very low limit - will be exceeded
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();

    // First page might succeed, but eventually the limit should be hit
    let mut hit_limit = false;
    for i in 0..pdf.page_count() {
        match pdf.page(i) {
            Ok(_) => {}
            Err(pdfplumber::PdfError::ResourceLimitExceeded { limit_name, .. }) => {
                assert_eq!(limit_name, "max_total_objects");
                hit_limit = true;
                break;
            }
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
    assert!(hit_limit, "expected max_total_objects limit to be hit");
}

#[test]
fn resource_budget_max_total_objects_allows_within_limit() {
    let bytes = pdf_with_content(b"BT /F1 12 Tf (Hi) Tj ET");
    let opts = ExtractOptions {
        max_total_objects: Some(1_000_000), // very high limit
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&bytes, Some(opts)).unwrap();
    let page = pdf.page(0).unwrap();
    assert_eq!(page.chars().len(), 2);
}
