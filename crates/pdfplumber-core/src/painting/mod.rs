//! Path painting operators, graphics state, and ExtGState types.
//!
//! Implements PDF path painting operators (S, s, f, F, f*, B, B*, b, b*, n)
//! that determine how constructed paths are rendered. Also provides
//! `DashPattern`, `ExtGState`, and extended `GraphicsState` for the
//! `gs` and `d` operators.

use crate::path::{Path, PathBuilder};

/// Color value from a PDF color space.
///
/// Supports the standard PDF color spaces: DeviceGray, DeviceRGB,
/// DeviceCMYK, and other (e.g., indexed, ICC-based) spaces.
///
/// `#[non_exhaustive]` — additional color spaces (Lab, CalGray, Separation,
/// DeviceN) may be added in minor releases as the color layer expands.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Color {
    /// DeviceGray: single component in [0.0, 1.0].
    Gray(f32),
    /// DeviceRGB: (r, g, b) components in [0.0, 1.0].
    Rgb(f32, f32, f32),
    /// DeviceCMYK: (c, m, y, k) components in [0.0, 1.0].
    Cmyk(f32, f32, f32, f32),
    /// Other color space (e.g., indexed, ICC-based).
    Other(Vec<f32>),
}

impl Color {
    /// Black color (gray 0).
    pub fn black() -> Self {
        Self::Gray(0.0)
    }

    /// Convert this color to an RGB triple `(r, g, b)` with components in `[0.0, 1.0]`.
    ///
    /// Returns `None` for `Color::Other` since the color space is unknown.
    pub fn to_rgb(&self) -> Option<(f32, f32, f32)> {
        match self {
            Color::Gray(g) => Some((*g, *g, *g)),
            Color::Rgb(r, g, b) => Some((*r, *g, *b)),
            Color::Cmyk(c, m, y, k) => {
                // Standard CMYK to RGB conversion
                let r = (1.0 - c) * (1.0 - k);
                let g = (1.0 - m) * (1.0 - k);
                let b = (1.0 - y) * (1.0 - k);
                Some((r, g, b))
            }
            Color::Other(_) => None,
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

/// Fill rule for path painting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FillRule {
    /// Nonzero winding number rule (default).
    #[default]
    NonZeroWinding,
    /// Even-odd rule.
    EvenOdd,
}

/// Dash pattern for stroking operations.
///
/// Corresponds to the PDF `d` operator and `/D` entry in ExtGState.
/// A solid line has an empty `dash_array` and `dash_phase` of 0.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DashPattern {
    /// Array of dash/gap lengths (alternating on/off).
    /// Empty array means a solid line.
    pub dash_array: Vec<f64>,
    /// Phase offset into the dash pattern.
    pub dash_phase: f64,
}

impl DashPattern {
    /// Create a new dash pattern.
    pub fn new(dash_array: Vec<f64>, dash_phase: f64) -> Self {
        Self {
            dash_array,
            dash_phase,
        }
    }

    /// Solid line (no dashes).
    pub fn solid() -> Self {
        Self {
            dash_array: Vec::new(),
            dash_phase: 0.0,
        }
    }

    /// Returns true if this is a solid line (no dashes).
    pub fn is_solid(&self) -> bool {
        self.dash_array.is_empty()
    }
}

impl Default for DashPattern {
    fn default() -> Self {
        Self::solid()
    }
}

/// Graphics state relevant to path painting.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphicsState {
    /// Current line width (default: 1.0 per PDF spec).
    pub line_width: f64,
    /// Current stroking color.
    pub stroke_color: Color,
    /// Current non-stroking (fill) color.
    pub fill_color: Color,
    /// Current dash pattern (default: solid line).
    pub dash_pattern: DashPattern,
    /// Stroking alpha / opacity (CA, default: 1.0 = fully opaque).
    pub stroke_alpha: f64,
    /// Non-stroking alpha / opacity (ca, default: 1.0 = fully opaque).
    pub fill_alpha: f64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            stroke_color: Color::black(),
            fill_color: Color::black(),
            dash_pattern: DashPattern::solid(),
            stroke_alpha: 1.0,
            fill_alpha: 1.0,
        }
    }
}

impl GraphicsState {
    /// Apply an `ExtGState` dictionary to this graphics state.
    ///
    /// Only fields that are `Some` in the `ExtGState` are overridden.
    pub fn apply_ext_gstate(&mut self, ext: &ExtGState) {
        if let Some(lw) = ext.line_width {
            self.line_width = lw;
        }
        if let Some(ref dp) = ext.dash_pattern {
            self.dash_pattern = dp.clone();
        }
        if let Some(ca) = ext.stroke_alpha {
            self.stroke_alpha = ca;
        }
        if let Some(ca) = ext.fill_alpha {
            self.fill_alpha = ca;
        }
    }

    /// Set the dash pattern directly (`d` operator).
    pub fn set_dash_pattern(&mut self, dash_array: Vec<f64>, dash_phase: f64) {
        self.dash_pattern = DashPattern::new(dash_array, dash_phase);
    }
}

/// Extended Graphics State parameters (from `gs` operator).
///
/// Represents the parsed contents of an ExtGState dictionary.
/// All fields are optional — only present entries override the current graphics state.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtGState {
    /// /LW — Line width override.
    pub line_width: Option<f64>,
    /// /D — Dash pattern override.
    pub dash_pattern: Option<DashPattern>,
    /// /CA — Stroking alpha (opacity).
    pub stroke_alpha: Option<f64>,
    /// /ca — Non-stroking alpha (opacity).
    pub fill_alpha: Option<f64>,
    /// /Font — Font name and size override (font_name, font_size).
    pub font: Option<(String, f64)>,
}

/// A painted path — the result of a painting operator applied to a constructed path.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaintedPath {
    /// The path segments.
    pub path: Path,
    /// Whether the path is stroked.
    pub stroke: bool,
    /// Whether the path is filled.
    pub fill: bool,
    /// Fill rule used (only meaningful when `fill` is true).
    pub fill_rule: FillRule,
    /// Line width at the time of painting.
    pub line_width: f64,
    /// Stroking color at the time of painting.
    pub stroke_color: Color,
    /// Fill color at the time of painting.
    pub fill_color: Color,
    /// Dash pattern at the time of painting.
    pub dash_pattern: DashPattern,
    /// Stroking alpha at the time of painting.
    pub stroke_alpha: f64,
    /// Non-stroking alpha at the time of painting.
    pub fill_alpha: f64,
}

impl PathBuilder {
    /// Create a `PaintedPath` capturing the current graphics state.
    fn paint(
        &mut self,
        gs: &GraphicsState,
        stroke: bool,
        fill: bool,
        fill_rule: FillRule,
    ) -> PaintedPath {
        let path = self.take_path();
        PaintedPath {
            path,
            stroke,
            fill,
            fill_rule,
            line_width: gs.line_width,
            stroke_color: gs.stroke_color.clone(),
            fill_color: gs.fill_color.clone(),
            dash_pattern: gs.dash_pattern.clone(),
            stroke_alpha: gs.stroke_alpha,
            fill_alpha: gs.fill_alpha,
        }
    }

    /// `S` operator: stroke the current path.
    ///
    /// Paints the path outline using the current stroking color and line width.
    /// Clears the current path after painting.
    pub fn stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.paint(gs, true, false, FillRule::NonZeroWinding)
    }

    /// `s` operator: close the current subpath, then stroke.
    ///
    /// Equivalent to `h S`.
    pub fn close_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.stroke(gs)
    }

    /// `f` or `F` operator: fill the current path using the nonzero winding rule.
    ///
    /// Any open subpaths are implicitly closed before filling.
    pub fn fill(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.paint(gs, false, true, FillRule::NonZeroWinding)
    }

    /// `f*` operator: fill the current path using the even-odd rule.
    pub fn fill_even_odd(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.paint(gs, false, true, FillRule::EvenOdd)
    }

    /// `B` operator: fill then stroke the current path (nonzero winding).
    pub fn fill_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.paint(gs, true, true, FillRule::NonZeroWinding)
    }

    /// `B*` operator: fill (even-odd) then stroke the current path.
    pub fn fill_even_odd_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.paint(gs, true, true, FillRule::EvenOdd)
    }

    /// `b` operator: close, fill (nonzero winding), then stroke.
    ///
    /// Equivalent to `h B`.
    pub fn close_fill_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.fill_and_stroke(gs)
    }

    /// `b*` operator: close, fill (even-odd), then stroke.
    ///
    /// Equivalent to `h B*`.
    pub fn close_fill_even_odd_and_stroke(&mut self, gs: &GraphicsState) -> PaintedPath {
        self.close_path();
        self.fill_even_odd_and_stroke(gs)
    }

    /// `n` operator: end the path without painting.
    ///
    /// Discards the current path. Used primarily for clipping paths.
    /// Returns `None` since no painted path is produced.
    pub fn end_path(&mut self) -> Option<PaintedPath> {
        self.take_path();
        None
    }

    /// Take the current path segments and reset the builder for the next path.
    fn take_path(&mut self) -> Path {
        self.take_and_reset()
    }
}

#[cfg(test)]
mod tests;
