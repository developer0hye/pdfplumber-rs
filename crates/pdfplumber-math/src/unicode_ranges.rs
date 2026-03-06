//! Unicode range classification for mathematical characters.
//!
//! Each function tests whether a Unicode codepoint belongs to a mathematical
//! Unicode block. These ranges are taken directly from the Unicode standard.

/// True if `c` is in any mathematical Unicode block.
///
/// Covers:
/// - Mathematical Operators (U+2200–U+22FF)
/// - Supplemental Mathematical Operators (U+2A00–U+2AFF)
/// - Mathematical Alphanumeric Symbols (U+1D400–U+1D7FF)
/// - Letterlike Symbols (U+2100–U+214F)
/// - Number Forms (U+2150–U+218F)
/// - Arrows (U+2190–U+21FF, mathematical subset)
/// - Greek and Coptic (U+0370–U+03FF)
/// - Miscellaneous Mathematical Symbols A/B
/// - Geometric Shapes (subset used in math)
pub fn is_math_char(c: char) -> bool {
    let cp = c as u32;
    is_math_operator(cp)
        || is_supplemental_math_operator(cp)
        || is_math_alphanumeric(cp)
        || is_letterlike(cp)
        || is_number_form(cp)
        || is_math_arrow(cp)
        || is_supplemental_arrows_a(cp)
        || is_supplemental_arrows_b(cp)
        || is_greek(cp)
        || is_misc_math_a(cp)
        || is_misc_math_b(cp)
        || is_common_math_ascii(c)
}

/// U+2200–U+22FF: Mathematical Operators
fn is_math_operator(cp: u32) -> bool {
    (0x2200..=0x22FF).contains(&cp)
}

/// U+2A00–U+2AFF: Supplemental Mathematical Operators
fn is_supplemental_math_operator(cp: u32) -> bool {
    (0x2A00..=0x2AFF).contains(&cp)
}

/// U+1D400–U+1D7FF: Mathematical Alphanumeric Symbols
fn is_math_alphanumeric(cp: u32) -> bool {
    (0x1D400..=0x1D7FF).contains(&cp)
}

/// U+2100–U+214F: Letterlike Symbols (ℝ, ℤ, ℕ, ∂, ℏ, etc.)
fn is_letterlike(cp: u32) -> bool {
    (0x2100..=0x214F).contains(&cp)
}

/// U+2150–U+218F: Number Forms (vulgar fractions, etc.)
fn is_number_form(cp: u32) -> bool {
    (0x2150..=0x218F).contains(&cp)
}

/// U+2190–U+21FF: Arrows (math subset: maps-to, long arrows, etc.)
fn is_math_arrow(cp: u32) -> bool {
    (0x2190..=0x21FF).contains(&cp)
}

/// U+27F0–U+27FF: Supplemental Arrows-A (long arrows, etc.)
fn is_supplemental_arrows_a(cp: u32) -> bool {
    (0x27F0..=0x27FF).contains(&cp)
}

/// U+2900–U+297F: Supplemental Arrows-B
fn is_supplemental_arrows_b(cp: u32) -> bool {
    (0x2900..=0x297F).contains(&cp)
}

/// U+0370–U+03FF: Greek and Coptic
pub fn is_greek(cp: u32) -> bool {
    (0x0370..=0x03FF).contains(&cp)
}

/// U+27C0–U+27EF: Miscellaneous Mathematical Symbols-A
fn is_misc_math_a(cp: u32) -> bool {
    (0x27C0..=0x27EF).contains(&cp)
}

/// U+2980–U+29FF: Miscellaneous Mathematical Symbols-B
fn is_misc_math_b(cp: u32) -> bool {
    (0x2980..=0x29FF).contains(&cp)
}

/// Common ASCII chars that are almost exclusively mathematical in context.
///
/// We don't flag every ASCII digit/letter, only the operator-like ones.
fn is_common_math_ascii(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '=' | '<' | '>' | '/' | '|' | '^' | '_' | '~' | '±'
    )
}

/// True if the given string contains ≥ 1 math character.
pub fn contains_math(s: &str) -> bool {
    s.chars().any(is_math_char)
}

/// Count of math characters in `s`.
pub fn math_char_count(s: &str) -> usize {
    s.chars().filter(|&c| is_math_char(c)).count()
}

/// Fraction of characters in `s` that are mathematical.
///
/// Returns 0.0 for empty strings.
pub fn math_density(s: &str) -> f64 {
    let total: usize = s.chars().count();
    if total == 0 {
        return 0.0;
    }
    math_char_count(s) as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greek_lowercase_alpha() {
        assert!(is_math_char('α'));
        assert!(is_math_char('β'));
        assert!(is_math_char('γ'));
        assert!(is_math_char('π'));
        assert!(is_math_char('σ'));
        assert!(is_math_char('ω'));
    }

    #[test]
    fn greek_uppercase() {
        assert!(is_math_char('Σ'));
        assert!(is_math_char('Δ'));
        assert!(is_math_char('Γ'));
        assert!(is_math_char('Ω'));
    }

    #[test]
    fn math_operators() {
        assert!(is_math_char('∀')); // U+2200 FOR ALL
        assert!(is_math_char('∃')); // U+2203 THERE EXISTS
        assert!(is_math_char('∈')); // U+2208 ELEMENT OF
        assert!(is_math_char('∑')); // U+2211 N-ARY SUMMATION
        assert!(is_math_char('∫')); // U+222B INTEGRAL
        assert!(is_math_char('∞')); // U+221E INFINITY
        assert!(is_math_char('≤')); // U+2264 LESS-THAN OR EQUAL TO
        assert!(is_math_char('≥')); // U+2265 GREATER-THAN OR EQUAL TO
        assert!(is_math_char('≠')); // U+2260 NOT EQUAL TO
        assert!(is_math_char('≈')); // U+2248 ALMOST EQUAL TO
    }

    #[test]
    fn arrows() {
        assert!(is_math_char('→')); // U+2192
        assert!(is_math_char('←')); // U+2190
        assert!(is_math_char('⟹')); // U+27F9
    }

    #[test]
    fn letterlike() {
        assert!(is_math_char('ℝ')); // U+211D DOUBLE-STRUCK CAPITAL R
        assert!(is_math_char('ℤ')); // U+2124 DOUBLE-STRUCK CAPITAL Z
        assert!(is_math_char('ℕ')); // U+2115 DOUBLE-STRUCK CAPITAL N
        assert!(is_math_char('∂')); // U+2202 PARTIAL DIFFERENTIAL
    }

    #[test]
    fn common_math_ascii() {
        assert!(is_math_char('+'));
        assert!(is_math_char('-'));
        assert!(is_math_char('='));
        assert!(is_math_char('^'));
        assert!(is_math_char('_'));
    }

    #[test]
    fn non_math_chars() {
        assert!(!is_math_char('a'));
        assert!(!is_math_char('Z'));
        assert!(!is_math_char('0'));
        assert!(!is_math_char('.'));
        assert!(!is_math_char(','));
        assert!(!is_math_char('!'));
    }

    #[test]
    fn math_density_pure_math() {
        let s = "α+β=γ";
        assert!(math_density(s) > 0.5);
    }

    #[test]
    fn math_density_empty() {
        assert_eq!(math_density(""), 0.0);
    }

    #[test]
    fn contains_math_true() {
        assert!(contains_math("Let x ∈ ℝ"));
        assert!(contains_math("∑αᵢ"));
    }

    #[test]
    fn contains_math_false() {
        assert!(!contains_math("This is plain text."));
        assert!(!contains_math("Hello world 123"));
    }
}
