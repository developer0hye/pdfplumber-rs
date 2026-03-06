//! Heading detection and level assignment.

use pdfplumber_core::BBox;

/// Heading level inferred from font size tier relative to body baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HeadingLevel {
    /// Largest heading tier (>= 2.0x body or explicitly top tier).
    H1,
    /// Second tier (>= 1.6x body).
    H2,
    /// Third tier (>= 1.3x body).
    H3,
    /// Fourth tier (>= 1.15x body, or bold-only at body size).
    H4,
}

impl HeadingLevel {
    /// Infer heading level from the ratio of block font size to body baseline.
    ///
    /// - `size_ratio >= 2.0` → H1
    /// - `size_ratio >= 1.6` → H2
    /// - `size_ratio >= 1.3` → H3
    /// - otherwise → H4
    pub fn from_size_ratio(size_ratio: f64) -> Self {
        if size_ratio >= 2.0 {
            HeadingLevel::H1
        } else if size_ratio >= 1.6 {
            HeadingLevel::H2
        } else if size_ratio >= 1.3 {
            HeadingLevel::H3
        } else {
            HeadingLevel::H4
        }
    }

    /// Returns the heading level as an integer (1–4).
    pub fn as_int(self) -> u8 {
        match self {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
        }
    }
}

impl std::fmt::Display for HeadingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "H{}", self.as_int())
    }
}

/// A heading extracted from a PDF page.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Heading {
    /// Heading text.
    pub text: String,
    /// Bounding box of the heading on the page.
    pub bbox: BBox,
    /// Page number (0-based).
    pub page_number: usize,
    /// Heading level (H1–H4).
    pub level: HeadingLevel,
    /// Mean font size of the heading characters.
    pub font_size: f64,
    /// Font name of the majority of heading characters.
    pub fontname: String,
}

impl Heading {
    /// Return the heading text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return the heading level.
    pub fn level(&self) -> HeadingLevel {
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_level_thresholds() {
        assert_eq!(HeadingLevel::from_size_ratio(2.5), HeadingLevel::H1);
        assert_eq!(HeadingLevel::from_size_ratio(2.0), HeadingLevel::H1);
        assert_eq!(HeadingLevel::from_size_ratio(1.8), HeadingLevel::H2);
        assert_eq!(HeadingLevel::from_size_ratio(1.6), HeadingLevel::H2);
        assert_eq!(HeadingLevel::from_size_ratio(1.4), HeadingLevel::H3);
        assert_eq!(HeadingLevel::from_size_ratio(1.3), HeadingLevel::H3);
        assert_eq!(HeadingLevel::from_size_ratio(1.2), HeadingLevel::H4);
        assert_eq!(HeadingLevel::from_size_ratio(1.15), HeadingLevel::H4);
    }

    #[test]
    fn heading_level_as_int() {
        assert_eq!(HeadingLevel::H1.as_int(), 1);
        assert_eq!(HeadingLevel::H2.as_int(), 2);
        assert_eq!(HeadingLevel::H3.as_int(), 3);
        assert_eq!(HeadingLevel::H4.as_int(), 4);
    }

    #[test]
    fn heading_level_ordering() {
        assert!(HeadingLevel::H1 < HeadingLevel::H2);
        assert!(HeadingLevel::H2 < HeadingLevel::H3);
        assert!(HeadingLevel::H3 < HeadingLevel::H4);
    }

    #[test]
    fn heading_level_display() {
        assert_eq!(HeadingLevel::H1.to_string(), "H1");
        assert_eq!(HeadingLevel::H3.to_string(), "H3");
    }

    #[test]
    fn heading_text_accessor() {
        let h = Heading {
            text: "Introduction".to_string(),
            bbox: BBox::new(72.0, 50.0, 300.0, 70.0),
            page_number: 0,
            level: HeadingLevel::H1,
            font_size: 18.0,
            fontname: "Helvetica-Bold".to_string(),
        };
        assert_eq!(h.text(), "Introduction");
        assert_eq!(h.level(), HeadingLevel::H1);
    }

    #[test]
    fn heading_level_bold_only_is_h4() {
        // Bold at body size → H4 (ratio ~1.0)
        assert_eq!(HeadingLevel::from_size_ratio(1.0), HeadingLevel::H4);
    }
}
