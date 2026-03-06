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
}
