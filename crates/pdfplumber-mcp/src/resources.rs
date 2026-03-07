//! MCP `resources/list` and `resources/read` handlers.
//!
//! Exposes PDFs as first-class MCP resources via the `pdf://` URI scheme.
//! `pdf:///absolute/path/to/file.pdf` — resource URI is just the file path
//! prefixed with `pdf://`.
//!
//! Owned by Agent F (feat/mcp-resources-prompts-transport) for URI
//! discovery and subscription. The read handler here is the canonical impl.

use serde_json::{Value, json};

/// Handle `resources/list` — return static resource list.
///
/// In the base implementation, resources are not enumerated (no directory scan).
/// Clients that know a PDF path can read it directly via `pdf://` URI.
/// Agent F may extend this with workspace scanning.
pub fn handle_list() -> Value {
    json!({
        "resources": [
            {
                "uri":         "pdf://",
                "name":        "PDF files",
                "description": "Any PDF file on the local filesystem. \
                               Use URI pdf:///absolute/path/to/file.pdf to read metadata.",
                "mimeType":    "application/pdf"
            }
        ]
    })
}

/// Handle `resources/read` — read a PDF resource by `pdf://` URI.
///
/// Returns MCP resource contents: a text/plain summary of the PDF metadata.
/// For full extraction use the `pdf.*` tools.
pub fn handle_read(uri: &str) -> Value {
    let path = uri_to_path(uri);

    let content = match pdfplumber::Pdf::open_file(&path, None) {
        Err(e) => format!("Error opening {path}: {e}"),
        Ok(pdf) => {
            let meta = pdf.metadata();
            let pages = pdf.page_count();
            let mut parts = vec![format!("PDF Resource: {path}"), format!("Pages: {pages}")];
            if let Some(t) = &meta.title   { parts.push(format!("Title: {t}")); }
            if let Some(a) = &meta.author  { parts.push(format!("Author: {a}")); }
            if let Some(s) = &meta.subject { parts.push(format!("Subject: {s}")); }
            if let Some(c) = &meta.creator { parts.push(format!("Creator: {c}")); }
            parts.join("\n")
        }
    };

    json!({
        "contents": [
            {
                "uri":      uri,
                "mimeType": "text/plain",
                "text":     content
            }
        ]
    })
}

/// Convert `pdf:///path/to/file.pdf` → `/path/to/file.pdf`.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("pdf://").unwrap_or(uri).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_to_path_strips_scheme() {
        assert_eq!(uri_to_path("pdf:///tmp/x.pdf"), "/tmp/x.pdf");
    }

    #[test]
    fn uri_to_path_passthrough_bare() {
        assert_eq!(uri_to_path("/tmp/x.pdf"), "/tmp/x.pdf");
    }

    #[test]
    fn handle_list_returns_array() {
        let r = handle_list();
        assert!(r["resources"].is_array());
    }

    #[test]
    fn handle_read_missing_file_returns_contents_array() {
        let r = handle_read("pdf:///does/not/exist.pdf");
        assert!(r["contents"].is_array());
        let text = r["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("Error"));
    }
}
