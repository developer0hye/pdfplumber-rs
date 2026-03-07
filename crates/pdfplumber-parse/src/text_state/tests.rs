use super::*;

fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "expected {expected}, got {actual}"
    );
}

fn assert_matrix_approx(ctm: &Ctm, expected: [f64; 6]) {
    assert_approx(ctm.a, expected[0]);
    assert_approx(ctm.b, expected[1]);
    assert_approx(ctm.c, expected[2]);
    assert_approx(ctm.d, expected[3]);
    assert_approx(ctm.e, expected[4]);
    assert_approx(ctm.f, expected[5]);
}

// --- TextRenderMode ---

#[test]
fn test_render_mode_from_i64_valid() {
    assert_eq!(TextRenderMode::from_i64(0), Some(TextRenderMode::Fill));
    assert_eq!(TextRenderMode::from_i64(1), Some(TextRenderMode::Stroke));
    assert_eq!(
        TextRenderMode::from_i64(2),
        Some(TextRenderMode::FillStroke)
    );
    assert_eq!(TextRenderMode::from_i64(3), Some(TextRenderMode::Invisible));
    assert_eq!(TextRenderMode::from_i64(4), Some(TextRenderMode::FillClip));
    assert_eq!(
        TextRenderMode::from_i64(5),
        Some(TextRenderMode::StrokeClip)
    );
    assert_eq!(
        TextRenderMode::from_i64(6),
        Some(TextRenderMode::FillStrokeClip)
    );
    assert_eq!(TextRenderMode::from_i64(7), Some(TextRenderMode::Clip));
}

#[test]
fn test_render_mode_from_i64_invalid() {
    assert_eq!(TextRenderMode::from_i64(-1), None);
    assert_eq!(TextRenderMode::from_i64(8), None);
    assert_eq!(TextRenderMode::from_i64(100), None);
}

#[test]
fn test_render_mode_default_is_fill() {
    assert_eq!(TextRenderMode::default(), TextRenderMode::Fill);
}

// --- TextState construction and defaults ---

#[test]
fn test_new_defaults() {
    let ts = TextState::new();
    assert_eq!(ts.char_spacing, 0.0);
    assert_eq!(ts.word_spacing, 0.0);
    assert_eq!(ts.h_scaling, 100.0);
    assert_eq!(ts.leading, 0.0);
    assert_eq!(ts.font_name, "");
    assert_eq!(ts.font_size, 0.0);
    assert_eq!(ts.render_mode, TextRenderMode::Fill);
    assert_eq!(ts.rise, 0.0);
    assert!(!ts.in_text_object());
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
}

#[test]
fn test_default_equals_new() {
    assert_eq!(TextState::default(), TextState::new());
}

#[test]
fn test_h_scaling_normalized() {
    let mut ts = TextState::new();
    assert_approx(ts.h_scaling_normalized(), 1.0);

    ts.set_h_scaling(50.0);
    assert_approx(ts.h_scaling_normalized(), 0.5);

    ts.set_h_scaling(200.0);
    assert_approx(ts.h_scaling_normalized(), 2.0);
}

// --- BT/ET operators ---

#[test]
fn test_begin_text_sets_in_text_object() {
    let mut ts = TextState::new();
    assert!(!ts.in_text_object());

    ts.begin_text();
    assert!(ts.in_text_object());
}

#[test]
fn test_end_text_clears_in_text_object() {
    let mut ts = TextState::new();
    ts.begin_text();
    assert!(ts.in_text_object());

    ts.end_text();
    assert!(!ts.in_text_object());
}

#[test]
fn test_begin_text_resets_matrices_to_identity() {
    let mut ts = TextState::new();
    ts.begin_text();

    // Modify text matrix via Td
    ts.move_text_position(100.0, 200.0);
    assert_ne!(*ts.text_matrix(), Ctm::identity());

    // BT should reset both matrices
    ts.begin_text();
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
}

// --- Tf operator ---

#[test]
fn test_set_font() {
    let mut ts = TextState::new();
    ts.set_font("Helvetica".to_string(), 12.0);

    assert_eq!(ts.font_name, "Helvetica");
    assert_eq!(ts.font_size, 12.0);
}

#[test]
fn test_set_font_changes_both_name_and_size() {
    let mut ts = TextState::new();
    ts.set_font("Helvetica".to_string(), 12.0);
    ts.set_font("Times-Roman".to_string(), 14.0);

    assert_eq!(ts.font_name, "Times-Roman");
    assert_eq!(ts.font_size, 14.0);
}

// --- Tc operator ---

#[test]
fn test_set_char_spacing() {
    let mut ts = TextState::new();
    ts.set_char_spacing(0.5);
    assert_eq!(ts.char_spacing, 0.5);
}

// --- Tw operator ---

#[test]
fn test_set_word_spacing() {
    let mut ts = TextState::new();
    ts.set_word_spacing(2.0);
    assert_eq!(ts.word_spacing, 2.0);
}

// --- Tz operator ---

#[test]
fn test_set_h_scaling() {
    let mut ts = TextState::new();
    ts.set_h_scaling(150.0);
    assert_eq!(ts.h_scaling, 150.0);
}

// --- TL operator ---

#[test]
fn test_set_leading() {
    let mut ts = TextState::new();
    ts.set_leading(14.0);
    assert_eq!(ts.leading, 14.0);
}

// --- Tr operator ---

#[test]
fn test_set_render_mode() {
    let mut ts = TextState::new();
    ts.set_render_mode(TextRenderMode::Stroke);
    assert_eq!(ts.render_mode, TextRenderMode::Stroke);
}

// --- Ts operator ---

#[test]
fn test_set_rise() {
    let mut ts = TextState::new();
    ts.set_rise(5.0);
    assert_eq!(ts.rise, 5.0);
}

#[test]
fn test_set_rise_negative() {
    let mut ts = TextState::new();
    ts.set_rise(-3.0);
    assert_eq!(ts.rise, -3.0);
}

// --- Tm operator ---

#[test]
fn test_set_text_matrix() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.set_text_matrix(12.0, 0.0, 0.0, 12.0, 72.0, 720.0);

    assert_matrix_approx(ts.text_matrix(), [12.0, 0.0, 0.0, 12.0, 72.0, 720.0]);
    // Line matrix is also set to the same value
    assert_matrix_approx(ts.line_matrix(), [12.0, 0.0, 0.0, 12.0, 72.0, 720.0]);
}

#[test]
fn test_set_text_matrix_replaces_not_concatenates() {
    let mut ts = TextState::new();
    ts.begin_text();

    // Set first matrix
    ts.set_text_matrix(2.0, 0.0, 0.0, 2.0, 100.0, 200.0);

    // Set second matrix — should replace, not multiply
    ts.set_text_matrix(1.0, 0.0, 0.0, 1.0, 50.0, 60.0);

    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 50.0, 60.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 50.0, 60.0]);
}

#[test]
fn test_text_matrix_array() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.set_text_matrix(12.0, 0.0, 0.0, 12.0, 72.0, 720.0);

    assert_eq!(ts.text_matrix_array(), [12.0, 0.0, 0.0, 12.0, 72.0, 720.0]);
}

// --- Td operator ---

#[test]
fn test_move_text_position_simple() {
    let mut ts = TextState::new();
    ts.begin_text();

    ts.move_text_position(100.0, 700.0);

    // After Td, text matrix should be translated
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 100.0, 700.0]);
    // Line matrix should match text matrix
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 100.0, 700.0]);
}

#[test]
fn test_move_text_position_cumulative() {
    let mut ts = TextState::new();
    ts.begin_text();

    ts.move_text_position(100.0, 700.0);
    ts.move_text_position(0.0, -14.0);

    // Second Td adds to the line matrix (not from identity)
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 100.0, 686.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 100.0, 686.0]);
}

#[test]
fn test_move_text_position_after_tm() {
    let mut ts = TextState::new();
    ts.begin_text();

    // Set text matrix with scaling
    ts.set_text_matrix(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);

    // Td should translate relative to the current line matrix
    ts.move_text_position(50.0, 100.0);

    // Translation is pre-multiplied: [1 0 0 1 50 100] × [2 0 0 2 0 0]
    // Result: [2 0 0 2 100 200]
    assert_matrix_approx(ts.text_matrix(), [2.0, 0.0, 0.0, 2.0, 100.0, 200.0]);
    assert_matrix_approx(ts.line_matrix(), [2.0, 0.0, 0.0, 2.0, 100.0, 200.0]);
}

// --- TD operator ---

#[test]
fn test_move_text_position_and_set_leading() {
    let mut ts = TextState::new();
    ts.begin_text();

    // TD with ty = -14 should set leading to 14
    ts.move_text_position_and_set_leading(0.0, -14.0);

    assert_eq!(ts.leading, 14.0); // leading = -ty = 14
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, -14.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, -14.0]);
}

#[test]
fn test_td_sets_leading_positive_ty() {
    let mut ts = TextState::new();
    ts.begin_text();

    // TD with ty = 10 sets leading to -10
    ts.move_text_position_and_set_leading(5.0, 10.0);

    assert_eq!(ts.leading, -10.0);
}

// --- T* operator ---

#[test]
fn test_move_to_next_line() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.set_leading(14.0);

    // Position at some starting point
    ts.move_text_position(72.0, 700.0);

    // T* should move by (0, -leading) = (0, -14)
    ts.move_to_next_line();

    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 686.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 686.0]);
}

#[test]
fn test_move_to_next_line_multiple_times() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.set_leading(12.0);

    ts.move_text_position(72.0, 700.0);
    ts.move_to_next_line();
    ts.move_to_next_line();
    ts.move_to_next_line();

    // 700 - 12*3 = 664
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 664.0]);
}

#[test]
fn test_move_to_next_line_zero_leading() {
    let mut ts = TextState::new();
    ts.begin_text();
    // Default leading is 0
    ts.move_text_position(72.0, 700.0);
    ts.move_to_next_line();

    // With leading=0, T* moves by (0, 0) — no change
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 700.0]);
}

// --- Text state persistence across BT/ET ---

#[test]
fn test_text_state_params_persist_across_bt_et() {
    let mut ts = TextState::new();

    // Set parameters before text object
    ts.set_font("Helvetica".to_string(), 12.0);
    ts.set_char_spacing(0.5);
    ts.set_word_spacing(1.0);
    ts.set_h_scaling(110.0);
    ts.set_leading(14.0);
    ts.set_render_mode(TextRenderMode::Stroke);
    ts.set_rise(3.0);

    // Enter and leave a text object
    ts.begin_text();
    ts.end_text();

    // All text state parameters should persist
    assert_eq!(ts.font_name, "Helvetica");
    assert_eq!(ts.font_size, 12.0);
    assert_eq!(ts.char_spacing, 0.5);
    assert_eq!(ts.word_spacing, 1.0);
    assert_eq!(ts.h_scaling, 110.0);
    assert_eq!(ts.leading, 14.0);
    assert_eq!(ts.render_mode, TextRenderMode::Stroke);
    assert_eq!(ts.rise, 3.0);
}

#[test]
fn test_bt_resets_matrices_not_params() {
    let mut ts = TextState::new();
    ts.set_font("Helvetica".to_string(), 12.0);
    ts.set_leading(14.0);

    ts.begin_text();
    ts.set_text_matrix(12.0, 0.0, 0.0, 12.0, 72.0, 720.0);
    ts.end_text();

    // Start new text object - matrices should reset, but params stay
    ts.begin_text();
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    assert_eq!(ts.font_name, "Helvetica");
    assert_eq!(ts.font_size, 12.0);
    assert_eq!(ts.leading, 14.0);
}

// --- advance_text_position ---

#[test]
fn test_advance_text_position() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.move_text_position(72.0, 700.0);

    // Advance by 10 units horizontally
    ts.advance_text_position(10.0);

    // Text matrix should advance horizontally but line matrix stays
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 82.0, 700.0]);
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 700.0]);
}

#[test]
fn test_advance_text_position_does_not_change_line_matrix() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.move_text_position(72.0, 700.0);

    let line_matrix_before = *ts.line_matrix();
    ts.advance_text_position(50.0);

    assert_eq!(*ts.line_matrix(), line_matrix_before);
}

#[test]
fn test_advance_text_position_cumulative() {
    let mut ts = TextState::new();
    ts.begin_text();
    ts.move_text_position(72.0, 700.0);

    ts.advance_text_position(10.0);
    ts.advance_text_position(5.0);
    ts.advance_text_position(8.0);

    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 95.0, 700.0]);
}

#[test]
fn test_advance_text_position_with_scaled_matrix() {
    let mut ts = TextState::new();
    ts.begin_text();
    // Text matrix with font size scaling
    ts.set_text_matrix(12.0, 0.0, 0.0, 12.0, 72.0, 700.0);

    // Advance by 10 units in text space
    // Translation [1 0 0 1 10 0] × [12 0 0 12 72 700]
    // e' = 1*72 + 0*700 + 10*1 ... wait, let's compute:
    // Actually: pre-multiply [1 0 0 1 10 0] × [12 0 0 12 72 700]
    // new_e = e_trans * a_tm + f_trans * c_tm + e_tm = 10 * 12 + 0 * 0 + 72 = 192
    // new_f = e_trans * b_tm + f_trans * d_tm + f_tm = 10 * 0 + 0 * 12 + 700 = 700
    ts.advance_text_position(10.0);

    assert_matrix_approx(ts.text_matrix(), [12.0, 0.0, 0.0, 12.0, 192.0, 700.0]);
}

// --- Realistic sequence ---

#[test]
fn test_realistic_text_rendering_sequence() {
    let mut ts = TextState::new();

    // Set up font and leading (before BT)
    ts.set_font("Helvetica".to_string(), 12.0);
    ts.set_leading(14.0);

    // BT
    ts.begin_text();
    assert!(ts.in_text_object());

    // 72 700 Td — position at top of page
    ts.move_text_position(72.0, 700.0);
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 700.0]);

    // Simulate rendering "Hello" — advance text matrix
    ts.advance_text_position(30.0); // approximate width
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 102.0, 700.0]);

    // T* — next line
    ts.move_to_next_line();
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 686.0]);
    // Line matrix reset to start of new line
    assert_matrix_approx(ts.line_matrix(), [1.0, 0.0, 0.0, 1.0, 72.0, 686.0]);

    // Simulate rendering "World"
    ts.advance_text_position(32.0);
    assert_matrix_approx(ts.text_matrix(), [1.0, 0.0, 0.0, 1.0, 104.0, 686.0]);

    // ET
    ts.end_text();
    assert!(!ts.in_text_object());
}

#[test]
fn test_td_td_sequence_with_tm() {
    let mut ts = TextState::new();
    ts.begin_text();

    // Tm sets absolute position with scaling
    ts.set_text_matrix(10.0, 0.0, 0.0, 10.0, 100.0, 500.0);

    // Td moves relative to current line matrix
    ts.move_text_position(5.0, -12.0);
    // [1 0 0 1 5 -12] × [10 0 0 10 100 500]
    // e' = 5*10 + (-12)*0 + 100 = 150
    // f' = 5*0 + (-12)*10 + 500 = 380
    assert_matrix_approx(ts.text_matrix(), [10.0, 0.0, 0.0, 10.0, 150.0, 380.0]);
}
