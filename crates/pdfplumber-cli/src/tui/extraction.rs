//! Bridge between the pdfplumber public API and TUI state structs.
//!
//! All extraction runs synchronously in a worker thread (via
//! [`std::thread::spawn`]) and posts results back through a channel so the
//! event loop is never blocked.  The TUI layer is entirely decoupled from the
//! extraction logic — it just reads `Vec<String>` lines.

use std::path::Path;

use pdfplumber::{Pdf, TableSettings, TextOptions, WordOptions};

use crate::tui::app::GrepMatch;

/// Plain text extraction for a single page.
///
/// Returns the lines for display in the Extract screen.
/// On error, returns an `Err` with a human-readable message.
pub fn extract_text_lines(file: &Path, page_index: usize) -> Result<Vec<String>, String> {
    let pdf = open_pdf(file)?;
    let page = pdf
        .page(page_index)
        .map_err(|e| format!("page {}: {e}", page_index + 1))?;
    let text = page.extract_text(&TextOptions::default());
    Ok(text.lines().map(|l| l.to_string()).collect())
}

/// Word extraction (one line per word with bbox).
pub fn extract_word_lines(file: &Path, page_index: usize) -> Result<Vec<String>, String> {
    let pdf = open_pdf(file)?;
    let page = pdf
        .page(page_index)
        .map_err(|e| format!("page {}: {e}", page_index + 1))?;
    let words = page.extract_words(&WordOptions::default());
    let lines = words
        .iter()
        .map(|w| {
            format!(
                "{:<40} x0={:.1} y0={:.1} x1={:.1} y1={:.1}",
                w.text,
                w.bbox.x0,
                w.bbox.y0,
                w.bbox.x1,
                w.bbox.y1,
            )
        })
        .collect();
    Ok(lines)
}

/// Table extraction — each cell on its own row, tables separated by blank line.
pub fn extract_table_lines(file: &Path, page_index: usize) -> Result<Vec<String>, String> {
    let pdf = open_pdf(file)?;
    let page = pdf
        .page(page_index)
        .map_err(|e| format!("page {}: {e}", page_index + 1))?;
    let tables = page.extract_tables(&TableSettings::default());
    if tables.is_empty() {
        return Ok(vec!["(no tables detected on this page)".to_string()]);
    }
    let mut lines = Vec::new();
    for (ti, table) in tables.iter().enumerate() {
        lines.push(format!("── table {} ──", ti + 1));
        for row in table {
            let cells: Vec<&str> = row.iter().map(|c| c.as_deref().unwrap_or("")).collect();
            lines.push(cells.join(" │ "));
        }
        lines.push(String::new()); // blank separator
    }
    Ok(lines)
}

/// Char extraction — one line per char with coordinates.
pub fn extract_char_lines(file: &Path, page_index: usize) -> Result<Vec<String>, String> {
    let pdf = open_pdf(file)?;
    let page = pdf
        .page(page_index)
        .map_err(|e| format!("page {}: {e}", page_index + 1))?;
    let chars = page.chars();
    if chars.is_empty() {
        return Ok(vec!["(no characters on this page)".to_string()]);
    }
    let lines = chars
        .iter()
        .map(|c| {
            format!(
                "{:>4}  x0={:.1} y0={:.1}  font={} size={:.1}",
                c.text,
                c.bbox.x0,
                c.bbox.y0,
                c.fontname.as_deref().unwrap_or("?"),
                c.size,
            )
        })
        .collect();
    Ok(lines)
}

/// Return the page count for a file (used when entering Extract view).
pub fn page_count(file: &Path) -> Result<usize, String> {
    let pdf = open_pdf(file)?;
    Ok(pdf.page_count())
}

/// Grep — search all PDF files in a directory for a query string.
///
/// `case_insensitive` folds both sides to lowercase before comparing.
/// Returns matched snippets with file + page context.
pub fn grep_dir(
    dir: &Path,
    query: &str,
    case_insensitive: bool,
) -> Result<(Vec<GrepMatch>, usize), String> {
    use std::fs;

    if query.is_empty() {
        return Ok((vec![], 0));
    }

    let needle = if case_insensitive {
        query.to_lowercase()
    } else {
        query.to_string()
    };

    let entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("cannot read dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        })
        .collect();

    let files_searched = entries.len();
    let mut results = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let pdf = match Pdf::open_file(&path, None) {
            Ok(p) => p,
            Err(_) => continue,
        };
        for page_result in pdf.pages_iter() {
            let page = match page_result {
                Ok(p) => p,
                Err(_) => continue,
            };
            let text = page.extract_text(&TextOptions::default());
            let haystack = if case_insensitive {
                text.to_lowercase()
            } else {
                text.clone()
            };
            if haystack.contains(&needle) {
                // Build a short snippet around the first match
                let snippet = build_snippet(&text, query, case_insensitive);
                results.push(GrepMatch {
                    file: path.clone(),
                    page: page.page_number(),
                    snippet,
                });
            }
        }
    }

    Ok((results, files_searched))
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn open_pdf(file: &Path) -> Result<Pdf, String> {
    Pdf::open_file(file, None).map_err(|e| format!("{}: {e}", file.display()))
}

/// Extract up to 80 chars surrounding the first match of `query` in `text`.
fn build_snippet(text: &str, query: &str, case_insensitive: bool) -> String {
    let haystack = if case_insensitive {
        text.to_lowercase()
    } else {
        text.to_string()
    };
    let needle = if case_insensitive {
        query.to_lowercase()
    } else {
        query.to_string()
    };

    let Some(pos) = haystack.find(&needle) else {
        return text.chars().take(80).collect();
    };

    let start = pos.saturating_sub(30);
    let end = (pos + needle.len() + 50).min(text.len());
    let mut snippet = text[start..end].replace('\n', " ");
    if start > 0 {
        snippet.insert_str(0, "…");
    }
    if end < text.len() {
        snippet.push('…');
    }
    snippet
}
