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

    /// Advance the text matrix by a vertical displacement (for WMode=1).
    ///
    /// Used for CJK vertical writing mode where text advances downward.
    /// The displacement `ty` is in text space units (typically negative for
    /// downward advance in PDF bottom-left coordinates).
    pub fn advance_text_position_vertical(&mut self, ty: f64) {
        let translation = Ctm::new(1.0, 0.0, 0.0, 1.0, 0.0, ty);
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
mod tests;
