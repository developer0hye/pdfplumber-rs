//! Font metrics extraction from PDF font dictionaries.
//!
//! Parses /Widths, /FirstChar, /LastChar, and /FontDescriptor to provide
//! glyph widths, ascent, and descent for character bounding box calculation.

use crate::error::BackendError;

/// Default ascent when not specified (750/1000 of text space).
const DEFAULT_ASCENT: f64 = 750.0;

/// Default descent when not specified (-250/1000 of text space).
const DEFAULT_DESCENT: f64 = -250.0;

/// Default character width when not specified (600/1000 of text space).
const DEFAULT_WIDTH: f64 = 600.0;

/// Font metrics extracted from a PDF font dictionary.
///
/// Stores glyph widths and font descriptor information (ascent, descent,
/// bounding box) needed to calculate character bounding boxes.
///
/// Width values are in glyph space units (1/1000 of text space).
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Glyph widths indexed by (char_code - first_char).
    widths: Vec<f64>,
    /// First character code in the widths array.
    first_char: u32,
    /// Last character code in the widths array.
    last_char: u32,
    /// Default width for characters outside [first_char, last_char].
    missing_width: f64,
    /// Font ascent in glyph space units (positive, above baseline).
    ascent: f64,
    /// Font descent in glyph space units (negative, below baseline).
    descent: f64,
    /// Font bounding box [llx, lly, urx, ury] in glyph space units.
    font_bbox: Option<[f64; 4]>,
}

impl FontMetrics {
    /// Create FontMetrics from parsed PDF font dictionary values.
    pub fn new(
        widths: Vec<f64>,
        first_char: u32,
        last_char: u32,
        missing_width: f64,
        ascent: f64,
        descent: f64,
        font_bbox: Option<[f64; 4]>,
    ) -> Self {
        Self {
            widths,
            first_char,
            last_char,
            missing_width,
            ascent,
            descent,
            font_bbox,
        }
    }

    /// Create default FontMetrics for when font info is unavailable.
    pub fn default_metrics() -> Self {
        Self {
            widths: Vec::new(),
            first_char: 0,
            last_char: 0,
            missing_width: DEFAULT_WIDTH,
            ascent: DEFAULT_ASCENT,
            descent: DEFAULT_DESCENT,
            font_bbox: None,
        }
    }

    /// Get the width for a character code in glyph space (1/1000 of text space).
    pub fn get_width(&self, char_code: u32) -> f64 {
        if char_code >= self.first_char && char_code <= self.last_char {
            let index = (char_code - self.first_char) as usize;
            if index < self.widths.len() {
                return self.widths[index];
            }
        }
        self.missing_width
    }

    /// Font ascent in glyph space units (positive, above baseline).
    pub fn ascent(&self) -> f64 {
        self.ascent
    }

    /// Font descent in glyph space units (negative, below baseline).
    pub fn descent(&self) -> f64 {
        self.descent
    }

    /// Font bounding box [llx, lly, urx, ury] in glyph space units.
    pub fn font_bbox(&self) -> Option<[f64; 4]> {
        self.font_bbox
    }

    /// Missing width used for characters outside the widths range.
    pub fn missing_width(&self) -> f64 {
        self.missing_width
    }

    /// First character code in the widths array.
    pub fn first_char(&self) -> u32 {
        self.first_char
    }

    /// Last character code in the widths array.
    pub fn last_char(&self) -> u32 {
        self.last_char
    }
}

/// Extract [`FontMetrics`] from a lopdf font dictionary.
///
/// Reads /Widths, /FirstChar, /LastChar from the font dictionary,
/// and /Ascent, /Descent, /FontBBox, /MissingWidth from the /FontDescriptor.
///
/// Returns default metrics if essential fields are missing.
pub fn extract_font_metrics(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Result<FontMetrics, BackendError> {
    // Parse /FirstChar and /LastChar
    let first_char = font_dict
        .get(b"FirstChar")
        .ok()
        .and_then(object_to_f64_opt)
        .map(|v| v as u32)
        .unwrap_or(0);

    let last_char = font_dict
        .get(b"LastChar")
        .ok()
        .and_then(object_to_f64_opt)
        .map(|v| v as u32)
        .unwrap_or(0);

    // Parse /Widths array
    let widths = match font_dict.get(b"Widths") {
        Ok(obj) => {
            let obj = resolve_object(doc, obj);
            match obj.as_array() {
                Ok(arr) => arr
                    .iter()
                    .map(|o| {
                        let o = resolve_object(doc, o);
                        object_to_f64_opt(o).unwrap_or(0.0)
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        Err(_) => Vec::new(),
    };

    // Parse /FontDescriptor
    let desc_info = parse_font_descriptor(doc, font_dict)?;

    Ok(FontMetrics::new(
        widths,
        first_char,
        last_char,
        desc_info.missing_width,
        desc_info.ascent,
        desc_info.descent,
        desc_info.font_bbox,
    ))
}

/// Parsed font descriptor values.
struct FontDescriptorInfo {
    ascent: f64,
    descent: f64,
    font_bbox: Option<[f64; 4]>,
    missing_width: f64,
}

/// Parse /FontDescriptor dictionary for ascent, descent, bbox, and missing width.
fn parse_font_descriptor(
    doc: &lopdf::Document,
    font_dict: &lopdf::Dictionary,
) -> Result<FontDescriptorInfo, BackendError> {
    let descriptor_dict = font_dict
        .get(b"FontDescriptor")
        .ok()
        .map(|obj| resolve_object(doc, obj))
        .and_then(|obj| obj.as_dict().ok());

    let Some(desc) = descriptor_dict else {
        return Ok(FontDescriptorInfo {
            ascent: DEFAULT_ASCENT,
            descent: DEFAULT_DESCENT,
            font_bbox: None,
            missing_width: DEFAULT_WIDTH,
        });
    };

    let ascent = desc
        .get(b"Ascent")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_ASCENT);

    let descent = desc
        .get(b"Descent")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_DESCENT);

    let missing_width = desc
        .get(b"MissingWidth")
        .ok()
        .and_then(object_to_f64_opt)
        .unwrap_or(DEFAULT_WIDTH);

    let font_bbox = desc
        .get(b"FontBBox")
        .ok()
        .and_then(|o| {
            let o = resolve_object(doc, o);
            o.as_array().ok()
        })
        .and_then(|arr| {
            if arr.len() == 4 {
                let vals: Vec<f64> = arr.iter().filter_map(object_to_f64_opt).collect();
                if vals.len() == 4 {
                    Some([vals[0], vals[1], vals[2], vals[3]])
                } else {
                    None
                }
            } else {
                None
            }
        });

    Ok(FontDescriptorInfo {
        ascent,
        descent,
        font_bbox,
        missing_width,
    })
}

/// Resolve an indirect reference to the actual object.
fn resolve_object<'a>(doc: &'a lopdf::Document, obj: &'a lopdf::Object) -> &'a lopdf::Object {
    match obj {
        lopdf::Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Convert a lopdf object to f64, returning None if not a number.
fn object_to_f64_opt(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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

        // No widths — all codes return missing width
        assert_eq!(metrics.get_width(65), 500.0);
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
}
