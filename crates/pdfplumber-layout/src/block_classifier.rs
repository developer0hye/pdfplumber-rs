//! Block-level classification of [`TextBlock`]s into semantic kinds.
//!
//! Classification is pure geometry + typography — no ML.

use pdfplumber_core::{BBox, TextBlock};

/// The inferred semantic role of a [`TextBlock`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    /// A section heading. Signals: large font, bold, single line, wide top gap.
    Heading,
    /// Body paragraph. Signals: normal font size, multiple lines, consistent margin.
    Paragraph,
    /// Figure or table caption. Signals: small font, single line, follows a table/figure bbox.
    Caption,
    /// List item. Signals: hanging indent or bullet character at start.
    ListItem,
    /// Other / unclassified (headers, footers, page numbers, etc.)
    Other,
}

/// Global font-size statistics derived from all chars in the document.
#[derive(Debug, Clone)]
pub struct FontStats {
    /// Median font size across all chars (approximates body text size).
    pub median: f64,
    /// 75th-percentile font size.
    pub p75: f64,
    /// 90th-percentile font size.
    pub p90: f64,
    /// Minimum observed font size.
    pub min: f64,
    /// Maximum observed font size.
    pub max: f64,
}

impl FontStats {
    /// Compute statistics from a flat list of font sizes.
    pub fn from_sizes(sizes: &[f64]) -> Self {
        if sizes.is_empty() {
            return FontStats {
                median: 12.0,
                p75: 14.0,
                p90: 18.0,
                min: 6.0,
                max: 72.0,
            };
        }
        let mut s = sizes.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = s.len();
        FontStats {
            min: s[0],
            max: s[n - 1],
            median: percentile(&s, 50.0),
            p75: percentile(&s, 75.0),
            p90: percentile(&s, 90.0),
        }
    }

    /// True if the given font size is significantly larger than body text.
    ///
    /// "Significantly larger" = at least 1.3× the median.
    pub fn is_heading_size(&self, size: f64) -> bool {
        size >= self.median * 1.3 || size >= self.p75
    }

    /// True if the given font size is notably smaller than body text (captions, footnotes).
    pub fn is_small_size(&self, size: f64) -> bool {
        size < self.median * 0.85
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Classification result for a single block.
#[derive(Debug, Clone)]
pub struct BlockClassification {
    /// The inferred block kind.
    pub kind: BlockKind,
    /// Page index (0-based).
    pub page: usize,
    /// Dominant font size in this block (median of chars).
    pub font_size: f64,
    /// True if the dominant font is bold.
    pub is_bold: bool,
    /// True if the dominant font is italic.
    pub is_italic: bool,
    /// Number of lines in this block.
    pub line_count: usize,
    /// Approximate left margin (x0 of the block bbox).
    pub left_margin: f64,
    /// The bbox of the block on the page.
    pub bbox: BBox,
    /// The plain-text content of the block.
    pub text: String,
}

impl BlockClassification {
    /// Is this block a heading?
    pub fn is_heading(&self) -> bool {
        self.kind == BlockKind::Heading
    }

    /// Is this block body text (paragraph or list item)?
    pub fn is_body(&self) -> bool {
        matches!(self.kind, BlockKind::Paragraph | BlockKind::ListItem)
    }
}

/// Bullet characters that signal a list item.
const BULLET_CHARS: &[char] = &[
    '•', '·', '◦', '▪', '▸', '▶', '–', '−', '―', '-', '*',
];

/// Classify a [`TextBlock`] into a [`BlockClassification`].
pub fn classify(block: &TextBlock, page: usize, stats: &FontStats) -> BlockClassification {
    // Collect all chars from all lines
    let all_chars: Vec<_> = block
        .lines
        .iter()
        .flat_map(|l| l.words.iter())
        .flat_map(|w| w.chars.iter())
        .collect();

    let line_count = block.lines.len();
    let text = block_text(block);

    // --- Font size ---
    let font_size = if all_chars.is_empty() {
        stats.median
    } else {
        let mut sizes: Vec<f64> = all_chars.iter().map(|c| c.size).collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        percentile(&sizes, 50.0)
    };

    // --- Bold / italic detection from fontname ---
    let fontname = all_chars
        .first()
        .map(|c| c.fontname.to_ascii_lowercase())
        .unwrap_or_else(String::new);
    let is_bold = fontname.contains("bold")
        || fontname.contains("heavy")
        || fontname.contains("black")
        || fontname.contains("demi");
    let is_italic = fontname.contains("italic")
        || fontname.contains("oblique")
        || fontname.contains("it-");

    // --- BBox ---
    let bbox = block
        .lines
        .iter()
        .flat_map(|l| l.words.iter())
        .map(|w| w.bbox)
        .reduce(|a, b| a.union(&b))
        .unwrap_or_else(|| BBox::new(0.0, 0.0, 0.0, 0.0));

    let left_margin = bbox.x0;

    // --- Classification rules ---

    // Rule 1: Caption — small font, short text (< 120 chars), likely after a table/figure
    if stats.is_small_size(font_size) && text.len() < 120 && line_count <= 2 {
        return BlockClassification {
            kind: BlockKind::Caption,
            page,
            font_size,
            is_bold,
            is_italic,
            line_count,
            left_margin,
            bbox,
            text,
        };
    }

    // Rule 2: List item — starts with bullet/dash character or "N." numbering
    let trimmed = text.trim();
    let starts_with_bullet = trimmed
        .chars()
        .next()
        .map(|c| BULLET_CHARS.contains(&c))
        .unwrap_or(false);
    let starts_with_numeral = starts_with_numeral(trimmed);

    if starts_with_bullet || starts_with_numeral {
        return BlockClassification {
            kind: BlockKind::ListItem,
            page,
            font_size,
            is_bold,
            is_italic,
            line_count,
            left_margin,
            bbox,
            text,
        };
    }

    // Rule 3: Heading — large font OR bold + single line, OR all-caps short text
    let is_large = stats.is_heading_size(font_size);
    let is_all_caps_short = text.len() < 80
        && !text.is_empty()
        && text
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase());

    if (is_large || (is_bold && line_count == 1) || is_all_caps_short) && line_count <= 4 {
        return BlockClassification {
            kind: BlockKind::Heading,
            page,
            font_size,
            is_bold,
            is_italic,
            line_count,
            left_margin,
            bbox,
            text,
        };
    }

    // Rule 4: Paragraph — default for multi-word body text
    let kind = if text.split_whitespace().count() >= 3 {
        BlockKind::Paragraph
    } else {
        BlockKind::Other
    };

    BlockClassification {
        kind,
        page,
        font_size,
        is_bold,
        is_italic,
        line_count,
        left_margin,
        bbox,
        text,
    }
}

/// Extract plain text from a [`TextBlock`] by joining lines with spaces.
pub fn block_text(block: &TextBlock) -> String {
    block
        .lines
        .iter()
        .map(|line| {
            line.words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// True if the text starts with "1." / "2." / "(a)" / "(1)" style list numbering.
fn starts_with_numeral(s: &str) -> bool {
    let s = s.trim();
    // "1. " or "12. "
    if let Some(dot_pos) = s.find('.') {
        if dot_pos > 0 && dot_pos <= 3 && s[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
            return s.len() > dot_pos + 1;
        }
    }
    // "(a) " or "(1) "
    if s.starts_with('(') {
        if let Some(close) = s.find(')') {
            if close >= 2 && close <= 4 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfplumber_core::{BBox, Char, TextBlock, TextDirection, TextLine, Word};

    fn make_char(text: &str, size: f64, fontname: &str) -> Char {
        Char {
            text: text.to_string(),
            bbox: BBox::new(0.0, 0.0, size * 0.6, size),
            fontname: fontname.to_string(),
            size,
            doctop: 0.0,
            upright: true,
            direction: TextDirection::Ltr,
        }
    }

    fn make_block_from_text(text: &str, size: f64, fontname: &str) -> TextBlock {
        let chars: Vec<Char> = text
            .split_whitespace()
            .flat_map(|word| {
                let mut chars: Vec<Char> = word.chars().map(|c| make_char(&c.to_string(), size, fontname)).collect();
                chars.push(make_char(" ", size, fontname));
                chars
            })
            .collect();

        let words = vec![Word {
            text: text.to_string(),
            bbox: BBox::new(0.0, 0.0, text.len() as f64 * size * 0.6, size),
            doctop: 0.0,
            direction: TextDirection::Ltr,
            chars,
        }];

        let line = TextLine {
            words: words.clone(),
            bbox: BBox::new(0.0, 0.0, text.len() as f64 * size * 0.6, size),
        };

        TextBlock {
            lines: vec![line],
            bbox: BBox::new(0.0, 0.0, text.len() as f64 * size * 0.6, size),
        }
    }

    fn default_stats() -> FontStats {
        FontStats {
            median: 12.0,
            p75: 14.0,
            p90: 18.0,
            min: 6.0,
            max: 72.0,
        }
    }

    #[test]
    fn large_font_classifies_as_heading() {
        let block = make_block_from_text("Introduction", 24.0, "Helvetica-Bold");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::Heading);
    }

    #[test]
    fn bold_single_line_classifies_as_heading() {
        let block = make_block_from_text("Section 1 Overview", 12.0, "Helvetica-Bold");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::Heading);
    }

    #[test]
    fn normal_body_text_is_paragraph() {
        let block = make_block_from_text(
            "This is a normal body paragraph with sufficient words to classify.",
            12.0,
            "Helvetica",
        );
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::Paragraph);
    }

    #[test]
    fn small_font_short_text_is_caption() {
        let block = make_block_from_text("Figure 1: The architecture overview.", 8.0, "Helvetica");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::Caption);
    }

    #[test]
    fn bullet_text_is_list_item() {
        let block = make_block_from_text("• First item in a bulleted list", 12.0, "Helvetica");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::ListItem);
    }

    #[test]
    fn numbered_list_item() {
        let block = make_block_from_text("1. First numbered item here", 12.0, "Helvetica");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::ListItem);
    }

    #[test]
    fn all_caps_short_text_is_heading() {
        let block = make_block_from_text("ABSTRACT", 12.0, "Helvetica");
        let stats = default_stats();
        let cls = classify(&block, 0, &stats);
        assert_eq!(cls.kind, BlockKind::Heading);
    }

    #[test]
    fn font_stats_median() {
        let sizes = vec![10.0, 10.0, 10.0, 12.0, 12.0, 14.0, 24.0];
        let stats = FontStats::from_sizes(&sizes);
        assert_eq!(stats.median, 12.0);
        assert!(stats.is_heading_size(24.0));
        assert!(!stats.is_heading_size(12.0));
    }

    #[test]
    fn font_stats_empty_has_defaults() {
        let stats = FontStats::from_sizes(&[]);
        assert_eq!(stats.median, 12.0);
    }

    #[test]
    fn starts_with_numeral_cases() {
        assert!(starts_with_numeral("1. First item"));
        assert!(starts_with_numeral("12. Twelfth item"));
        assert!(starts_with_numeral("(a) sub-item"));
        assert!(!starts_with_numeral("100 dollars"));
        assert!(!starts_with_numeral("Normal text"));
    }
}
