//! Content stream interpreter integration tests.

use super::*;
use crate::handler::{CharEvent, ContentHandler, ImageEvent};
use lopdf::Object;

// --- Collecting handler ---

struct CollectingHandler {
    chars: Vec<CharEvent>,
    images: Vec<ImageEvent>,
    warnings: Vec<ExtractWarning>,
}

impl CollectingHandler {
    fn new() -> Self {
        Self {
            chars: Vec::new(),
            images: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl ContentHandler for CollectingHandler {
    fn on_char(&mut self, event: CharEvent) {
        self.chars.push(event);
    }
    fn on_image(&mut self, event: ImageEvent) {
        self.images.push(event);
    }
    fn on_warning(&mut self, warning: ExtractWarning) {
        self.warnings.push(warning);
    }
}

// --- Helper to create a minimal lopdf document for testing ---

fn empty_resources() -> lopdf::Dictionary {
    lopdf::Dictionary::new()
}

fn default_options() -> ExtractOptions {
    ExtractOptions::default()
}

// --- Basic text interpretation tests ---

#[test]
fn interpret_simple_text() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // "Hello" = 5 characters
    assert_eq!(handler.chars.len(), 5);
    assert_eq!(handler.chars[0].char_code, b'H' as u32);
    assert_eq!(handler.chars[1].char_code, b'e' as u32);
    assert_eq!(handler.chars[4].char_code, b'o' as u32);
    assert_eq!(handler.chars[0].font_size, 12.0);
}

#[test]
fn interpret_tj_array() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT /F1 12 Tf [(H) -20 (i)] TJ ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 2);
    assert_eq!(handler.chars[0].char_code, b'H' as u32);
    assert_eq!(handler.chars[1].char_code, b'i' as u32);
}

#[test]
fn interpret_ctm_passed_to_char_events() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"1 0 0 1 10 20 cm BT /F1 12 Tf (A) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 1);
    assert_eq!(handler.chars[0].ctm, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
}

// --- Recursion limit tests ---

#[test]
fn recursion_depth_zero_allowed() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    let result = interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    );
    assert!(result.is_ok());
}

#[test]
fn recursion_depth_exceeds_limit() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    let mut opts = ExtractOptions::default();
    opts.max_recursion_depth = 3;

    let result = interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &opts,
        4, // depth > max
        &mut gstate,
        &mut tstate,
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("recursion depth"));
}

// --- Graphics state tests ---

#[test]
fn interpret_q_q_state_save_restore() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    // Set color, save, change color, restore
    let stream = b"0.5 g q 1 0 0 rg Q";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // After Q, fill color should be restored to gray 0.5
    assert_eq!(
        gstate.graphics_state().fill_color,
        pdfplumber_core::Color::Gray(0.5)
    );
}

// --- CID font / Identity-H tests ---

/// Build a resources dictionary containing a Type0 font with Identity-H encoding.
fn make_cid_font_resources(doc: &mut lopdf::Document) -> lopdf::Dictionary {
    use lopdf::{Object, Stream, dictionary};

    // ToUnicode CMap: map 0x4E2D → U+4E2D (中), 0x6587 → U+6587 (文)
    let tounicode_data = b"\
            /CIDInit /ProcSet findresource begin\n\
            12 dict begin\n\
            begincmap\n\
            /CMapName /Adobe-Identity-UCS def\n\
            /CMapType 2 def\n\
            1 begincodespacerange\n\
            <0000> <FFFF>\n\
            endcodespacerange\n\
            2 beginbfchar\n\
            <4E2D> <4E2D>\n\
            <6587> <6587>\n\
            endbfchar\n\
            endcmap\n";
    let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
    let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

    // CIDFont dictionary
    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
        "DW" => Object::Integer(1000),
        "CIDToGIDMap" => "Identity",
        "CIDSystemInfo" => Object::Dictionary(dictionary! {
            "Registry" => Object::String("Adobe".as_bytes().to_vec(), lopdf::StringFormat::Literal),
            "Ordering" => Object::String("Identity".as_bytes().to_vec(), lopdf::StringFormat::Literal),
            "Supplement" => Object::Integer(0),
        }),
    };
    let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

    // Type0 font dictionary with Identity-H encoding
    let type0_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MSGothic",
        "Encoding" => "Identity-H",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        "ToUnicode" => Object::Reference(tounicode_id),
    };
    let type0_id = doc.add_object(Object::Dictionary(type0_dict));

    // Resources with Font entry
    dictionary! {
        "Font" => Object::Dictionary(dictionary! {
            "F1" => Object::Reference(type0_id),
        }),
    }
}

#[test]
fn interpret_cid_font_identity_h_two_byte_codes() {
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_cid_font_resources(&mut doc);

    // Content stream: use CID font F1 and show 2-byte character codes
    // 0x4E2D = 中, 0x6587 = 文
    let stream = b"BT /F1 12 Tf <4E2D6587> Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // Should produce 2 characters (2-byte codes), not 4 (1-byte)
    assert_eq!(handler.chars.len(), 2);
    assert_eq!(handler.chars[0].char_code, 0x4E2D);
    assert_eq!(handler.chars[1].char_code, 0x6587);
    // Unicode should be resolved via ToUnicode CMap
    assert_eq!(handler.chars[0].unicode, Some("中".to_string()));
    assert_eq!(handler.chars[1].unicode, Some("文".to_string()));
    assert_eq!(handler.chars[0].font_name, "MSGothic");
}

#[test]
fn interpret_cid_font_tj_array_two_byte_codes() {
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_cid_font_resources(&mut doc);

    // TJ array with 2-byte CID strings and adjustments
    let stream = b"BT /F1 12 Tf [<4E2D> -100 <6587>] TJ ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 2);
    assert_eq!(handler.chars[0].char_code, 0x4E2D);
    assert_eq!(handler.chars[1].char_code, 0x6587);
}

#[test]
fn interpret_subset_font_name_stripped() {
    let mut doc = lopdf::Document::with_version("1.5");

    use lopdf::{Object, Stream, dictionary};

    // Create a ToUnicode CMap
    let tounicode_data = b"\
            beginbfchar\n\
            <4E2D> <4E2D>\n\
            endbfchar\n";
    let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
    let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

    // CIDFont with subset prefix
    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "ABCDEF+MSGothic",
        "DW" => Object::Integer(1000),
        "CIDToGIDMap" => "Identity",
    };
    let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

    // Type0 font with subset prefix in BaseFont
    let type0_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "ABCDEF+MSGothic",
        "Encoding" => "Identity-H",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        "ToUnicode" => Object::Reference(tounicode_id),
    };
    let type0_id = doc.add_object(Object::Dictionary(type0_dict));

    let resources = dictionary! {
        "Font" => Object::Dictionary(dictionary! {
            "F1" => Object::Reference(type0_id),
        }),
    };

    let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 1);
    // Subset prefix should be stripped
    assert_eq!(handler.chars[0].font_name, "MSGothic");
}

/// Build resources for Identity-V (vertical writing mode).
fn make_cid_font_resources_identity_v(doc: &mut lopdf::Document) -> lopdf::Dictionary {
    use lopdf::{Object, Stream, dictionary};

    let tounicode_data = b"\
            beginbfchar\n\
            <4E2D> <4E2D>\n\
            endbfchar\n";
    let tounicode_stream = Stream::new(dictionary! {}, tounicode_data.to_vec());
    let tounicode_id = doc.add_object(Object::Stream(tounicode_stream));

    let cid_font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "CIDFontType2",
        "BaseFont" => "MSGothic",
        "DW" => Object::Integer(1000),
        "CIDToGIDMap" => "Identity",
    };
    let cid_font_id = doc.add_object(Object::Dictionary(cid_font_dict));

    let type0_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "MSGothic",
        "Encoding" => "Identity-V",
        "DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        "ToUnicode" => Object::Reference(tounicode_id),
    };
    let type0_id = doc.add_object(Object::Dictionary(type0_dict));

    dictionary! {
        "Font" => Object::Dictionary(dictionary! {
            "F1" => Object::Reference(type0_id),
        }),
    }
}

#[test]
fn interpret_cid_font_identity_v_detected() {
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_cid_font_resources_identity_v(&mut doc);

    // Show a CID character with Identity-V encoding
    let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // Should still produce characters (Identity-V uses same CID=charcode mapping)
    assert_eq!(handler.chars.len(), 1);
    assert_eq!(handler.chars[0].char_code, 0x4E2D);
    assert_eq!(handler.chars[0].unicode, Some("中".to_string()));
}

// --- Warning emission tests ---

#[test]
fn interpret_missing_font_emits_warning() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources(); // No fonts defined
    // Use font F1 which is not in resources
    let stream = b"BT /F1 12 Tf (Hi) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // Should emit a warning about missing font
    assert!(!handler.warnings.is_empty());
    assert!(
        handler.warnings[0]
            .description
            .contains("font not found in page resources"),
        "expected 'font not found' warning, got: {}",
        handler.warnings[0].description
    );
    assert_eq!(
        handler.warnings[0].font_name,
        Some("F1".to_string()),
        "warning should include font name"
    );
    assert!(
        handler.warnings[0].operator_index.is_some(),
        "warning should include operator index"
    );

    // Characters should still be extracted (using default metrics)
    assert_eq!(handler.chars.len(), 2);
}

#[test]
fn interpret_no_warnings_when_collection_disabled() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT /F1 12 Tf (Hi) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    let opts = ExtractOptions {
        collect_warnings: false,
        ..ExtractOptions::default()
    };

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &opts,
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // No warnings should be collected
    assert!(handler.warnings.is_empty());

    // Characters should still be extracted normally
    assert_eq!(handler.chars.len(), 2);
}

#[test]
fn interpret_warnings_do_not_affect_output() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    let stream = b"BT /F1 12 Tf (AB) Tj ET";

    // With warnings enabled
    let mut handler_on = CollectingHandler::new();
    let mut gstate_on = InterpreterState::new();
    let mut tstate_on = TextState::new();
    let opts_on = ExtractOptions {
        collect_warnings: true,
        ..ExtractOptions::default()
    };
    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler_on,
        &opts_on,
        0,
        &mut gstate_on,
        &mut tstate_on,
    )
    .unwrap();

    // With warnings disabled
    let mut handler_off = CollectingHandler::new();
    let mut gstate_off = InterpreterState::new();
    let mut tstate_off = TextState::new();
    let opts_off = ExtractOptions {
        collect_warnings: false,
        ..ExtractOptions::default()
    };
    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler_off,
        &opts_off,
        0,
        &mut gstate_off,
        &mut tstate_off,
    )
    .unwrap();

    // Same output regardless of warning collection
    assert_eq!(handler_on.chars.len(), handler_off.chars.len());
    for (a, b) in handler_on.chars.iter().zip(handler_off.chars.iter()) {
        assert_eq!(a.char_code, b.char_code);
    }
}

#[test]
fn interpret_valid_font_no_warnings() {
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_cid_font_resources(&mut doc);
    let stream = b"BT /F1 12 Tf <4E2D> Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // Valid font should not produce warnings
    assert!(
        handler.warnings.is_empty(),
        "expected no warnings for valid font, got: {:?}",
        handler.warnings
    );
    assert_eq!(handler.chars.len(), 1);
}

// --- ExtGState (gs operator) tests ---

/// Helper to create resources with an ExtGState dictionary.
fn resources_with_ext_gstate(name: &str, ext_gstate_dict: lopdf::Dictionary) -> lopdf::Dictionary {
    use lopdf::dictionary;
    dictionary! {
        "ExtGState" => Object::Dictionary(dictionary! {
            name => Object::Dictionary(ext_gstate_dict),
        }),
    }
}

#[test]
fn gs_applies_line_width() {
    use lopdf::dictionary;
    let doc = lopdf::Document::with_version("1.5");
    let resources = resources_with_ext_gstate(
        "GS1",
        dictionary! {
            "LW" => Object::Real(2.5),
        },
    );
    let stream = b"/GS1 gs";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert!(
        (gstate.graphics_state().line_width - 2.5).abs() < f64::EPSILON,
        "expected line_width 2.5, got {}",
        gstate.graphics_state().line_width
    );
}

#[test]
fn gs_applies_dash_pattern() {
    use lopdf::dictionary;
    let doc = lopdf::Document::with_version("1.5");
    // /D [[3 5] 6] — dash array [3, 5] with phase 6
    let resources = resources_with_ext_gstate(
        "GS1",
        dictionary! {
            "D" => Object::Array(vec![
                Object::Array(vec![Object::Integer(3), Object::Integer(5)]),
                Object::Integer(6),
            ]),
        },
    );
    let stream = b"/GS1 gs";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    let dp = &gstate.graphics_state().dash_pattern;
    assert_eq!(dp.dash_array, vec![3.0, 5.0]);
    assert!((dp.dash_phase - 6.0).abs() < f64::EPSILON);
}

#[test]
fn gs_applies_alpha() {
    use lopdf::dictionary;
    let doc = lopdf::Document::with_version("1.5");
    let resources = resources_with_ext_gstate(
        "GS1",
        dictionary! {
            "CA" => Object::Real(0.7),
            "ca" => Object::Real(0.3),
        },
    );
    let stream = b"/GS1 gs";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert!(
        (gstate.graphics_state().stroke_alpha - 0.7).abs() < 1e-6,
        "expected stroke_alpha ~0.7, got {}",
        gstate.graphics_state().stroke_alpha
    );
    assert!(
        (gstate.graphics_state().fill_alpha - 0.3).abs() < 1e-6,
        "expected fill_alpha ~0.3, got {}",
        gstate.graphics_state().fill_alpha
    );
}

#[test]
fn gs_missing_name_produces_no_error() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    // gs with a name that doesn't exist in resources — should not error
    let stream = b"/GS_nonexistent gs";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    // Should not return an error
    let result = interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    );
    assert!(result.is_ok(), "missing ExtGState should not produce error");
}

#[test]
fn gs_unknown_keys_silently_ignored() {
    use lopdf::dictionary;
    let doc = lopdf::Document::with_version("1.5");
    let resources = resources_with_ext_gstate(
        "GS1",
        dictionary! {
            "LW" => Object::Real(3.0),
            "BM" => "Normal",  // blend mode — not handled
            "SM" => Object::Real(0.01),  // smoothness — not handled
        },
    );
    let stream = b"/GS1 gs";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    // LW should be applied, unknown keys silently ignored
    assert!(
        (gstate.graphics_state().line_width - 3.0).abs() < f64::EPSILON,
        "expected line_width 3.0, got {}",
        gstate.graphics_state().line_width
    );
}

// --- Inline image (BI/ID/EI) tests ---

#[test]
fn interpret_inline_image_emits_image_event() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    // Inline image: 2x2 DeviceRGB, 8 bpc, raw data (12 bytes)
    // BI /W 2 /H 2 /CS /RGB /BPC 8 ID <12 bytes of pixel data> EI
    let mut stream: Vec<u8> = Vec::new();
    stream.extend_from_slice(b"q 100 0 0 50 72 700 cm BI /W 2 /H 2 /CS /RGB /BPC 8 ID ");
    // 2x2 RGB = 12 bytes of image data
    stream.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]);
    stream.extend_from_slice(b" EI Q");

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        &stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.images.len(), 1);
    let img = &handler.images[0];
    assert_eq!(img.width, 2);
    assert_eq!(img.height, 2);
    assert_eq!(img.colorspace, Some("DeviceRGB".to_string()));
    assert_eq!(img.bits_per_component, Some(8));
    // CTM should reflect the transformation: 100 0 0 50 72 700
    assert_eq!(img.ctm, [100.0, 0.0, 0.0, 50.0, 72.0, 700.0]);
    // Name should indicate inline image
    assert!(img.name.starts_with("inline-"));
}

#[test]
fn interpret_inline_image_abbreviated_keys() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    // Use abbreviated keys: /W, /H, /BPC, /CS with abbreviated color space /G
    let mut stream: Vec<u8> = Vec::new();
    stream.extend_from_slice(b"q 50 0 0 50 10 10 cm BI /W 1 /H 1 /CS /G /BPC 8 ID ");
    stream.push(128); // 1x1 grayscale = 1 byte
    stream.extend_from_slice(b" EI Q");

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        &stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.images.len(), 1);
    let img = &handler.images[0];
    assert_eq!(img.width, 1);
    assert_eq!(img.height, 1);
    assert_eq!(img.colorspace, Some("DeviceGray".to_string()));
    assert_eq!(img.bits_per_component, Some(8));
}

#[test]
fn interpret_inline_image_with_filter() {
    let doc = lopdf::Document::with_version("1.5");
    let resources = empty_resources();
    // BI with abbreviated filter /F /DCT
    let mut stream: Vec<u8> = Vec::new();
    stream.extend_from_slice(b"q 200 0 0 100 0 0 cm BI /W 10 /H 10 /CS /RGB /BPC 8 /F /DCT ID ");
    // Fake JPEG data (just a few bytes for testing)
    stream.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]);
    stream.extend_from_slice(b" EI Q");

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        &stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.images.len(), 1);
    let img = &handler.images[0];
    assert_eq!(img.width, 10);
    assert_eq!(img.height, 10);
    assert_eq!(img.filter, Some("DCTDecode".to_string()));
}

#[test]
fn interpret_inline_image_abbreviated_filter_names() {
    // Test all abbreviated filter name mappings
    let abbreviated_to_full = [
        ("AHx", "ASCIIHexDecode"),
        ("A85", "ASCII85Decode"),
        ("LZW", "LZWDecode"),
        ("Fl", "FlateDecode"),
        ("RL", "RunLengthDecode"),
        ("CCF", "CCITTFaxDecode"),
        ("DCT", "DCTDecode"),
    ];

    for (abbrev, full_name) in &abbreviated_to_full {
        let doc = lopdf::Document::with_version("1.5");
        let resources = empty_resources();

        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(
            format!("q 10 0 0 10 0 0 cm BI /W 1 /H 1 /CS /G /BPC 8 /F /{abbrev} ID ").as_bytes(),
        );
        stream.push(0); // 1 byte image data
        stream.extend_from_slice(b" EI Q");

        let mut handler = CollectingHandler::new();
        let mut gstate = InterpreterState::new();
        let mut tstate = TextState::new();

        interpret_content_stream(
            &doc,
            &stream,
            &resources,
            &mut handler,
            &default_options(),
            0,
            &mut gstate,
            &mut tstate,
        )
        .unwrap();

        assert_eq!(
            handler.images.len(),
            1,
            "no image emitted for filter abbreviation /{abbrev}"
        );
        assert_eq!(
            handler.images[0].filter,
            Some(full_name.to_string()),
            "filter mismatch for /{abbrev}: expected {full_name}"
        );
    }
}

// --- Marked content (BMC/BDC/EMC) tests ---

#[test]
fn bdc_with_mcid_sets_char_mcid() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream = b"/P <</MCID 5>> BDC BT /F1 12 Tf (Hi) Tj ET EMC";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 2);
    assert_eq!(handler.chars[0].mcid, Some(5));
    assert_eq!(handler.chars[0].tag.as_deref(), Some("P"));
    assert_eq!(handler.chars[1].mcid, Some(5));
    assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
}

#[test]
fn bmc_sets_tag_without_mcid() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream = b"/Artifact BMC BT /F1 12 Tf (X) Tj ET EMC";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 1);
    assert_eq!(handler.chars[0].tag.as_deref(), Some("Artifact"));
    assert_eq!(handler.chars[0].mcid, None);
}

#[test]
fn nested_bdc_maintains_correct_stack() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream =
        b"/P <</MCID 1>> BDC BT /F1 12 Tf (A) Tj /Span <</MCID 2>> BDC (B) Tj EMC (C) Tj ET EMC";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 3);
    assert_eq!(handler.chars[0].mcid, Some(1));
    assert_eq!(handler.chars[0].tag.as_deref(), Some("P"));
    assert_eq!(handler.chars[1].mcid, Some(2));
    assert_eq!(handler.chars[1].tag.as_deref(), Some("Span"));
    assert_eq!(handler.chars[2].mcid, Some(1));
    assert_eq!(handler.chars[2].tag.as_deref(), Some("P"));
}

#[test]
fn emc_without_matching_bmc_handled_gracefully() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream = b"EMC BT /F1 12 Tf (A) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    let result = interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    );

    assert!(result.is_ok());
    assert_eq!(handler.chars.len(), 1);
    assert_eq!(handler.chars[0].mcid, None);
    assert_eq!(handler.chars[0].tag, None);
}

#[test]
fn chars_outside_marked_content_have_none() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream = b"BT /F1 12 Tf (A) Tj /P <</MCID 3>> BDC (B) Tj EMC (C) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 3);
    assert_eq!(handler.chars[0].mcid, None);
    assert_eq!(handler.chars[0].tag, None);
    assert_eq!(handler.chars[1].mcid, Some(3));
    assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
    assert_eq!(handler.chars[2].mcid, None);
    assert_eq!(handler.chars[2].tag, None);
}

#[test]
fn nested_bmc_inside_bdc_inherits_mcid() {
    let doc = lopdf::Document::with_version("1.7");
    let resources = empty_resources();
    let stream = b"/P <</MCID 7>> BDC BT /F1 12 Tf /Artifact BMC (A) Tj EMC (B) Tj ET EMC";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 2);
    assert_eq!(handler.chars[0].tag.as_deref(), Some("Artifact"));
    assert_eq!(handler.chars[0].mcid, Some(7));
    assert_eq!(handler.chars[1].tag.as_deref(), Some("P"));
    assert_eq!(handler.chars[1].mcid, Some(7));
}

// --- US-182-1: StandardEncoding fallback for Type1 fonts ---

/// Create resources with a standard Type1 font (e.g. Helvetica) that has NO
/// explicit /Encoding entry.  Per the PDF spec, StandardEncoding should be
/// used as the implicit base encoding for such fonts.
fn make_standard_type1_font_resources(doc: &mut lopdf::Document) -> lopdf::Dictionary {
    use lopdf::{Object, dictionary};

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        // No /Encoding — StandardEncoding should be applied implicitly
    };
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    dictionary! {
        "Font" => Object::Dictionary(dictionary! {
            "F1" => Object::Reference(font_id),
        }),
    }
}

#[test]
fn standard_type1_font_uses_standard_encoding_for_0x27() {
    // Byte 0x27 in StandardEncoding maps to 'quoteright' (U+2019),
    // NOT ASCII apostrophe (U+0027).
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_standard_type1_font_resources(&mut doc);

    // Content stream: render byte 0x27 with Helvetica
    let stream = b"BT /F1 12 Tf (I\x27ll) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 4); // I, quoteright, l, l
    // The critical assertion: byte 0x27 must decode to U+2019, not U+0027
    assert_eq!(
        handler.chars[1].unicode.as_deref(),
        Some("\u{2019}"),
        "byte 0x27 in StandardEncoding should be quoteright (U+2019), got {:?}",
        handler.chars[1].unicode
    );
}

#[test]
fn standard_type1_font_keeps_ascii_letters_unchanged() {
    // ASCII letters (0x41-0x5A, 0x61-0x7A) are the same in StandardEncoding
    // and ASCII, so they should decode normally.
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_standard_type1_font_resources(&mut doc);

    let stream = b"BT /F1 12 Tf (Hello) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 5);
    assert_eq!(handler.chars[0].unicode.as_deref(), Some("H"));
    assert_eq!(handler.chars[1].unicode.as_deref(), Some("e"));
    assert_eq!(handler.chars[2].unicode.as_deref(), Some("l"));
    assert_eq!(handler.chars[3].unicode.as_deref(), Some("l"));
    assert_eq!(handler.chars[4].unicode.as_deref(), Some("o"));
}

#[test]
fn standard_type1_font_0x60_maps_to_quoteleft() {
    // Byte 0x60 in StandardEncoding maps to 'quoteleft' (U+2018),
    // NOT grave accent (U+0060).
    let mut doc = lopdf::Document::with_version("1.5");
    let resources = make_standard_type1_font_resources(&mut doc);

    let stream = b"BT /F1 12 Tf (\x60) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 1);
    assert_eq!(
        handler.chars[0].unicode.as_deref(),
        Some("\u{2018}"),
        "byte 0x60 in StandardEncoding should be quoteleft (U+2018), got {:?}",
        handler.chars[0].unicode
    );
}

#[test]
fn explicit_encoding_not_overridden_by_standard_fallback() {
    // When a font has an explicit /Encoding (e.g. WinAnsiEncoding),
    // it must NOT be overridden by the StandardEncoding fallback.
    let mut doc = lopdf::Document::with_version("1.5");

    use lopdf::{Object, dictionary};

    let font_dict = dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    };
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    let resources = dictionary! {
        "Font" => Object::Dictionary(dictionary! {
            "F1" => Object::Reference(font_id),
        }),
    };

    // Byte 0x27 in WinAnsiEncoding maps to quotesingle (U+0027)
    let stream = b"BT /F1 12 Tf (\x27) Tj ET";

    let mut handler = CollectingHandler::new();
    let mut gstate = InterpreterState::new();
    let mut tstate = TextState::new();

    interpret_content_stream(
        &doc,
        stream,
        &resources,
        &mut handler,
        &default_options(),
        0,
        &mut gstate,
        &mut tstate,
    )
    .unwrap();

    assert_eq!(handler.chars.len(), 1);
    // WinAnsiEncoding: 0x27 = quotesingle (U+0027), not quoteright
    assert_eq!(
        handler.chars[0].unicode.as_deref(),
        Some("'"),
        "WinAnsiEncoding byte 0x27 should be quotesingle (U+0027), got {:?}",
        handler.chars[0].unicode
    );
}
