//! List detection: bullet and numbered list items within a paragraph sequence.
//!
//! Lists are detected from the character content of paragraph blocks.
//! A block is a list item if its text begins with a bullet marker or an
//! ordinal prefix and shares a left-indent pattern with adjacent blocks.

use pdfplumber_core::BBox;

/// A list type inferred from item prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ListKind {
    /// Bullet list — items start with •, -, *, ◦, ▪, ▸, ›, or similar.
    Unordered,
    /// Numbered list — items start with `1.`, `(1)`, `a)`, `i.`, etc.
    Ordered,
}

/// A list item extracted from a paragraph-level block.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListItem {
    /// Item text (without the bullet/number prefix).
    pub text: String,
    /// Bounding box of this list item.
    pub bbox: BBox,
    /// Page number (0-based).
    pub page_number: usize,
    /// The raw prefix string (e.g. `"•"`, `"1."`, `"(a)"`).
    pub prefix: String,
    /// Nesting depth inferred from x0 indentation (0-based).
    pub depth: usize,
}

/// A detected list: a contiguous run of list items of the same kind.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct List {
    /// List type.
    pub kind: ListKind,
    /// Items in order.
    pub items: Vec<ListItem>,
    /// Bounding box spanning all items.
    pub bbox: BBox,
    /// Page number of the first item (0-based).
    pub page_number: usize,
}

impl List {
    /// Full text of the list as a plain string, one item per line.
    pub fn text(&self) -> String {
        self.items
            .iter()
            .map(|i| format!("{} {}", i.prefix, i.text))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── detection ────────────────────────────────────────────────────────────────

const BULLET_CHARS: &[char] = &['•', '·', '◦', '▪', '▸', '›', '‣', '⁃', '–', '—'];

/// Test if a text string starts with a bullet or list-item marker.
///
/// Returns `Some((prefix, rest, kind))` or `None`.
pub fn parse_list_prefix(text: &str) -> Option<(String, String, ListKind)> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Bullet character at start
    let first = trimmed.chars().next().unwrap();
    if BULLET_CHARS.contains(&first) || first == '*' || first == '-' || first == '+' {
        // Make sure it's actually a list item, not just a dash in text.
        // A list item has the bullet followed by a space.
        let rest: &str = trimmed.trim_start_matches(first).trim_start();
        if rest != trimmed.trim_start_matches(first) {
            // There was whitespace after the bullet.
            return Some((first.to_string(), rest.to_string(), ListKind::Unordered));
        }
    }

    // Numeric prefix: "1." / "1)" / "(1)" / "a." / "a)" / "i." / "ii." etc.
    if let Some((prefix, rest)) = parse_numeric_prefix(trimmed) {
        return Some((prefix, rest.trim_start().to_string(), ListKind::Ordered));
    }

    None
}

/// Parse numeric/alpha ordinal list prefixes.
fn parse_numeric_prefix(text: &str) -> Option<(String, &str)> {
    // Pattern: optional '(' + digits/letters + '.' or ')' + whitespace
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut prefix = String::new();

    // Optional opening paren
    let has_open_paren = if chars.first() == Some(&'(') {
        prefix.push('(');
        i += 1;
        true
    } else {
        false
    };

    // Digits or letters (1-3 chars max for an ordinal)
    let start = i;
    while i < chars.len() && (chars[i].is_ascii_alphanumeric()) && i - start < 4 {
        prefix.push(chars[i]);
        i += 1;
    }
    if i == start {
        return None; // no ordinal chars
    }

    // Closing delimiter: '.' or ')'
    if i >= chars.len() {
        return None;
    }
    if chars[i] == '.' || chars[i] == ')' {
        if has_open_paren && chars[i] != ')' {
            return None; // opened with '(' must close with ')'
        }
        prefix.push(chars[i]);
        i += 1;
    } else {
        return None;
    }

    // Must be followed by whitespace
    if i < chars.len() && chars[i].is_ascii_whitespace() {
        let rest = &text[i..];
        Some((prefix, rest))
    } else {
        None
    }
}

/// Estimate nesting depth from the x0 coordinate relative to a baseline x0.
///
/// Every `indent_step` points of indentation = 1 level deeper.
pub fn indent_depth(x0: f64, base_x0: f64, indent_step: f64) -> usize {
    if indent_step <= 0.0 || x0 <= base_x0 {
        return 0;
    }
    ((x0 - base_x0) / indent_step).round() as usize
}

/// Extract all lists from a `Section`'s paragraphs.
///
/// Scans the section's paragraph blocks in order and groups contiguous
/// list-item paragraphs of the same kind into [`List`] instances.
/// Non-list paragraphs reset the current run.
///
/// # Indentation
///
/// Nesting depth is estimated with `indent_step = 12.0` points (one pica).
/// This heuristic works for most body-text PDFs. Callers can post-process
/// the returned lists if their document uses a different indent step.
///
/// # Example
///
/// ```no_run
/// use pdfplumber_layout::{Document, lists::extract_lists_from_section};
/// use pdfplumber::Pdf;
///
/// let pdf = Pdf::open_file("report.pdf", None).unwrap();
/// let doc = Document::from_pdf(&pdf);
/// for section in doc.sections() {
///     for list in extract_lists_from_section(section) {
///         println!("{}: {} items", list.kind as u8, list.items.len());
///     }
/// }
/// ```
pub fn extract_lists_from_section(section: &crate::sections::Section) -> Vec<List> {
    use crate::LayoutBlock;

    let mut result: Vec<List> = Vec::new();
    let mut current_items: Vec<ListItem> = Vec::new();
    let mut current_kind: Option<ListKind> = None;
    let mut base_x0 = 0.0_f64;

    for block in &section.blocks {
        if let LayoutBlock::Paragraph(para) = block {
            if para.is_list_item {
                if let Some((prefix, rest, kind)) = parse_list_prefix(&para.text) {
                    // If kind changes, flush the current run.
                    if let Some(ck) = current_kind {
                        if ck != kind {
                            flush_list_run(&mut current_items, current_kind, &mut result);
                            // current_kind will be set below to `Some(kind)`
                            base_x0 = para.bbox.x0;
                        }
                    } else {
                        base_x0 = para.bbox.x0;
                    }
                    current_kind = Some(kind);
                    let depth = indent_depth(para.bbox.x0, base_x0, 12.0);
                    current_items.push(ListItem {
                        text: rest,
                        bbox: para.bbox,
                        page_number: para.page_number,
                        prefix,
                        depth,
                    });
                    continue;
                }
            }
            // Non-list paragraph: flush current run.
            flush_list_run(&mut current_items, current_kind, &mut result);
            current_kind = None;
        } else {
            // Non-paragraph block (table, figure, heading): flush.
            flush_list_run(&mut current_items, current_kind, &mut result);
            current_kind = None;
        }
    }
    flush_list_run(&mut current_items, current_kind, &mut result);
    result
}

/// Flush a pending list run into `result`.
fn flush_list_run(items: &mut Vec<ListItem>, kind: Option<ListKind>, result: &mut Vec<List>) {
    if items.is_empty() {
        return;
    }
    let kind = match kind {
        Some(k) => k,
        None => {
            items.clear();
            return;
        }
    };
    let x0 = items.iter().map(|i| i.bbox.x0).fold(f64::MAX, f64::min);
    let x1 = items.iter().map(|i| i.bbox.x1).fold(f64::MIN, f64::max);
    let top = items.iter().map(|i| i.bbox.top).fold(f64::MAX, f64::min);
    let bottom = items.iter().map(|i| i.bbox.bottom).fold(f64::MIN, f64::max);
    let page = items[0].page_number;
    result.push(List {
        kind,
        bbox: BBox::new(x0, top, x1, bottom),
        page_number: page,
        items: std::mem::take(items),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_char_detected() {
        let (prefix, rest, kind) = parse_list_prefix("• First item").unwrap();
        assert_eq!(prefix, "•");
        assert_eq!(rest, "First item");
        assert_eq!(kind, ListKind::Unordered);
    }

    #[test]
    fn dash_bullet_detected() {
        let (prefix, rest, kind) = parse_list_prefix("- Second item").unwrap();
        assert_eq!(prefix, "-");
        assert_eq!(rest, "Second item");
        assert_eq!(kind, ListKind::Unordered);
    }

    #[test]
    fn numeric_period_detected() {
        let (prefix, rest, kind) = parse_list_prefix("1. First item").unwrap();
        assert_eq!(prefix, "1.");
        assert_eq!(rest, "First item");
        assert_eq!(kind, ListKind::Ordered);
    }

    #[test]
    fn paren_numeric_detected() {
        let (prefix, rest, kind) = parse_list_prefix("(1) First item").unwrap();
        assert_eq!(prefix, "(1)");
        assert_eq!(rest, "First item");
        assert_eq!(kind, ListKind::Ordered);
    }

    #[test]
    fn alpha_suffix_paren_detected() {
        let (prefix, rest, kind) = parse_list_prefix("a) First item").unwrap();
        assert_eq!(prefix, "a)");
        assert_eq!(rest, "First item");
        assert_eq!(kind, ListKind::Ordered);
    }

    #[test]
    fn normal_text_not_detected() {
        assert!(parse_list_prefix("This is just a sentence.").is_none());
    }

    #[test]
    fn dash_without_space_not_detected() {
        // "---" is not a list item
        assert!(parse_list_prefix("---").is_none());
    }

    #[test]
    fn indent_depth_calculation() {
        assert_eq!(indent_depth(72.0, 72.0, 12.0), 0);
        assert_eq!(indent_depth(84.0, 72.0, 12.0), 1);
        assert_eq!(indent_depth(96.0, 72.0, 12.0), 2);
    }

    #[test]
    fn indent_depth_negative_returns_zero() {
        assert_eq!(indent_depth(60.0, 72.0, 12.0), 0);
    }

    #[test]
    fn extract_lists_empty_section() {
        use crate::sections::Section;
        let section = Section {
            heading: None,
            blocks: vec![],
            bbox: None,
            start_page: 0,
        };
        let lists = extract_lists_from_section(&section);
        assert!(lists.is_empty());
    }

    #[test]
    fn extract_lists_from_bullet_paragraphs() {
        use crate::LayoutBlock;
        use crate::paragraphs::Paragraph;
        use crate::sections::Section;
        use pdfplumber_core::BBox;

        fn list_para(text: &str, x0: f64) -> LayoutBlock {
            LayoutBlock::Paragraph(Paragraph {
                text: text.to_string(),
                bbox: BBox::new(x0, 100.0, 400.0, 114.0),
                page_number: 0,
                line_count: 1,
                font_size: 10.0,
                fontname: "Helvetica".to_string(),
                is_caption: false,
                is_list_item: true,
            })
        }

        let section = Section {
            heading: None,
            blocks: vec![
                list_para("• First item", 72.0),
                list_para("• Second item", 72.0),
                list_para("• Third item", 72.0),
            ],
            bbox: None,
            start_page: 0,
        };
        let lists = extract_lists_from_section(&section);
        assert_eq!(lists.len(), 1, "three bullet items should form one list");
        assert_eq!(lists[0].items.len(), 3);
        assert_eq!(lists[0].kind, ListKind::Unordered);
    }

    #[test]
    fn extract_lists_splits_on_kind_change() {
        use crate::LayoutBlock;
        use crate::paragraphs::Paragraph;
        use crate::sections::Section;
        use pdfplumber_core::BBox;

        fn list_para(text: &str) -> LayoutBlock {
            LayoutBlock::Paragraph(Paragraph {
                text: text.to_string(),
                bbox: BBox::new(72.0, 100.0, 400.0, 114.0),
                page_number: 0,
                line_count: 1,
                font_size: 10.0,
                fontname: "Helvetica".to_string(),
                is_caption: false,
                is_list_item: true,
            })
        }

        let section = Section {
            heading: None,
            blocks: vec![list_para("• Bullet one"), list_para("1. Number one")],
            bbox: None,
            start_page: 0,
        };
        let lists = extract_lists_from_section(&section);
        // Kind changes from Unordered to Ordered → 2 lists.
        assert_eq!(lists.len(), 2, "kind change should produce two lists");
        assert_eq!(lists[0].kind, ListKind::Unordered);
        assert_eq!(lists[1].kind, ListKind::Ordered);
    }

    #[test]
    fn list_text_method() {
        let items = vec![
            ListItem {
                text: "First".to_string(),
                bbox: BBox::new(72.0, 100.0, 400.0, 114.0),
                page_number: 0,
                prefix: "1.".to_string(),
                depth: 0,
            },
            ListItem {
                text: "Second".to_string(),
                bbox: BBox::new(72.0, 116.0, 400.0, 130.0),
                page_number: 0,
                prefix: "2.".to_string(),
                depth: 0,
            },
        ];
        let list = List {
            kind: ListKind::Ordered,
            items,
            bbox: BBox::new(72.0, 100.0, 400.0, 130.0),
            page_number: 0,
        };
        let text = list.text();
        assert!(text.contains("1. First"));
        assert!(text.contains("2. Second"));
    }
}
