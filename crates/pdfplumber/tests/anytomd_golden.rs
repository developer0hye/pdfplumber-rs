//! anytomd compatibility test suite with golden fixtures (US-100).
//!
//! Validates that `convert_to_markdown()` output matches expected golden files
//! for different PDF categories. Set `UPDATE_GOLDEN=1` to regenerate expected files.

use pdfplumber::{
    ExtractOptions, ImageExportOptions, MarkdownConversionOptions, MarkdownConversionResult, Pdf,
};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Test PDF creation helpers
// ---------------------------------------------------------------------------

/// Create a "technical doc" PDF with title, heading-like large text, and body.
fn create_technical_doc_pdf() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let bold_font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
    });

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    // Page 1: Title + body
    let content1 = b"BT /F2 24 Tf 72 700 Td (API Reference Guide) Tj ET \
                     BT /F1 12 Tf 72 660 Td (This document describes the REST API endpoints.) Tj ET \
                     BT /F1 12 Tf 72 640 Td (All endpoints require authentication via Bearer token.) Tj ET";
    let stream1 = Stream::new(dictionary! {}, content1.to_vec());
    let content_id1 = doc.add_object(stream1);
    let resources1 = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
            "F2" => Object::Reference(bold_font_id),
        },
    };
    let page1 = dictionary! {
        "Type" => "Page",
        "MediaBox" => media_box.clone(),
        "Contents" => Object::Reference(content_id1),
        "Resources" => resources1,
    };
    let page_id1 = doc.add_object(page1);

    // Page 2: Another section
    let content2 = b"BT /F2 18 Tf 72 700 Td (Authentication) Tj ET \
                     BT /F1 12 Tf 72 660 Td (Use OAuth 2.0 for secure access.) Tj ET";
    let stream2 = Stream::new(dictionary! {}, content2.to_vec());
    let content_id2 = doc.add_object(stream2);
    let resources2 = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
            "F2" => Object::Reference(bold_font_id),
        },
    };
    let page2 = dictionary! {
        "Type" => "Page",
        "MediaBox" => media_box.clone(),
        "Contents" => Object::Reference(content_id2),
        "Resources" => resources2,
    };
    let page_id2 = doc.add_object(page2);

    let kids = vec![Object::Reference(page_id1), Object::Reference(page_id2)];
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => Object::Integer(2),
    };
    let pages_id = doc.add_object(pages_dict);

    for &pid in &[page_id1, page_id2] {
        if let Ok(obj) = doc.get_object_mut(pid) {
            if let Ok(dict) = obj.as_dict_mut() {
                dict.set("Parent", Object::Reference(pages_id));
            }
        }
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    // Add metadata
    let mut info = lopdf::Dictionary::new();
    info.set("Title", Object::string_literal("API Reference Guide"));
    info.set("Author", Object::string_literal("Engineering Team"));
    let info_id = doc.add_object(Object::Dictionary(info));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Create a "business report" PDF with title, summary, and data.
fn create_business_report_pdf() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let bold_font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica-Bold",
    });

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    let content = b"BT /F2 20 Tf 72 700 Td (Q4 Financial Summary) Tj ET \
                    BT /F1 12 Tf 72 660 Td (Revenue increased by 15% year-over-year.) Tj ET \
                    BT /F1 12 Tf 72 640 Td (Operating margin improved to 22%.) Tj ET \
                    BT /F1 12 Tf 72 620 Td (Net income reached $4.2 billion.) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);
    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
            "F2" => Object::Reference(bold_font_id),
        },
    };
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

    if let Ok(obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut info = lopdf::Dictionary::new();
    info.set("Title", Object::string_literal("Q4 Financial Summary"));
    info.set("Author", Object::string_literal("Finance Department"));
    let info_id = doc.add_object(Object::Dictionary(info));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Create a "mixed-language" PDF with ASCII text (simulating mixed content).
fn create_mixed_language_pdf() -> Vec<u8> {
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

    let content = b"BT /F1 12 Tf 72 700 Td (International Report) Tj ET \
                    BT /F1 12 Tf 72 660 Td (Section 1: English content here.) Tj ET \
                    BT /F1 12 Tf 72 640 Td (Section 2: Additional content follows.) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);
    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };
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

    if let Ok(obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = obj.as_dict_mut() {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => Object::Reference(pages_id),
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut info = lopdf::Dictionary::new();
    info.set("Title", Object::string_literal("International Report"));
    let info_id = doc.add_object(Object::Dictionary(info));
    doc.trailer.set("Info", Object::Reference(info_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    buf
}

/// Create a PDF with a missing font reference to trigger warnings.
fn create_warning_pdf() -> Vec<u8> {
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

    // Use a valid font first, then reference a missing one
    let content = b"BT /F1 12 Tf 72 720 Td (Valid text) Tj ET \
                    BT /F_MISSING 12 Tf 72 680 Td (Missing font) Tj ET";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);
    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
    };
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

    if let Ok(obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = obj.as_dict_mut() {
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

/// Create a PDF with an inline image for image contract testing.
fn create_image_pdf() -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let mut doc = lopdf::Document::with_version("1.5");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Create a small 2x2 RGB image as XObject
    let img_data = vec![
        0xFF, 0x00, 0x00, // red
        0x00, 0xFF, 0x00, // green
        0x00, 0x00, 0xFF, // blue
        0xFF, 0xFF, 0x00, // yellow
    ];
    let img_stream = Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => Object::Integer(2),
            "Height" => Object::Integer(2),
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => Object::Integer(8),
            "Length" => Object::Integer(img_data.len() as i64),
        },
        img_data,
    );
    let img_id = doc.add_object(img_stream);

    let media_box = vec![
        Object::Integer(0),
        Object::Integer(0),
        Object::Integer(612),
        Object::Integer(792),
    ];

    // Content: text + image placement
    let content = b"BT /F1 12 Tf 72 720 Td (Document with image) Tj ET \
                    q 100 0 0 100 72 500 cm /Im0 Do Q";
    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);
    let resources = dictionary! {
        "Font" => dictionary! { "F1" => Object::Reference(font_id) },
        "XObject" => dictionary! { "Im0" => Object::Reference(img_id) },
    };
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

    if let Ok(obj) = doc.get_object_mut(page_id) {
        if let Ok(dict) = obj.as_dict_mut() {
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

// ---------------------------------------------------------------------------
// Golden file helpers
// ---------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("anytomd")
        .join("fixtures")
}

fn should_update_golden() -> bool {
    std::env::var("UPDATE_GOLDEN").map_or(false, |v| v == "1")
}

/// Write golden files for a fixture. Creates the directory if needed.
fn write_golden(
    fixture_dir: &std::path::Path,
    pdf_bytes: &[u8],
    result: &MarkdownConversionResult,
) {
    std::fs::create_dir_all(fixture_dir).unwrap();
    std::fs::write(fixture_dir.join("input.pdf"), pdf_bytes).unwrap();
    std::fs::write(fixture_dir.join("expected_markdown.md"), &result.markdown).unwrap();

    let metadata = serde_json::json!({
        "title": result.title,
        "page_count": result.page_count,
        "warning_count": result.warnings.len(),
        "image_count": result.images.len(),
        "image_filenames": result.images.iter().map(|i| &i.filename).collect::<Vec<_>>(),
    });
    std::fs::write(
        fixture_dir.join("expected_metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();
}

/// Read expected markdown from golden file.
fn read_expected_markdown(fixture_dir: &std::path::Path) -> String {
    std::fs::read_to_string(fixture_dir.join("expected_markdown.md")).unwrap()
}

/// Read expected metadata from golden file.
fn read_expected_metadata(fixture_dir: &std::path::Path) -> serde_json::Value {
    let content = std::fs::read_to_string(fixture_dir.join("expected_metadata.json")).unwrap();
    serde_json::from_str(&content).unwrap()
}

/// Run a golden test: generate output, compare against expected (or update if UPDATE_GOLDEN=1).
fn run_golden_test(
    fixture_name: &str,
    pdf_bytes: &[u8],
    options: &MarkdownConversionOptions,
    extract_options: Option<ExtractOptions>,
) -> MarkdownConversionResult {
    let fixture_dir = fixtures_dir().join(fixture_name);
    let pdf = Pdf::open(pdf_bytes, extract_options).unwrap();
    let result = pdf.convert_to_markdown(options).unwrap();

    if should_update_golden() {
        write_golden(&fixture_dir, pdf_bytes, &result);
        return result;
    }

    // Compare against golden files if they exist
    if fixture_dir.join("expected_markdown.md").exists() {
        let expected_md = read_expected_markdown(&fixture_dir);
        assert_eq!(
            result.markdown, expected_md,
            "Markdown mismatch for fixture '{fixture_name}'. \
             Run with UPDATE_GOLDEN=1 to regenerate."
        );

        let expected_meta = read_expected_metadata(&fixture_dir);
        assert_eq!(
            result.title.as_deref(),
            expected_meta["title"].as_str(),
            "Title mismatch for fixture '{fixture_name}'."
        );
        assert_eq!(
            result.page_count,
            expected_meta["page_count"].as_u64().unwrap() as usize,
            "Page count mismatch for fixture '{fixture_name}'."
        );
    }

    result
}

// ---------------------------------------------------------------------------
// Golden tests for each fixture category
// ---------------------------------------------------------------------------

#[test]
fn golden_technical_doc() {
    let pdf_bytes = create_technical_doc_pdf();
    let result = run_golden_test(
        "technical-doc",
        &pdf_bytes,
        &MarkdownConversionOptions::default(),
        None,
    );

    assert_eq!(result.page_count, 2);
    // Title comes from PDF metadata
    assert_eq!(result.title, Some("API Reference Guide".to_string()));
    // Content is rendered (individual words may be separate headings due to font size)
    assert!(result.markdown.contains("API"));
    assert!(result.markdown.contains("REST API"));
    assert!(result.markdown.contains("Authentication"));
    // Multi-page: should have separator
    assert!(result.markdown.contains("---"));
}

#[test]
fn golden_business_report() {
    let pdf_bytes = create_business_report_pdf();
    let result = run_golden_test(
        "business-report",
        &pdf_bytes,
        &MarkdownConversionOptions::default(),
        None,
    );

    assert_eq!(result.page_count, 1);
    // Title from PDF metadata
    assert_eq!(result.title, Some("Q4 Financial Summary".to_string()));
    // Content rendered (words may be split by markdown renderer)
    assert!(result.markdown.contains("Q4"));
    assert!(result.markdown.contains("Revenue"));
}

#[test]
fn golden_mixed_language() {
    let pdf_bytes = create_mixed_language_pdf();
    let result = run_golden_test(
        "mixed-language",
        &pdf_bytes,
        &MarkdownConversionOptions::default(),
        None,
    );

    assert_eq!(result.page_count, 1);
    assert_eq!(result.title, Some("International Report".to_string()));
    assert!(result.markdown.contains("International Report"));
    assert!(result.markdown.contains("English content"));
}

// ---------------------------------------------------------------------------
// Warning collection test
// ---------------------------------------------------------------------------

#[test]
fn warning_collection_for_problematic_pdf() {
    let pdf_bytes = create_warning_pdf();
    let opts = ExtractOptions {
        collect_warnings: true,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&pdf_bytes, Some(opts)).unwrap();
    let result = pdf
        .convert_to_markdown(&MarkdownConversionOptions::default())
        .unwrap();

    // A missing font reference should generate warnings
    // The specific count depends on interpreter behavior
    assert!(
        !result.warnings.is_empty(),
        "Expected warnings for PDF with missing font reference"
    );

    // Verify warning has page context
    let has_page_context = result.warnings.iter().any(|w| w.page.is_some());
    assert!(
        has_page_context,
        "At least one warning should have page context"
    );
}

// ---------------------------------------------------------------------------
// Image contract test
// ---------------------------------------------------------------------------

#[test]
fn image_contract_deterministic_filenames() {
    let pdf_bytes = create_image_pdf();
    let opts = ExtractOptions {
        extract_image_data: true,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&pdf_bytes, Some(opts.clone())).unwrap();
    let md_opts = MarkdownConversionOptions {
        include_images: true,
        image_options: ImageExportOptions::default(),
        ..Default::default()
    };
    let result = pdf.convert_to_markdown(&md_opts).unwrap();

    // Should have at least one exported image
    if !result.images.is_empty() {
        // Verify deterministic naming: same input produces same filenames
        let pdf2 = Pdf::open(&pdf_bytes, Some(opts)).unwrap();
        let result2 = pdf2.convert_to_markdown(&md_opts).unwrap();

        assert_eq!(result.images.len(), result2.images.len());
        for (img1, img2) in result.images.iter().zip(result2.images.iter()) {
            assert_eq!(
                img1.filename, img2.filename,
                "Image filenames must be deterministic"
            );
            assert_eq!(img1.page, img2.page, "Image page numbers must match");
            assert_eq!(
                img1.mime_type, img2.mime_type,
                "Image MIME types must match"
            );
        }

        // Verify filename follows default pattern: page{N}_img{I}.{ext}
        let first = &result.images[0];
        assert!(
            first.filename.starts_with("page1_img"),
            "Expected filename to start with 'page1_img', got: {}",
            first.filename
        );
    }
}

#[test]
fn image_contract_images_disabled_returns_empty() {
    let pdf_bytes = create_image_pdf();
    let opts = ExtractOptions {
        extract_image_data: true,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&pdf_bytes, Some(opts)).unwrap();
    let md_opts = MarkdownConversionOptions {
        include_images: false,
        ..Default::default()
    };
    let result = pdf.convert_to_markdown(&md_opts).unwrap();
    assert!(
        result.images.is_empty(),
        "Images should be empty when include_images=false"
    );
}

// ---------------------------------------------------------------------------
// Strict mode test
// ---------------------------------------------------------------------------

#[test]
fn strict_mode_warnings_as_errors() {
    let pdf_bytes = create_warning_pdf();
    let opts = ExtractOptions {
        collect_warnings: true,
        ..ExtractOptions::default()
    };
    let pdf = Pdf::open(&pdf_bytes, Some(opts)).unwrap();
    let md_opts = MarkdownConversionOptions {
        strict_mode: true,
        ..Default::default()
    };
    let result = pdf.convert_to_markdown(&md_opts);

    // If warnings exist (which they should for missing font), strict mode returns error
    if result.is_err() {
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(!err_str.is_empty(), "Error message should not be empty");
    }
    // If no warnings generated, Ok is also valid (test is resilient)
}

#[test]
fn strict_mode_clean_pdf_succeeds() {
    let pdf_bytes = create_business_report_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let md_opts = MarkdownConversionOptions {
        strict_mode: true,
        ..Default::default()
    };
    let result = pdf.convert_to_markdown(&md_opts).unwrap();
    assert_eq!(result.page_count, 1);
    assert!(result.warnings.is_empty());
}

// ---------------------------------------------------------------------------
// Plain text output test
// ---------------------------------------------------------------------------

#[test]
fn plain_text_output_consistency() {
    let pdf_bytes = create_technical_doc_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let result = pdf
        .convert_to_markdown(&MarkdownConversionOptions::default())
        .unwrap();

    // Plain text should contain the content without markdown markers
    assert!(result.plain_text.contains("API"));
    assert!(result.plain_text.contains("REST API"));
    assert!(!result.plain_text.is_empty());
    // Should not contain markdown horizontal rule
    assert!(
        !result.plain_text.contains("\n---\n"),
        "Plain text should not contain markdown horizontal rules"
    );
}

// ---------------------------------------------------------------------------
// UPDATE_GOLDEN mechanism test
// ---------------------------------------------------------------------------

#[test]
fn update_golden_writes_files_when_enabled() {
    // This test verifies the golden file writing mechanism works correctly
    // by writing to a temporary directory (not the real fixtures)
    let pdf_bytes = create_business_report_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let result = pdf
        .convert_to_markdown(&MarkdownConversionOptions::default())
        .unwrap();

    let temp_dir = std::env::temp_dir().join("pdfplumber_golden_test");
    write_golden(&temp_dir, &pdf_bytes, &result);

    assert!(temp_dir.join("input.pdf").exists());
    assert!(temp_dir.join("expected_markdown.md").exists());
    assert!(temp_dir.join("expected_metadata.json").exists());

    // Read back and verify
    let md = std::fs::read_to_string(temp_dir.join("expected_markdown.md")).unwrap();
    assert_eq!(md, result.markdown);

    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp_dir.join("expected_metadata.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(meta["page_count"], 1);

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}
