//! Character bounding box calculation from content stream events.
//!
//! Combines font metrics, text state, and CTM to calculate the final
//! bounding box for each character in top-left origin coordinates.
//! This bridges Layer 2 (interpreter) and Layer 3 (object extraction).

use pdfplumber_core::geometry::{BBox, Ctm, Point};
use pdfplumber_core::painting::Color;
use pdfplumber_core::text::{Char, TextDirection};

use crate::handler::CharEvent;

/// Convert a `CharEvent` into a fully-populated `Char`
/// with bounding box in top-left origin page coordinates.
///
/// # Arguments
///
/// * `event` - Character rendering event from the content stream interpreter.
///   Contains per-font ascent/descent for accurate vertical bounding boxes.
/// * `page_height` - Page height in PDF units (for y-flip from bottom-left to top-left origin).
/// * `stroking_color` - Current stroking color from the graphics state.
/// * `non_stroking_color` - Current non-stroking color from the graphics state.
///
/// # Coordinate System
///
/// PDF uses bottom-left origin. This function converts to top-left origin
/// (pdfplumber convention) by flipping: `top = page_height - max_y`.
pub fn char_from_event(
    event: &CharEvent,
    page_height: f64,
    stroking_color: Option<Color>,
    non_stroking_color: Option<Color>,
) -> Char {
    let font_size = event.font_size;
    let h_scaling = event.h_scaling;

    // Build the Text Rendering Matrix (Trm) per PDF spec 9.4.4:
    // Trm = [Tfs*Th, 0, 0, Tfs, 0, Trise] x Tm x CTM
    let font_matrix = Ctm::new(font_size * h_scaling, 0.0, 0.0, font_size, 0.0, event.rise);
    let tm = ctm_from_array(&event.text_matrix);
    let ctm = ctm_from_array(&event.ctm);
    let trm = font_matrix.concat(&tm).concat(&ctm);

    // Character width in glyph-normalized space.
    // The bounding box should cover only the glyph's visual extent (w0/1000),
    // NOT the full advance width which includes char_spacing and word_spacing.
    // Those inter-glyph spacings affect text position advance (handled in
    // text_renderer.rs) but not the individual character's visual bbox.
    let w_norm = event.displacement / 1000.0;

    // Ascent/descent from per-font metrics carried in the event
    let ascent_norm = event.ascent / 1000.0;
    let descent_norm = event.descent / 1000.0;

    // Vertical origin displacement in glyph-normalized space.
    // For vertical writing (WMode=1), the text position is the vertical origin,
    // displaced from the horizontal origin by (vx, vy). Shift the bbox
    // by (-vx/1000, -vy/1000) to position relative to the horizontal origin.
    let (vx, vy) = event.vertical_origin;
    let ox = -vx / 1000.0;
    let oy = -vy / 1000.0;

    // Four corners of the character rectangle in glyph-normalized space,
    // transformed through Trm to page space (PDF bottom-left origin).
    let corners = [
        trm.transform_point(Point::new(ox, oy + descent_norm)),
        trm.transform_point(Point::new(ox + w_norm, oy + descent_norm)),
        trm.transform_point(Point::new(ox + w_norm, oy + ascent_norm)),
        trm.transform_point(Point::new(ox, oy + ascent_norm)),
    ];

    // Axis-aligned bounding box in PDF page space
    let min_x = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);

    // Y-flip: PDF bottom-left origin → top-left origin
    let top = page_height - max_y;
    let bottom = page_height - min_y;

    let bbox = BBox::new(min_x, top, max_x, bottom);

    // Upright: no rotation/shear AND positive x-scale.
    // Matches Python pdfplumber: `upright = (trm[1] == 0 and trm[2] == 0 and trm[0] > 0)`.
    // A negative x-scale (horizontal mirror, matrix a < 0) produces upright=False in Python
    // and must produce upright=false here too, so that the word extractor routes these chars
    // through TTB column-based grouping instead of LTR x-gap grouping (issue #221, issue-848).
    let upright = trm.b.abs() < 1e-6 && trm.c.abs() < 1e-6 && trm.a > 0.0;

    // Text direction from the dominant axis of the text rendering matrix
    let direction = if trm.a.abs() >= trm.b.abs() {
        if trm.a >= 0.0 {
            TextDirection::Ltr
        } else {
            TextDirection::Rtl
        }
    } else if trm.b > 0.0 {
        TextDirection::Btt
    } else {
        TextDirection::Ttb
    };

    // Unicode text with fallback
    let text = event.unicode.clone().unwrap_or_else(|| {
        char::from_u32(event.char_code)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "\u{FFFD}".to_string())
    });

    Char {
        text,
        bbox,
        fontname: event.font_name.clone(),
        size: font_size,
        doctop: top,
        upright,
        direction,
        stroking_color,
        non_stroking_color,
        ctm: event.ctm,
        char_code: event.char_code,
        mcid: event.mcid,
        tag: event.tag.clone(),
    }
}

/// Create a [`Ctm`] from a 6-element array `[a, b, c, d, e, f]`.
fn ctm_from_array(arr: &[f64; 6]) -> Ctm {
    Ctm::new(arr[0], arr[1], arr[2], arr[3], arr[4], arr[5])
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE_HEIGHT: f64 = 792.0; // US Letter

    /// Helper: create a default CharEvent for testing.
    fn default_event() -> CharEvent {
        CharEvent {
            char_code: 65, // 'A'
            unicode: Some("A".to_string()),
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 720.0],
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            displacement: 667.0, // glyph width in 1/1000 units
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scaling: 1.0,
            rise: 0.0,
            ascent: 750.0,
            descent: -250.0,
            vertical_origin: (0.0, 0.0),
            mcid: None,
            tag: None,
        }
    }

    fn assert_approx(actual: f64, expected: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{msg}: expected {expected}, got {actual}"
        );
    }

    // ===== Test 1: Simple horizontal text bbox =====

    #[test]
    fn simple_horizontal_text_bbox() {
        let event = default_event();
        let ch = char_from_event(&event, PAGE_HEIGHT, None, Some(Color::black()));

        // Trm = [12, 0, 0, 12, 72, 720]
        // w_norm = 0.667, ascent_norm = 0.75, descent_norm = -0.25
        // BL→(72, 717), BR→(80.004, 717), TR→(80.004, 729), TL→(72, 729)
        // Y-flip: top = 792-729 = 63, bottom = 792-717 = 75
        assert_approx(ch.bbox.x0, 72.0, "x0");
        assert_approx(ch.bbox.top, 63.0, "top");
        assert_approx(ch.bbox.x1, 80.004, "x1");
        assert_approx(ch.bbox.bottom, 75.0, "bottom");
        assert_approx(ch.bbox.width(), 8.004, "width");
        assert_approx(ch.bbox.height(), 12.0, "height");

        assert_eq!(ch.text, "A");
        assert_eq!(ch.fontname, "Helvetica");
        assert_eq!(ch.size, 12.0);
        assert!(ch.upright);
        assert_eq!(ch.direction, TextDirection::Ltr);
        assert_eq!(ch.char_code, 65);
        assert_eq!(ch.ctm, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    // ===== Test 2: Scaled text (font_size = 24) =====

    #[test]
    fn scaled_text_bbox() {
        let event = CharEvent {
            font_size: 24.0,
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Trm = [24, 0, 0, 24, 72, 720]
        // BL→(72, 714), BR→(88.008, 714), TR→(88.008, 738), TL→(72, 738)
        // Y-flip: top = 792-738 = 54, bottom = 792-714 = 78
        assert_approx(ch.bbox.x0, 72.0, "x0");
        assert_approx(ch.bbox.top, 54.0, "top");
        assert_approx(ch.bbox.x1, 88.008, "x1");
        assert_approx(ch.bbox.bottom, 78.0, "bottom");
        assert_approx(ch.bbox.width(), 16.008, "width");
        assert_approx(ch.bbox.height(), 24.0, "height");
        assert_eq!(ch.size, 24.0);
    }

    // ===== Test 3: Text with rise (superscript) =====

    #[test]
    fn text_with_rise_bbox() {
        let event = CharEvent {
            rise: 5.0,
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // font_matrix = [12, 0, 0, 12, 0, 5]
        // Trm = font_matrix.concat(tm).concat(ctm) = [12, 0, 0, 12, 72, 725]
        // BL→(72, 722), BR→(80.004, 722), TR→(80.004, 734), TL→(72, 734)
        // Y-flip: top = 792-734 = 58, bottom = 792-722 = 70
        assert_approx(ch.bbox.x0, 72.0, "x0");
        assert_approx(ch.bbox.top, 58.0, "top");
        assert_approx(ch.bbox.x1, 80.004, "x1");
        assert_approx(ch.bbox.bottom, 70.0, "bottom");
        // Same size, just shifted up by 5 points
        assert_approx(ch.bbox.height(), 12.0, "height");
    }

    // ===== Test 4: Rotated text matrix (90 degrees CCW) =====

    #[test]
    fn rotated_text_matrix_bbox() {
        let event = CharEvent {
            text_matrix: [0.0, 1.0, -1.0, 0.0, 200.0, 400.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // font_matrix = [12, 0, 0, 12, 0, 0]
        // tm = [0, 1, -1, 0, 200, 400]
        // trm = [0, 12, -12, 0, 200, 400]
        // BL(0,-0.25)→(203, 400), BR(0.667,-0.25)→(203, 408.004)
        // TR(0.667,0.75)→(191, 408.004), TL(0,0.75)→(191, 400)
        // min_x=191, max_x=203, min_y=400, max_y=408.004
        // Y-flip: top=792-408.004=383.996, bottom=792-400=392
        assert_approx(ch.bbox.x0, 191.0, "x0");
        assert_approx(ch.bbox.top, 383.996, "top");
        assert_approx(ch.bbox.x1, 203.0, "x1");
        assert_approx(ch.bbox.bottom, 392.0, "bottom");
        // Rotated: width and height swap
        assert_approx(ch.bbox.width(), 12.0, "width");
        assert_approx(ch.bbox.height(), 8.004, "height");

        assert!(!ch.upright);
        // Text goes bottom-to-top in this rotation
        assert_eq!(ch.direction, TextDirection::Btt);
    }

    // ===== Test 5: CTM transformation (translation) =====

    #[test]
    fn ctm_translation_bbox() {
        let event = CharEvent {
            ctm: [1.0, 0.0, 0.0, 1.0, 50.0, 50.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Trm = [12, 0, 0, 12, 72, 720].concat([1,0,0,1,50,50])
        //      = [12, 0, 0, 12, 122, 770]
        // BL→(122, 767), BR→(130.004, 767), TR→(130.004, 779), TL→(122, 779)
        // Y-flip: top=792-779=13, bottom=792-767=25
        assert_approx(ch.bbox.x0, 122.0, "x0");
        assert_approx(ch.bbox.top, 13.0, "top");
        assert_approx(ch.bbox.x1, 130.004, "x1");
        assert_approx(ch.bbox.bottom, 25.0, "bottom");
    }

    // ===== Test 6: Char spacing does NOT affect bbox width =====

    #[test]
    fn char_spacing_does_not_affect_bbox_width() {
        let event = CharEvent {
            char_spacing: 2.0, // 2 units extra spacing (inter-glyph, not visual)
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // char_spacing is inter-glyph spacing, not part of the glyph's visual bbox.
        // w_norm = 667/1000 = 0.667 (same as without spacing)
        // Width in page space = 12 * 0.667 = 8.004
        assert_approx(ch.bbox.width(), 8.004, "width unaffected by char_spacing");
        // Height unchanged
        assert_approx(ch.bbox.height(), 12.0, "height");
    }

    // ===== Test 7: Word spacing does NOT affect bbox width =====

    #[test]
    fn word_spacing_does_not_affect_bbox_width() {
        let event = CharEvent {
            char_code: 32, // space
            unicode: Some(" ".to_string()),
            displacement: 250.0, // typical space width
            word_spacing: 3.0,
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // word_spacing is inter-glyph spacing, not part of the glyph's visual bbox.
        // w_norm = 250/1000 = 0.25 (glyph width only)
        // Width in page space = 12 * 0.25 = 3.0
        assert_approx(ch.bbox.width(), 3.0, "width unaffected by word_spacing");
    }

    #[test]
    fn word_spacing_not_applied_for_non_space() {
        let event = CharEvent {
            word_spacing: 3.0, // should be ignored for non-space
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Word spacing should not affect non-space characters
        // w_norm = 667/1000 = 0.667, same as no spacing
        assert_approx(ch.bbox.width(), 8.004, "width without word_spacing");
    }

    // ===== Test 8: Upright detection =====

    #[test]
    fn upright_for_horizontal_text() {
        let event = default_event();
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert!(ch.upright);
    }

    #[test]
    fn not_upright_for_rotated_text() {
        let event = CharEvent {
            text_matrix: [0.0, 1.0, -1.0, 0.0, 100.0, 500.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert!(!ch.upright);
    }

    #[test]
    fn not_upright_for_horizontal_mirror_text() {
        // Matrix (-1, 0, 0, 1): horizontal mirror (negative x-scale).
        // Python pdfplumber: upright = trm[0] > 0 → False for a=-1.
        // Rust must match — these chars route through TTB column grouping,
        // not LTR x-gap grouping (issue #221, issue-848 word collapse fix).
        let event = CharEvent {
            text_matrix: [-1.0, 0.0, 0.0, 1.0, 300.0, 720.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert!(
            !ch.upright,
            "horizontally mirrored text must be non-upright"
        );
        assert_eq!(
            ch.direction,
            TextDirection::Rtl,
            "horizontally mirrored text must have Rtl direction"
        );
    }

    // ===== Test 9: Text direction detection =====

    #[test]
    fn direction_ltr_for_normal_text() {
        let event = default_event();
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.direction, TextDirection::Ltr);
    }

    #[test]
    fn direction_rtl_for_mirrored_text() {
        let event = CharEvent {
            text_matrix: [-1.0, 0.0, 0.0, 1.0, 300.0, 720.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.direction, TextDirection::Rtl);
    }

    #[test]
    fn direction_ttb_for_downward_text() {
        // 90 degrees CW rotation: text flows top to bottom
        let event = CharEvent {
            text_matrix: [0.0, -1.0, 1.0, 0.0, 100.0, 700.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.direction, TextDirection::Ttb);
    }

    #[test]
    fn direction_btt_for_upward_text() {
        // 90 degrees CCW rotation: text flows bottom to top
        let event = CharEvent {
            text_matrix: [0.0, 1.0, -1.0, 0.0, 100.0, 100.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.direction, TextDirection::Btt);
    }

    // ===== Test 10: Unicode fallback =====

    #[test]
    fn unicode_from_event() {
        let event = CharEvent {
            unicode: Some("B".to_string()),
            char_code: 66,
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.text, "B");
    }

    #[test]
    fn unicode_fallback_to_char_code() {
        let event = CharEvent {
            unicode: None,
            char_code: 65, // valid Unicode code point for 'A'
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.text, "A"); // falls back to char::from_u32(65) = 'A'
    }

    #[test]
    fn unicode_fallback_replacement_for_invalid() {
        let event = CharEvent {
            unicode: None,
            char_code: 0xFFFFFFFF, // invalid Unicode code point
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.text, "\u{FFFD}");
    }

    // ===== Test 11: Y-flip verification =====

    #[test]
    fn y_flip_converts_to_top_left_origin() {
        // Character at bottom of page in PDF coords (y=100)
        let event = CharEvent {
            text_matrix: [1.0, 0.0, 0.0, 1.0, 72.0, 100.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // In PDF space: min_y = 97, max_y = 109
        // In top-left: top = 792-109 = 683, bottom = 792-97 = 695
        assert_approx(ch.bbox.top, 683.0, "top near page bottom");
        assert_approx(ch.bbox.bottom, 695.0, "bottom near page bottom");
        // doctop equals top for single-page
        assert_approx(ch.doctop, 683.0, "doctop");
    }

    // ===== Test 12: Colors passed through =====

    #[test]
    fn colors_passed_through() {
        let event = default_event();

        let stroking = Some(Color::Rgb(1.0, 0.0, 0.0));
        let non_stroking = Some(Color::Cmyk(0.0, 0.0, 0.0, 1.0));

        let ch = char_from_event(&event, PAGE_HEIGHT, stroking.clone(), non_stroking.clone());

        assert_eq!(ch.stroking_color, stroking);
        assert_eq!(ch.non_stroking_color, non_stroking);
    }

    // ===== Test 13: Horizontal scaling =====

    #[test]
    fn horizontal_scaling_affects_width() {
        let event = CharEvent {
            h_scaling: 0.5, // 50% horizontal scaling
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // font_matrix = [12*0.5, 0, 0, 12, 0, 0] = [6, 0, 0, 12, 0, 0]
        // Trm = [6, 0, 0, 12, 72, 720]
        // w_norm = 0.667 (no spacing change)
        // Width = 6 * 0.667 = 4.002
        assert_approx(ch.bbox.width(), 4.002, "width at 50% h_scaling");
        // Height unchanged
        assert_approx(ch.bbox.height(), 12.0, "height at 50% h_scaling");
    }

    // ===== Test 14: Default/missing font metrics =====

    #[test]
    fn default_metrics_produce_reasonable_bbox() {
        let event = default_event();
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Default ascent=750, descent=-250, width=600 (missing width)
        // displacement=667 from event, so w_norm=0.667
        // Height = (750+250)/1000 * 12 = 12.0
        assert_approx(ch.bbox.height(), 12.0, "height with default metrics");
        // Width = 12 * 0.667 = 8.004
        assert_approx(ch.bbox.width(), 8.004, "width with default metrics");
    }

    // ===== Test 15: CTM scaling =====

    #[test]
    fn ctm_scaling_affects_bbox() {
        let event = CharEvent {
            text_matrix: [1.0, 0.0, 0.0, 1.0, 36.0, 360.0],
            ctm: [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Trm = [12,0,0,12,36,360].concat([2,0,0,2,0,0]) = [24,0,0,24,72,720]
        // Same as font_size=24 test
        assert_approx(ch.bbox.width(), 16.008, "width with 2x CTM");
        assert_approx(ch.bbox.height(), 24.0, "height with 2x CTM");
    }

    // ===== Test 16: Zero font size edge case =====

    #[test]
    fn zero_font_size_does_not_panic() {
        let event = CharEvent {
            font_size: 0.0,
            ..default_event()
        };

        // Should not panic, even though bbox will be degenerate
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.size, 0.0);
    }

    // ===== Test 17: Combined spacing does NOT affect bbox width =====

    #[test]
    fn combined_spacing_does_not_affect_bbox_width() {
        let event = CharEvent {
            char_code: 32,
            unicode: Some(" ".to_string()),
            displacement: 250.0,
            char_spacing: 1.0,
            word_spacing: 2.0,
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // Both char_spacing and word_spacing are inter-glyph, not visual.
        // w_norm = 250/1000 = 0.25 (glyph width only)
        // Width = 12 * 0.25 = 3.0
        assert_approx(ch.bbox.width(), 3.0, "width unaffected by combined spacing");
    }

    #[test]
    fn mcid_and_tag_propagated_from_event() {
        let event = CharEvent {
            mcid: Some(5),
            tag: Some("P".to_string()),
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.mcid, Some(5));
        assert_eq!(ch.tag.as_deref(), Some("P"));
    }

    #[test]
    fn mcid_none_when_not_in_marked_content() {
        let event = default_event();
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);
        assert_eq!(ch.mcid, None);
        assert_eq!(ch.tag, None);
    }

    // ===== Vertical origin offset tests (WMode=1 / vertical writing) =====

    /// When a glyph has a non-zero vertical_origin (vx, vy), the bbox must be
    /// shifted by (-vx/1000, -vy/1000) in glyph-normalized space before applying
    /// the text rendering matrix. This is required for CJK vertical fonts where the
    /// vertical origin sits at (500, 880) in glyph units (a typical default).
    #[test]
    fn vertical_origin_x_offset_shifts_bbox_horizontally() {
        // vx = 500 → ox = -500/1000 = -0.5 in glyph space
        // With Trm = [12, 0, 0, 12, 72, 720] the x shift in page space = -0.5 * 12 = -6
        // So bbox.x0 should be 72 + (-6) = 66, not 72.
        let event = CharEvent {
            vertical_origin: (500.0, 0.0),
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // ox = -0.5; Trm.a = 12; shift = -6.0
        // x0 = 72 + 12*(-0.5) = 66, x1 = x0 + width
        assert_approx(ch.bbox.x0, 66.0, "x0 shifted by vx=500");
    }

    #[test]
    fn vertical_origin_y_offset_shifts_bbox_vertically() {
        // vy = 880 → oy = -880/1000 = -0.88 in glyph space
        // With Trm = [12, 0, 0, 12, 72, 720]:
        // The descent corner y in page = 720 + 12*(-0.88 + descent_norm)
        // descent_norm = -0.25, so lower_y = 720 + 12*(-0.88 - 0.25) = 720 + 12*(-1.13) = 720 - 13.56 = 706.44
        // The ascent corner y in page = 720 + 12*(-0.88 + ascent_norm)
        // ascent_norm = 0.75, so upper_y = 720 + 12*(-0.88 + 0.75) = 720 + 12*(-0.13) = 720 - 1.56 = 718.44
        // Y-flip with page_height=792: top = 792 - 718.44 = 73.56, bottom = 792 - 706.44 = 85.56
        let event = CharEvent {
            vertical_origin: (0.0, 880.0),
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // height should still be 12 (unchanged by origin offset)
        assert_approx(ch.bbox.height(), 12.0, "height unchanged with vy offset");
        // top must be shifted down relative to no-offset case (top was 63.0)
        assert!(
            ch.bbox.top > 63.0,
            "top should be shifted when vy=880: expected > 63, got {}",
            ch.bbox.top
        );
    }

    #[test]
    fn vertical_origin_zero_is_identity() {
        // When vertical_origin = (0, 0), bbox must be identical to the base case.
        let event_zero = CharEvent {
            vertical_origin: (0.0, 0.0),
            ..default_event()
        };
        let event_base = default_event();

        let ch_zero = char_from_event(&event_zero, PAGE_HEIGHT, None, None);
        let ch_base = char_from_event(&event_base, PAGE_HEIGHT, None, None);

        assert_approx(ch_zero.bbox.x0, ch_base.bbox.x0, "x0 equal when vorigin=0");
        assert_approx(
            ch_zero.bbox.top,
            ch_base.bbox.top,
            "top equal when vorigin=0",
        );
        assert_approx(ch_zero.bbox.x1, ch_base.bbox.x1, "x1 equal when vorigin=0");
        assert_approx(
            ch_zero.bbox.bottom,
            ch_base.bbox.bottom,
            "bottom equal when vorigin=0",
        );
    }

    #[test]
    fn vertical_origin_both_axes_combined() {
        // vx=500, vy=880 combined — verify x0 shifts by the vx contribution
        let event = CharEvent {
            vertical_origin: (500.0, 880.0),
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // x0 = 72 + 12*(-500/1000) = 72 - 6 = 66
        assert_approx(ch.bbox.x0, 66.0, "x0 with combined vx=500,vy=880");
        // height is unchanged (ascent-descent span = 12 units always)
        assert_approx(
            ch.bbox.height(),
            12.0,
            "height unchanged with combined origin",
        );
    }

    #[test]
    fn vertical_origin_negative_vx_shifts_right() {
        // Negative vx = -500 → ox = 500/1000 = 0.5 in glyph space → x shifts +6
        let event = CharEvent {
            vertical_origin: (-500.0, 0.0),
            ..default_event()
        };
        let ch = char_from_event(&event, PAGE_HEIGHT, None, None);

        // ox = -(-500)/1000 = 0.5; shift = 0.5 * 12 = 6.0
        // x0 = 72 + 6 = 78
        assert_approx(ch.bbox.x0, 78.0, "x0 shifted right with vx=-500");
    }
}
