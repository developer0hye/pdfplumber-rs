//! LaTeX sequence reconstruction from sorted [`Char`] sequences.

use pdfplumber_core::Char;

use crate::symbols::to_latex;

const SCRIPT_THRESHOLD: f64 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptPos {
    Baseline,
    Super,
    Sub,
}

/// Reconstruct LaTeX from a left-to-right sorted sequence of [`Char`]s.
pub fn reconstruct_latex(chars: &[Char]) -> String {
    if chars.is_empty() {
        return String::new();
    }
    let baseline = compute_baseline(chars);
    let classified: Vec<(ScriptPos, &Char)> = chars
        .iter()
        .map(|c| (classify_char(c, baseline), c))
        .collect();
    build_latex_string(&classified)
}

/// Compute the baseline y-coordinate from a cluster of chars.
///
/// Strategy: find the dominant vertical level using a 2-pass approach.
///
/// **Pass 1**: Identify the largest font size in the cluster — these are the
/// "body" chars and most likely to be at the true baseline.
///
/// **Pass 2**: Among chars within 20% of the largest font size, take the
/// maximum bottom value. This is the baseline.
///
/// Rationale: subscript/superscript chars are typically rendered at 60–80%
/// of the body font size. Filtering by large-font chars and then taking their
/// maximum bottom correctly identifies the baseline even in `x^{mn}` (where
/// x is larger than m,n) and `x_i` (where x is larger than i).
fn compute_baseline(chars: &[Char]) -> f64 {
    if chars.is_empty() {
        return 0.0;
    }
    let max_size = chars.iter().map(|c| c.size).fold(0.0_f64, f64::max);

    // Include chars that are within 80% of the max font size
    let size_threshold = max_size * 0.80;
    let main_chars: Vec<&Char> = chars.iter().filter(|c| c.size >= size_threshold).collect();

    // Among main-body chars, the baseline = the maximum bottom
    // (the char whose bottom sits lowest on the page = largest y in top-left coords)
    main_chars
        .iter()
        .map(|c| c.bbox.bottom)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(0.0)
}

/// In PDF top-left coords: smaller `bbox.bottom` = higher on page = superscript.
fn classify_char(c: &Char, baseline: f64) -> ScriptPos {
    let shift = baseline - c.bbox.bottom;
    if shift > SCRIPT_THRESHOLD {
        ScriptPos::Super
    } else if shift < -SCRIPT_THRESHOLD {
        ScriptPos::Sub
    } else {
        ScriptPos::Baseline
    }
}

fn build_latex_string(classified: &[(ScriptPos, &Char)]) -> String {
    if classified.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut i = 0;
    while i < classified.len() {
        let (pos, ch) = classified[i];
        match pos {
            ScriptPos::Baseline => {
                out.push_str(&to_latex(first_char(&ch.text)));
                i += 1;
            }
            ScriptPos::Super | ScriptPos::Sub => {
                let prefix = if pos == ScriptPos::Super { "^" } else { "_" };
                let mut group = String::new();
                while i < classified.len() && classified[i].0 == pos {
                    group.push_str(&to_latex(first_char(&classified[i].1.text)));
                    i += 1;
                }
                if group.chars().count() == 1 {
                    out.push_str(&format!("{prefix}{group}"));
                } else {
                    out.push_str(&format!("{prefix}{{{group}}}"));
                }
            }
        }
    }
    out
}

fn first_char(s: &str) -> char {
    s.chars().next().unwrap_or(' ')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, TextDirection};

    fn make_char(text: &str, x0: f64, bottom: f64) -> Char {
        make_char_sized(text, x0, bottom, 12.0)
    }

    fn make_char_sized(text: &str, x0: f64, bottom: f64, size: f64) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(x0, bottom - size, x0 + size * 0.6, bottom),
            fontname: "TestFont".to_string(),
            size,
            doctop: bottom - size,
            upright: true,
            direction: TextDirection::Ltr,
            stroking_color: None,
            non_stroking_color: None,
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            char_code: text.chars().next().unwrap_or(' ') as u32,
            mcid: None,
            tag: None,
        }
    }

    #[test]
    fn empty_gives_empty() {
        assert_eq!(reconstruct_latex(&[]), "");
    }

    #[test]
    fn plain_letters_passthrough() {
        let chars = vec![
            make_char("x", 0.0, 20.0),
            make_char("+", 10.0, 20.0),
            make_char("y", 20.0, 20.0),
        ];
        let latex = reconstruct_latex(&chars);
        assert!(latex.contains('x') && latex.contains('+') && latex.contains('y'));
    }

    #[test]
    fn superscript_detected() {
        // x at baseline (size 12, bottom=20); 2 is superscript (size 8, bottom=14)
        // Baseline filter: max_size=12, threshold=9.6 → x qualifies, 2 doesn't
        // Baseline = max bottom of qualifying chars = 20
        // Shift for "2": 20 - 14 = 6 > 2.5 → Super ✓
        let chars = vec![
            make_char_sized("x", 0.0, 20.0, 12.0),
            make_char_sized("2", 10.0, 14.0, 8.0), // smaller font, higher up
        ];
        let latex = reconstruct_latex(&chars);
        assert!(latex.contains('^'), "Expected ^ in: {latex}");
    }

    #[test]
    fn subscript_detected() {
        // x at baseline (size 12, bottom=20); i is subscript (size 8, bottom=26)
        // Baseline filter: max_size=12, threshold=9.6 → x qualifies, i doesn't
        // Baseline = max bottom of qualifying chars = 20
        // Shift for "i": 20 - 26 = -6 < -2.5 → Sub ✓
        let chars = vec![
            make_char_sized("x", 0.0, 20.0, 12.0),
            make_char_sized("i", 10.0, 26.0, 8.0), // smaller font, lower down
        ];
        let latex = reconstruct_latex(&chars);
        assert!(latex.contains('_'), "Expected _ in: {latex}");
    }

    #[test]
    fn greek_letter_alpha() {
        let chars = vec![make_char("α", 0.0, 20.0)];
        assert_eq!(reconstruct_latex(&chars), "\\alpha");
    }

    #[test]
    fn integral_symbol() {
        let chars = vec![make_char("∫", 0.0, 20.0)];
        assert_eq!(reconstruct_latex(&chars), "\\int");
    }

    #[test]
    fn multi_char_superscript_braced() {
        // x at body size 12 (bottom=20); m,n at script size 8 (bottom=14)
        let chars = vec![
            make_char_sized("x", 0.0, 20.0, 12.0),
            make_char_sized("m", 10.0, 14.0, 8.0),
            make_char_sized("n", 16.0, 14.0, 8.0),
        ];
        let latex = reconstruct_latex(&chars);
        assert!(latex.contains("^{"), "Expected ^{{ in: {latex}");
    }
}
