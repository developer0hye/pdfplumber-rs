//! Color conversion utilities for the rasterizer.
//!
//! Converts pdfplumber-core [`Color`] variants to tiny-skia's [`ColorU8`] RGBA.

use pdfplumber_core::Color;
use tiny_skia::ColorU8;

/// Convert a pdfplumber [`Color`] to tiny-skia [`ColorU8`] with full opacity.
///
/// Gray, RGB, and CMYK are handled. `Color::Other` falls back to black.
/// The returned value uses pre-multiplied alpha (alpha = 255, RGB components
/// passed through unchanged — tiny-skia expects pre-multiplied but since
/// alpha is 255 no scaling is needed).
pub fn to_skia_color(c: &Color) -> ColorU8 {
    match c {
        Color::Gray(g) => {
            let v = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
            ColorU8::from_rgba(v, v, v, 255)
        }
        Color::Rgb(r, g, b) => ColorU8::from_rgba(
            (r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (b.clamp(0.0, 1.0) * 255.0).round() as u8,
            255,
        ),
        Color::Cmyk(c, m, y, k) => {
            // CMYK → RGB conversion per ICC spec.
            let r = 1.0 - (c + k).min(1.0);
            let g = 1.0 - (m + k).min(1.0);
            let b = 1.0 - (y + k).min(1.0);
            ColorU8::from_rgba(
                (r.clamp(0.0, 1.0) * 255.0).round() as u8,
                (g.clamp(0.0, 1.0) * 255.0).round() as u8,
                (b.clamp(0.0, 1.0) * 255.0).round() as u8,
                255,
            )
        }
        Color::Other(_) => ColorU8::from_rgba(0, 0, 0, 255),
    }
}

/// Convert a pdfplumber [`Color`] to a tiny-skia [`tiny_skia::Color`].
///
/// Returns an RGBA color with alpha 1.0 (fully opaque).
pub fn to_skia_color_f(c: &Color) -> tiny_skia::Color {
    let cu = to_skia_color(c);
    tiny_skia::Color::from_rgba8(cu.red(), cu.green(), cu.blue(), cu.alpha())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_0_is_black() {
        let c = to_skia_color(&Color::Gray(0.0));
        assert_eq!((c.red(), c.green(), c.blue(), c.alpha()), (0, 0, 0, 255));
    }

    #[test]
    fn gray_1_is_white() {
        let c = to_skia_color(&Color::Gray(1.0));
        assert_eq!(
            (c.red(), c.green(), c.blue(), c.alpha()),
            (255, 255, 255, 255)
        );
    }

    #[test]
    fn rgb_red() {
        let c = to_skia_color(&Color::Rgb(1.0, 0.0, 0.0));
        assert_eq!((c.red(), c.green(), c.blue()), (255, 0, 0));
    }

    #[test]
    fn cmyk_pure_black() {
        // K=1 → all channels zero → black.
        let c = to_skia_color(&Color::Cmyk(0.0, 0.0, 0.0, 1.0));
        assert_eq!((c.red(), c.green(), c.blue()), (0, 0, 0));
    }

    #[test]
    fn cmyk_pure_white() {
        let c = to_skia_color(&Color::Cmyk(0.0, 0.0, 0.0, 0.0));
        assert_eq!((c.red(), c.green(), c.blue()), (255, 255, 255));
    }

    #[test]
    fn other_fallback_is_black() {
        let c = to_skia_color(&Color::Other(vec![]));
        assert_eq!((c.red(), c.green(), c.blue()), (0, 0, 0));
    }
}
