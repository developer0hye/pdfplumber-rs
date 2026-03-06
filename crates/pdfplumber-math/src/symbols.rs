//! Unicode → LaTeX symbol mapping table.
//!
//! Covers the most common mathematical symbols found in academic papers.
//! Entries are hand-curated from the Unicode math blocks and LaTeX standard
//! symbol tables (AMSmath, amssymb, stmaryrd).
//!
//! The mapping is a static lookup: given a Unicode character, return the
//! canonical LaTeX command string (without backslash, for ease of use).
//! The caller adds `\` when rendering.

/// Look up the LaTeX command for a Unicode character.
///
/// Returns `None` if the character has no known math mapping.
/// The returned string does NOT include a leading backslash.
///
/// # Examples
///
/// ```
/// use pdfplumber_math::symbols::latex_for;
/// assert_eq!(latex_for('∫'), Some("int"));
/// assert_eq!(latex_for('α'), Some("alpha"));
/// assert_eq!(latex_for('a'), None); // plain ASCII — no mapping
/// ```
pub fn latex_for(c: char) -> Option<&'static str> {
    // Fast path: ASCII passthrough for digits and letters
    if c.is_ascii_alphanumeric() {
        return None;
    }

    SYMBOL_TABLE
        .iter()
        .find(|(ch, _)| *ch == c)
        .map(|(_, s)| *s)
}

/// True if we have a LaTeX mapping for this character.
pub fn has_mapping(c: char) -> bool {
    latex_for(c).is_some()
}

/// Convert a char to its LaTeX representation.
///
/// For ASCII alphanumerics: returns the char as a string.
/// For known math symbols: returns `\cmd`.
/// For unknown non-ASCII: returns the char as a UTF-8 string (best effort).
pub fn to_latex(c: char) -> String {
    if c.is_ascii_alphanumeric() {
        return c.to_string();
    }
    if c == ' ' {
        return " ".to_string();
    }
    match latex_for(c) {
        Some(cmd) => format!("\\{cmd}"),
        None => c.to_string(),
    }
}

// (char, latex_command_without_backslash)
// Sorted by Unicode codepoint within each block for readability.
static SYMBOL_TABLE: &[(char, &str)] = &[
    // ── ASCII operators (treated as symbols in math mode) ─────────────────
    ('+', "+"),
    ('-', "-"),
    ('=', "="),
    ('<', "<"),
    ('>', ">"),
    ('/', "/"),
    ('|', "|"),
    ('^', "^"),
    ('_', "_"),
    ('~', "sim"),
    ('±', "pm"),
    // ── Greek lowercase (U+03B1–U+03C9) ──────────────────────────────────
    ('α', "alpha"),
    ('β', "beta"),
    ('γ', "gamma"),
    ('δ', "delta"),
    ('ε', "epsilon"),
    ('ζ', "zeta"),
    ('η', "eta"),
    ('θ', "theta"),
    ('ι', "iota"),
    ('κ', "kappa"),
    ('λ', "lambda"),
    ('μ', "mu"),
    ('ν', "nu"),
    ('ξ', "xi"),
    ('π', "pi"),
    ('ρ', "rho"),
    ('σ', "sigma"),
    ('τ', "tau"),
    ('υ', "upsilon"),
    ('φ', "phi"),
    ('χ', "chi"),
    ('ψ', "psi"),
    ('ω', "omega"),
    // ── Greek uppercase ───────────────────────────────────────────────────
    ('Γ', "Gamma"),
    ('Δ', "Delta"),
    ('Θ', "Theta"),
    ('Λ', "Lambda"),
    ('Ξ', "Xi"),
    ('Π', "Pi"),
    ('Σ', "Sigma"),
    ('Υ', "Upsilon"),
    ('Φ', "Phi"),
    ('Ψ', "Psi"),
    ('Ω', "Omega"),
    // ── Greek variants ────────────────────────────────────────────────────
    ('ϕ', "varphi"),
    ('ϵ', "varepsilon"),
    ('ϑ', "vartheta"),
    ('ϱ', "varrho"),
    ('ϖ', "varpi"),
    ('ς', "varsigma"),
    // ── Letterlike Symbols (U+2100–U+214F) ───────────────────────────────
    ('ℵ', "aleph"),
    ('ℶ', "beth"),
    ('ℷ', "gimel"),
    ('ℸ', "daleth"),
    ('ℏ', "hbar"),
    ('ℑ', "Im"),
    ('ℜ', "Re"),
    ('℘', "wp"),
    ('ℕ', "mathbb{N}"),
    ('ℤ', "mathbb{Z}"),
    ('ℚ', "mathbb{Q}"),
    ('ℝ', "mathbb{R}"),
    ('ℂ', "mathbb{C}"),
    ('ℓ', "ell"),
    ('∂', "partial"),
    ('ı', "imath"),
    // ── Mathematical Operators (U+2200–U+22FF) ───────────────────────────
    ('∀', "forall"),
    ('∁', "complement"),
    ('∂', "partial"),
    ('∃', "exists"),
    ('∄', "nexists"),
    ('∅', "emptyset"),
    ('∆', "triangle"),
    ('∇', "nabla"),
    ('∈', "in"),
    ('∉', "notin"),
    ('∊', "in"),
    ('∋', "ni"),
    ('∌', "nni"),
    ('∏', "prod"),
    ('∐', "coprod"),
    ('∑', "sum"),
    ('−', "-"),
    ('∓', "mp"),
    ('∔', "dotplus"),
    ('∗', "ast"),
    ('∘', "circ"),
    ('∙', "bullet"),
    ('√', "sqrt"),
    ('∛', "sqrt[3]"),
    ('∜', "sqrt[4]"),
    ('∝', "propto"),
    ('∞', "infty"),
    ('∠', "angle"),
    ('∡', "measuredangle"),
    ('∢', "sphericalangle"),
    ('∣', "mid"),
    ('∤', "nmid"),
    ('∥', "parallel"),
    ('∦', "nparallel"),
    ('∧', "wedge"),
    ('∨', "vee"),
    ('∩', "cap"),
    ('∪', "cup"),
    ('∫', "int"),
    ('∬', "iint"),
    ('∭', "iiint"),
    ('∮', "oint"),
    ('∯', "oiint"),
    ('∰', "oiiint"),
    ('∴', "therefore"),
    ('∵', "because"),
    ('∶', ":"),
    ('∷', "::"),
    ('∸', "dotminus"),
    ('∼', "sim"),
    ('∽', "backsim"),
    ('≀', "wr"),
    ('≁', "nsim"),
    ('≂', "eqsim"),
    ('≃', "simeq"),
    ('≄', "nsimeq"),
    ('≅', "cong"),
    ('≆', "ncong"),
    ('≇', "ncong"),
    ('≈', "approx"),
    ('≉', "napprox"),
    ('≊', "approxeq"),
    ('≋', "approxeq"),
    ('≌', "backcong"),
    ('≍', "asymp"),
    ('≎', "Bumpeq"),
    ('≏', "bumpeq"),
    ('≐', "doteq"),
    ('≑', "doteqdot"),
    ('≒', "fallingdotseq"),
    ('≓', "risingdotseq"),
    ('≔', ":="),
    ('≕', "=:"),
    ('≖', "eqcirc"),
    ('≗', "circeq"),
    ('≙', "wedgeq"),
    ('≚', "veeeq"),
    ('≜', "triangleq"),
    ('≟', "?="),
    ('≠', "neq"),
    ('≡', "equiv"),
    ('≢', "nequiv"),
    ('≤', "leq"),
    ('≥', "geq"),
    ('≦', "leqq"),
    ('≧', "geqq"),
    ('≨', "lneqq"),
    ('≩', "gneqq"),
    ('≪', "ll"),
    ('≫', "gg"),
    ('≬', "between"),
    ('≮', "nless"),
    ('≯', "ngtr"),
    ('≰', "nleq"),
    ('≱', "ngeq"),
    ('≲', "lesssim"),
    ('≳', "gtrsim"),
    ('≺', "prec"),
    ('≻', "succ"),
    ('≼', "preceq"),
    ('≽', "succeq"),
    ('≾', "precsim"),
    ('≿', "succsim"),
    ('⊂', "subset"),
    ('⊃', "supset"),
    ('⊄', "nsubset"),
    ('⊅', "nsupset"),
    ('⊆', "subseteq"),
    ('⊇', "supseteq"),
    ('⊈', "nsubseteq"),
    ('⊉', "nsupseteq"),
    ('⊊', "subsetneq"),
    ('⊋', "supsetneq"),
    ('⊎', "uplus"),
    ('⊏', "sqsubset"),
    ('⊐', "sqsupset"),
    ('⊑', "sqsubseteq"),
    ('⊒', "sqsupseteq"),
    ('⊓', "sqcap"),
    ('⊔', "sqcup"),
    ('⊕', "oplus"),
    ('⊖', "ominus"),
    ('⊗', "otimes"),
    ('⊘', "oslash"),
    ('⊙', "odot"),
    ('⊚', "circledcirc"),
    ('⊛', "circledast"),
    ('⊜', "circledequal"),
    ('⊝', "circleddash"),
    ('⊞', "boxplus"),
    ('⊟', "boxminus"),
    ('⊠', "boxtimes"),
    ('⊡', "boxdot"),
    ('⊢', "vdash"),
    ('⊣', "dashv"),
    ('⊤', "top"),
    ('⊥', "bot"),
    ('⊦', "vdash"),
    ('⊧', "models"),
    ('⊨', "models"),
    ('⊩', "Vdash"),
    ('⊪', "Vvdash"),
    ('⊫', "VDash"),
    ('⊬', "nvdash"),
    ('⊭', "nvDash"),
    ('⊮', "nVdash"),
    ('⊯', "nVDash"),
    ('⊲', "vartriangleleft"),
    ('⊳', "vartriangleright"),
    ('⊴', "trianglelefteq"),
    ('⊵', "trianglerighteq"),
    ('⊸', "multimap"),
    ('⊹', "intercal"),
    ('⊺', "intercal"),
    ('⊻', "veebar"),
    ('⊼', "barwedge"),
    ('⊽', "barvee"),
    ('⋀', "bigwedge"),
    ('⋁', "bigvee"),
    ('⋂', "bigcap"),
    ('⋃', "bigcup"),
    ('⋄', "diamond"),
    ('⋅', "cdot"),
    ('⋆', "star"),
    ('⋇', "divideontimes"),
    ('⋈', "bowtie"),
    ('⋉', "ltimes"),
    ('⋊', "rtimes"),
    ('⋋', "leftthreetimes"),
    ('⋌', "rightthreetimes"),
    ('⋍', "backsimeq"),
    ('⋎', "curlyvee"),
    ('⋏', "curlywedge"),
    ('⋐', "Subset"),
    ('⋑', "Supset"),
    ('⋒', "Cap"),
    ('⋓', "Cup"),
    ('⋔', "pitchfork"),
    ('⋕', "equalparallel"),
    ('⋖', "lessdot"),
    ('⋗', "gtrdot"),
    ('⋘', "lll"),
    ('⋙', "ggg"),
    ('⋚', "lesseqgtr"),
    ('⋛', "gtreqless"),
    ('⋞', "curlyeqprec"),
    ('⋟', "curlyeqsucc"),
    ('⋠', "npreceq"),
    ('⋡', "nsucceq"),
    ('⋢', "nsqsubseteq"),
    ('⋣', "nsqsupseteq"),
    ('⋦', "lnsim"),
    ('⋧', "gnsim"),
    ('⋨', "precnsim"),
    ('⋩', "succnsim"),
    ('⋪', "ntriangleleft"),
    ('⋫', "ntriangleright"),
    ('⋬', "ntrianglelefteq"),
    ('⋭', "ntrianglerighteq"),
    ('⋮', "vdots"),
    ('⋯', "cdots"),
    ('⋰', "adots"),
    ('⋱', "ddots"),
    // ── Arrows (U+2190–U+21FF) ────────────────────────────────────────────
    ('←', "leftarrow"),
    ('↑', "uparrow"),
    ('→', "rightarrow"),
    ('↓', "downarrow"),
    ('↔', "leftrightarrow"),
    ('↕', "updownarrow"),
    ('↖', "nwarrow"),
    ('↗', "nearrow"),
    ('↘', "searrow"),
    ('↙', "swarrow"),
    ('↚', "nleftarrow"),
    ('↛', "nrightarrow"),
    ('↜', "leftwave"),
    ('↝', "rightwave"),
    ('↞', "twoheadleftarrow"),
    ('↠', "twoheadrightarrow"),
    ('↢', "leftarrowtail"),
    ('↣', "rightarrowtail"),
    ('↦', "mapsto"),
    ('↩', "hookleftarrow"),
    ('↪', "hookrightarrow"),
    ('↫', "looparrowleft"),
    ('↬', "looparrowright"),
    ('↭', "leftrightsquigarrow"),
    ('↮', "nleftrightarrow"),
    ('↰', "Lsh"),
    ('↱', "Rsh"),
    ('↶', "curvearrowleft"),
    ('↷', "curvearrowright"),
    ('↺', "circlearrowleft"),
    ('↻', "circlearrowright"),
    ('⇐', "Leftarrow"),
    ('⇑', "Uparrow"),
    ('⇒', "Rightarrow"),
    ('⇓', "Downarrow"),
    ('⇔', "Leftrightarrow"),
    ('⇕', "Updownarrow"),
    ('⇚', "Lleftarrow"),
    ('⇛', "Rrightarrow"),
    ('⇝', "rightsquigarrow"),
    ('⇠', "dashleftarrow"),
    ('⇢', "dashrightarrow"),
    ('⇤', "LeftArrowBar"),
    ('⇥', "RightArrowBar"),
    // ── Geometric Shapes used in math ────────────────────────────────────
    ('△', "triangle"),
    ('▷', "triangleright"),
    ('◁', "triangleleft"),
    ('▽', "triangledown"),
    ('□', "square"),
    ('◯', "bigcirc"),
    ('⬡', "hexagon"),
    // ── Miscellaneous math ────────────────────────────────────────────────
    ('⌈', "lceil"),
    ('⌉', "rceil"),
    ('⌊', "lfloor"),
    ('⌋', "rfloor"),
    ('〈', "langle"),
    ('〉', "rangle"),
    ('⟨', "langle"),
    ('⟩', "rangle"),
    ('⟦', "llbracket"),
    ('⟧', "rrbracket"),
    ('⟵', "longleftarrow"),
    ('⟶', "longrightarrow"),
    ('⟷', "longleftrightarrow"),
    ('⟸', "Longleftarrow"),
    ('⟹', "Longrightarrow"),
    ('⟺', "Longleftrightarrow"),
    ('⟼', "longmapsto"),
    ('⨁', "bigoplus"),
    ('⨂', "bigotimes"),
    ('⨀', "bigodot"),
    ('⨄', "biguplus"),
    ('⨆', "bigsqcup"),
    ('⨅', "bigsqcap"),
    ('⨉', "bigtimes"),
    ('∫', "int"),
    // ── Common punctuation in math mode ───────────────────────────────────
    ('·', "cdot"),
    ('…', "ldots"),
    ('′', "'"),
    ('″', "''"),
    ('‴', "'''"),
    ('‵', "`"),
    // ── Superscript/subscript digits (U+2070–U+209F) ─────────────────────
    ('⁰', "^{0}"),
    ('¹', "^{1}"),
    ('²', "^{2}"),
    ('³', "^{3}"),
    ('⁴', "^{4}"),
    ('⁵', "^{5}"),
    ('⁶', "^{6}"),
    ('⁷', "^{7}"),
    ('⁸', "^{8}"),
    ('⁹', "^{9}"),
    ('⁺', "^{+}"),
    ('⁻', "^{-}"),
    ('⁼', "^{=}"),
    ('⁽', "^{(}"),
    ('⁾', "^{)}"),
    ('ⁿ', "^{n}"),
    ('ⁱ', "^{i}"),
    ('₀', "_{0}"),
    ('₁', "_{1}"),
    ('₂', "_{2}"),
    ('₃', "_{3}"),
    ('₄', "_{4}"),
    ('₅', "_{5}"),
    ('₆', "_{6}"),
    ('₇', "_{7}"),
    ('₈', "_{8}"),
    ('₉', "_{9}"),
    ('₊', "_{+}"),
    ('₋', "_{-}"),
    ('₌', "_{=}"),
    ('₍', "_{(}"),
    ('₎', "_{)}"),
    ('ₐ', "_{a}"),
    ('ₑ', "_{e}"),
    ('ₒ', "_{o}"),
    ('ₓ', "_{x}"),
    ('ₙ', "_{n}"),
    ('ᵢ', "_{i}"),
    ('ⱼ', "_{j}"),
    ('ₖ', "_{k}"),
    ('ₗ', "_{l}"),
    ('ₘ', "_{m}"),
    ('ₚ', "_{p}"),
    ('ₛ', "_{s}"),
    ('ₜ', "_{t}"),
    // ── Fractions ────────────────────────────────────────────────────────
    ('½', "frac{1}{2}"),
    ('⅓', "frac{1}{3}"),
    ('¼', "frac{1}{4}"),
    ('¾', "frac{3}{4}"),
    ('⅔', "frac{2}{3}"),
    ('⅕', "frac{1}{5}"),
    ('⅖', "frac{2}{5}"),
    ('⅗', "frac{3}{5}"),
    ('⅘', "frac{4}{5}"),
    ('⅙', "frac{1}{6}"),
    ('⅚', "frac{5}{6}"),
    ('⅛', "frac{1}{8}"),
    ('⅜', "frac{3}{8}"),
    ('⅝', "frac{5}{8}"),
    ('⅞', "frac{7}{8}"),
    // ── Common math decorators ────────────────────────────────────────────
    ('°', "degree"),
    ('℃', "celsius"),
    ('℉', "fahrenheit"),
    ('‰', "permil"),
    ('‱', "permyriad"),
    ('×', "times"),
    ('÷', "div"),
    ('∞', "infty"),
    ('¬', "lnot"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_maps_correctly() {
        assert_eq!(latex_for('α'), Some("alpha"));
    }

    #[test]
    fn integral_maps() {
        assert_eq!(latex_for('∫'), Some("int"));
    }

    #[test]
    fn infinity_maps() {
        assert_eq!(latex_for('∞'), Some("infty"));
    }

    #[test]
    fn real_numbers_maps() {
        assert_eq!(latex_for('ℝ'), Some("mathbb{R}"));
    }

    #[test]
    fn plain_ascii_letter_has_no_mapping() {
        assert_eq!(latex_for('a'), None);
        assert_eq!(latex_for('Z'), None);
        assert_eq!(latex_for('5'), None);
    }

    #[test]
    fn to_latex_alpha() {
        assert_eq!(to_latex('α'), "\\alpha");
    }

    #[test]
    fn to_latex_digit_passthrough() {
        assert_eq!(to_latex('3'), "3");
    }

    #[test]
    fn to_latex_unknown_unicode() {
        // Char not in table — returns the char itself
        let result = to_latex('é');
        assert_eq!(result, "é");
    }

    #[test]
    fn sum_maps() {
        assert_eq!(latex_for('∑'), Some("sum"));
    }

    #[test]
    fn rightarrow_maps() {
        assert_eq!(latex_for('→'), Some("rightarrow"));
    }

    #[test]
    fn superscript_two_maps() {
        assert_eq!(latex_for('²'), Some("^{2}"));
    }

    #[test]
    fn subscript_zero_maps() {
        assert_eq!(latex_for('₀'), Some("_{0}"));
    }

    #[test]
    fn half_fraction_maps() {
        assert_eq!(latex_for('½'), Some("frac{1}{2}"));
    }

    #[test]
    fn times_maps() {
        assert_eq!(latex_for('×'), Some("times"));
    }
}
