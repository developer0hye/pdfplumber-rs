//! Text state machine for the content stream interpreter.
//!
//! Implements the PDF text state model: text object tracking (BT/ET),
//! font selection (Tf), text matrix (Tm) and line matrix management,
//! and text positioning operators (Td, TD, T*).

use pdfplumber_core::geometry::Ctm;

/// Text rendering mode values (Tr operator).
///
/// Determines how character glyphs are painted (filled, stroked, clipped, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextRenderMode {
    /// Fill character glyphs (default).
    #[default]
    Fill = 0,
    /// Stroke (outline) character glyphs.
    Stroke = 1,
    /// Fill and stroke character glyphs.
    FillStroke = 2,
    /// Neither fill nor stroke (invisible text).
    Invisible = 3,
    /// Fill and add to clipping path.
    FillClip = 4,
    /// Stroke and add to clipping path.
    StrokeClip = 5,
    /// Fill, stroke, and add to clipping path.
    FillStrokeClip = 6,
    /// Add to clipping path only.
    Clip = 7,
}

impl TextRenderMode {
    /// Create a TextRenderMode from an integer value (0-7).
    /// Returns None for invalid values.
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Fill),
            1 => Some(Self::Stroke),
            2 => Some(Self::FillStroke),
            3 => Some(Self::Invisible),
            4 => Some(Self::FillClip),
            5 => Some(Self::StrokeClip),
            6 => Some(Self::FillStrokeClip),
            7 => Some(Self::Clip),
            _ => None,
        }
    }
}

/// Snapshot of text state parameters that are part of the graphics state
/// and must be saved/restored by q/Q operators (PDF spec Table 52).
///
/// This does NOT include text_matrix/line_matrix (managed by BT/ET/Tm/Td)
/// or in_text_object (not part of the graphics state).
#[derive(Debug, Clone, PartialEq)]
pub struct TextStateSnapshot {
    /// Character spacing (Tc).
    pub char_spacing: f64,
    /// Word spacing (Tw).
    pub word_spacing: f64,
    /// Horizontal scaling (Tz).
    pub h_scaling: f64,
    /// Text leading (TL).
    pub leading: f64,
    /// Current font name.
    pub font_name: String,
    /// Current font size.
    pub font_size: f64,
    /// Text rendering mode (Tr).
    pub render_mode: TextRenderMode,
    /// Text rise (Ts).
    pub rise: f64,
}

/// Text state parameters tracked during content stream interpretation.
///
/// These parameters are set by text state operators (Tc, Tw, Tz, TL, Tf, Tr, Ts)
/// and persist across text objects (BT/ET blocks). They are part of the graphics
/// state and are saved/restored by q/Q.
#[derive(Debug, Clone, PartialEq)]
pub struct TextState {
    /// Character spacing (Tc operator). Extra space added after each character glyph.
    pub char_spacing: f64,
    /// Word spacing (Tw operator). Extra space added after each space character (code 32).
    pub word_spacing: f64,
    /// Horizontal scaling (Tz operator). Percentage value where 100 = normal.
    /// Stored as percentage (e.g., 100.0 for 100%).
    pub h_scaling: f64,
    /// Text leading (TL operator). Distance between baselines of consecutive text lines.
    pub leading: f64,
    /// Current font name set by Tf operator.
    pub font_name: String,
    /// Current font size set by Tf operator.
    pub font_size: f64,
    /// Text rendering mode (Tr operator).
    pub render_mode: TextRenderMode,
    /// Text rise (Ts operator). Vertical offset for superscript/subscript.
    pub rise: f64,
    /// Whether we are inside a BT/ET text object.
    in_text_object: bool,
    /// The text matrix (set by Tm, updated by Td/TD/T*/Tj/TJ).
    text_matrix: Ctm,
    /// The text line matrix (set by BT, Td, TD, T*, Tm — records the start of each line).
    line_matrix: Ctm,
}

impl Default for TextState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextState {
    /// Create a new TextState with default values per PDF spec.
    pub fn new() -> Self {
        Self {
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scaling: 100.0,
            leading: 0.0,
            font_name: String::new(),
            font_size: 0.0,
            render_mode: TextRenderMode::default(),
            rise: 0.0,
            in_text_object: false,
            text_matrix: Ctm::identity(),
            line_matrix: Ctm::identity(),
        }
    }

    /// Whether we are currently inside a BT/ET text object.
    pub fn in_text_object(&self) -> bool {
        self.in_text_object
    }

    /// Get the current text matrix.
    pub fn text_matrix(&self) -> &Ctm {
        &self.text_matrix
    }

    /// Get the current text matrix as a 6-element array.
    pub fn text_matrix_array(&self) -> [f64; 6] {
        [
            self.text_matrix.a,
            self.text_matrix.b,
            self.text_matrix.c,
            self.text_matrix.d,
            self.text_matrix.e,
            self.text_matrix.f,
        ]
    }

    /// Get the current line matrix.
    pub fn line_matrix(&self) -> &Ctm {
        &self.line_matrix
    }

    /// Get the horizontal scaling as a fraction (1.0 = 100%).
    pub fn h_scaling_normalized(&self) -> f64 {
        self.h_scaling / 100.0
    }

    // --- BT operator ---

    /// `BT` operator: begin text object.
    ///
    /// Resets the text matrix and line matrix to identity.
    /// Sets in_text_object to true.
    pub fn begin_text(&mut self) {
        self.text_matrix = Ctm::identity();
        self.line_matrix = Ctm::identity();
        self.in_text_object = true;
    }

    // --- ET operator ---

    /// `ET` operator: end text object.
    ///
    /// Sets in_text_object to false. Text matrix and line matrix
    /// become undefined (but we keep them for potential inspection).
    pub fn end_text(&mut self) {
        self.in_text_object = false;
    }

    // --- Tf operator ---

    /// `Tf` operator: set text font and size.
    pub fn set_font(&mut self, font_name: String, font_size: f64) {
        self.font_name = font_name;
        self.font_size = font_size;
    }

    // --- Tc operator ---

    /// `Tc` operator: set character spacing.
    pub fn set_char_spacing(&mut self, spacing: f64) {
        self.char_spacing = spacing;
    }

    // --- Tw operator ---

    /// `Tw` operator: set word spacing.
    pub fn set_word_spacing(&mut self, spacing: f64) {
        self.word_spacing = spacing;
    }

    // --- Tz operator ---

    /// `Tz` operator: set horizontal scaling (percentage).
    pub fn set_h_scaling(&mut self, scale: f64) {
        self.h_scaling = scale;
    }

    // --- TL operator ---

    /// `TL` operator: set text leading.
    pub fn set_leading(&mut self, leading: f64) {
        self.leading = leading;
    }

    // --- Tr operator ---

    /// `Tr` operator: set text rendering mode.
    pub fn set_render_mode(&mut self, mode: TextRenderMode) {
        self.render_mode = mode;
    }

    // --- Ts operator ---

    /// `Ts` operator: set text rise.
    pub fn set_rise(&mut self, rise: f64) {
        self.rise = rise;
    }

    // --- Tm operator ---

    /// `Tm` operator: set the text matrix and line matrix directly.
    ///
    /// Both text matrix and line matrix are set to the given matrix.
    /// This replaces (not concatenates) the current text matrix.
    pub fn set_text_matrix(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let m = Ctm::new(a, b, c, d, e, f);
        self.text_matrix = m;
        self.line_matrix = m;
    }

    // --- Td operator ---

    /// `Td` operator: move to start of next line, offset from start of current line.
    ///
    /// Translates the line matrix by (tx, ty) and sets the text matrix
    /// to the new line matrix value.
    pub fn move_text_position(&mut self, tx: f64, ty: f64) {
        let translation = Ctm::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.line_matrix = translation.concat(&self.line_matrix);
        self.text_matrix = self.line_matrix;
    }

    // --- TD operator ---

    /// `TD` operator: move to start of next line and set leading.
    ///
    /// Equivalent to: `-ty TL` then `tx ty Td`.
    /// Sets leading to `-ty` then moves text position by (tx, ty).
    pub fn move_text_position_and_set_leading(&mut self, tx: f64, ty: f64) {
        self.leading = -ty;
        self.move_text_position(tx, ty);
    }

    // --- T* operator ---

    /// `T*` operator: move to start of next line.
    ///
    /// Equivalent to `0 -TL Td` (using current leading value).
    pub fn move_to_next_line(&mut self) {
        let leading = self.leading;
        self.move_text_position(0.0, -leading);
    }

    // --- Text position advancement (for Tj/TJ) ---

    /// Advance the text matrix by a horizontal displacement.
    ///
    /// Used after rendering a character glyph to move to the next glyph position.
    /// The displacement is in text space units, already accounting for font size
    /// and horizontal scaling.
    pub fn advance_text_position(&mut self, tx: f64) {
        // Translate text matrix horizontally in text space
        let translation = Ctm::new(1.0, 0.0, 0.0, 1.0, tx, 0.0);
        self.text_matrix = translation.concat(&self.text_matrix);
    }

    // --- q/Q save/restore (graphics state portion) ---

    /// Save the text state parameters that are part of the graphics state.
    /// Called by the `q` operator. Does NOT save text_matrix/line_matrix.
    pub fn save_snapshot(&self) -> TextStateSnapshot {
        TextStateSnapshot {
            char_spacing: self.char_spacing,
            word_spacing: self.word_spacing,
            h_scaling: self.h_scaling,
            leading: self.leading,
            font_name: self.font_name.clone(),
            font_size: self.font_size,
            render_mode: self.render_mode,
            rise: self.rise,
        }
    }

    /// Restore text state parameters from a snapshot.
    /// Called by the `Q` operator. Does NOT restore text_matrix/line_matrix.
    pub fn restore_snapshot(&mut self, snapshot: TextStateSnapshot) {
        self.char_spacing = snapshot.char_spacing;
        self.word_spacing = snapshot.word_spacing;
        self.h_scaling = snapshot.h_scaling;
        self.leading = snapshot.leading;
        self.font_name = snapshot.font_name;
        self.font_size = snapshot.font_size;
        self.render_mode = snapshot.render_mode;
        self.rise = snapshot.rise;
    }
}

#[cfg(test)]
mod tests {
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
}
