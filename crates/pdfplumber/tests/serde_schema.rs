//! Frozen producer and consumer fixtures for the curated Serde JSON schema.

#![cfg(feature = "serde")]

use pdfplumber::models::{
    BBox, Cell, Char, Color, ColumnMode, Curve, DedupeOptions, DocumentMetadata, ExplicitLines,
    ExtractOptions, ExtractWarning, ExtractWarningCode, Line, MetadataEntry, MetadataReference,
    MetadataValue, Orientation, RawDocumentMetadata, Rect, SERDE_JSON_SCHEMA, Strategy, Table,
    TableQuality, TableSettings, TextBlock, TextDirection, TextLine, TextOptions, UnicodeNorm,
    Word, WordOptions,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn fixture_manifest() -> Value {
    serde_json::from_str(include_str!("fixtures/serde-schema-v1.json"))
        .expect("the committed Serde schema fixture must be valid JSON")
}

fn assert_fixtures<T>(manifest: &Value, model: &str, values: &[T])
where
    T: Serialize + DeserializeOwned,
{
    let fixtures = manifest["models"][model]
        .as_array()
        .unwrap_or_else(|| panic!("missing fixture array for {model}"));
    assert_eq!(fixtures.len(), values.len(), "fixture count for {model}");

    for (index, (expected, value)) in fixtures.iter().zip(values).enumerate() {
        let actual = serde_json::to_value(value)
            .unwrap_or_else(|error| panic!("serialize {model}[{index}]: {error}"));
        assert_eq!(
            &actual, expected,
            "producer schema drift for {model}[{index}]"
        );

        let restored: T = serde_json::from_value(expected.clone())
            .unwrap_or_else(|error| panic!("read v1 {model}[{index}]: {error}"));
        let restored_json = serde_json::to_value(restored)
            .unwrap_or_else(|error| panic!("reserialize v1 {model}[{index}]: {error}"));
        assert_eq!(
            &restored_json, expected,
            "consumer schema drift for {model}[{index}]"
        );
    }
}

fn bbox() -> BBox {
    BBox::new(1.0, 2.0, 3.0, 4.0)
}

fn dedupe() -> DedupeOptions {
    DedupeOptions {
        tolerance: 1.25,
        extra_attrs: vec!["fontname".to_owned(), "upright".to_owned()],
    }
}

#[test]
fn serde_json_v1_struct_shapes_are_frozen_in_both_directions() {
    let manifest = fixture_manifest();
    assert_eq!(manifest["schema"], SERDE_JSON_SCHEMA);

    assert_fixtures(&manifest, "BBox", &[BBox::new(1.25, 2.5, 31.75, 42.0)]);
    assert_fixtures(
        &manifest,
        "Cell",
        &[
            Cell {
                bbox: bbox(),
                text: Some("cell".to_owned()),
            },
            Cell {
                bbox: bbox(),
                text: None,
            },
        ],
    );
    assert_fixtures(
        &manifest,
        "Char",
        &[
            Char {
                text: "A".to_owned(),
                bbox: bbox(),
                fontname: "FixtureFont".to_owned(),
                size: 12.5,
                advance: 6.25,
                doctop: 102.0,
                upright: true,
                direction: TextDirection::Rtl,
                stroking_color: Some(Color::Gray(0.25)),
                non_stroking_color: Some(Color::Rgb(0.25, 0.5, 0.75)),
                ctm: [1.0, 0.0, 0.0, 1.0, 5.0, 6.0],
                char_code: 65,
                mcid: Some(7),
                tag: Some("Span".to_owned()),
            },
            Char {
                text: "B".to_owned(),
                bbox: bbox(),
                fontname: "FixtureFont".to_owned(),
                size: 10.0,
                advance: 5.0,
                doctop: 2.0,
                upright: false,
                direction: TextDirection::Ltr,
                stroking_color: None,
                non_stroking_color: None,
                ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                char_code: 66,
                mcid: None,
                tag: None,
            },
        ],
    );
    assert_fixtures(
        &manifest,
        "Curve",
        &[Curve {
            x0: 1.0,
            top: 2.0,
            x1: 9.0,
            bottom: 10.0,
            pts: vec![(1.0, 2.0), (3.0, 4.0)],
            line_width: 0.5,
            stroke: true,
            fill: false,
            stroke_color: Color::Gray(0.25),
            fill_color: Color::Pattern("Fill".to_owned()),
        }],
    );
    assert_fixtures(&manifest, "DedupeOptions", &[dedupe()]);
    assert_fixtures(
        &manifest,
        "DocumentMetadata",
        &[
            DocumentMetadata {
                title: Some("Title".to_owned()),
                author: Some("Author".to_owned()),
                subject: Some("Subject".to_owned()),
                keywords: Some("one,two".to_owned()),
                creator: Some("Creator".to_owned()),
                producer: Some("Producer".to_owned()),
                creation_date: Some("D:20260102030405Z".to_owned()),
                mod_date: Some("D:20260708091011Z".to_owned()),
            },
            DocumentMetadata::default(),
        ],
    );
    assert_fixtures(
        &manifest,
        "ExplicitLines",
        &[ExplicitLines {
            horizontal_lines: vec![1.0, 2.0],
            vertical_lines: vec![3.0, 4.0],
        }],
    );
    assert_fixtures(
        &manifest,
        "ExtractOptions",
        &[
            ExtractOptions {
                max_recursion_depth: 11,
                max_objects_per_page: 1_200,
                max_stream_bytes: 1_300,
                collect_warnings: false,
                unicode_norm: UnicodeNorm::Nfkc,
                extract_image_data: true,
                strict_mode: true,
                max_input_bytes: Some(1_400),
                max_pages: Some(15),
                max_total_image_bytes: Some(1_600),
                max_total_objects: Some(1_700),
                dedupe: Some(DedupeOptions {
                    tolerance: 1.25,
                    extra_attrs: vec!["size".to_owned()],
                }),
            },
            ExtractOptions::default(),
        ],
    );
    assert_fixtures(
        &manifest,
        "ExtractWarning",
        &[
            ExtractWarning {
                code: ExtractWarningCode::Other("fixture-code".to_owned()),
                description: "fixture warning".to_owned(),
                page: Some(2),
                element: Some("char 3".to_owned()),
                operator_index: Some(4),
                font_name: Some("FixtureFont".to_owned()),
            },
            ExtractWarning {
                code: ExtractWarningCode::MissingFont,
                description: "missing context".to_owned(),
                page: None,
                element: None,
                operator_index: None,
                font_name: None,
            },
        ],
    );
    assert_fixtures(
        &manifest,
        "Line",
        &[Line {
            x0: 1.0,
            top: 2.0,
            x1: 3.0,
            bottom: 4.0,
            line_width: 0.5,
            stroke_color: Color::Gray(0.25),
            orientation: Orientation::Diagonal,
        }],
    );
    assert_fixtures(
        &manifest,
        "MetadataEntry",
        &[
            MetadataEntry {
                key: "Key".to_owned(),
                value: MetadataValue::Integer(-7),
                resolution_error: Some("fixture error".to_owned()),
            },
            MetadataEntry {
                key: "Other".to_owned(),
                value: MetadataValue::Null,
                resolution_error: None,
            },
        ],
    );
    assert_fixtures(
        &manifest,
        "MetadataReference",
        &[MetadataReference {
            object_number: 7,
            generation_number: 2,
        }],
    );
    assert_fixtures(
        &manifest,
        "RawDocumentMetadata",
        &[RawDocumentMetadata {
            entries: vec![MetadataEntry {
                key: "Key".to_owned(),
                value: MetadataValue::Integer(-7),
                resolution_error: None,
            }],
        }],
    );
    assert_fixtures(
        &manifest,
        "Rect",
        &[Rect {
            x0: 1.0,
            top: 2.0,
            x1: 3.0,
            bottom: 4.0,
            line_width: 0.5,
            stroke: true,
            fill: false,
            stroke_color: Color::Gray(0.25),
            fill_color: Color::Rgb(0.25, 0.5, 0.75),
        }],
    );
    assert_fixtures(
        &manifest,
        "Table",
        &[Table {
            bbox: bbox(),
            cells: vec![],
            rows: vec![],
            columns: vec![],
        }],
    );
    assert_fixtures(
        &manifest,
        "TableQuality",
        &[TableQuality {
            accuracy: 0.75,
            whitespace: 0.25,
        }],
    );
    assert_fixtures(
        &manifest,
        "TableSettings",
        &[
            TableSettings {
                strategy: Strategy::Stream,
                vertical_strategy: Some(Strategy::LatticeStrict),
                horizontal_strategy: Some(Strategy::Explicit),
                snap_tolerance: 1.0,
                snap_x_tolerance: 1.25,
                snap_y_tolerance: 1.5,
                join_tolerance: 2.0,
                join_x_tolerance: 2.25,
                join_y_tolerance: 2.5,
                edge_min_length: 3.0,
                edge_min_length_prefilter: 0.75,
                min_words_vertical: 4,
                min_words_horizontal: 2,
                text_tolerance: 3.25,
                text_x_tolerance: 3.5,
                text_y_tolerance: 3.75,
                intersection_tolerance: 4.0,
                intersection_x_tolerance: 4.25,
                intersection_y_tolerance: 4.5,
                explicit_lines: Some(ExplicitLines {
                    horizontal_lines: vec![1.0],
                    vertical_lines: vec![2.0],
                }),
                min_accuracy: Some(0.875),
                duplicate_merged_content: true,
            },
            TableSettings::default(),
        ],
    );
    assert_fixtures(
        &manifest,
        "TextBlock",
        &[TextBlock {
            lines: vec![],
            bbox: bbox(),
        }],
    );
    assert_fixtures(
        &manifest,
        "TextLine",
        &[TextLine {
            words: vec![],
            bbox: bbox(),
        }],
    );
    assert_fixtures(
        &manifest,
        "TextOptions",
        &[TextOptions {
            layout: true,
            y_tolerance: 1.25,
            y_density: 2.5,
            x_density: 3.75,
            expand_ligatures: false,
            column_mode: ColumnMode::Explicit(vec![10.0, 20.0]),
            min_column_gap: 21.0,
            max_columns: 7,
        }],
    );
    assert_fixtures(
        &manifest,
        "Word",
        &[Word {
            text: "word".to_owned(),
            bbox: bbox(),
            doctop: 102.0,
            direction: TextDirection::Ttb,
            chars: vec![],
        }],
    );
    assert_fixtures(
        &manifest,
        "WordOptions",
        &[
            WordOptions {
                x_tolerance: 1.25,
                y_tolerance: 2.5,
                keep_blank_chars: true,
                use_text_flow: true,
                text_direction: TextDirection::Btt,
                expand_ligatures: false,
                x_tolerance_ratio: Some(0.5),
                y_tolerance_ratio: Some(0.75),
                split_at_punctuation: Some(",;".to_owned()),
            },
            WordOptions::default(),
        ],
    );
}

#[test]
fn serde_json_v1_enum_variants_are_frozen_in_both_directions() {
    let manifest = fixture_manifest();

    assert_fixtures(
        &manifest,
        "Color",
        &[
            Color::Gray(0.25),
            Color::Rgb(0.25, 0.5, 0.75),
            Color::Cmyk(0.125, 0.25, 0.5, 0.75),
            Color::Pattern("P1".to_owned()),
            Color::PatternWithBase(Box::new(Color::Gray(0.5)), "P2".to_owned()),
            Color::Other(vec![0.125, 0.625]),
        ],
    );
    assert_fixtures(
        &manifest,
        "ColumnMode",
        &[
            ColumnMode::None,
            ColumnMode::Auto,
            ColumnMode::Explicit(vec![10.0, 20.0]),
        ],
    );
    assert_fixtures(
        &manifest,
        "ExtractWarningCode",
        &[
            ExtractWarningCode::MissingFont,
            ExtractWarningCode::UnsupportedOperator,
            ExtractWarningCode::MalformedObject,
            ExtractWarningCode::ResourceLimitReached,
            ExtractWarningCode::EncodingFallback,
            ExtractWarningCode::Other("custom".to_owned()),
        ],
    );
    assert_fixtures(
        &manifest,
        "MetadataValue",
        &[
            MetadataValue::Null,
            MetadataValue::Boolean(true),
            MetadataValue::Integer(-7),
            MetadataValue::Real(1.25),
            MetadataValue::String("value".to_owned()),
            MetadataValue::Array(vec![MetadataValue::Null, MetadataValue::Integer(2)]),
            MetadataValue::Dictionary(vec![(
                "Key".to_owned(),
                MetadataValue::String("value".to_owned()),
            )]),
            MetadataValue::Reference(MetadataReference {
                object_number: 7,
                generation_number: 2,
            }),
            MetadataValue::Stream {
                dictionary: vec![("Length".to_owned(), MetadataValue::Integer(3))],
                data: vec![1, 2, 3],
            },
        ],
    );
    assert_fixtures(
        &manifest,
        "Orientation",
        &[
            Orientation::Horizontal,
            Orientation::Vertical,
            Orientation::Diagonal,
        ],
    );
    assert_fixtures(
        &manifest,
        "Strategy",
        &[
            Strategy::Lattice,
            Strategy::LatticeStrict,
            Strategy::Stream,
            Strategy::Explicit,
        ],
    );
    assert_fixtures(
        &manifest,
        "TextDirection",
        &[
            TextDirection::Ltr,
            TextDirection::Rtl,
            TextDirection::Ttb,
            TextDirection::Btt,
        ],
    );
    assert_fixtures(
        &manifest,
        "UnicodeNorm",
        &[
            UnicodeNorm::None,
            UnicodeNorm::Nfc,
            UnicodeNorm::Nfd,
            UnicodeNorm::Nfkc,
            UnicodeNorm::Nfkd,
        ],
    );
}
