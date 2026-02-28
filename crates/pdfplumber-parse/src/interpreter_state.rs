//! Graphics state stack for the content stream interpreter.
//!
//! Implements the PDF graphics state model: a stack of states managed by
//! `q` (save) and `Q` (restore) operators, with CTM management via `cm`,
//! and color setting via G/g, RG/rg, K/k, SC/SCN/sc/scn operators.

use pdfplumber_core::geometry::Ctm;
use pdfplumber_core::painting::{Color, GraphicsState};

use crate::color_space::ResolvedColorSpace;

/// Full interpreter state that combines the CTM with the graphics state.
///
/// This is the interpreter-level state that tracks everything needed
/// during content stream processing. The `q` operator pushes a copy
/// onto the stack; `Q` restores from the stack.
#[derive(Debug, Clone)]
pub struct InterpreterState {
    /// Current transformation matrix.
    ctm: Ctm,
    /// Current graphics state (colors, line width, dash, alpha).
    graphics_state: GraphicsState,
    /// Saved state stack for q/Q operators.
    stack: Vec<SavedState>,
    /// Current stroking color space (set by CS operator).
    stroking_color_space: Option<ResolvedColorSpace>,
    /// Current non-stroking color space (set by cs operator).
    non_stroking_color_space: Option<ResolvedColorSpace>,
}

impl PartialEq for InterpreterState {
    fn eq(&self, other: &Self) -> bool {
        self.ctm == other.ctm && self.graphics_state == other.graphics_state
    }
}

/// A snapshot of the interpreter state saved by the `q` operator.
#[derive(Debug, Clone)]
struct SavedState {
    ctm: Ctm,
    graphics_state: GraphicsState,
    stroking_color_space: Option<ResolvedColorSpace>,
    non_stroking_color_space: Option<ResolvedColorSpace>,
}

impl Default for InterpreterState {
    fn default() -> Self {
        Self::new()
    }
}

impl InterpreterState {
    /// Create a new interpreter state with identity CTM and default graphics state.
    pub fn new() -> Self {
        Self {
            ctm: Ctm::identity(),
            graphics_state: GraphicsState::default(),
            stack: Vec::new(),
            stroking_color_space: None,
            non_stroking_color_space: None,
        }
    }

    /// Get the current transformation matrix.
    pub fn ctm(&self) -> &Ctm {
        &self.ctm
    }

    /// Get the current CTM as a 6-element array `[a, b, c, d, e, f]`.
    pub fn ctm_array(&self) -> [f64; 6] {
        [
            self.ctm.a, self.ctm.b, self.ctm.c, self.ctm.d, self.ctm.e, self.ctm.f,
        ]
    }

    /// Get the current graphics state.
    pub fn graphics_state(&self) -> &GraphicsState {
        &self.graphics_state
    }

    /// Get a mutable reference to the current graphics state.
    pub fn graphics_state_mut(&mut self) -> &mut GraphicsState {
        &mut self.graphics_state
    }

    /// Returns the current stack depth.
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    // --- q/Q operators ---

    /// `q` operator: save the current graphics state onto the stack.
    pub fn save_state(&mut self) {
        self.stack.push(SavedState {
            ctm: self.ctm,
            graphics_state: self.graphics_state.clone(),
            stroking_color_space: self.stroking_color_space.clone(),
            non_stroking_color_space: self.non_stroking_color_space.clone(),
        });
    }

    /// `Q` operator: restore the most recently saved graphics state.
    ///
    /// Returns `false` if the stack is empty (unbalanced Q).
    pub fn restore_state(&mut self) -> bool {
        if let Some(saved) = self.stack.pop() {
            self.ctm = saved.ctm;
            self.graphics_state = saved.graphics_state;
            self.stroking_color_space = saved.stroking_color_space;
            self.non_stroking_color_space = saved.non_stroking_color_space;
            true
        } else {
            false
        }
    }

    // --- cm operator ---

    /// `cm` operator: concatenate a matrix with the current CTM.
    ///
    /// The new matrix is pre-multiplied: CTM' = new_matrix × CTM_current.
    /// This follows the PDF spec where `cm` modifies the CTM by pre-concatenating.
    pub fn concat_matrix(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let new_matrix = Ctm::new(a, b, c, d, e, f);
        self.ctm = new_matrix.concat(&self.ctm);
    }

    // --- w operator ---

    /// `w` operator: set line width.
    pub fn set_line_width(&mut self, width: f64) {
        self.graphics_state.line_width = width;
    }

    // --- d operator ---

    /// `d` operator: set dash pattern.
    pub fn set_dash_pattern(&mut self, dash_array: Vec<f64>, dash_phase: f64) {
        self.graphics_state.set_dash_pattern(dash_array, dash_phase);
    }

    // --- Color operators ---

    /// `G` operator: set stroking color to DeviceGray.
    pub fn set_stroking_gray(&mut self, gray: f32) {
        self.stroking_color_space = None;
        self.graphics_state.stroke_color = Color::Gray(gray);
    }

    /// `g` operator: set non-stroking color to DeviceGray.
    pub fn set_non_stroking_gray(&mut self, gray: f32) {
        self.non_stroking_color_space = None;
        self.graphics_state.fill_color = Color::Gray(gray);
    }

    /// `RG` operator: set stroking color to DeviceRGB.
    pub fn set_stroking_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.stroking_color_space = None;
        self.graphics_state.stroke_color = Color::Rgb(r, g, b);
    }

    /// `rg` operator: set non-stroking color to DeviceRGB.
    pub fn set_non_stroking_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.non_stroking_color_space = None;
        self.graphics_state.fill_color = Color::Rgb(r, g, b);
    }

    /// `K` operator: set stroking color to DeviceCMYK.
    pub fn set_stroking_cmyk(&mut self, c: f32, m: f32, y: f32, k: f32) {
        self.stroking_color_space = None;
        self.graphics_state.stroke_color = Color::Cmyk(c, m, y, k);
    }

    /// `k` operator: set non-stroking color to DeviceCMYK.
    pub fn set_non_stroking_cmyk(&mut self, c: f32, m: f32, y: f32, k: f32) {
        self.non_stroking_color_space = None;
        self.graphics_state.fill_color = Color::Cmyk(c, m, y, k);
    }

    /// `SC`/`SCN` operator: set stroking color from components.
    ///
    /// If a stroking color space has been set (via CS), uses it to resolve
    /// the color. Otherwise falls back to inferring from component count.
    pub fn set_stroking_color(&mut self, components: &[f32]) {
        self.graphics_state.stroke_color = if let Some(ref cs) = self.stroking_color_space {
            cs.resolve_color(components)
        } else {
            color_from_components(components)
        };
    }

    /// `sc`/`scn` operator: set non-stroking color from components.
    ///
    /// If a non-stroking color space has been set (via cs), uses it to resolve
    /// the color. Otherwise falls back to inferring from component count.
    pub fn set_non_stroking_color(&mut self, components: &[f32]) {
        self.graphics_state.fill_color = if let Some(ref cs) = self.non_stroking_color_space {
            cs.resolve_color(components)
        } else {
            color_from_components(components)
        };
    }

    /// `CS` operator: set the stroking color space.
    pub fn set_stroking_color_space(&mut self, cs: ResolvedColorSpace) {
        self.stroking_color_space = Some(cs);
    }

    /// `cs` operator: set the non-stroking color space.
    pub fn set_non_stroking_color_space(&mut self, cs: ResolvedColorSpace) {
        self.non_stroking_color_space = Some(cs);
    }
}

/// Convert a slice of color components to a `Color` value.
fn color_from_components(components: &[f32]) -> Color {
    match components.len() {
        1 => Color::Gray(components[0]),
        3 => Color::Rgb(components[0], components[1], components[2]),
        4 => Color::Cmyk(components[0], components[1], components[2], components[3]),
        _ => Color::Other(components.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::geometry::Point;
    use pdfplumber_core::painting::DashPattern;

    // --- Construction and defaults ---

    #[test]
    fn test_new_has_identity_ctm() {
        let state = InterpreterState::new();
        assert_eq!(*state.ctm(), Ctm::identity());
    }

    #[test]
    fn test_new_has_default_graphics_state() {
        let state = InterpreterState::new();
        let gs = state.graphics_state();
        assert_eq!(gs.line_width, 1.0);
        assert_eq!(gs.stroke_color, Color::black());
        assert_eq!(gs.fill_color, Color::black());
        assert!(gs.dash_pattern.is_solid());
        assert_eq!(gs.stroke_alpha, 1.0);
        assert_eq!(gs.fill_alpha, 1.0);
    }

    #[test]
    fn test_new_has_empty_stack() {
        let state = InterpreterState::new();
        assert_eq!(state.stack_depth(), 0);
    }

    #[test]
    fn test_default_equals_new() {
        assert_eq!(InterpreterState::default(), InterpreterState::new());
    }

    #[test]
    fn test_ctm_array() {
        let state = InterpreterState::new();
        assert_eq!(state.ctm_array(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    // --- q/Q: push/pop state ---

    #[test]
    fn test_save_state_increments_depth() {
        let mut state = InterpreterState::new();
        state.save_state();
        assert_eq!(state.stack_depth(), 1);
        state.save_state();
        assert_eq!(state.stack_depth(), 2);
    }

    #[test]
    fn test_restore_state_decrements_depth() {
        let mut state = InterpreterState::new();
        state.save_state();
        state.save_state();
        assert_eq!(state.stack_depth(), 2);

        assert!(state.restore_state());
        assert_eq!(state.stack_depth(), 1);

        assert!(state.restore_state());
        assert_eq!(state.stack_depth(), 0);
    }

    #[test]
    fn test_restore_on_empty_stack_returns_false() {
        let mut state = InterpreterState::new();
        assert!(!state.restore_state());
    }

    #[test]
    fn test_save_restore_preserves_ctm() {
        let mut state = InterpreterState::new();

        // Save, then modify CTM
        state.save_state();
        state.concat_matrix(2.0, 0.0, 0.0, 2.0, 10.0, 20.0);
        assert_ne!(*state.ctm(), Ctm::identity());

        // Restore: CTM should be back to identity
        state.restore_state();
        assert_eq!(*state.ctm(), Ctm::identity());
    }

    #[test]
    fn test_save_restore_preserves_graphics_state() {
        let mut state = InterpreterState::new();

        // Save, then modify state
        state.save_state();
        state.set_line_width(5.0);
        state.set_stroking_rgb(1.0, 0.0, 0.0);
        state.set_non_stroking_gray(0.5);
        state.set_dash_pattern(vec![3.0, 2.0], 1.0);

        // Verify changes took effect
        assert_eq!(state.graphics_state().line_width, 5.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
        assert_eq!(state.graphics_state().fill_color, Color::Gray(0.5));

        // Restore: all should be back to defaults
        state.restore_state();
        assert_eq!(state.graphics_state().line_width, 1.0);
        assert_eq!(state.graphics_state().stroke_color, Color::black());
        assert_eq!(state.graphics_state().fill_color, Color::black());
        assert!(state.graphics_state().dash_pattern.is_solid());
    }

    #[test]
    fn test_nested_save_restore() {
        let mut state = InterpreterState::new();

        // Level 0: set red stroke
        state.set_stroking_rgb(1.0, 0.0, 0.0);

        // Save level 0
        state.save_state();

        // Level 1: set blue stroke
        state.set_stroking_rgb(0.0, 0.0, 1.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(0.0, 0.0, 1.0)
        );

        // Save level 1
        state.save_state();

        // Level 2: set green stroke
        state.set_stroking_rgb(0.0, 1.0, 0.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(0.0, 1.0, 0.0)
        );

        // Restore to level 1: blue
        state.restore_state();
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(0.0, 0.0, 1.0)
        );

        // Restore to level 0: red
        state.restore_state();
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
    }

    // --- cm: CTM multiplication ---

    #[test]
    fn test_concat_matrix_translation() {
        let mut state = InterpreterState::new();
        state.concat_matrix(1.0, 0.0, 0.0, 1.0, 100.0, 200.0);

        let p = state.ctm().transform_point(Point::new(0.0, 0.0));
        assert_approx(p.x, 100.0);
        assert_approx(p.y, 200.0);
    }

    #[test]
    fn test_concat_matrix_scaling() {
        let mut state = InterpreterState::new();
        state.concat_matrix(2.0, 0.0, 0.0, 3.0, 0.0, 0.0);

        let p = state.ctm().transform_point(Point::new(5.0, 10.0));
        assert_approx(p.x, 10.0);
        assert_approx(p.y, 30.0);
    }

    #[test]
    fn test_concat_matrix_cumulative() {
        let mut state = InterpreterState::new();

        // First: scale by 2x
        state.concat_matrix(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        // Second: translate by (10, 20) — in the scaled coordinate system
        state.concat_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 20.0);

        // Point (0,0) in user space:
        // After translate: (10, 20) in intermediate space
        // After scale: (20, 40) in device space
        let p = state.ctm().transform_point(Point::new(0.0, 0.0));
        assert_approx(p.x, 20.0);
        assert_approx(p.y, 40.0);
    }

    #[test]
    fn test_concat_identity_no_change() {
        let mut state = InterpreterState::new();
        state.concat_matrix(2.0, 0.0, 0.0, 3.0, 10.0, 20.0);
        let ctm_before = *state.ctm();

        // Concatenate identity — no change
        state.concat_matrix(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        assert_eq!(*state.ctm(), ctm_before);
    }

    #[test]
    fn test_ctm_array_after_concat() {
        let mut state = InterpreterState::new();
        state.concat_matrix(2.0, 0.0, 0.0, 3.0, 10.0, 20.0);
        assert_eq!(state.ctm_array(), [2.0, 0.0, 0.0, 3.0, 10.0, 20.0]);
    }

    // --- w: line width ---

    #[test]
    fn test_set_line_width() {
        let mut state = InterpreterState::new();
        state.set_line_width(3.5);
        assert_eq!(state.graphics_state().line_width, 3.5);
    }

    #[test]
    fn test_set_line_width_zero() {
        let mut state = InterpreterState::new();
        state.set_line_width(0.0);
        assert_eq!(state.graphics_state().line_width, 0.0);
    }

    // --- d: dash pattern ---

    #[test]
    fn test_set_dash_pattern() {
        let mut state = InterpreterState::new();
        state.set_dash_pattern(vec![3.0, 2.0], 1.0);

        let dp = &state.graphics_state().dash_pattern;
        assert_eq!(dp.dash_array, vec![3.0, 2.0]);
        assert_eq!(dp.dash_phase, 1.0);
        assert!(!dp.is_solid());
    }

    #[test]
    fn test_set_dash_pattern_solid() {
        let mut state = InterpreterState::new();
        state.set_dash_pattern(vec![3.0, 2.0], 0.0);
        assert!(!state.graphics_state().dash_pattern.is_solid());

        state.set_dash_pattern(vec![], 0.0);
        assert!(state.graphics_state().dash_pattern.is_solid());
    }

    // --- G/g: DeviceGray color ---

    #[test]
    fn test_set_stroking_gray() {
        let mut state = InterpreterState::new();
        state.set_stroking_gray(0.5);
        assert_eq!(state.graphics_state().stroke_color, Color::Gray(0.5));
    }

    #[test]
    fn test_set_non_stroking_gray() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_gray(0.75);
        assert_eq!(state.graphics_state().fill_color, Color::Gray(0.75));
    }

    // --- RG/rg: DeviceRGB color ---

    #[test]
    fn test_set_stroking_rgb() {
        let mut state = InterpreterState::new();
        state.set_stroking_rgb(1.0, 0.0, 0.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn test_set_non_stroking_rgb() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_rgb(0.0, 1.0, 0.0);
        assert_eq!(state.graphics_state().fill_color, Color::Rgb(0.0, 1.0, 0.0));
    }

    // --- K/k: DeviceCMYK color ---

    #[test]
    fn test_set_stroking_cmyk() {
        let mut state = InterpreterState::new();
        state.set_stroking_cmyk(0.1, 0.2, 0.3, 0.4);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Cmyk(0.1, 0.2, 0.3, 0.4)
        );
    }

    #[test]
    fn test_set_non_stroking_cmyk() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_cmyk(0.5, 0.6, 0.7, 0.8);
        assert_eq!(
            state.graphics_state().fill_color,
            Color::Cmyk(0.5, 0.6, 0.7, 0.8)
        );
    }

    // --- SC/SCN/sc/scn: generic color operators ---

    #[test]
    fn test_set_stroking_color_1_component_is_gray() {
        let mut state = InterpreterState::new();
        state.set_stroking_color(&[0.5]);
        assert_eq!(state.graphics_state().stroke_color, Color::Gray(0.5));
    }

    #[test]
    fn test_set_stroking_color_3_components_is_rgb() {
        let mut state = InterpreterState::new();
        state.set_stroking_color(&[1.0, 0.0, 0.0]);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn test_set_stroking_color_4_components_is_cmyk() {
        let mut state = InterpreterState::new();
        state.set_stroking_color(&[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Cmyk(0.1, 0.2, 0.3, 0.4)
        );
    }

    #[test]
    fn test_set_stroking_color_other_component_count() {
        let mut state = InterpreterState::new();
        state.set_stroking_color(&[0.1, 0.2]);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Other(vec![0.1, 0.2])
        );
    }

    #[test]
    fn test_set_non_stroking_color_1_component() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_color(&[0.3]);
        assert_eq!(state.graphics_state().fill_color, Color::Gray(0.3));
    }

    #[test]
    fn test_set_non_stroking_color_3_components() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_color(&[0.0, 0.0, 1.0]);
        assert_eq!(state.graphics_state().fill_color, Color::Rgb(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_set_non_stroking_color_5_components_is_other() {
        let mut state = InterpreterState::new();
        state.set_non_stroking_color(&[0.1, 0.2, 0.3, 0.4, 0.5]);
        assert_eq!(
            state.graphics_state().fill_color,
            Color::Other(vec![0.1, 0.2, 0.3, 0.4, 0.5])
        );
    }

    // --- Color state independence ---

    #[test]
    fn test_stroking_and_non_stroking_independent() {
        let mut state = InterpreterState::new();
        state.set_stroking_rgb(1.0, 0.0, 0.0);
        state.set_non_stroking_rgb(0.0, 0.0, 1.0);

        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
        assert_eq!(state.graphics_state().fill_color, Color::Rgb(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_color_changes_across_color_spaces() {
        let mut state = InterpreterState::new();

        // Start gray
        state.set_stroking_gray(0.5);
        assert_eq!(state.graphics_state().stroke_color, Color::Gray(0.5));

        // Switch to RGB
        state.set_stroking_rgb(1.0, 0.0, 0.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );

        // Switch to CMYK
        state.set_stroking_cmyk(0.0, 1.0, 0.0, 0.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Cmyk(0.0, 1.0, 0.0, 0.0)
        );
    }

    // --- Combined q/Q with all state changes ---

    #[test]
    fn test_full_state_save_restore_cycle() {
        let mut state = InterpreterState::new();

        // Set up initial state
        state.concat_matrix(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        state.set_line_width(2.0);
        state.set_stroking_rgb(1.0, 0.0, 0.0);
        state.set_non_stroking_gray(0.5);
        state.set_dash_pattern(vec![5.0, 3.0], 0.0);

        // Save (q)
        state.save_state();

        // Modify everything
        state.concat_matrix(1.0, 0.0, 0.0, 1.0, 50.0, 50.0);
        state.set_line_width(0.5);
        state.set_stroking_cmyk(0.0, 0.0, 0.0, 1.0);
        state.set_non_stroking_rgb(0.0, 1.0, 0.0);
        state.set_dash_pattern(vec![], 0.0);

        // Verify modifications
        assert_eq!(state.graphics_state().line_width, 0.5);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Cmyk(0.0, 0.0, 0.0, 1.0)
        );
        assert!(state.graphics_state().dash_pattern.is_solid());

        // Restore (Q) — should revert to pre-save state
        state.restore_state();

        // Check CTM was restored (scale 2x only)
        assert_eq!(state.ctm_array(), [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);

        // Check graphics state was restored
        assert_eq!(state.graphics_state().line_width, 2.0);
        assert_eq!(
            state.graphics_state().stroke_color,
            Color::Rgb(1.0, 0.0, 0.0)
        );
        assert_eq!(state.graphics_state().fill_color, Color::Gray(0.5));
        assert_eq!(
            state.graphics_state().dash_pattern,
            DashPattern::new(vec![5.0, 3.0], 0.0)
        );
    }

    #[test]
    fn test_multiple_unbalanced_restores_return_false() {
        let mut state = InterpreterState::new();
        state.save_state();

        assert!(state.restore_state());
        assert!(!state.restore_state()); // empty stack
        assert!(!state.restore_state()); // still empty
    }

    #[test]
    fn test_graphics_state_mut_access() {
        let mut state = InterpreterState::new();
        state.graphics_state_mut().stroke_alpha = 0.5;
        assert_eq!(state.graphics_state().stroke_alpha, 0.5);
    }

    // --- Helper ---

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-10,
            "expected {expected}, got {actual}"
        );
    }
}
