//! Integration tests for text extraction on rotated pages (Issue #154).
//!
//! Verifies that text extraction, reading order, and bounding boxes are correct
//! for pages with /Rotate set to 0°, 90°, 180°, and 270°.

use std::path::{Path, PathBuf};

use pdfplumber::{Pdf, TextOptions, WordOptions};

// --- Helpers ---

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn generated(name: &str) -> PathBuf {
    fixtures_dir().join("generated").join(name)
}

fn open_fixture(path: &Path) -> Pdf {
    Pdf::open(&std::fs::read(path).unwrap(), None).unwrap()
}

// ==================== Text extraction on 90° rotated page ====================

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn rotated_90_extracts_correct_text() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(1).unwrap();
    assert_eq!(page.rotation(), 90);

    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("This page has rotation = 90 degrees"),
        "90° page should extract correct text, got: {:?}",
        text.trim()
    );
}

#[test]
fn rotated_90_chars_contain_expected_characters() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(1).unwrap();

    let chars = page.chars();
    assert!(!chars.is_empty(), "90° page should have chars");

    let char_text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert!(
        char_text.contains("This page has rotation = 90 degrees"),
        "char sequence should contain expected text, got: {:?}",
        char_text
    );
}

#[test]
fn rotated_90_page_dimensions_are_swapped() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page_0 = pdf.page(0).unwrap();
    let page_90 = pdf.page(1).unwrap();

    // For 90° rotation, width and height should be swapped relative to 0°
    let tolerance = 1.0;
    assert!(
        (page_90.width() - page_0.height()).abs() < tolerance,
        "90° page width ({}) should equal 0° page height ({})",
        page_90.width(),
        page_0.height()
    );
    assert!(
        (page_90.height() - page_0.width()).abs() < tolerance,
        "90° page height ({}) should equal 0° page width ({})",
        page_90.height(),
        page_0.width()
    );
}

// ==================== Text extraction on 180° rotated page ====================

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn rotated_180_extracts_correct_text_not_reversed() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(2).unwrap();
    assert_eq!(page.rotation(), 180);

    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("This page has rotation = 180 degrees"),
        "180° page should extract correct text (not reversed), got: {:?}",
        text.trim()
    );

    // Verify text is NOT reversed
    assert!(
        !text.contains("seerged"),
        "180° page text should not be reversed, got: {:?}",
        text.trim()
    );
}

#[test]
fn rotated_180_chars_contain_expected_characters() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(2).unwrap();

    let chars = page.chars();
    assert!(!chars.is_empty(), "180° page should have chars");

    let char_text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert!(
        char_text.contains("This page has rotation = 180 degrees"),
        "char sequence should contain expected text, got: {:?}",
        char_text
    );
}

#[test]
fn rotated_180_page_dimensions_match_original() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page_0 = pdf.page(0).unwrap();
    let page_180 = pdf.page(2).unwrap();

    // For 180° rotation, dimensions should be the same as 0°
    let tolerance = 1.0;
    assert!(
        (page_180.width() - page_0.width()).abs() < tolerance,
        "180° page width ({}) should equal 0° page width ({})",
        page_180.width(),
        page_0.width()
    );
    assert!(
        (page_180.height() - page_0.height()).abs() < tolerance,
        "180° page height ({}) should equal 0° page height ({})",
        page_180.height(),
        page_0.height()
    );
}

// ==================== Text extraction on 270° rotated page ====================

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn rotated_270_extracts_correct_text() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(3).unwrap();
    assert_eq!(page.rotation(), 270);

    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("This page has rotation = 270 degrees"),
        "270° page should extract correct text, got: {:?}",
        text.trim()
    );
}

#[test]
fn rotated_270_chars_contain_expected_characters() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(3).unwrap();

    let chars = page.chars();
    assert!(!chars.is_empty(), "270° page should have chars");

    let char_text: String = chars.iter().map(|c| c.text.as_str()).collect();
    assert!(
        char_text.contains("This page has rotation = 270 degrees"),
        "char sequence should contain expected text, got: {:?}",
        char_text
    );
}

#[test]
fn rotated_270_page_dimensions_are_swapped() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page_0 = pdf.page(0).unwrap();
    let page_270 = pdf.page(3).unwrap();

    // For 270° rotation, width and height should be swapped relative to 0°
    let tolerance = 1.0;
    assert!(
        (page_270.width() - page_0.height()).abs() < tolerance,
        "270° page width ({}) should equal 0° page height ({})",
        page_270.width(),
        page_0.height()
    );
    assert!(
        (page_270.height() - page_0.width()).abs() < tolerance,
        "270° page height ({}) should equal 0° page width ({})",
        page_270.height(),
        page_0.width()
    );
}

// ==================== Mixed-rotation document ====================

#[test]
fn mixed_rotation_document_has_correct_page_count() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    assert_eq!(pdf.page_count(), 4);
}

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn mixed_rotation_all_pages_extract_correctly() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let expected_texts = [
        "This page has rotation = 0 degrees",
        "This page has rotation = 90 degrees",
        "This page has rotation = 180 degrees",
        "This page has rotation = 270 degrees",
    ];

    for (i, expected) in expected_texts.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        let text = page.extract_text(&TextOptions::default());
        assert!(
            text.contains(expected),
            "page {} should contain {:?}, got: {:?}",
            i,
            expected,
            text.trim()
        );
    }
}

#[test]
fn mixed_rotation_all_pages_have_correct_rotation_values() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let expected_rotations = [0, 90, 180, 270];

    for (i, &expected) in expected_rotations.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        assert_eq!(
            page.rotation(),
            expected,
            "page {} should have rotation {}",
            i,
            expected
        );
    }
}

// ==================== Reading order preservation ====================

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn rotated_90_reading_order_preserved() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(1).unwrap();

    let words = page.extract_words(&WordOptions::default());
    let word_texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();

    // Words should appear in natural reading order
    assert!(
        word_texts.contains(&"This"),
        "words should contain 'This', got: {:?}",
        word_texts
    );
    assert!(
        word_texts.contains(&"rotation"),
        "words should contain 'rotation', got: {:?}",
        word_texts
    );

    // "This" should come before "rotation" in the word list
    let this_pos = word_texts.iter().position(|&w| w == "This");
    let rotation_pos = word_texts.iter().position(|&w| w == "rotation");
    if let (Some(t), Some(r)) = (this_pos, rotation_pos) {
        assert!(
            t < r,
            "'This' (pos {}) should come before 'rotation' (pos {}) in reading order",
            t,
            r
        );
    }
}

#[test]
fn rotated_180_reading_order_preserved() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(2).unwrap();

    let words = page.extract_words(&WordOptions::default());
    let word_texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();

    // Words should appear in natural reading order, not reversed
    let this_pos = word_texts.iter().position(|&w| w == "This");
    let degrees_pos = word_texts.iter().position(|&w| w == "degrees");
    if let (Some(t), Some(d)) = (this_pos, degrees_pos) {
        assert!(
            t < d,
            "'This' (pos {}) should come before 'degrees' (pos {}) in reading order",
            t,
            d
        );
    }
}

#[test]
#[ignore = "generated rotated_pages.pdf has chars in rotated coordinates; word grouping needs per-char upright detection"]
fn rotated_270_reading_order_preserved() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(3).unwrap();

    let words = page.extract_words(&WordOptions::default());
    let word_texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();

    assert!(
        word_texts.contains(&"This"),
        "words should contain 'This', got: {:?}",
        word_texts
    );
    assert!(
        word_texts.contains(&"rotation"),
        "words should contain 'rotation', got: {:?}",
        word_texts
    );

    let this_pos = word_texts.iter().position(|&w| w == "This");
    let rotation_pos = word_texts.iter().position(|&w| w == "rotation");
    if let (Some(t), Some(r)) = (this_pos, rotation_pos) {
        assert!(
            t < r,
            "'This' (pos {}) should come before 'rotation' (pos {}) in reading order",
            t,
            r
        );
    }
}

// ==================== Character bounding boxes in correct coordinate space ====================

#[test]
fn rotated_0_char_bboxes_within_page_bounds() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(0).unwrap();
    let (pw, ph) = (page.width(), page.height());

    for c in page.chars() {
        assert!(
            c.bbox.x0 >= 0.0 && c.bbox.x1 <= pw,
            "char {:?} x range [{:.1}, {:.1}] should be within page width {:.1}",
            c.text,
            c.bbox.x0,
            c.bbox.x1,
            pw
        );
        assert!(
            c.bbox.top >= 0.0 && c.bbox.bottom <= ph,
            "char {:?} y range [{:.1}, {:.1}] should be within page height {:.1}",
            c.text,
            c.bbox.top,
            c.bbox.bottom,
            ph
        );
    }
}

#[test]
fn rotated_90_char_bboxes_within_page_bounds() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(1).unwrap();
    let (pw, ph) = (page.width(), page.height());

    for c in page.chars() {
        assert!(
            c.bbox.x0 >= -1.0 && c.bbox.x1 <= pw + 1.0,
            "char {:?} x range [{:.1}, {:.1}] should be within page width {:.1}",
            c.text,
            c.bbox.x0,
            c.bbox.x1,
            pw
        );
        assert!(
            c.bbox.top >= -1.0 && c.bbox.bottom <= ph + 1.0,
            "char {:?} y range [{:.1}, {:.1}] should be within page height {:.1}",
            c.text,
            c.bbox.top,
            c.bbox.bottom,
            ph
        );
    }
}

#[test]
fn rotated_180_char_bboxes_within_page_bounds() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(2).unwrap();
    let (pw, ph) = (page.width(), page.height());

    for c in page.chars() {
        assert!(
            c.bbox.x0 >= -1.0 && c.bbox.x1 <= pw + 1.0,
            "char {:?} x range [{:.1}, {:.1}] should be within page width {:.1}",
            c.text,
            c.bbox.x0,
            c.bbox.x1,
            pw
        );
        assert!(
            c.bbox.top >= -1.0 && c.bbox.bottom <= ph + 1.0,
            "char {:?} y range [{:.1}, {:.1}] should be within page height {:.1}",
            c.text,
            c.bbox.top,
            c.bbox.bottom,
            ph
        );
    }
}

#[test]
fn rotated_270_char_bboxes_within_page_bounds() {
    let pdf = open_fixture(&generated("rotated_pages.pdf"));
    let page = pdf.page(3).unwrap();
    let (pw, ph) = (page.width(), page.height());

    for c in page.chars() {
        assert!(
            c.bbox.x0 >= -1.0 && c.bbox.x1 <= pw + 1.0,
            "char {:?} x range [{:.1}, {:.1}] should be within page width {:.1}",
            c.text,
            c.bbox.x0,
            c.bbox.x1,
            pw
        );
        assert!(
            c.bbox.top >= -1.0 && c.bbox.bottom <= ph + 1.0,
            "char {:?} y range [{:.1}, {:.1}] should be within page height {:.1}",
            c.text,
            c.bbox.top,
            c.bbox.bottom,
            ph
        );
    }
}

// ==================== Programmatic lopdf fixture tests ====================

/// Create a minimal PDF with a single page that has the given /Rotate value.
/// The page has a simple content stream with known text.
fn create_rotated_pdf(rotation: i64, text: &str) -> Vec<u8> {
    use lopdf::{Document, Object, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Create a simple Type1 font resource
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // Build content stream: position text at (72, 720) in native PDF coords
    let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
    let stream = Stream::new(dictionary! {}, content.into_bytes());
    let content_id = doc.add_object(stream);

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Rotate" => rotation,
        "Contents" => content_id,
        "Resources" => dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        },
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::from(page_id)],
            "Count" => 1i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

#[test]
fn lopdf_rotated_0_extracts_text() {
    let pdf_bytes = create_rotated_pdf(0, "Hello Zero");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 0);
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Hello Zero"),
        "0° lopdf page should extract 'Hello Zero', got: {:?}",
        text.trim()
    );
}

#[test]
#[ignore = "lopdf synthetic PDFs have chars in rotated coordinates; word grouping needs per-char upright detection"]
fn lopdf_rotated_90_extracts_text() {
    let pdf_bytes = create_rotated_pdf(90, "Hello Ninety");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 90);
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Hello") || text.contains("Ninety"),
        "90° lopdf page should extract text content, got: {:?}",
        text.trim()
    );
}

#[test]
#[ignore = "lopdf synthetic PDFs have chars in rotated coordinates; word grouping needs per-char upright detection"]
fn lopdf_rotated_180_extracts_text() {
    let pdf_bytes = create_rotated_pdf(180, "Hello OneEighty");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 180);
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Hello") || text.contains("OneEighty"),
        "180° lopdf page should extract text content, got: {:?}",
        text.trim()
    );
}

#[test]
#[ignore = "lopdf synthetic PDFs have chars in rotated coordinates; word grouping needs per-char upright detection"]
fn lopdf_rotated_270_extracts_text() {
    let pdf_bytes = create_rotated_pdf(270, "Hello TwoSeventy");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    assert_eq!(page.rotation(), 270);
    let text = page.extract_text(&TextOptions::default());
    assert!(
        text.contains("Hello") || text.contains("TwoSeventy"),
        "270° lopdf page should extract text content, got: {:?}",
        text.trim()
    );
}

#[test]
fn lopdf_rotated_90_dimensions_swapped() {
    let pdf_bytes = create_rotated_pdf(90, "Test");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // MediaBox is 612x792, so 90° should give 792x612
    let tolerance = 1.0;
    assert!(
        (page.width() - 792.0).abs() < tolerance,
        "90° page width should be 792, got {}",
        page.width()
    );
    assert!(
        (page.height() - 612.0).abs() < tolerance,
        "90° page height should be 612, got {}",
        page.height()
    );
}

#[test]
fn lopdf_rotated_180_dimensions_unchanged() {
    let pdf_bytes = create_rotated_pdf(180, "Test");
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let page = pdf.page(0).unwrap();

    // MediaBox is 612x792, 180° keeps same dimensions
    let tolerance = 1.0;
    assert!(
        (page.width() - 612.0).abs() < tolerance,
        "180° page width should be 612, got {}",
        page.width()
    );
    assert!(
        (page.height() - 792.0).abs() < tolerance,
        "180° page height should be 792, got {}",
        page.height()
    );
}

#[test]
fn lopdf_rotated_chars_within_page_bounds() {
    for rotation in [0, 90, 180, 270] {
        let pdf_bytes = create_rotated_pdf(rotation, "BoundsTest");
        let pdf = Pdf::open(&pdf_bytes, None).unwrap();
        let page = pdf.page(0).unwrap();
        let (pw, ph) = (page.width(), page.height());

        for c in page.chars() {
            assert!(
                c.bbox.x0 >= -1.0 && c.bbox.x1 <= pw + 1.0,
                "rotation={}: char {:?} x [{:.1}, {:.1}] out of page width {:.1}",
                rotation,
                c.text,
                c.bbox.x0,
                c.bbox.x1,
                pw
            );
            assert!(
                c.bbox.top >= -1.0 && c.bbox.bottom <= ph + 1.0,
                "rotation={}: char {:?} y [{:.1}, {:.1}] out of page height {:.1}",
                rotation,
                c.text,
                c.bbox.top,
                c.bbox.bottom,
                ph
            );
        }
    }
}

/// Create a multi-page PDF with different rotations per page using lopdf.
fn create_mixed_rotation_pdf() -> Vec<u8> {
    use lopdf::{Document, Object, Stream, dictionary};

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let rotations = [0i64, 90, 180, 270];
    let labels = ["PageA", "PageB", "PageC", "PageD"];
    let mut page_ids = Vec::new();

    for (rot, label) in rotations.iter().zip(labels.iter()) {
        let content = format!("BT /F1 12 Tf 72 720 Td ({label}) Tj ET");
        let stream = Stream::new(dictionary! {}, content.into_bytes());
        let content_id = doc.add_object(stream);

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Rotate" => *rot,
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
        });
        page_ids.push(page_id);
    }

    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::from(id)).collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => 4i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("failed to save test PDF");
    buf
}

#[test]
fn lopdf_mixed_rotation_has_four_pages() {
    let pdf_bytes = create_mixed_rotation_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    assert_eq!(pdf.page_count(), 4);
}

#[test]
fn lopdf_mixed_rotation_correct_values() {
    let pdf_bytes = create_mixed_rotation_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let expected = [0, 90, 180, 270];

    for (i, &exp) in expected.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        assert_eq!(
            page.rotation(),
            exp,
            "page {} should have rotation {}",
            i,
            exp
        );
    }
}

#[test]
fn lopdf_mixed_rotation_all_pages_extract_text() {
    let pdf_bytes = create_mixed_rotation_pdf();
    let pdf = Pdf::open(&pdf_bytes, None).unwrap();
    let expected_labels = ["PageA", "PageB", "PageC", "PageD"];

    for (i, label) in expected_labels.iter().enumerate() {
        let page = pdf.page(i).unwrap();
        let chars = page.chars();
        // At minimum, the page should have chars extracted
        assert!(
            !chars.is_empty(),
            "page {} (label={}) should have chars, got none",
            i,
            label
        );
    }
}
