//! Document-level Markdown conversion types.
//!
//! Provides [`MarkdownConversionOptions`] and [`MarkdownConversionResult`] for
//! converting entire PDF documents to Markdown with metadata, images, and warnings.

use crate::error::ExtractWarning;
use crate::images::{ExportedImage, ImageExportOptions};

/// Options for document-level Markdown conversion.
///
/// Controls how pages are combined, whether images are included,
/// and strict-mode behavior for warnings.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MarkdownConversionOptions {
    /// Separator inserted between page Markdown outputs.
    /// Default: `"\n\n---\n\n"`
    pub page_separator: String,
    /// Whether to include extracted images in the result.
    /// Default: `true`
    pub include_images: bool,
    /// Options for image export (naming pattern, deduplication).
    pub image_options: ImageExportOptions,
    /// When true, any warning is escalated to an error.
    /// Default: `false`
    pub strict_mode: bool,
}

impl Default for MarkdownConversionOptions {
    fn default() -> Self {
        Self {
            page_separator: "\n\n---\n\n".to_string(),
            include_images: true,
            image_options: ImageExportOptions::default(),
            strict_mode: false,
        }
    }
}

/// Result of a document-level Markdown conversion.
///
/// Contains the combined Markdown text, a plain-text version,
/// optional title, extracted images, warnings, and page count.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MarkdownConversionResult {
    /// Combined Markdown text from all pages.
    pub markdown: String,
    /// Plain text version with Markdown formatting stripped.
    pub plain_text: String,
    /// Document title from PDF metadata or first heading.
    pub title: Option<String>,
    /// Extracted images (empty if `include_images` is false).
    pub images: Vec<ExportedImage>,
    /// Warnings collected during conversion.
    pub warnings: Vec<ExtractWarning>,
    /// Number of pages processed.
    pub page_count: usize,
}

/// Strip Markdown formatting from text, returning plain text.
///
/// Removes heading markers (`# `), emphasis markers (`*`, `**`, `_`),
/// horizontal rules (`---`), and collapses whitespace.
pub fn strip_markdown(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());

    for line in markdown.lines() {
        let trimmed = line.trim();

        // Skip horizontal rules
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            result.push('\n');
            continue;
        }

        // Strip heading markers
        let stripped = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim_start()
        } else {
            trimmed
        };

        // Strip bold/italic markers
        let stripped = strip_emphasis(stripped);

        if !stripped.is_empty() {
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&stripped);
        } else if !result.is_empty() {
            result.push('\n');
        }
    }

    // Clean up trailing whitespace
    result.trim().to_string()
}

/// Strip bold/italic emphasis markers from a string.
fn strip_emphasis(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '*' || chars[i] == '_' {
            // Skip consecutive emphasis markers
            while i < len && (chars[i] == '*' || chars[i] == '_') {
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Extract a title from the first Markdown heading in the text.
///
/// Looks for lines starting with `# ` (H1) and returns the heading text.
/// Returns `None` if no H1 heading is found.
pub fn extract_title_from_markdown(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- MarkdownConversionOptions tests ---

    #[test]
    fn default_options() {
        let opts = MarkdownConversionOptions::default();
        assert_eq!(opts.page_separator, "\n\n---\n\n");
        assert!(opts.include_images);
        assert!(!opts.strict_mode);
        assert_eq!(opts.image_options, ImageExportOptions::default());
    }

    #[test]
    fn custom_options() {
        let opts = MarkdownConversionOptions {
            page_separator: "\n\n".to_string(),
            include_images: false,
            strict_mode: true,
            ..Default::default()
        };
        assert_eq!(opts.page_separator, "\n\n");
        assert!(!opts.include_images);
        assert!(opts.strict_mode);
    }

    #[test]
    fn options_clone_and_eq() {
        let opts = MarkdownConversionOptions::default();
        let cloned = opts.clone();
        assert_eq!(opts, cloned);
    }

    // --- MarkdownConversionResult tests ---

    #[test]
    fn result_default_construction() {
        let result = MarkdownConversionResult {
            markdown: "# Hello\n\nWorld".to_string(),
            plain_text: "Hello\nWorld".to_string(),
            title: Some("Hello".to_string()),
            images: vec![],
            warnings: vec![],
            page_count: 1,
        };
        assert_eq!(result.page_count, 1);
        assert_eq!(result.title, Some("Hello".to_string()));
        assert!(result.images.is_empty());
        assert!(result.warnings.is_empty());
    }

    // --- strip_markdown tests ---

    #[test]
    fn strip_markdown_headings() {
        assert_eq!(strip_markdown("# Hello"), "Hello");
        assert_eq!(strip_markdown("## Subtitle"), "Subtitle");
        assert_eq!(strip_markdown("### Third"), "Third");
    }

    #[test]
    fn strip_markdown_emphasis() {
        assert_eq!(strip_markdown("**bold**"), "bold");
        assert_eq!(strip_markdown("*italic*"), "italic");
        assert_eq!(strip_markdown("***bold italic***"), "bold italic");
    }

    #[test]
    fn strip_markdown_horizontal_rules() {
        let md = "Page 1\n\n---\n\nPage 2";
        let plain = strip_markdown(md);
        assert!(plain.contains("Page 1"));
        assert!(plain.contains("Page 2"));
        assert!(!plain.contains("---"));
    }

    #[test]
    fn strip_markdown_preserves_plain_text() {
        assert_eq!(strip_markdown("Hello World"), "Hello World");
    }

    #[test]
    fn strip_markdown_empty_input() {
        assert_eq!(strip_markdown(""), "");
    }

    // --- extract_title_from_markdown tests ---

    #[test]
    fn extract_title_from_h1() {
        assert_eq!(
            extract_title_from_markdown("# My Document\n\nSome text"),
            Some("My Document".to_string())
        );
    }

    #[test]
    fn extract_title_ignores_h2() {
        assert_eq!(
            extract_title_from_markdown("## Not a title\n\nSome text"),
            None
        );
    }

    #[test]
    fn extract_title_returns_first_h1() {
        assert_eq!(
            extract_title_from_markdown("# First\n\n# Second"),
            Some("First".to_string())
        );
    }

    #[test]
    fn extract_title_none_for_empty() {
        assert_eq!(extract_title_from_markdown(""), None);
    }

    #[test]
    fn extract_title_none_for_no_heading() {
        assert_eq!(extract_title_from_markdown("Just some text"), None);
    }
}
