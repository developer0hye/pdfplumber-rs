//! Contracts for the curated public data models and their semantics.

use std::any::TypeId;

use pdfplumber::models::{
    BBox, Cell, Char, Color, ColumnMode, Curve, DedupeOptions, DocumentMetadata, ExplicitLines,
    ExtractOptions, ExtractWarning, ExtractWarningCode, Line, MetadataEntry, MetadataReference,
    MetadataValue, Orientation, RawDocumentMetadata, Rect, Strategy, Table, TableQuality,
    TableSettings, TextBlock, TextDirection, TextLine, TextOptions, UnicodeNorm, Word, WordOptions,
};
use pdfplumber::{cells_to_tables, extract_text_for_cells};

fn assert_public<T: 'static>() {
    let _ = TypeId::of::<T>();
}

fn cell(text: &str, x0: f64, top: f64, x1: f64, bottom: f64) -> Cell {
    Cell {
        bbox: BBox::new(x0, top, x1, bottom),
        text: Some(text.to_owned()),
    }
}

#[test]
fn curated_model_families_are_importable_from_one_module() {
    assert_public::<Char>();
    assert_public::<Word>();
    assert_public::<TextLine>();
    assert_public::<TextBlock>();
    assert_public::<TextDirection>();
    assert_public::<BBox>();
    assert_public::<Line>();
    assert_public::<Rect>();
    assert_public::<Curve>();
    assert_public::<Color>();
    assert_public::<Orientation>();
    assert_public::<Cell>();
    assert_public::<Table>();
    assert_public::<TableQuality>();
    assert_public::<DocumentMetadata>();
    assert_public::<RawDocumentMetadata>();
    assert_public::<MetadataEntry>();
    assert_public::<MetadataValue>();
    assert_public::<MetadataReference>();
    assert_public::<ExtractWarning>();
    assert_public::<ExtractWarningCode>();
    assert_public::<ExtractOptions>();
    assert_public::<DedupeOptions>();
    assert_public::<UnicodeNorm>();
    assert_public::<WordOptions>();
    assert_public::<TextOptions>();
    assert_public::<ColumnMode>();
    assert_public::<TableSettings>();
    assert_public::<Strategy>();
    assert_public::<ExplicitLines>();
}

#[test]
fn table_cells_have_stable_row_major_order_independent_of_input_order() {
    let cells = vec![
        cell("bottom-right", 10.0, 10.0, 20.0, 20.0),
        cell("top-left", 0.0, 0.0, 10.0, 10.0),
        cell("bottom-left", 0.0, 10.0, 10.0, 20.0),
        cell("top-right", 10.0, 0.0, 20.0, 10.0),
    ];

    let tables = cells_to_tables(cells);
    let texts: Vec<_> = tables[0]
        .cells
        .iter()
        .map(|entry| entry.text.as_deref().unwrap())
        .collect();

    assert_eq!(
        texts,
        ["top-left", "top-right", "bottom-left", "bottom-right"]
    );
}

#[test]
fn optional_model_fields_distinguish_absence_and_empty_values() {
    let metadata = DocumentMetadata::default();
    assert!(metadata.is_empty());
    assert!(metadata.title.is_none());

    let raw = RawDocumentMetadata {
        entries: vec![MetadataEntry {
            key: "Broken".to_owned(),
            value: MetadataValue::Reference(MetadataReference {
                object_number: 7,
                generation_number: 0,
            }),
            resolution_error: Some("cycle".to_owned()),
        }],
    };
    assert_eq!(raw.entries[0].key, "Broken");
    assert_eq!(raw.entries[0].resolution_error.as_deref(), Some("cycle"));

    let warning =
        ExtractWarning::with_code(ExtractWarningCode::MissingFont, "font resource is absent");
    assert!(warning.page.is_none());
    assert!(warning.element.is_none());
    assert!(warning.operator_index.is_none());
    assert!(warning.font_name.is_none());

    let mut cells = [
        Cell {
            bbox: BBox::new(0.0, 0.0, 10.0, 10.0),
            text: None,
        },
        Cell {
            bbox: BBox::new(0.0, 0.0, 0.0, 0.0),
            text: None,
        },
    ];
    extract_text_for_cells(&mut cells, &[]);
    assert_eq!(cells[0].text.as_deref(), Some(""));
    assert_eq!(cells[1].text, None);

    let extract = ExtractOptions::default();
    assert!(extract.max_input_bytes.is_none());
    assert!(extract.max_pages.is_none());
    assert!(extract.max_total_image_bytes.is_none());
    assert!(extract.max_total_objects.is_none());
    assert!(extract.dedupe.is_none());

    let words = WordOptions::default();
    assert!(words.x_tolerance_ratio.is_none());
    assert!(words.y_tolerance_ratio.is_none());
    assert!(words.split_at_punctuation.is_none());

    let tables = TableSettings::default();
    assert!(tables.vertical_strategy.is_none());
    assert!(tables.horizontal_strategy.is_none());
    assert!(tables.explicit_lines.is_none());
    assert!(tables.min_accuracy.is_none());

    let text = TextOptions::default();
    assert!(matches!(text.column_mode, ColumnMode::None));
}
