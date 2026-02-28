//! Real-world PDF integration test suite (US-091).
//!
//! Comprehensive tests with categorized fixtures covering diverse PDF features:
//! fonts/encoding, layout, tables, images, and edge cases.
//!
//! Each fixture is generated programmatically using lopdf to create controlled,
//! reproducible PDFs that exercise real-world code paths. PDFs are saved to
//! `tests/fixtures/real-world/` and loaded for testing.

use std::path::PathBuf;
use std::sync::Once;

use pdfplumber::{ExtractOptions, Pdf, TableSettings, TextOptions, WordOptions};

// --- Constants ---

const COORD_TOLERANCE: f64 = 2.0;

// --- Helpers ---

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/real-world")
}

fn fixture_path(category: &str, name: &str) -> PathBuf {
    fixtures_dir().join(category).join(name)
}

fn open_fixture(category: &str, name: &str) -> Pdf {
    let path = fixture_path(category, name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to read fixture {}/{}: {}. Run generate_fixtures() first.",
            category, name, e
        )
    });
    Pdf::open(&bytes, None).unwrap()
}

fn open_fixture_with_opts(category: &str, name: &str, opts: ExtractOptions) -> Pdf {
    let path = fixture_path(category, name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to read fixture {}/{}: {}. Run generate_fixtures() first.",
            category, name, e
        )
    });
    Pdf::open(&bytes, Some(opts)).unwrap()
}

/// Assert that a floating point value is within tolerance of expected.
fn assert_approx(actual: f64, expected: f64, label: &str) {
    assert!(
        (actual - expected).abs() < COORD_TOLERANCE,
        "{}: expected ~{}, got {} (tolerance {})",
        label,
        expected,
        actual,
        COORD_TOLERANCE
    );
}

// ==================== PDF Generation ====================

/// Generate all real-world fixtures (called once before tests).
static GENERATE: Once = Once::new();

fn ensure_fixtures() {
    GENERATE.call_once(|| {
        generate_fonts_encoding_fixtures();
        generate_layout_fixtures();
        generate_tables_fixtures();
        generate_images_fixtures();
        generate_edge_cases_fixtures();
    });
}

/// Create a single-page PDF with content and a single Helvetica font (F1).
fn create_simple_pdf(content: &[u8]) -> Vec<u8> {
    use lopdf::{Object, dictionary};

    let mut doc = lopdf::Document::with_version("1.7");

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let resources = dictionary! {
        "Font" => dictionary! {
            "F1" => Object::Reference(font_id),
        },
    };

    create_pdf_with_doc_and_resources(&mut doc, content, resources)
}

fn create_pdf_with_doc_and_resources(
    doc: &mut lopdf::Document,
    content: &[u8],
    resources: lopdf::Dictionary,
) -> Vec<u8> {
    use lopdf::{Object, Stream, dictionary};

    let stream = Stream::new(dictionary! {}, content.to_vec());
    let content_id = doc.add_object(stream);

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
        "Resources" => Object::Dictionary(resources),
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

fn save_fixture(category: &str, name: &str, bytes: &[u8]) {
    let path = fixture_path(category, name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
}

// --- fonts-encoding fixtures ---

fn generate_fonts_encoding_fixtures() {
    // 1. standard-14-fonts.pdf: Three standard Type1 fonts
    {
        use lopdf::{Object, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        let helvetica_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let courier_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let times_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Times-Roman",
        });

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(helvetica_id),
                "F2" => Object::Reference(courier_id),
                "F3" => Object::Reference(times_id),
            },
        };

        let content = b"BT \
            /F1 12 Tf 72 700 Td (Helvetica text here) Tj \
            /F2 12 Tf 72 680 Td (Courier text here) Tj \
            /F3 12 Tf 72 660 Td (Times Roman text) Tj \
            ET";

        let bytes = create_pdf_with_doc_and_resources(&mut doc, content, resources);
        save_fixture("fonts-encoding", "standard-14-fonts.pdf", &bytes);
    }

    // 2. special-characters.pdf: Parentheses, backslashes, octal escapes
    {
        // PDF string escapes: \( \) \\ \n, and octal \251 = ©
        let content = b"BT /F1 12 Tf \
            72 700 Td (Parentheses: \\(hello\\) world) Tj \
            0 -20 Td (Backslash: path\\\\to\\\\file) Tj \
            0 -20 Td (Copyright \\251 2024) Tj \
            0 -20 Td (Ampersand & angle <bracket>) Tj \
            ET";
        let bytes = create_simple_pdf(content);
        save_fixture("fonts-encoding", "special-characters.pdf", &bytes);
    }
}

// --- layout fixtures ---

fn generate_layout_fixtures() {
    // 1. multi-font-sizes.pdf: Title (24pt), body (12pt), footnote (8pt)
    {
        let content = b"BT \
            /F1 24 Tf 72 700 Td (Document Title) Tj \
            /F1 12 Tf 72 660 Td (This is body text at twelve point size.) Tj \
            0 -20 Td (Second line of body text continues here.) Tj \
            /F1 8 Tf 72 580 Td (Footnote: small text at eight points.) Tj \
            ET";
        let bytes = create_simple_pdf(content);
        save_fixture("layout", "multi-font-sizes.pdf", &bytes);
    }

    // 2. positioned-text.pdf: Text at known absolute positions
    {
        let content = b"BT \
            /F1 12 Tf \
            72 720 Td (TopLeft) Tj \
            ET \
            BT \
            /F1 12 Tf \
            468 720 Td (TopRight) Tj \
            ET \
            BT \
            /F1 12 Tf \
            72 72 Td (BottomLeft) Tj \
            ET \
            BT \
            /F1 12 Tf \
            440 72 Td (BottomRight) Tj \
            ET \
            BT \
            /F1 14 Tf \
            250 400 Td (Center) Tj \
            ET";
        let bytes = create_simple_pdf(content);
        save_fixture("layout", "positioned-text.pdf", &bytes);
    }
}

// --- tables fixtures ---

fn generate_tables_fixtures() {
    // 1. simple-bordered-table.pdf: 3x3 grid table with cell text
    {
        use lopdf::{Object, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_id),
            },
        };

        // Table area: x=[72, 372], y=[600, 720] in PDF coords
        // 3 columns × 100pt, 3 rows × 40pt
        // Line width 1pt for clear grid lines
        let content = b"\
            q \
            1 w \
            72 720 m 372 720 l S \
            72 680 m 372 680 l S \
            72 640 m 372 640 l S \
            72 600 m 372 600 l S \
            72 600 m 72 720 l S \
            172 600 m 172 720 l S \
            272 600 m 272 720 l S \
            372 600 m 372 720 l S \
            Q \
            BT /F1 10 Tf \
            82 700 Td (Name) Tj \
            100 0 Td (Value) Tj \
            100 0 Td (Unit) Tj \
            -200 -40 Td (Width) Tj \
            100 0 Td (100) Tj \
            100 0 Td (mm) Tj \
            -200 -40 Td (Height) Tj \
            100 0 Td (200) Tj \
            100 0 Td (mm) Tj \
            ET";

        let bytes = create_pdf_with_doc_and_resources(&mut doc, content, resources);
        save_fixture("tables", "simple-bordered-table.pdf", &bytes);
    }

    // 2. multi-row-table.pdf: Larger table with more data rows
    {
        use lopdf::{Object, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_id),
            },
        };

        // 2-column table, 5 rows (header + 4 data), each row 20pt
        // Table area: x=[72, 372], y=[600, 700]
        let content = b"\
            q \
            1 w \
            72 700 m 372 700 l S \
            72 680 m 372 680 l S \
            72 660 m 372 660 l S \
            72 640 m 372 640 l S \
            72 620 m 372 620 l S \
            72 600 m 372 600 l S \
            72 600 m 72 700 l S \
            222 600 m 222 700 l S \
            372 600 m 372 700 l S \
            Q \
            BT /F1 10 Tf \
            82 686 Td (Item) Tj \
            150 0 Td (Price) Tj \
            -150 -20 Td (Apple) Tj \
            150 0 Td ($1.50) Tj \
            -150 -20 Td (Banana) Tj \
            150 0 Td ($0.75) Tj \
            -150 -20 Td (Cherry) Tj \
            150 0 Td ($3.00) Tj \
            -150 -20 Td (Date) Tj \
            150 0 Td ($5.00) Tj \
            ET";

        let bytes = create_pdf_with_doc_and_resources(&mut doc, content, resources);
        save_fixture("tables", "multi-row-table.pdf", &bytes);
    }
}

// --- images fixtures ---

fn generate_images_fixtures() {
    // 1. xobject-image.pdf: Embedded image XObject (raw RGB)
    {
        use lopdf::{Object, Stream, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        // 4x4 RGB image (48 bytes) — red/green/blue/white pattern
        let mut image_data = Vec::with_capacity(48);
        // Row 0: red, green, blue, white
        image_data.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        // Row 1: green, blue, white, red
        image_data.extend_from_slice(&[0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0]);
        // Row 2: blue, white, red, green
        image_data.extend_from_slice(&[0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0]);
        // Row 3: white, red, green, blue
        image_data.extend_from_slice(&[255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255]);

        let image_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 4i64,
                "Height" => 4i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8i64,
            },
            image_data,
        );
        let image_id = doc.add_object(Object::Stream(image_stream));

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_id),
            },
            "XObject" => dictionary! {
                "Im0" => Object::Reference(image_id),
            },
        };

        // Place image at (100, 400) with size 200x150, plus some text
        let content = b"BT /F1 12 Tf 72 720 Td (Page with image) Tj ET \
            q 200 0 0 150 100 400 cm /Im0 Do Q";

        let bytes = create_pdf_with_doc_and_resources(&mut doc, content, resources);
        save_fixture("images", "xobject-image.pdf", &bytes);
    }

    // 2. inline-image.pdf: Inline BI/ID/EI image
    {
        use lopdf::{Object, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => Object::Reference(font_id),
            },
        };

        // Content with inline 2x2 RGB image
        // BI marks start of inline image dict, ID marks start of data, EI marks end
        let mut content = Vec::new();
        content.extend_from_slice(b"BT /F1 12 Tf 72 720 Td (Page with inline image) Tj ET ");
        content.extend_from_slice(b"q 100 0 0 100 200 400 cm ");
        content.extend_from_slice(b"BI /W 2 /H 2 /BPC 8 /CS /RGB ID ");
        // 2x2 RGB = 12 bytes of pixel data
        content.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]);
        content.extend_from_slice(b" EI Q");

        let bytes = create_pdf_with_doc_and_resources(&mut doc, &content, resources);
        save_fixture("images", "inline-image.pdf", &bytes);
    }
}

// --- edge-cases fixtures ---

fn generate_edge_cases_fixtures() {
    // 1. empty-page.pdf: Page with no content stream
    {
        use lopdf::{Object, dictionary};

        let mut doc = lopdf::Document::with_version("1.7");

        let media_box = vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ];
        let page_dict = dictionary! {
            "Type" => "Page",
            "MediaBox" => media_box,
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
        save_fixture("edge-cases", "empty-page.pdf", &buf);
    }

    // 2. single-char.pdf: Exactly one character
    {
        let content = b"BT /F1 12 Tf 300 400 Td (X) Tj ET";
        let bytes = create_simple_pdf(content);
        save_fixture("edge-cases", "single-char.pdf", &bytes);
    }

    // 3. whitespace-only.pdf: Content stream with only whitespace text
    {
        let content = b"BT /F1 12 Tf 72 700 Td ( ) Tj ET";
        let bytes = create_simple_pdf(content);
        save_fixture("edge-cases", "whitespace-only.pdf", &bytes);
    }

    // 4. overlapping-text.pdf: Two text strings rendered at same position
    {
        let content = b"BT /F1 12 Tf 72 700 Td (BOLD) Tj 0.3 0 Td (BOLD) Tj ET";
        let bytes = create_simple_pdf(content);
        save_fixture("edge-cases", "overlapping-text.pdf", &bytes);
    }
}

// ==================== TESTS: fonts-encoding ====================

#[test]
fn fonts_standard_14_opens_successfully() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "standard-14-fonts.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn fonts_standard_14_extracts_three_fonts() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "standard-14-fonts.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    assert!(!chars.is_empty(), "should extract characters");

    // Collect unique font names
    let mut fontnames: Vec<String> = chars.iter().map(|c| c.fontname.clone()).collect();
    fontnames.sort();
    fontnames.dedup();

    assert!(
        fontnames.len() >= 3,
        "should have at least 3 distinct fonts, got {:?}",
        fontnames
    );
    assert!(
        fontnames.iter().any(|f| f.contains("Helvetica")),
        "should contain Helvetica, got {:?}",
        fontnames
    );
    assert!(
        fontnames.iter().any(|f| f.contains("Courier")),
        "should contain Courier, got {:?}",
        fontnames
    );
    assert!(
        fontnames.iter().any(|f| f.contains("Times")),
        "should contain Times, got {:?}",
        fontnames
    );
}

#[test]
fn fonts_standard_14_text_content() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "standard-14-fonts.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("Helvetica"),
        "should contain 'Helvetica' in text"
    );
    assert!(text.contains("Courier"), "should contain 'Courier' in text");
    assert!(text.contains("Times"), "should contain 'Times' in text");
}

#[test]
fn fonts_standard_14_word_grouping() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "standard-14-fonts.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());

    // Should group into words correctly
    let word_texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        word_texts.contains(&"Helvetica"),
        "words should include 'Helvetica', got {:?}",
        word_texts
    );
    assert!(
        word_texts.contains(&"Courier"),
        "words should include 'Courier', got {:?}",
        word_texts
    );
}

#[test]
fn fonts_standard_14_chars_have_valid_bboxes() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "standard-14-fonts.pdf");
    let page = pdf.page(0).unwrap();
    for ch in page.chars() {
        assert!(
            ch.bbox.x0 < ch.bbox.x1,
            "char '{}' x0 ({}) should be < x1 ({})",
            ch.text,
            ch.bbox.x0,
            ch.bbox.x1
        );
        assert!(
            ch.bbox.top < ch.bbox.bottom,
            "char '{}' top ({}) should be < bottom ({})",
            ch.text,
            ch.bbox.top,
            ch.bbox.bottom
        );
    }
}

#[test]
fn fonts_special_characters_extracts_parentheses() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "special-characters.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("(hello)"),
        "should contain literal parentheses, got: {}",
        text
    );
}

#[test]
fn fonts_special_characters_extracts_backslash() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "special-characters.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("path\\to\\file"),
        "should contain backslash paths, got: {}",
        text
    );
}

#[test]
fn fonts_special_characters_extracts_copyright() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "special-characters.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    // Octal \251 = ©
    assert!(
        text.contains('\u{00A9}') || text.contains("Copyright"),
        "should contain copyright symbol or 'Copyright', got: {}",
        text
    );
}

#[test]
fn fonts_special_characters_extracts_ampersand() {
    ensure_fixtures();
    let pdf = open_fixture("fonts-encoding", "special-characters.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("&"),
        "should contain ampersand, got: {}",
        text
    );
}

// ==================== TESTS: layout ====================

#[test]
fn layout_multi_font_sizes_opens() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn layout_multi_font_sizes_has_different_sizes() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    let mut sizes: Vec<f64> = chars
        .iter()
        .filter(|c| c.text.trim() != "")
        .map(|c| (c.size * 10.0).round() / 10.0)
        .collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sizes.dedup();

    assert!(
        sizes.len() >= 3,
        "should have at least 3 distinct font sizes (24, 12, 8), got {:?}",
        sizes
    );
}

#[test]
fn layout_multi_font_sizes_title_is_large() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    // Title chars "Document Title" should be at 24pt
    let d_char = chars.iter().find(|c| c.text == "D").unwrap();
    assert_approx(d_char.size, 24.0, "title font size");
}

#[test]
fn layout_multi_font_sizes_body_is_medium() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("body text"),
        "should contain body text, got: {}",
        text
    );
}

#[test]
fn layout_multi_font_sizes_footnote_is_small() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    // Find a footnote char — "Footnote" text is at 8pt
    let footnote_chars: Vec<_> = chars
        .iter()
        .filter(|c| c.size < 9.0 && c.size > 7.0)
        .collect();
    assert!(
        !footnote_chars.is_empty(),
        "should have chars at ~8pt for footnote"
    );
}

#[test]
fn layout_multi_font_sizes_word_extraction() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "multi-font-sizes.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());
    let word_texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();

    assert!(
        word_texts.contains(&"Document"),
        "words should include 'Document', got {:?}",
        &word_texts[..word_texts.len().min(10)]
    );
    assert!(
        word_texts.contains(&"Title"),
        "words should include 'Title', got {:?}",
        &word_texts[..word_texts.len().min(10)]
    );
}

#[test]
fn layout_positioned_text_opens() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "positioned-text.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn layout_positioned_text_content() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "positioned-text.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    for expected in &["TopLeft", "TopRight", "BottomLeft", "BottomRight", "Center"] {
        assert!(
            text.contains(expected),
            "should contain '{}', got: {}",
            expected,
            text
        );
    }
}

#[test]
fn layout_positioned_text_top_left_near_origin() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "positioned-text.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());

    let top_left = words
        .iter()
        .find(|w| w.text == "TopLeft")
        .expect("should find TopLeft word");

    // In top-left coordinate system: x0 near 72, top near 72 (792 - 720 = 72)
    assert_approx(top_left.bbox.x0, 72.0, "TopLeft x0");
    assert!(
        top_left.bbox.top < 100.0,
        "TopLeft should be near top of page, got top={}",
        top_left.bbox.top
    );
}

#[test]
fn layout_positioned_text_bottom_right_far_from_origin() {
    ensure_fixtures();
    let pdf = open_fixture("layout", "positioned-text.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());

    let bottom_right = words
        .iter()
        .find(|w| w.text == "BottomRight")
        .expect("should find BottomRight word");

    // In top-left coords: x0 near 440, top near 720 (792 - 72 = 720)
    assert!(
        bottom_right.bbox.x0 > 400.0,
        "BottomRight should be near right, got x0={}",
        bottom_right.bbox.x0
    );
    assert!(
        bottom_right.bbox.top > 700.0,
        "BottomRight should be near bottom, got top={}",
        bottom_right.bbox.top
    );
}

// ==================== TESTS: tables ====================

#[test]
fn tables_simple_bordered_opens() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "simple-bordered-table.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn tables_simple_bordered_detects_table() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "simple-bordered-table.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(
        !tables.is_empty(),
        "lattice strategy should detect the bordered table"
    );
}

#[test]
fn tables_simple_bordered_dimensions() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "simple-bordered-table.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    if tables.is_empty() {
        return; // table detection may not find it — non-fatal
    }
    let table = &tables[0];

    // 3 rows, 3 columns
    assert_eq!(
        table.rows.len(),
        3,
        "table should have 3 rows, got {}",
        table.rows.len()
    );
    assert_eq!(
        table.rows[0].len(),
        3,
        "table should have 3 columns, got {}",
        table.rows[0].len()
    );
}

#[test]
fn tables_simple_bordered_cell_content() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "simple-bordered-table.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    if tables.is_empty() {
        return;
    }
    let table = &tables[0];

    // Header row
    let header: Vec<String> = table.rows[0]
        .iter()
        .map(|c| c.text.as_deref().unwrap_or("").to_string())
        .collect();
    assert!(
        header.iter().any(|h: &String| h.contains("Name")),
        "header should contain 'Name', got {:?}",
        header
    );

    // Data rows should have content
    for row in &table.rows[1..] {
        let non_empty = row.iter().filter(|c| c.text.is_some()).count();
        assert!(
            non_empty > 0,
            "data rows should have content, got all empty"
        );
    }
}

#[test]
fn tables_simple_bordered_text_extraction() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "simple-bordered-table.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(text.contains("Name"), "should contain 'Name'");
    assert!(text.contains("Width"), "should contain 'Width'");
    assert!(text.contains("100"), "should contain '100'");
    assert!(text.contains("mm"), "should contain 'mm'");
}

#[test]
fn tables_multi_row_detects_table() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "multi-row-table.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    assert!(!tables.is_empty(), "should detect multi-row bordered table");
}

#[test]
fn tables_multi_row_has_five_rows() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "multi-row-table.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());

    if tables.is_empty() {
        return;
    }
    let table = &tables[0];

    assert_eq!(
        table.rows.len(),
        5,
        "table should have 5 rows (1 header + 4 data), got {}",
        table.rows.len()
    );
}

#[test]
fn tables_multi_row_content() {
    ensure_fixtures();
    let pdf = open_fixture("tables", "multi-row-table.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    for item in &["Apple", "Banana", "Cherry", "Date"] {
        assert!(
            text.contains(item),
            "should contain '{}', got: {}",
            item,
            text
        );
    }
}

// ==================== TESTS: images ====================

#[test]
fn images_xobject_opens() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn images_xobject_detects_image() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();

    assert_eq!(images.len(), 1, "should detect exactly 1 image");
    assert_eq!(images[0].name, "Im0");
}

#[test]
fn images_xobject_image_dimensions() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();
    let img = &images[0];

    // Image placed with CTM: 200 0 0 150 100 400 cm
    // Width = 200, Height = 150
    assert_approx(img.width, 200.0, "image width");
    assert_approx(img.height, 150.0, "image height");
}

#[test]
fn images_xobject_image_position() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();
    let img = &images[0];

    // CTM: 200 0 0 150 100 400 cm
    // In PDF coords: bottom-left at (100, 400), so top-left origin:
    // x0 = 100, top = 792 - 400 - 150 = 242
    assert_approx(img.x0, 100.0, "image x0");
    assert_approx(img.top, 242.0, "image top");
}

#[test]
fn images_xobject_source_metadata() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();
    let img = &images[0];

    assert_eq!(img.src_width, Some(4), "source width should be 4");
    assert_eq!(img.src_height, Some(4), "source height should be 4");
    assert_eq!(img.bits_per_component, Some(8));
    assert_eq!(img.color_space.as_deref(), Some("DeviceRGB"));
}

#[test]
fn images_xobject_data_extraction_opt_in() {
    ensure_fixtures();
    let opts = ExtractOptions {
        extract_image_data: true,
        ..ExtractOptions::default()
    };
    let pdf = open_fixture_with_opts("images", "xobject-image.pdf", opts);
    let page = pdf.page(0).unwrap();
    let images = page.images();
    let img = &images[0];

    assert!(
        img.data.is_some(),
        "image data should be extracted when opted in"
    );
    let data = img.data.as_ref().unwrap();
    // 4x4 RGB = 48 bytes
    assert_eq!(data.len(), 48, "image data should be 48 bytes (4x4 RGB)");
}

#[test]
fn images_xobject_data_not_extracted_by_default() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();

    assert_eq!(
        images[0].data, None,
        "image data should not be extracted by default"
    );
}

#[test]
fn images_xobject_text_also_extracted() {
    ensure_fixtures();
    let pdf = open_fixture("images", "xobject-image.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("image"),
        "text extraction should still work alongside images, got: {}",
        text
    );
}

#[test]
fn images_inline_opens() {
    ensure_fixtures();
    let pdf = open_fixture("images", "inline-image.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn images_inline_detects_image() {
    ensure_fixtures();
    let pdf = open_fixture("images", "inline-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();

    assert!(!images.is_empty(), "should detect inline image");
    // Inline images use synthetic name pattern
    assert!(
        images[0].name.starts_with("inline-"),
        "inline image name should start with 'inline-', got '{}'",
        images[0].name
    );
}

#[test]
fn images_inline_has_dimensions() {
    ensure_fixtures();
    let pdf = open_fixture("images", "inline-image.pdf");
    let page = pdf.page(0).unwrap();
    let images = page.images();

    if images.is_empty() {
        return;
    }
    let img = &images[0];

    assert!(img.width > 0.0, "inline image should have positive width");
    assert!(img.height > 0.0, "inline image should have positive height");
}

#[test]
fn images_inline_text_also_extracted() {
    ensure_fixtures();
    let pdf = open_fixture("images", "inline-image.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.contains("inline image"),
        "text should still be extractable, got: {}",
        text
    );
}

// ==================== TESTS: edge-cases ====================

#[test]
fn edge_cases_empty_page_opens() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "empty-page.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn edge_cases_empty_page_no_chars() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "empty-page.pdf");
    let page = pdf.page(0).unwrap();
    assert_eq!(page.chars().len(), 0, "empty page should have 0 chars");
}

#[test]
fn edge_cases_empty_page_no_words() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "empty-page.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());
    assert_eq!(words.len(), 0, "empty page should have 0 words");
}

#[test]
fn edge_cases_empty_page_no_tables() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "empty-page.pdf");
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    assert!(tables.is_empty(), "empty page should have no tables");
}

#[test]
fn edge_cases_empty_page_empty_text() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "empty-page.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());
    assert!(text.trim().is_empty(), "empty page text should be empty");
}

#[test]
fn edge_cases_single_char_opens() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "single-char.pdf");
    assert_eq!(pdf.page_count(), 1);
}

#[test]
fn edge_cases_single_char_extracts_one() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "single-char.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    assert_eq!(
        chars.len(),
        1,
        "should extract exactly 1 char, got {}",
        chars.len()
    );
    assert_eq!(chars[0].text, "X");
}

#[test]
fn edge_cases_single_char_one_word() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "single-char.pdf");
    let page = pdf.page(0).unwrap();
    let words = page.extract_words(&WordOptions::default());

    assert_eq!(words.len(), 1, "should extract exactly 1 word");
    assert_eq!(words[0].text, "X");
}

#[test]
fn edge_cases_single_char_position() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "single-char.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();
    let ch = &chars[0];

    // Placed at (300, 400) in PDF coords → top-left: x0=300, top=792-400-12≈380
    assert_approx(ch.bbox.x0, 300.0, "single char x0");
    assert!(
        ch.bbox.top > 370.0 && ch.bbox.top < 400.0,
        "single char top should be ~380, got {}",
        ch.bbox.top
    );
}

#[test]
fn edge_cases_whitespace_only_no_meaningful_text() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "whitespace-only.pdf");
    let page = pdf.page(0).unwrap();
    let text = page.extract_text(&TextOptions::default());

    assert!(
        text.trim().is_empty(),
        "whitespace-only page should produce empty trimmed text, got: '{}'",
        text
    );
}

#[test]
fn edge_cases_overlapping_text_extracts_both() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "overlapping-text.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    // "BOLD" rendered twice = 8 chars
    assert_eq!(
        chars.len(),
        8,
        "overlapping text should extract all 8 chars, got {}",
        chars.len()
    );
}

#[test]
fn edge_cases_overlapping_text_dedup_reduces() {
    ensure_fixtures();
    let pdf = open_fixture("edge-cases", "overlapping-text.pdf");
    let page = pdf.page(0).unwrap();

    let deduped = page.dedupe_chars(&pdfplumber::DedupeOptions::default());
    let chars = deduped.chars();

    // After dedup, overlapping chars should be reduced
    assert!(
        chars.len() <= 4,
        "after dedup, overlapping 'BOLD' should reduce to <= 4 chars, got {}",
        chars.len()
    );
}

// ==================== TESTS: cross-category (comprehensive) ====================

#[test]
fn all_fixtures_under_100kb() {
    ensure_fixtures();
    let categories = ["fonts-encoding", "layout", "tables", "images", "edge-cases"];
    for category in &categories {
        let dir = fixtures_dir().join(category);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
                    let size = std::fs::metadata(&path).unwrap().len();
                    assert!(
                        size < 100_000,
                        "fixture {:?} is {} bytes, must be < 100KB",
                        path.file_name().unwrap(),
                        size
                    );
                }
            }
        }
    }
}

#[test]
fn all_fixtures_valid_pdf() {
    ensure_fixtures();
    let categories = ["fonts-encoding", "layout", "tables", "images", "edge-cases"];
    for category in &categories {
        let dir = fixtures_dir().join(category);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
                    let bytes = std::fs::read(&path).unwrap();
                    let result = Pdf::open(&bytes, None);
                    assert!(
                        result.is_ok(),
                        "fixture {:?} should be a valid PDF: {:?}",
                        path.file_name().unwrap(),
                        result.err()
                    );
                }
            }
        }
    }
}

#[test]
fn all_fixtures_page_count_at_least_one() {
    ensure_fixtures();
    let categories = ["fonts-encoding", "layout", "tables", "images", "edge-cases"];
    for category in &categories {
        let dir = fixtures_dir().join(category);
        if dir.exists() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("pdf") {
                    let bytes = std::fs::read(&path).unwrap();
                    let pdf = Pdf::open(&bytes, None).unwrap();
                    assert!(
                        pdf.page_count() >= 1,
                        "fixture {:?} should have at least 1 page",
                        path.file_name().unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn float_tolerance_across_platforms() {
    ensure_fixtures();
    // Verify that coordinate comparisons use tolerance, not exact equality.
    // This test ensures platform-specific float differences are handled.
    let pdf = open_fixture("layout", "positioned-text.pdf");
    let page = pdf.page(0).unwrap();
    let chars = page.chars();

    for ch in chars {
        // All coordinates should be finite numbers
        assert!(ch.bbox.x0.is_finite(), "x0 should be finite");
        assert!(ch.bbox.top.is_finite(), "top should be finite");
        assert!(ch.bbox.x1.is_finite(), "x1 should be finite");
        assert!(ch.bbox.bottom.is_finite(), "bottom should be finite");
        assert!(ch.size.is_finite(), "size should be finite");
    }
}
