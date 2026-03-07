use super::*;
use lopdf::{Document, Object, dictionary};

// ========== FontMetrics struct tests (TDD: Red phase) ==========

#[test]
fn width_lookup_within_range() {
    let metrics = FontMetrics::new(
        vec![250.0, 500.0, 750.0],
        65, // 'A'
        67, // 'C'
        0.0,
        DEFAULT_ASCENT,
        DEFAULT_DESCENT,
        None,
    );
    assert_eq!(metrics.get_width(65), 250.0); // 'A'
    assert_eq!(metrics.get_width(66), 500.0); // 'B'
    assert_eq!(metrics.get_width(67), 750.0); // 'C'
}

#[test]
fn width_lookup_out_of_range_returns_missing_width() {
    let metrics = FontMetrics::new(
        vec![250.0, 500.0],
        65,
        66,
        300.0, // missing width
        DEFAULT_ASCENT,
        DEFAULT_DESCENT,
        None,
    );
    // Below first_char
    assert_eq!(metrics.get_width(64), 300.0);
    // Above last_char
    assert_eq!(metrics.get_width(67), 300.0);
}

#[test]
fn width_lookup_with_zero_missing_width() {
    let metrics = FontMetrics::new(
        vec![600.0],
        32, // space
        32,
        0.0,
        DEFAULT_ASCENT,
        DEFAULT_DESCENT,
        None,
    );
    assert_eq!(metrics.get_width(32), 600.0);
    assert_eq!(metrics.get_width(65), 0.0); // out of range
}

#[test]
fn width_lookup_empty_widths_returns_missing_width() {
    let metrics = FontMetrics::new(vec![], 0, 0, 500.0, DEFAULT_ASCENT, DEFAULT_DESCENT, None);
    assert_eq!(metrics.get_width(0), 500.0);
    assert_eq!(metrics.get_width(65), 500.0);
}

#[test]
fn width_lookup_widths_shorter_than_range() {
    // LastChar - FirstChar + 1 > widths.len()
    let metrics = FontMetrics::new(
        vec![250.0, 500.0], // only 2 widths
        65,
        70, // but range is 65..70 (6 chars)
        300.0,
        DEFAULT_ASCENT,
        DEFAULT_DESCENT,
        None,
    );
    assert_eq!(metrics.get_width(65), 250.0);
    assert_eq!(metrics.get_width(66), 500.0);
    assert_eq!(metrics.get_width(67), 300.0); // index 2 > widths.len(), fallback
}

#[test]
fn ascent_and_descent() {
    let metrics = FontMetrics::new(vec![], 0, 0, 0.0, 800.0, -200.0, None);
    assert_eq!(metrics.ascent(), 800.0);
    assert_eq!(metrics.descent(), -200.0);
}

#[test]
fn font_bbox_some() {
    let bbox = [-100.0, -250.0, 1100.0, 900.0];
    let metrics = FontMetrics::new(vec![], 0, 0, 0.0, 0.0, 0.0, Some(bbox));
    assert_eq!(metrics.font_bbox(), Some([-100.0, -250.0, 1100.0, 900.0]));
}

#[test]
fn font_bbox_none() {
    let metrics = FontMetrics::new(vec![], 0, 0, 0.0, 0.0, 0.0, None);
    assert_eq!(metrics.font_bbox(), None);
}

#[test]
fn default_metrics_values() {
    let metrics = FontMetrics::default_metrics();
    assert_eq!(metrics.ascent(), DEFAULT_ASCENT);
    assert_eq!(metrics.descent(), DEFAULT_DESCENT);
    assert_eq!(metrics.missing_width(), DEFAULT_WIDTH);
    assert_eq!(metrics.first_char(), 0);
    assert_eq!(metrics.last_char(), 0);
    assert_eq!(metrics.font_bbox(), None);
    // Any char code returns default width
    assert_eq!(metrics.get_width(65), DEFAULT_WIDTH);
}

#[test]
fn first_char_last_char_accessors() {
    let metrics = FontMetrics::new(vec![500.0], 32, 32, 0.0, 0.0, 0.0, None);
    assert_eq!(metrics.first_char(), 32);
    assert_eq!(metrics.last_char(), 32);
}

#[test]
fn width_lookup_large_char_code() {
    let metrics = FontMetrics::new(vec![600.0], 0xFFFF, 0xFFFF, 0.0, 0.0, 0.0, None);
    assert_eq!(metrics.get_width(0xFFFF), 600.0);
    assert_eq!(metrics.get_width(0xFFFE), 0.0);
}

// ========== extract_font_metrics tests (lopdf parsing) ==========

/// Helper: create a lopdf font dictionary with /Widths, /FirstChar, /LastChar.
fn create_font_dict_with_widths(
    doc: &mut Document,
    widths: &[f64],
    first_char: i64,
    last_char: i64,
) -> lopdf::Dictionary {
    let width_objects: Vec<Object> = widths.iter().map(|w| Object::Real(*w as f32)).collect();
    let widths_id = doc.add_object(Object::Array(width_objects));

    dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "FirstChar" => first_char,
        "LastChar" => last_char,
        "Widths" => widths_id,
    }
}

/// Helper: add a /FontDescriptor to a font dictionary.
fn add_font_descriptor(
    doc: &mut Document,
    font_dict: &mut lopdf::Dictionary,
    ascent: f64,
    descent: f64,
    missing_width: Option<f64>,
    font_bbox: Option<[f64; 4]>,
) {
    let mut desc = dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "Helvetica",
        "Ascent" => Object::Real(ascent as f32),
        "Descent" => Object::Real(descent as f32),
    };
    if let Some(mw) = missing_width {
        desc.set("MissingWidth", Object::Real(mw as f32));
    }
    if let Some(bbox) = font_bbox {
        desc.set(
            "FontBBox",
            Object::Array(bbox.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    let desc_id = doc.add_object(Object::Dictionary(desc));
    font_dict.set("FontDescriptor", desc_id);
}

#[test]
fn extract_metrics_with_widths_and_descriptor() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[278.0, 556.0, 722.0], 65, 67);
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        718.0,
        -207.0,
        Some(278.0),
        Some([-166.0, -225.0, 1000.0, 931.0]),
    );

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 278.0); // A
    assert_eq!(metrics.get_width(66), 556.0); // B
    assert_eq!(metrics.get_width(67), 722.0); // C
    assert_eq!(metrics.get_width(68), 278.0); // D — missing width
    assert!((metrics.ascent() - 718.0).abs() < 1.0);
    assert!((metrics.descent() - (-207.0)).abs() < 1.0);
    assert!(metrics.font_bbox().is_some());
}

#[test]
fn extract_metrics_without_font_descriptor() {
    let mut doc = Document::with_version("1.5");
    let font_dict = create_font_dict_with_widths(&mut doc, &[500.0, 600.0], 32, 33);
    // No FontDescriptor added

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(32), 500.0);
    assert_eq!(metrics.get_width(33), 600.0);
    // Defaults for missing descriptor
    assert_eq!(metrics.ascent(), DEFAULT_ASCENT);
    assert_eq!(metrics.descent(), DEFAULT_DESCENT);
    assert_eq!(metrics.missing_width(), DEFAULT_WIDTH);
}

#[test]
fn extract_metrics_without_widths() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // No /Widths — Helvetica is a standard font, so standard widths are used
    assert_eq!(metrics.get_width(65), 667.0); // Helvetica 'A' = 667
    assert!((metrics.ascent() - 800.0).abs() < 1.0);
    assert!((metrics.descent() - (-200.0)).abs() < 1.0);
}

#[test]
fn extract_metrics_empty_font_dict() {
    let doc = Document::with_version("1.5");
    let font_dict = dictionary! {};

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Everything defaults
    assert_eq!(metrics.ascent(), DEFAULT_ASCENT);
    assert_eq!(metrics.descent(), DEFAULT_DESCENT);
    assert_eq!(metrics.missing_width(), DEFAULT_WIDTH);
    assert_eq!(metrics.get_width(65), DEFAULT_WIDTH);
}

#[test]
fn extract_metrics_descriptor_without_missing_width() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[400.0], 65, 65);
    add_font_descriptor(&mut doc, &mut font_dict, 700.0, -300.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 400.0);
    // MissingWidth defaults to DEFAULT_WIDTH when not in descriptor
    assert_eq!(metrics.missing_width(), DEFAULT_WIDTH);
}

#[test]
fn extract_metrics_with_integer_widths() {
    let mut doc = Document::with_version("1.5");
    // Use Integer objects instead of Real for widths
    let width_objects: Vec<Object> = vec![Object::Integer(250), Object::Integer(500)];
    let widths_id = doc.add_object(Object::Array(width_objects));

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "Arial",
        "FirstChar" => 65i64,
        "LastChar" => 66i64,
        "Widths" => widths_id,
    };

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 250.0);
    assert_eq!(metrics.get_width(66), 500.0);
}

#[test]
fn extract_metrics_with_font_bbox() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    };
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        629.0,
        -157.0,
        Some(600.0),
        Some([-23.0, -250.0, 715.0, 805.0]),
    );

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    let bbox = metrics.font_bbox().unwrap();
    assert!((bbox[0] - (-23.0)).abs() < 1.0);
    assert!((bbox[1] - (-250.0)).abs() < 1.0);
    assert!((bbox[2] - 715.0).abs() < 1.0);
    assert!((bbox[3] - 805.0).abs() < 1.0);
}

#[test]
fn extract_metrics_integer_first_last_char() {
    let mut doc = Document::with_version("1.5");
    let widths_id = doc.add_object(Object::Array(vec![Object::Integer(600)]));

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
        "FirstChar" => 32i64,
        "LastChar" => 32i64,
        "Widths" => widths_id,
    };

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.first_char(), 32);
    assert_eq!(metrics.last_char(), 32);
    assert_eq!(metrics.get_width(32), 600.0);
}

#[test]
fn extract_metrics_indirect_font_descriptor() {
    let mut doc = Document::with_version("1.5");
    let desc_id = doc.add_object(Object::Dictionary(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "Times-Roman",
        "Ascent" => Object::Real(683.0),
        "Descent" => Object::Real(-217.0),
        "MissingWidth" => Object::Integer(250),
    }));

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Times-Roman",
        "FontDescriptor" => desc_id,
    };

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert!((metrics.ascent() - 683.0).abs() < 1.0);
    assert!((metrics.descent() - (-217.0)).abs() < 1.0);
    assert!((metrics.missing_width() - 250.0).abs() < 1.0);
}

#[test]
fn width_as_get_width_callback() {
    // Verify FontMetrics works as the width callback for text_renderer
    let metrics = FontMetrics::new(
        vec![278.0, 556.0, 722.0],
        65,
        67,
        278.0,
        718.0,
        -207.0,
        None,
    );
    let get_width: &dyn Fn(u32) -> f64 = &|code| metrics.get_width(code);
    assert_eq!(get_width(65), 278.0);
    assert_eq!(get_width(66), 556.0);
    assert_eq!(get_width(68), 278.0); // missing
}

// ========== US-104: Standard font fallback tests ==========

#[test]
fn fallback_helvetica_no_widths_uses_standard_widths() {
    // When /Widths is absent and BaseFont is Helvetica, use standard widths
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Helvetica 'A'(65) = 667, space(32) = 278 — proportional, NOT 600
    assert_eq!(metrics.get_width(65), 667.0); // A
    assert_eq!(metrics.get_width(32), 278.0); // space
    assert_eq!(metrics.get_width(66), 667.0); // B
}

#[test]
fn fallback_courier_no_widths_uses_standard_widths() {
    // Courier is monospaced — all widths should be 600
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 629.0, -157.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 600.0); // A
    assert_eq!(metrics.get_width(32), 600.0); // space
    assert_eq!(metrics.get_width(97), 600.0); // a
}

#[test]
fn fallback_times_roman_no_widths_uses_standard_widths() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Times-Roman",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 683.0, -217.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Times-Roman 'A'(65) = 722
    assert_eq!(metrics.get_width(65), 722.0); // A
    // Times-Roman space(32) = 250
    assert_eq!(metrics.get_width(32), 250.0); // space
}

#[test]
fn fallback_subset_prefix_stripped() {
    // ABCDEF+Helvetica should match Helvetica
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 667.0); // A = Helvetica width
}

#[test]
fn fallback_unknown_font_uses_default_width() {
    // Non-standard font without /Widths should fall back to DEFAULT_WIDTH
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "CustomFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 700.0, -300.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), DEFAULT_WIDTH); // not a standard font
}

#[test]
fn fallback_does_not_affect_embedded_widths() {
    // PDFs with /Widths arrays must be completely unaffected
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[500.0, 600.0, 700.0], 65, 67);
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should use embedded widths, NOT standard Helvetica widths (667, 667, 722)
    assert_eq!(metrics.get_width(65), 500.0);
    assert_eq!(metrics.get_width(66), 600.0);
    assert_eq!(metrics.get_width(67), 700.0);
}

#[test]
fn fallback_descriptor_ascent_descent_override_standard() {
    // FontDescriptor ascent/descent should override standard defaults
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -250.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Ascent/descent from descriptor, not standard defaults
    assert!((metrics.ascent() - 800.0).abs() < 1.0);
    assert!((metrics.descent() - (-250.0)).abs() < 1.0);
}

#[test]
fn fallback_standard_font_bbox_used_when_descriptor_lacks_bbox() {
    // When descriptor has no FontBBox, use standard font's bbox
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    // Descriptor without FontBBox
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should use Helvetica's standard bbox: [-166, -225, 1000, 931]
    let bbox = metrics.font_bbox().expect("should have standard font bbox");
    assert!((bbox[0] - (-166.0)).abs() < 1.0);
    assert!((bbox[1] - (-225.0)).abs() < 1.0);
    assert!((bbox[2] - 1000.0).abs() < 1.0);
    assert!((bbox[3] - 931.0).abs() < 1.0);
}

#[test]
fn fallback_descriptor_bbox_overrides_standard() {
    // When descriptor has FontBBox, it should take precedence
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    let custom_bbox = [-100.0, -200.0, 900.0, 800.0];
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        718.0,
        -207.0,
        None,
        Some(custom_bbox),
    );

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    let bbox = metrics.font_bbox().unwrap();
    assert!((bbox[0] - (-100.0)).abs() < 1.0);
    assert!((bbox[1] - (-200.0)).abs() < 1.0);
}

#[test]
fn fallback_no_basefont_uses_default() {
    // If no BaseFont at all, should use defaults
    let doc = Document::with_version("1.5");
    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
    };

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), DEFAULT_WIDTH);
}

// ========== US-186-2: Positive Descent normalization ==========

#[test]
fn positive_descent_normalized_to_negative() {
    // Some PDFs (e.g., annotations.pdf BAAAAA+Arial-BoldMT) have a positive
    // Descent value in the FontDescriptor, which violates the PDF spec.
    // The parser should normalize positive Descent to negative.
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[722.0], 65, 65);
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        905.0,
        211.0, // positive — should be -211
        None,
        None,
    );

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Descent should be normalized to -211
    assert!(
        metrics.descent() < 0.0,
        "positive Descent should be normalized to negative, got {}",
        metrics.descent()
    );
    assert!((metrics.descent() - (-211.0)).abs() < 1.0);
}

#[test]
fn negative_descent_unchanged() {
    // Normal negative descent should remain unchanged
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[722.0], 65, 65);
    add_font_descriptor(&mut doc, &mut font_dict, 905.0, -212.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();
    assert!((metrics.descent() - (-212.0)).abs() < 1.0);
}

#[test]
fn zero_descent_unchanged() {
    // Zero descent should remain zero (triggers special handling in interpreter)
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[722.0], 65, 65);
    add_font_descriptor(&mut doc, &mut font_dict, 0.0, 0.0, None, None);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();
    assert_eq!(metrics.descent(), 0.0);
}

// ========== US-205-7: TrueType /FontFile2 hmtx fallback ==========

/// Helper: build minimal TrueType data for testing /FontFile2 integration.
fn build_test_truetype_data(units_per_em: u16, widths: &[u16]) -> Vec<u8> {
    let num_h_metrics = widths.len() as u16;
    let num_glyphs = num_h_metrics;
    let num_tables: u16 = 4;

    let head_len: u32 = 54;
    let hhea_len: u32 = 36;
    let maxp_len: u32 = 6;
    let hmtx_len: u32 = num_h_metrics as u32 * 4;

    let dir_end: u32 = 12 + num_tables as u32 * 16;
    let head_off = dir_end;
    let hhea_off = head_off + head_len;
    let maxp_off = hhea_off + hhea_len;
    let hmtx_off = maxp_off + maxp_len;
    let total_len = hmtx_off + hmtx_len;

    let mut buf = vec![0u8; total_len as usize];

    // Offset table
    buf[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    buf[4..6].copy_from_slice(&num_tables.to_be_bytes());

    // Table directory
    let tables: [(&[u8; 4], u32, u32); 4] = [
        (b"head", head_off, head_len),
        (b"hhea", hhea_off, hhea_len),
        (b"maxp", maxp_off, maxp_len),
        (b"hmtx", hmtx_off, hmtx_len),
    ];
    for (i, (tag, off, len)) in tables.iter().enumerate() {
        let entry = 12 + i * 16;
        buf[entry..entry + 4].copy_from_slice(*tag);
        buf[entry + 8..entry + 12].copy_from_slice(&off.to_be_bytes());
        buf[entry + 12..entry + 16].copy_from_slice(&len.to_be_bytes());
    }

    // head table: unitsPerEm at offset 18
    buf[head_off as usize..head_off as usize + 4].copy_from_slice(&0x00010000u32.to_be_bytes());
    buf[head_off as usize + 18..head_off as usize + 20]
        .copy_from_slice(&units_per_em.to_be_bytes());

    // hhea table: numberOfHMetrics at offset 34
    buf[hhea_off as usize..hhea_off as usize + 4].copy_from_slice(&0x00010000u32.to_be_bytes());
    buf[hhea_off as usize + 34..hhea_off as usize + 36]
        .copy_from_slice(&num_h_metrics.to_be_bytes());

    // maxp table
    buf[maxp_off as usize..maxp_off as usize + 4].copy_from_slice(&0x00005000u32.to_be_bytes());
    buf[maxp_off as usize + 4..maxp_off as usize + 6].copy_from_slice(&num_glyphs.to_be_bytes());

    // hmtx table
    for (i, &w) in widths.iter().enumerate() {
        let pos = hmtx_off as usize + i * 4;
        buf[pos..pos + 2].copy_from_slice(&w.to_be_bytes());
    }

    buf
}

/// Helper: add a /FontFile2 stream to a /FontDescriptor in a font dictionary.
fn add_font_file2(doc: &mut Document, font_dict: &mut lopdf::Dictionary, truetype_data: Vec<u8>) {
    // Get existing FontDescriptor or create one
    let desc_id = if let Ok(obj) = font_dict.get(b"FontDescriptor") {
        if let lopdf::Object::Reference(id) = obj {
            *id
        } else {
            // Shouldn't happen in tests, but fallback
            return;
        }
    } else {
        // Create a minimal descriptor
        let desc = dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "TestFont",
            "Ascent" => Object::Real(750.0),
            "Descent" => Object::Real(-250.0),
        };
        let id = doc.add_object(Object::Dictionary(desc));
        font_dict.set("FontDescriptor", id);
        id
    };

    // Create /FontFile2 stream (uncompressed for testing)
    let stream = lopdf::Stream::new(lopdf::Dictionary::new(), truetype_data);
    let ff2_id = doc.add_object(Object::Stream(stream));

    // Add /FontFile2 to the descriptor
    if let Ok(desc_obj) = doc.get_object_mut(desc_id) {
        if let Object::Dictionary(desc) = desc_obj {
            desc.set("FontFile2", ff2_id);
        }
    }
}

#[test]
fn truetype_fallback_when_no_widths() {
    // TrueType font with /FontFile2 but no /Widths array
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "ABCDEF+CustomTTFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    // Add TrueType data with known widths: glyph 0=0, glyph 1=278, glyph 2=556
    let tt_data = build_test_truetype_data(1000, &[0, 278, 556]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should use TrueType hmtx widths, not DEFAULT_WIDTH
    assert!((metrics.get_width(0) - 0.0).abs() < 0.01); // .notdef
    assert!((metrics.get_width(1) - 278.0).abs() < 0.01);
    assert!((metrics.get_width(2) - 556.0).abs() < 0.01);
    // Out of range falls back to missing_width
    assert!((metrics.get_width(3) - 500.0).abs() < 0.01);
}

#[test]
fn truetype_fallback_with_2048_upem() {
    // TrueType font with 2048 upem
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "TestFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, None, None);

    // 1024 in 2048 upem = 500 in 1000 upem
    let tt_data = build_test_truetype_data(2048, &[0, 1024, 2048]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert!((metrics.get_width(1) - 500.0).abs() < 0.1);
    assert!((metrics.get_width(2) - 1000.0).abs() < 0.1);
}

#[test]
fn truetype_fallback_does_not_override_explicit_widths() {
    // When /Widths IS present, /FontFile2 should NOT be used
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[400.0, 600.0], 65, 66);
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, None, None);

    // TrueType data with different widths
    let tt_data = build_test_truetype_data(1000, &[0, 278, 556, 722, 833]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should use /Widths, not hmtx widths
    assert_eq!(metrics.get_width(65), 400.0);
    assert_eq!(metrics.get_width(66), 600.0);
}

#[test]
fn truetype_fallback_preserves_descriptor_values() {
    // TrueType fallback should preserve ascent/descent/bbox from descriptor
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "TestFont",
    };
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        850.0,
        -150.0,
        Some(300.0),
        Some([-100.0, -200.0, 1100.0, 900.0]),
    );
    let tt_data = build_test_truetype_data(1000, &[500]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert!((metrics.ascent() - 850.0).abs() < 1.0);
    assert!((metrics.descent() - (-150.0)).abs() < 1.0);
    assert!((metrics.missing_width() - 300.0).abs() < 1.0);
    assert!(metrics.font_bbox().is_some());
}

#[test]
fn truetype_fallback_standard_font_takes_priority() {
    // Standard font fallback should take priority over TrueType
    // (if font is both standard AND has /FontFile2, standard widths win)
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    // Add TrueType data (shouldn't be used since standard font matches first)
    let tt_data = build_test_truetype_data(1000, &[999]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should use standard Helvetica widths, not TrueType
    assert_eq!(metrics.get_width(65), 667.0); // Helvetica 'A'
}

#[test]
fn truetype_fallback_invalid_data_falls_through() {
    // If /FontFile2 contains invalid data, should fall through to default
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "ABCDEF+BrokenFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    // Add invalid TrueType data
    add_font_file2(&mut doc, &mut font_dict, vec![0xFF; 100]);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // Should fall back to missing_width since TrueType parse failed
    assert_eq!(metrics.get_width(65), 500.0); // missing_width
}

// ========== US-205-8: CFF /FontFile3 fallback ==========

/// Helper: build minimal CFF data for testing /FontFile3 integration.
fn build_test_cff_data(glyph_widths: &[i32]) -> Vec<u8> {
    crate::cff::tests::build_cff_data_for_test(0, 0, glyph_widths)
}

/// Helper: add a /FontFile3 stream with Type1C subtype to a font descriptor.
fn add_font_file3(doc: &mut Document, font_dict: &mut lopdf::Dictionary, cff_data: Vec<u8>) {
    let desc_id = if let Ok(obj) = font_dict.get(b"FontDescriptor") {
        if let lopdf::Object::Reference(id) = obj {
            *id
        } else {
            return;
        }
    } else {
        let desc = dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "TestCFFFont",
            "Ascent" => Object::Real(750.0),
            "Descent" => Object::Real(-250.0),
        };
        let id = doc.add_object(Object::Dictionary(desc));
        font_dict.set("FontDescriptor", id);
        id
    };

    // Create /FontFile3 stream with Subtype=Type1C
    let mut stream_dict = lopdf::Dictionary::new();
    stream_dict.set("Subtype", Object::Name(b"Type1C".to_vec()));
    let stream = lopdf::Stream::new(stream_dict, cff_data);
    let ff3_id = doc.add_object(Object::Stream(stream));

    if let Ok(desc_obj) = doc.get_object_mut(desc_id) {
        if let Object::Dictionary(desc) = desc_obj {
            desc.set("FontFile3", ff3_id);
        }
    }
}

#[test]
fn cff_fallback_when_no_widths() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+CustomCFFFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    let cff_data = build_test_cff_data(&[0, 278, 556]);
    add_font_file3(&mut doc, &mut font_dict, cff_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert!((metrics.get_width(0) - 0.0).abs() < 0.01);
    assert!((metrics.get_width(1) - 278.0).abs() < 0.01);
    assert!((metrics.get_width(2) - 556.0).abs() < 0.01);
    // Out of range falls back to missing_width
    assert!((metrics.get_width(3) - 500.0).abs() < 0.01);
}

#[test]
fn cff_fallback_does_not_override_explicit_widths() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = create_font_dict_with_widths(&mut doc, &[400.0, 600.0], 65, 66);
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, None, None);

    let cff_data = build_test_cff_data(&[0, 278, 556, 722]);
    add_font_file3(&mut doc, &mut font_dict, cff_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 400.0);
    assert_eq!(metrics.get_width(66), 600.0);
}

#[test]
fn cff_fallback_standard_font_takes_priority() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 718.0, -207.0, None, None);

    let cff_data = build_test_cff_data(&[999]);
    add_font_file3(&mut doc, &mut font_dict, cff_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 667.0); // Helvetica 'A'
}

#[test]
fn cff_fallback_preserves_descriptor_values() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+MyCFF",
    };
    add_font_descriptor(
        &mut doc,
        &mut font_dict,
        850.0,
        -150.0,
        Some(300.0),
        Some([-100.0, -200.0, 1100.0, 900.0]),
    );
    let cff_data = build_test_cff_data(&[500]);
    add_font_file3(&mut doc, &mut font_dict, cff_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert!((metrics.ascent() - 850.0).abs() < 1.0);
    assert!((metrics.descent() - (-150.0)).abs() < 1.0);
    assert!((metrics.missing_width() - 300.0).abs() < 1.0);
    assert!(metrics.font_bbox().is_some());
}

#[test]
fn cff_fallback_invalid_data_falls_through() {
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "ABCDEF+BrokenCFF",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    add_font_file3(&mut doc, &mut font_dict, vec![0xFF; 100]);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    assert_eq!(metrics.get_width(65), 500.0); // missing_width
}

#[test]
fn cff_fallback_truetype_takes_priority_over_cff() {
    // If both /FontFile2 and /FontFile3 are present, TrueType should be tried first
    let mut doc = Document::with_version("1.5");
    let mut font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "ABCDEF+DualFont",
    };
    add_font_descriptor(&mut doc, &mut font_dict, 800.0, -200.0, Some(500.0), None);

    // Add TrueType data (should win)
    let tt_data = build_test_truetype_data(1000, &[0, 333, 666]);
    add_font_file2(&mut doc, &mut font_dict, tt_data);

    // Add CFF data (should not be used)
    let cff_data = build_test_cff_data(&[0, 999, 999]);
    add_font_file3(&mut doc, &mut font_dict, cff_data);

    let metrics = extract_font_metrics(&doc, &font_dict).unwrap();

    // TrueType widths should be used
    assert!((metrics.get_width(1) - 333.0).abs() < 0.01);
    assert!((metrics.get_width(2) - 666.0).abs() < 0.01);
}
