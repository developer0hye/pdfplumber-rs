//! Content handler callback trait for content stream interpretation.
//!
//! Defines the [`ContentHandler`] trait that bridges Layer 2 (content stream
//! interpreter) and Layer 3 (object extraction). The interpreter calls handler
//! methods as it processes PDF content stream operators.

use pdfplumber_core::{Color, DashPattern, ExtractWarning, FillRule, PathSegment};

/// The type of paint operation applied to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintOp {
    /// Path is stroked (outlined).
    Stroke,
    /// Path is filled.
    Fill,
    /// Path is both filled and stroked.
    FillAndStroke,
}

/// Information about a rendered character glyph.
///
/// Produced by the interpreter when processing text rendering operators
/// (Tj, TJ, ', "). Contains all positioning and font context needed
/// to compute the final character bounding box.
#[derive(Debug, Clone)]
pub struct CharEvent {
    /// The character code from the PDF content stream.
    pub char_code: u32,
    /// Unicode text if a ToUnicode mapping is available.
    pub unicode: Option<String>,
    /// Font name (e.g., "Helvetica", "BCDFEE+ArialMT").
    pub font_name: String,
    /// Font size in text space units.
    pub font_size: f64,
    /// The text rendering matrix at the time of rendering (6-element affine).
    pub text_matrix: [f64; 6],
    /// The current transformation matrix at the time of rendering.
    pub ctm: [f64; 6],
    /// Glyph displacement (advance width) in glyph space units (1/1000 of text space).
    pub displacement: f64,
    /// Character spacing value (Tc operator).
    pub char_spacing: f64,
    /// Word spacing value (Tw operator), applied for space characters.
    pub word_spacing: f64,
    /// Horizontal scaling factor (Tz operator, as a fraction: 100% = 1.0).
    pub h_scaling: f64,
    /// Text rise value (Ts operator) for superscript/subscript.
    pub rise: f64,
    /// Marked content identifier (MCID) from BDC operator, if inside a marked content sequence.
    pub mcid: Option<u32>,
    /// Structure tag name (e.g., "P", "Span", "H1") from BMC/BDC operator.
    pub tag: Option<String>,
}

/// Information about a painted path.
///
/// Produced by the interpreter when a path is stroked, filled, or both.
/// Contains the path geometry, paint operation, and graphics state at
/// the time of painting.
#[derive(Debug, Clone)]
pub struct PathEvent {
    /// The path segments making up this path.
    pub segments: Vec<PathSegment>,
    /// The paint operation applied.
    pub paint_op: PaintOp,
    /// Stroke line width.
    pub line_width: f64,
    /// Stroking (outline) color.
    pub stroking_color: Option<Color>,
    /// Non-stroking (fill) color.
    pub non_stroking_color: Option<Color>,
    /// Current transformation matrix at the time of painting.
    pub ctm: [f64; 6],
    /// Dash pattern for stroked paths.
    pub dash_pattern: Option<DashPattern>,
    /// Fill rule for filled paths.
    pub fill_rule: Option<FillRule>,
}

/// Information about a placed image.
///
/// Produced by the interpreter when a Do operator references an Image
/// XObject. The CTM determines the image's position and size on the page.
#[derive(Debug, Clone)]
pub struct ImageEvent {
    /// Image XObject name reference (e.g., "Im0").
    pub name: String,
    /// CTM at the time of image placement (determines position and size).
    pub ctm: [f64; 6],
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Color space name (e.g., "DeviceRGB", "DeviceGray").
    pub colorspace: Option<String>,
    /// Bits per component.
    pub bits_per_component: Option<u32>,
    /// PDF stream filter name (e.g., "DCTDecode", "FlateDecode").
    pub filter: Option<String>,
}

/// Callback handler for content stream interpretation.
///
/// The content stream interpreter calls these methods as it processes
/// PDF page content. Implementors collect the events to build extraction
/// results (characters, paths, images).
///
/// All methods have default no-op implementations, allowing handlers to
/// subscribe only to the event types they care about.
///
/// # Text Operations
///
/// [`on_char`](ContentHandler::on_char) is called for each rendered
/// character glyph with full positioning and font context.
///
/// # Path Operations
///
/// [`on_path_painted`](ContentHandler::on_path_painted) is called when
/// a path is stroked, filled, or both.
///
/// # Image Operations
///
/// [`on_image`](ContentHandler::on_image) is called when an image
/// XObject is placed on the page.
pub trait ContentHandler {
    /// Called when a character glyph is rendered.
    fn on_char(&mut self, _event: CharEvent) {}

    /// Called when a path is painted (stroked, filled, or both).
    fn on_path_painted(&mut self, _event: PathEvent) {}

    /// Called when an image XObject is placed on the page.
    fn on_image(&mut self, _event: ImageEvent) {}

    /// Called when a non-fatal warning is encountered during interpretation.
    ///
    /// Warnings indicate best-effort degradation (e.g., missing font metrics,
    /// unresolvable references). They do not affect extraction correctness —
    /// the interpreter continues with sensible defaults.
    fn on_warning(&mut self, _warning: ExtractWarning) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::Point;

    // --- CollectingHandler: captures all events for assertion ---

    struct CollectingHandler {
        chars: Vec<CharEvent>,
        paths: Vec<PathEvent>,
        images: Vec<ImageEvent>,
        warnings: Vec<ExtractWarning>,
    }

    impl CollectingHandler {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                paths: Vec::new(),
                images: Vec::new(),
                warnings: Vec::new(),
            }
        }
    }

    impl ContentHandler for CollectingHandler {
        fn on_char(&mut self, event: CharEvent) {
            self.chars.push(event);
        }

        fn on_path_painted(&mut self, event: PathEvent) {
            self.paths.push(event);
        }

        fn on_image(&mut self, event: ImageEvent) {
            self.images.push(event);
        }

        fn on_warning(&mut self, warning: ExtractWarning) {
            self.warnings.push(warning);
        }
    }

    // --- NoopHandler: verifies default no-op implementations compile ---

    struct NoopHandler;
    impl ContentHandler for NoopHandler {}

    // --- Helper to create a sample CharEvent ---

    fn sample_char_event() -> CharEvent {
        CharEvent {
            char_code: 65,
            unicode: Some("A".to_string()),
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 720.0],
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            displacement: 667.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scaling: 1.0,
            rise: 0.0,
            mcid: None,
            tag: None,
        }
    }

    fn sample_path_event() -> PathEvent {
        PathEvent {
            segments: vec![
                PathSegment::MoveTo(Point::new(0.0, 0.0)),
                PathSegment::LineTo(Point::new(100.0, 0.0)),
                PathSegment::LineTo(Point::new(100.0, 50.0)),
                PathSegment::LineTo(Point::new(0.0, 50.0)),
                PathSegment::ClosePath,
            ],
            paint_op: PaintOp::Stroke,
            line_width: 1.0,
            stroking_color: Some(Color::black()),
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            dash_pattern: None,
            fill_rule: None,
        }
    }

    fn sample_image_event() -> ImageEvent {
        ImageEvent {
            name: "Im0".to_string(),
            ctm: [200.0, 0.0, 0.0, 150.0, 100.0, 300.0],
            width: 800,
            height: 600,
            colorspace: Some("DeviceRGB".to_string()),
            bits_per_component: Some(8),
            filter: None,
        }
    }

    // --- PaintOp tests ---

    #[test]
    fn paint_op_variants() {
        assert_ne!(PaintOp::Stroke, PaintOp::Fill);
        assert_ne!(PaintOp::Fill, PaintOp::FillAndStroke);
        assert_ne!(PaintOp::Stroke, PaintOp::FillAndStroke);
    }

    #[test]
    fn paint_op_copy() {
        let op = PaintOp::Stroke;
        let op2 = op; // Copy
        assert_eq!(op, op2);
    }

    // --- CharEvent tests ---

    #[test]
    fn char_event_construction() {
        let event = sample_char_event();
        assert_eq!(event.char_code, 65);
        assert_eq!(event.unicode.as_deref(), Some("A"));
        assert_eq!(event.font_name, "Helvetica");
        assert_eq!(event.font_size, 12.0);
        assert_eq!(event.displacement, 667.0);
        assert_eq!(event.h_scaling, 1.0);
        assert_eq!(event.rise, 0.0);
    }

    #[test]
    fn char_event_without_unicode() {
        let event = CharEvent {
            unicode: None,
            ..sample_char_event()
        };
        assert_eq!(event.unicode, None);
    }

    #[test]
    fn char_event_clone() {
        let event = sample_char_event();
        let cloned = event.clone();
        assert_eq!(cloned.char_code, 65);
        assert_eq!(cloned.font_name, "Helvetica");
    }

    // --- PathEvent tests ---

    #[test]
    fn path_event_construction() {
        let event = sample_path_event();
        assert_eq!(event.segments.len(), 5);
        assert_eq!(event.paint_op, PaintOp::Stroke);
        assert_eq!(event.line_width, 1.0);
        assert!(event.stroking_color.is_some());
        assert!(event.non_stroking_color.is_none());
    }

    #[test]
    fn path_event_fill_with_rule() {
        let event = PathEvent {
            paint_op: PaintOp::Fill,
            fill_rule: Some(FillRule::EvenOdd),
            stroking_color: None,
            non_stroking_color: Some(Color::Rgb(1.0, 0.0, 0.0)),
            ..sample_path_event()
        };
        assert_eq!(event.paint_op, PaintOp::Fill);
        assert_eq!(event.fill_rule, Some(FillRule::EvenOdd));
    }

    #[test]
    fn path_event_with_dash_pattern() {
        let event = PathEvent {
            dash_pattern: Some(DashPattern {
                dash_array: vec![3.0, 2.0],
                dash_phase: 0.0,
            }),
            ..sample_path_event()
        };
        let dp = event.dash_pattern.unwrap();
        assert_eq!(dp.dash_array, vec![3.0, 2.0]);
    }

    // --- ImageEvent tests ---

    #[test]
    fn image_event_construction() {
        let event = sample_image_event();
        assert_eq!(event.name, "Im0");
        assert_eq!(event.width, 800);
        assert_eq!(event.height, 600);
        assert_eq!(event.colorspace.as_deref(), Some("DeviceRGB"));
        assert_eq!(event.bits_per_component, Some(8));
    }

    #[test]
    fn image_event_without_optional_fields() {
        let event = ImageEvent {
            colorspace: None,
            bits_per_component: None,
            filter: None,
            ..sample_image_event()
        };
        assert_eq!(event.colorspace, None);
        assert_eq!(event.bits_per_component, None);
        assert_eq!(event.filter, None);
    }

    #[test]
    fn image_event_with_filter() {
        let event = ImageEvent {
            filter: Some("DCTDecode".to_string()),
            ..sample_image_event()
        };
        assert_eq!(event.filter, Some("DCTDecode".to_string()));
    }

    // --- ContentHandler with CollectingHandler ---

    #[test]
    fn collecting_handler_receives_char_events() {
        let mut handler = CollectingHandler::new();
        handler.on_char(sample_char_event());
        handler.on_char(CharEvent {
            char_code: 66,
            unicode: Some("B".to_string()),
            ..sample_char_event()
        });

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.chars[0].char_code, 65);
        assert_eq!(handler.chars[1].char_code, 66);
    }

    #[test]
    fn collecting_handler_receives_path_events() {
        let mut handler = CollectingHandler::new();
        handler.on_path_painted(sample_path_event());

        assert_eq!(handler.paths.len(), 1);
        assert_eq!(handler.paths[0].paint_op, PaintOp::Stroke);
    }

    #[test]
    fn collecting_handler_receives_image_events() {
        let mut handler = CollectingHandler::new();
        handler.on_image(sample_image_event());

        assert_eq!(handler.images.len(), 1);
        assert_eq!(handler.images[0].name, "Im0");
    }

    #[test]
    fn collecting_handler_receives_mixed_events() {
        let mut handler = CollectingHandler::new();
        handler.on_char(sample_char_event());
        handler.on_path_painted(sample_path_event());
        handler.on_image(sample_image_event());
        handler.on_char(CharEvent {
            char_code: 66,
            unicode: Some("B".to_string()),
            ..sample_char_event()
        });

        assert_eq!(handler.chars.len(), 2);
        assert_eq!(handler.paths.len(), 1);
        assert_eq!(handler.images.len(), 1);
    }

    // --- NoopHandler: default implementations ---

    #[test]
    fn noop_handler_accepts_all_events() {
        let mut handler = NoopHandler;
        handler.on_char(sample_char_event());
        handler.on_path_painted(sample_path_event());
        handler.on_image(sample_image_event());
        // No panics, no state change — verifies default no-op implementations work
    }

    // --- ContentHandler as trait object ---

    #[test]
    fn content_handler_is_object_safe() {
        let mut handler = CollectingHandler::new();
        let handler_ref: &mut dyn ContentHandler = &mut handler;
        handler_ref.on_char(sample_char_event());
        // Verifies the trait can be used as a trait object
    }

    // --- on_warning tests ---

    #[test]
    fn noop_handler_on_warning_does_nothing() {
        let mut handler = NoopHandler;
        handler.on_warning(ExtractWarning::new("test warning"));
        // No panics — verifies default no-op implementation works
    }

    #[test]
    fn collecting_handler_receives_warnings() {
        let mut handler = CollectingHandler::new();
        handler.on_warning(ExtractWarning::new("warning 1"));
        handler.on_warning(ExtractWarning::on_page("warning 2", 0));
        handler.on_warning(ExtractWarning::with_operator_context(
            "font issue",
            5,
            "Helvetica",
        ));

        assert_eq!(handler.warnings.len(), 3);
        assert_eq!(handler.warnings[0].description, "warning 1");
        assert_eq!(handler.warnings[1].page, Some(0));
        assert_eq!(handler.warnings[2].font_name, Some("Helvetica".to_string()));
    }

    #[test]
    fn on_warning_via_trait_object() {
        let mut handler = CollectingHandler::new();
        let handler_ref: &mut dyn ContentHandler = &mut handler;
        handler_ref.on_warning(ExtractWarning::new("test"));

        assert_eq!(handler.warnings.len(), 1);
    }
}
