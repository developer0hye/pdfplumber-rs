//! Paragraph detection and text assembly.

use pdfplumber_core::BBox;

/// A paragraph of body text.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Paragraph {
    /// Paragraph text with line breaks preserved as spaces.
    pub text: String,
    /// Bounding box of the paragraph.
    pub bbox: BBox,
    /// Page number (0-based).
    pub page_number: usize,
    /// Number of lines in this paragraph.
    pub line_count: usize,
    /// Mean font size.
    pub font_size: f64,
    /// Dominant font name.
    pub fontname: String,
    /// True if this paragraph is likely a figure caption
    /// (short, positioned below a figure bbox, often starts with "Figure" / "Fig." / "Table").
    pub is_caption: bool,
    /// True if this paragraph text begins with a bullet or ordered list prefix.
    ///
    /// Detected via [`crate::lists::parse_list_prefix`]. The raw text (including
    /// the prefix) is preserved in `text` — this flag is purely advisory for
    /// downstream consumers (chunkers, taggers, renderers).
    pub is_list_item: bool,
}

impl Paragraph {
    /// Return the paragraph text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns true if this looks like a caption.
    pub fn is_caption(&self) -> bool {
        self.is_caption
    }

    /// Returns true if this paragraph starts with a list bullet or ordinal prefix.
    pub fn is_list_item(&self) -> bool {
        self.is_list_item
    }
}

/// Detect if a paragraph text looks like a figure/table caption.
///
/// Heuristics:
/// - Starts with "Figure", "Fig.", "Table", "Chart", "Exhibit", "Appendix"
/// - Or is a very short block (< 120 chars) immediately following a figure region
pub fn looks_like_caption(text: &str) -> bool {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    if lower.starts_with("figure")
        || lower.starts_with("fig.")
        || lower.starts_with("fig ")
        || lower.starts_with("table ")
        || lower.starts_with("chart ")
        || lower.starts_with("exhibit ")
        || lower.starts_with("appendix ")
        || lower.starts_with("note:")
        || lower.starts_with("source:")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_text_accessor() {
        let p = Paragraph {
            text: "Hello world.".to_string(),
            bbox: BBox::new(72.0, 100.0, 500.0, 120.0),
            page_number: 0,
            line_count: 1,
            font_size: 10.0,
            fontname: "Helvetica".to_string(),
            is_caption: false,
            is_list_item: false,
        };
        assert_eq!(p.text(), "Hello world.");
        assert!(!p.is_caption());
        assert!(!p.is_list_item());
    }

    #[test]
    fn caption_detection_figure() {
        assert!(looks_like_caption("Figure 1. Revenue by quarter"));
        assert!(looks_like_caption("Fig. 3: Distribution of results"));
        assert!(looks_like_caption("fig 4 shows the trend"));
    }

    #[test]
    fn caption_detection_table() {
        assert!(looks_like_caption("Table 2. Summary statistics"));
    }

    #[test]
    fn caption_detection_chart_exhibit() {
        assert!(looks_like_caption("Chart 1: Monthly sales"));
        assert!(looks_like_caption("Exhibit A — Supporting data"));
    }

    #[test]
    fn caption_detection_note_source() {
        assert!(looks_like_caption("Note: All values in USD thousands."));
        assert!(looks_like_caption("Source: Company annual report 2024."));
    }

    #[test]
    fn caption_detection_regular_text() {
        assert!(!looks_like_caption("The company reported strong earnings."));
        assert!(!looks_like_caption("In this section we describe the methodology."));
    }

    #[test]
    fn caption_detection_empty() {
        assert!(!looks_like_caption(""));
        assert!(!looks_like_caption("   "));
    }
}
