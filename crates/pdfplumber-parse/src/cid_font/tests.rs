use super::*;
use lopdf::{Document, Object, Stream, dictionary};

// ========== CidToGidMap tests ==========

#[test]
fn identity_map_returns_same_cid() {
    let map = CidToGidMap::Identity;
    assert_eq!(map.map(0), 0);
    assert_eq!(map.map(100), 100);
    assert_eq!(map.map(65535), 65535);
}

#[test]
fn explicit_map_looks_up_table() {
    let table = vec![10, 20, 30, 40, 50];
    let map = CidToGidMap::Explicit(table);
    assert_eq!(map.map(0), 10);
    assert_eq!(map.map(1), 20);
    assert_eq!(map.map(4), 50);
}

#[test]
fn explicit_map_out_of_range_returns_cid() {
    let table = vec![10, 20, 30];
    let map = CidToGidMap::Explicit(table);
    assert_eq!(map.map(5), 5); // out of range → fallback to CID
}

#[test]
fn from_stream_parses_big_endian_u16() {
    // CID 0 → GID 5, CID 1 → GID 10
    let data = vec![0x00, 0x05, 0x00, 0x0A];
    let map = CidToGidMap::from_stream(&data);
    assert_eq!(map.map(0), 5);
    assert_eq!(map.map(1), 10);
}

#[test]
fn from_stream_handles_odd_length() {
    // Only one complete pair, last byte ignored
    let data = vec![0x00, 0x05, 0x00];
    let map = CidToGidMap::from_stream(&data);
    assert_eq!(map.map(0), 5);
    assert_eq!(map.map(1), 1); // out of range
}

#[test]
fn from_stream_empty() {
    let map = CidToGidMap::from_stream(&[]);
    assert_eq!(map.map(0), 0); // out of range, falls back to CID
}

// ========== CidSystemInfo tests ==========

#[test]
fn cid_system_info_adobe_japan1() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "Japan1".to_string(),
        supplement: 6,
    };
    assert!(info.is_adobe_cjk());
}

#[test]
fn cid_system_info_adobe_gb1() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "GB1".to_string(),
        supplement: 5,
    };
    assert!(info.is_adobe_cjk());
}

#[test]
fn cid_system_info_adobe_cns1() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "CNS1".to_string(),
        supplement: 7,
    };
    assert!(info.is_adobe_cjk());
}

#[test]
fn cid_system_info_adobe_korea1() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "Korea1".to_string(),
        supplement: 2,
    };
    assert!(info.is_adobe_cjk());
}

#[test]
fn cid_system_info_non_adobe_not_cjk() {
    let info = CidSystemInfo {
        registry: "Custom".to_string(),
        ordering: "Japan1".to_string(),
        supplement: 0,
    };
    assert!(!info.is_adobe_cjk());
}

#[test]
fn cid_system_info_adobe_non_cjk_ordering() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "Identity".to_string(),
        supplement: 0,
    };
    assert!(!info.is_adobe_cjk());
}

// ========== CidFontMetrics tests ==========

#[test]
fn cid_font_metrics_get_width_from_map() {
    let mut widths = HashMap::new();
    widths.insert(1, 500.0);
    widths.insert(2, 600.0);
    widths.insert(100, 250.0);

    let metrics = CidFontMetrics::new(
        widths,
        1000.0,
        880.0,
        -120.0,
        None,
        CidFontType::Type2,
        CidToGidMap::Identity,
        None,
    );

    assert_eq!(metrics.get_width(1), 500.0);
    assert_eq!(metrics.get_width(2), 600.0);
    assert_eq!(metrics.get_width(100), 250.0);
}

#[test]
fn cid_font_metrics_get_width_returns_default() {
    let metrics = CidFontMetrics::new(
        HashMap::new(),
        1000.0,
        880.0,
        -120.0,
        None,
        CidFontType::Type2,
        CidToGidMap::Identity,
        None,
    );

    assert_eq!(metrics.get_width(0), 1000.0);
    assert_eq!(metrics.get_width(999), 1000.0);
}

#[test]
fn cid_font_metrics_custom_default_width() {
    let metrics = CidFontMetrics::new(
        HashMap::new(),
        500.0,
        880.0,
        -120.0,
        None,
        CidFontType::Type0,
        CidToGidMap::Identity,
        None,
    );

    assert_eq!(metrics.get_width(0), 500.0);
    assert_eq!(metrics.default_width(), 500.0);
}

#[test]
fn cid_font_metrics_accessors() {
    let info = CidSystemInfo {
        registry: "Adobe".to_string(),
        ordering: "Japan1".to_string(),
        supplement: 6,
    };
    let metrics = CidFontMetrics::new(
        HashMap::new(),
        1000.0,
        880.0,
        -120.0,
        Some([-100.0, -200.0, 1100.0, 900.0]),
        CidFontType::Type0,
        CidToGidMap::Identity,
        Some(info),
    );

    assert_eq!(metrics.ascent(), 880.0);
    assert_eq!(metrics.descent(), -120.0);
    assert_eq!(metrics.font_bbox(), Some([-100.0, -200.0, 1100.0, 900.0]));
    assert_eq!(metrics.font_type(), CidFontType::Type0);
    assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
    assert!(metrics.system_info().unwrap().is_adobe_cjk());
}

#[test]
fn cid_font_metrics_map_cid_to_gid() {
    let table = vec![10, 20, 30];
    let metrics = CidFontMetrics::new(
        HashMap::new(),
        1000.0,
        880.0,
        -120.0,
        None,
        CidFontType::Type2,
        CidToGidMap::Explicit(table),
        None,
    );

    assert_eq!(metrics.map_cid_to_gid(0), 10);
    assert_eq!(metrics.map_cid_to_gid(1), 20);
    assert_eq!(metrics.map_cid_to_gid(2), 30);
    assert_eq!(metrics.map_cid_to_gid(5), 5); // fallback
}

#[test]
fn cid_font_metrics_default() {
    let metrics = CidFontMetrics::default_metrics();
    assert_eq!(metrics.default_width(), DEFAULT_CID_WIDTH);
    assert_eq!(metrics.ascent(), DEFAULT_CID_ASCENT);
    assert_eq!(metrics.descent(), DEFAULT_CID_DESCENT);
    assert_eq!(metrics.font_bbox(), None);
    assert_eq!(metrics.font_type(), CidFontType::Type2);
    assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
    assert!(metrics.system_info().is_none());
}

// ========== parse_w_array tests ==========

#[test]
fn parse_w_array_individual_widths() {
    // [1 [500 600 700]] → CID 1=500, CID 2=600, CID 3=700
    let doc = Document::with_version("1.5");
    let objects = vec![
        Object::Integer(1),
        Object::Array(vec![
            Object::Integer(500),
            Object::Integer(600),
            Object::Integer(700),
        ]),
    ];

    let widths = parse_w_array(&objects, &doc);
    assert_eq!(widths.get(&1), Some(&500.0));
    assert_eq!(widths.get(&2), Some(&600.0));
    assert_eq!(widths.get(&3), Some(&700.0));
    assert_eq!(widths.get(&0), None);
    assert_eq!(widths.get(&4), None);
}

#[test]
fn parse_w_array_range_format() {
    // [10 20 500] → CIDs 10-20 all have width 500
    let doc = Document::with_version("1.5");
    let objects = vec![
        Object::Integer(10),
        Object::Integer(20),
        Object::Integer(500),
    ];

    let widths = parse_w_array(&objects, &doc);
    for cid in 10..=20 {
        assert_eq!(widths.get(&cid), Some(&500.0), "CID {} should be 500", cid);
    }
    assert_eq!(widths.get(&9), None);
    assert_eq!(widths.get(&21), None);
}

#[test]
fn parse_w_array_mixed_formats() {
    // [1 [250 300] 10 20 500]
    let doc = Document::with_version("1.5");
    let objects = vec![
        Object::Integer(1),
        Object::Array(vec![Object::Integer(250), Object::Integer(300)]),
        Object::Integer(10),
        Object::Integer(20),
        Object::Integer(500),
    ];

    let widths = parse_w_array(&objects, &doc);
    assert_eq!(widths.get(&1), Some(&250.0));
    assert_eq!(widths.get(&2), Some(&300.0));
    for cid in 10..=20 {
        assert_eq!(widths.get(&cid), Some(&500.0));
    }
}

#[test]
fn parse_w_array_empty() {
    let doc = Document::with_version("1.5");
    let widths = parse_w_array(&[], &doc);
    assert!(widths.is_empty());
}

#[test]
fn parse_w_array_real_values() {
    let doc = Document::with_version("1.5");
    let objects = vec![
        Object::Integer(1),
        Object::Array(vec![Object::Real(500.5), Object::Real(600.5)]),
    ];

    let widths = parse_w_array(&objects, &doc);
    assert!((widths[&1] - 500.5).abs() < 0.1);
    assert!((widths[&2] - 600.5).abs() < 0.1);
}

#[test]
fn parse_w_array_single_cid_range() {
    // [5 5 700] → CID 5 = 700
    let doc = Document::with_version("1.5");
    let objects = vec![Object::Integer(5), Object::Integer(5), Object::Integer(700)];

    let widths = parse_w_array(&objects, &doc);
    assert_eq!(widths.get(&5), Some(&700.0));
    assert_eq!(widths.len(), 1);
}

// ========== extract_cid_font_metrics tests ==========

#[test]
fn extract_cid_font_metrics_basic() {
    let mut doc = Document::with_version("1.5");

    // Create a CIDFont dictionary
    let w_array = Object::Array(vec![
        Object::Integer(1),
        Object::Array(vec![Object::Integer(500), Object::Integer(600)]),
    ]);
    let w_id = doc.add_object(w_array);

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
        "DW" => Object::Integer(1000),
        "W" => w_id,
        "CIDToGIDMap" => "Identity",
    };

    let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
    assert_eq!(metrics.font_type(), CidFontType::Type2);
    assert_eq!(metrics.default_width(), 1000.0);
    assert_eq!(metrics.get_width(1), 500.0);
    assert_eq!(metrics.get_width(2), 600.0);
    assert_eq!(metrics.get_width(3), 1000.0); // default
    assert_eq!(metrics.cid_to_gid(), &CidToGidMap::Identity);
}

#[test]
fn extract_cid_font_metrics_type0() {
    let doc = Document::with_version("1.5");

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType0",
        "BaseFont" => "KozMinPro-Regular",
    };

    let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
    assert_eq!(metrics.font_type(), CidFontType::Type0);
    assert_eq!(metrics.default_width(), DEFAULT_CID_WIDTH);
}

#[test]
fn extract_cid_font_metrics_with_descriptor() {
    let mut doc = Document::with_version("1.5");

    let desc_id = doc.add_object(Object::Dictionary(dictionary! {
        "Type" => "FontDescriptor",
        "FontName" => "MSGothic",
        "Ascent" => Object::Integer(859),
        "Descent" => Object::Integer(-140),
        "FontBBox" => Object::Array(vec![
            Object::Integer(0),
            Object::Integer(-137),
            Object::Integer(1000),
            Object::Integer(859),
        ]),
    }));

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
        "FontDescriptor" => desc_id,
    };

    let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
    assert_eq!(metrics.ascent(), 859.0);
    assert_eq!(metrics.descent(), -140.0);
    assert!(metrics.font_bbox().is_some());
}

#[test]
fn extract_cid_font_metrics_with_system_info() {
    let doc = Document::with_version("1.5");

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
        "CIDSystemInfo" => Object::Dictionary(dictionary! {
            "Registry" => Object::String("Adobe".as_bytes().to_vec(), lopdf::StringFormat::Literal),
            "Ordering" => Object::String("Japan1".as_bytes().to_vec(), lopdf::StringFormat::Literal),
            "Supplement" => Object::Integer(6),
        }),
    };

    let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
    let info = metrics.system_info().unwrap();
    assert_eq!(info.registry, "Adobe");
    assert_eq!(info.ordering, "Japan1");
    assert_eq!(info.supplement, 6);
    assert!(info.is_adobe_cjk());
}

#[test]
fn extract_cid_font_metrics_explicit_gid_map() {
    let mut doc = Document::with_version("1.5");

    // CIDToGIDMap stream: CID 0→GID 5, CID 1→GID 10
    let gid_data = vec![0x00, 0x05, 0x00, 0x0A];
    let gid_stream = Stream::new(dictionary! {}, gid_data);
    let gid_stream_id = doc.add_object(Object::Stream(gid_stream));

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "CustomFont",
        "CIDToGIDMap" => gid_stream_id,
    };

    let metrics = extract_cid_font_metrics(&doc, &cid_font_dict).unwrap();
    assert_eq!(metrics.map_cid_to_gid(0), 5);
    assert_eq!(metrics.map_cid_to_gid(1), 10);
}

// ========== Predefined CMap name parsing tests ==========

#[test]
fn parse_identity_h() {
    let info = parse_predefined_cmap_name("Identity-H").unwrap();
    assert_eq!(info.name, "Identity-H");
    assert_eq!(info.writing_mode, 0);
    assert!(info.is_identity);
}

#[test]
fn parse_identity_v() {
    let info = parse_predefined_cmap_name("Identity-V").unwrap();
    assert_eq!(info.name, "Identity-V");
    assert_eq!(info.writing_mode, 1);
    assert!(info.is_identity);
}

#[test]
fn parse_adobe_japan1() {
    let info = parse_predefined_cmap_name("Adobe-Japan1-6").unwrap();
    assert_eq!(info.registry, "Adobe");
    assert_eq!(info.ordering, "Japan1");
    assert!(!info.is_identity);
}

#[test]
fn parse_adobe_gb1() {
    let info = parse_predefined_cmap_name("Adobe-GB1-5").unwrap();
    assert_eq!(info.ordering, "GB1");
}

#[test]
fn parse_adobe_cns1() {
    let info = parse_predefined_cmap_name("Adobe-CNS1-7").unwrap();
    assert_eq!(info.ordering, "CNS1");
}

#[test]
fn parse_adobe_korea1() {
    let info = parse_predefined_cmap_name("Adobe-Korea1-2").unwrap();
    assert_eq!(info.ordering, "Korea1");
}

#[test]
fn parse_unijis_utf16_h() {
    let info = parse_predefined_cmap_name("UniJIS-UTF16-H").unwrap();
    assert_eq!(info.ordering, "Japan1");
    assert_eq!(info.writing_mode, 0);
}

#[test]
fn parse_unijis_utf16_v() {
    let info = parse_predefined_cmap_name("UniJIS-UTF16-V").unwrap();
    assert_eq!(info.ordering, "Japan1");
    assert_eq!(info.writing_mode, 1);
}

#[test]
fn parse_unigb_utf16_h() {
    let info = parse_predefined_cmap_name("UniGB-UTF16-H").unwrap();
    assert_eq!(info.ordering, "GB1");
}

#[test]
fn parse_uniksc_utf16_h() {
    let info = parse_predefined_cmap_name("UniKS-UTF16-H").unwrap();
    assert_eq!(info.ordering, "Korea1");
}

#[test]
fn parse_90ms_rksj_h() {
    let info = parse_predefined_cmap_name("90ms-RKSJ-H").unwrap();
    assert_eq!(info.ordering, "Japan1");
    assert_eq!(info.writing_mode, 0);
}

#[test]
fn parse_unknown_cmap_returns_none() {
    assert!(parse_predefined_cmap_name("UnknownCMap").is_none());
}

#[test]
fn parse_empty_cmap_returns_none() {
    assert!(parse_predefined_cmap_name("").is_none());
}

// ========== Type0 font detection tests ==========

#[test]
fn detect_type0_font() {
    let dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "SomeFont",
    };
    assert!(is_type0_font(&dict));
}

#[test]
fn detect_non_type0_font() {
    let dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    };
    assert!(!is_type0_font(&dict));
}

#[test]
fn detect_truetype_font() {
    let dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "Arial",
    };
    assert!(!is_type0_font(&dict));
}

// ========== get_descendant_font tests ==========

#[test]
fn get_descendant_font_basic() {
    let mut doc = Document::with_version("1.5");

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
    };
    let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

    let type0_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MSGothic",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
    };

    let desc = get_descendant_font(&doc, &type0_dict);
    assert!(desc.is_some());
    let desc = desc.unwrap();
    assert_eq!(
        desc.get(b"Subtype").unwrap().as_name().unwrap(),
        b"CIDFontType2"
    );
}

#[test]
fn get_descendant_font_missing() {
    let doc = Document::with_version("1.5");
    let type0_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MSGothic",
    };

    assert!(get_descendant_font(&doc, &type0_dict).is_none());
}

// ========== get_type0_encoding tests ==========

#[test]
fn get_encoding_identity_h() {
    let dict = dictionary! {
        "Subtype" => "Type0",
        "Encoding" => "Identity-H",
    };
    assert_eq!(get_type0_encoding(&dict), Some("Identity-H".to_string()));
}

#[test]
fn get_encoding_missing() {
    let dict = dictionary! {
        "Subtype" => "Type0",
    };
    assert_eq!(get_type0_encoding(&dict), None);
}

// ========== Subset font detection tests ==========

#[test]
fn is_subset_font_valid() {
    assert!(is_subset_font("ABCDEF+ArialMT"));
    assert!(is_subset_font("XYZABC+TimesNewRoman"));
    assert!(is_subset_font("AAAAAA+A")); // minimal real name
}

#[test]
fn is_subset_font_invalid() {
    assert!(!is_subset_font("ArialMT")); // no prefix
    assert!(!is_subset_font("abcdef+ArialMT")); // lowercase
    assert!(!is_subset_font("ABCDE+ArialMT")); // only 5 uppercase
    assert!(!is_subset_font("ABCDEF-ArialMT")); // dash not plus
    assert!(!is_subset_font("ABC1EF+ArialMT")); // digit in prefix
    assert!(!is_subset_font("")); // empty
    assert!(!is_subset_font("ABCDEF+")); // nothing after +
}

#[test]
fn strip_subset_prefix_with_prefix() {
    assert_eq!(strip_subset_prefix("ABCDEF+ArialMT"), "ArialMT");
    assert_eq!(strip_subset_prefix("XYZABC+TimesNewRoman"), "TimesNewRoman");
}

#[test]
fn strip_subset_prefix_without_prefix() {
    assert_eq!(strip_subset_prefix("ArialMT"), "ArialMT");
    assert_eq!(strip_subset_prefix("Helvetica"), "Helvetica");
    assert_eq!(strip_subset_prefix(""), "");
}

// ========== Identity-H/V encoding behavior tests ==========

#[test]
fn identity_h_encoding_detected() {
    let dict = dictionary! {
        "Subtype" => "Type0",
        "Encoding" => "Identity-H",
    };
    let enc = get_type0_encoding(&dict).unwrap();
    let info = parse_predefined_cmap_name(&enc).unwrap();
    assert!(info.is_identity);
    assert_eq!(info.writing_mode, 0); // horizontal
}

#[test]
fn identity_v_encoding_detected() {
    let dict = dictionary! {
        "Subtype" => "Type0",
        "Encoding" => "Identity-V",
    };
    let enc = get_type0_encoding(&dict).unwrap();
    let info = parse_predefined_cmap_name(&enc).unwrap();
    assert!(info.is_identity);
    assert_eq!(info.writing_mode, 1); // vertical
}
