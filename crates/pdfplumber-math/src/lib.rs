//! Math region detection and LaTeX reconstruction from PDF documents.
//!
//! Identifies mathematical content in PDFs using Unicode range analysis
//! and typographic heuristics, then reconstructs approximate LaTeX.
//!
//! # Approach
//!
//! PDF math rendering is notoriously hard: equations are sequences of glyphs
//! positioned with precise CTM transforms rather than semantic markup. This
//! crate uses three complementary strategies:
//!
//! 1. **Unicode range detection** — characters in math Unicode blocks
//!    (Mathematical Operators U+2200–U+22FF, Mathematical Alphanumerics
//!    U+1D400–U+1D7FF, Letterlike Symbols U+2100–U+214F, etc.) signal
//!    math content even in mixed text/math passages.
//!
//! 2. **Spacing anomaly detection** — math typesetting uses tighter and more
//!    irregular inter-character spacing than body text. We flag clusters of
//!    chars with unusual horizontal/vertical offsets as potential equations.
//!
//! 3. **Vertical offset detection** — subscripts/superscripts appear at
//!    y-positions displaced from the baseline. We use this to reconstruct
//!    `x^{n}` and `x_{i}` patterns.
//!
//! The output is best-effort LaTeX. It will be correct for simple inline
//! equations (Greek letters, simple fractions, basic operators) and
//! approximate for complex display equations. For anything complex, the
//! `ollama-escalation` feature flag adds a hook to pass the region image
//! to a math-aware vision model (GLM-OCR, LaTeX-OCR, etc.) via Ollama.
//!
//! # Quick Start
//!
//! ```no_run
//! use pdfplumber::Pdf;
//! use pdfplumber_math::{MathExtractor, MathOptions};
//!
//! let pdf = Pdf::open_file("paper.pdf", None).unwrap();
//! let page = pdf.page(0).unwrap();
//!
//! let extractor = MathExtractor::new(MathOptions::default());
//! let regions = extractor.extract_page(&page, 0);
//!
//! for region in &regions {
//!     println!("Math at {:?}: ${}", region.bbox, region.latex);
//! }
//! ```

#![deny(missing_docs)]

mod detector;
mod latex;
/// Unicode → LaTeX symbol mapping table.
pub mod symbols;
/// Unicode range classification for mathematical characters.
pub mod unicode_ranges;

pub use detector::{MathExtractor, MathKind, MathOptions, MathRegion};
