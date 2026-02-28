use std::io::{self, IsTerminal, Write};
use std::path::Path;

use pdfplumber::{Pdf, TextDirection};

use crate::page_range::parse_page_range;

/// Open a PDF file with user-friendly error messages.
///
/// Returns `Err(1)` with a message printed to stderr if the file is not found
/// or cannot be parsed as a valid PDF.
pub fn open_pdf(file: &Path) -> Result<Pdf, i32> {
    if !file.exists() {
        eprintln!("Error: file not found: {}", file.display());
        return Err(1);
    }

    Pdf::open_file(file, None).map_err(|e| {
        eprintln!("Error: failed to open PDF: {e}");
        1
    })
}

/// Resolve an optional page range string into 0-indexed page indices.
///
/// If `pages` is `None`, returns all pages (0..page_count).
/// If `pages` is `Some`, parses the range string and validates against page_count.
pub fn resolve_pages(pages: Option<&str>, page_count: usize) -> Result<Vec<usize>, i32> {
    match pages {
        Some(range) => parse_page_range(range, page_count).map_err(|e| {
            eprintln!("Error: {e}");
            1
        }),
        None => Ok((0..page_count).collect()),
    }
}

/// Convert a `TextDirection` enum value to a lowercase string.
pub fn direction_str(dir: &TextDirection) -> &'static str {
    match dir {
        TextDirection::Ltr => "ltr",
        TextDirection::Rtl => "rtl",
        TextDirection::Ttb => "ttb",
        TextDirection::Btt => "btt",
    }
}

/// Escape a string for CSV output.
///
/// If the text contains commas, double quotes, or newlines, wraps it in
/// double quotes and escapes any internal double quotes by doubling them.
pub fn csv_escape(text: &str) -> String {
    if text.contains(',') || text.contains('"') || text.contains('\n') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// A progress reporter that prints "Processing page N/M..." to stderr,
/// but only when stderr is connected to a TTY (terminal).
pub struct ProgressReporter {
    total: usize,
    is_tty: bool,
}

impl ProgressReporter {
    /// Create a new progress reporter for `total` pages.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            is_tty: io::stderr().is_terminal(),
        }
    }

    /// Report progress for page `current` (1-indexed).
    pub fn report(&self, current: usize) {
        if self.is_tty {
            eprint!("\rProcessing page {}/{}...", current, self.total);
            let _ = io::stderr().flush();
        }
    }

    /// Clear the progress line (if TTY).
    pub fn finish(&self) {
        if self.is_tty {
            // Clear the line with carriage return and spaces
            eprint!("\r{}\r", " ".repeat(40));
            let _ = io::stderr().flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_str_ltr() {
        assert_eq!(direction_str(&TextDirection::Ltr), "ltr");
    }

    #[test]
    fn direction_str_rtl() {
        assert_eq!(direction_str(&TextDirection::Rtl), "rtl");
    }

    #[test]
    fn direction_str_ttb() {
        assert_eq!(direction_str(&TextDirection::Ttb), "ttb");
    }

    #[test]
    fn direction_str_btt() {
        assert_eq!(direction_str(&TextDirection::Btt), "btt");
    }

    #[test]
    fn csv_escape_plain_text() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn csv_escape_with_comma() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_escape_with_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_with_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn csv_escape_empty_string() {
        assert_eq!(csv_escape(""), "");
    }

    #[test]
    fn open_pdf_file_not_found() {
        let result = open_pdf(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
        match result {
            Err(code) => assert_eq!(code, 1),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn resolve_pages_none_returns_all() {
        let pages = resolve_pages(None, 5).unwrap();
        assert_eq!(pages, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn resolve_pages_with_range() {
        let pages = resolve_pages(Some("1,3"), 5).unwrap();
        assert_eq!(pages, vec![0, 2]);
    }

    #[test]
    fn resolve_pages_invalid_range() {
        let result = resolve_pages(Some("0"), 5);
        assert_eq!(result.unwrap_err(), 1);
    }

    #[test]
    fn progress_reporter_creation() {
        let reporter = ProgressReporter::new(10);
        assert_eq!(reporter.total, 10);
        // is_tty depends on test environment; just verify it doesn't panic
    }
}
