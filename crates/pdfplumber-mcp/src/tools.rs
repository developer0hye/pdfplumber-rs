//! Tool implementations for the pdfplumber MCP server.
//!
//! Each function takes `serde_json::Value` arguments and returns
//! `Ok(Vec<Value>)` (MCP content array) or `Err(String)` (`isError: true`).

use pdfplumber::{Pdf, TableSettings, TextOptions, WordOptions};
use serde_json::{Value, json};

// ── helpers ───────────────────────────────────────────────────────────────────

fn open(args: &Value) -> Result<Pdf, String> {
    let path = args["path"]
        .as_str()
        .ok_or_else(|| "missing required argument 'path'".to_string())?;
    Pdf::open_file(path, None).map_err(|e| format!("failed to open {path:?}: {e}"))
}

fn require_page_idx(args: &Value) -> Result<usize, String> {
    args["page"]
        .as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| "missing required argument 'page' (0-based integer)".to_string())
}

fn text(s: impl Into<String>) -> Vec<Value> {
    vec![json!({ "type": "text", "text": s.into() })]
}

fn json_pretty(v: &Value) -> Vec<Value> {
    text(serde_json::to_string_pretty(v).unwrap_or_default())
}

// ── tool definitions ──────────────────────────────────────────────────────────

/// MCP tool definitions for `tools/list`. Schema per MCP 2024-11-05 spec.
pub fn definitions() -> Vec<Value> {
    let out = vec![
        json!({
            "name": "pdf.metadata",
            "description": "Document metadata: title, author, subject, keywords, creator, producer, creation date, modification date, and page count.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the PDF file." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "pdf.extract_text",
            "description": "Extract plain text. Returns all pages (with page separators) or a single page when 'page' is set. Set layout=true to preserve whitespace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path":   { "type": "string",  "description": "Path to the PDF file." },
                    "page":   { "type": "integer", "description": "0-based page index. Omit for all pages." },
                    "layout": { "type": "boolean", "description": "Preserve spatial layout. Default false." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "pdf.extract_tables",
            "description": "Detect tables and return cells as 2-D arrays. Returns all pages grouped by page index, or a single page when 'page' is set.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string",  "description": "Path to the PDF file." },
                    "page": { "type": "integer", "description": "0-based page index. Omit for all pages." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "pdf.extract_chars",
            "description": "Character-level extraction: text, bounding box (x0/top/x1/bottom), font name, and font size. Requires 'page'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path":  { "type": "string",  "description": "Path to the PDF file." },
                    "page":  { "type": "integer", "description": "0-based page index." },
                    "limit": { "type": "integer", "description": "Max chars to return (default 500)." }
                },
                "required": ["path", "page"]
            }
        }),
        json!({
            "name": "pdf.extract_words",
            "description": "Word-level extraction: text and bounding box. Words formed by clustering characters spatially. Requires 'page'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string",  "description": "Path to the PDF file." },
                    "page": { "type": "integer", "description": "0-based page index." }
                },
                "required": ["path", "page"]
            }
        }),
        json!({
            "name": "pdf.layout",
            "description": "Semantic layout inference: headings, paragraphs, sections, lists, tables, and figures. Pure geometric analysis — no ML, no network.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the PDF file." }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "pdf.to_markdown",
            "description": "Convert a PDF to GitHub-Flavored Markdown via layout inference. Headings, paragraphs, tables, and lists are preserved. Ideal for LLM input.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the PDF file." }
                },
                "required": ["path"]
            }
        }),
    ];

    #[cfg(feature = "raster")]
    out.push(json!({
        "name": "pdf.render_page",
        "description": "Rasterize a page to PNG, returned as base64. Pure Rust — no system library dependencies. scale=2.0 (default) = 144 dpi.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path":  { "type": "string",  "description": "Path to the PDF file." },
                "page":  { "type": "integer", "description": "0-based page index (default 0)." },
                "scale": { "type": "number",  "description": "Scale factor (default 2.0)." }
            },
            "required": ["path"]
        }
    }));

    out
}

// ── dispatch ──────────────────────────────────────────────────────────────────

/// Route a tool call by name.
pub fn call(name: &str, args: Value) -> Result<Vec<Value>, String> {
    match name {
        "pdf.metadata" => metadata(args),
        "pdf.extract_text" => extract_text(args),
        "pdf.extract_tables" => extract_tables(args),
        "pdf.extract_chars" => extract_chars(args),
        "pdf.extract_words" => extract_words(args),
        "pdf.layout" => layout(args),
        "pdf.to_markdown" => to_markdown(args),
        #[cfg(feature = "raster")]
        "pdf.render_page" => render_page(args),
        _ => Err(format!("unknown tool '{name}'")),
    }
}

// ── implementations ───────────────────────────────────────────────────────────

fn metadata(args: Value) -> Result<Vec<Value>, String> {
    let pdf = open(&args)?;
    let meta = pdf.metadata();
    Ok(json_pretty(&json!({
        "page_count":    pdf.page_count(),
        "title":         meta.title,
        "author":        meta.author,
        "subject":       meta.subject,
        "keywords":      meta.keywords,
        "creator":       meta.creator,
        "producer":      meta.producer,
        "creation_date": meta.creation_date,
        "mod_date":      meta.mod_date,
    })))
}

fn extract_text(args: Value) -> Result<Vec<Value>, String> {
    let pdf = open(&args)?;
    let layout = args["layout"].as_bool().unwrap_or(false);
    let opts = TextOptions {
        layout,
        ..Default::default()
    };

    if let Some(idx) = args["page"].as_u64().map(|n| n as usize) {
        let page = pdf.page(idx).map_err(|e| format!("page {idx}: {e}"))?;
        return Ok(text(page.extract_text(&opts)));
    }

    let mut buf = String::new();
    for (i, result) in pdf.pages_iter().enumerate() {
        match result {
            Ok(page) => {
                let t = page.extract_text(&opts);
                if !t.is_empty() {
                    if !buf.is_empty() {
                        buf.push_str("\n\n");
                    }
                    buf.push_str(&format!("--- Page {i} ---\n{t}"));
                }
            }
            Err(e) => buf.push_str(&format!("\n--- Page {i} (error) ---\n{e}")),
        }
    }
    Ok(text(buf))
}

fn extract_tables(args: Value) -> Result<Vec<Value>, String> {
    let pdf = open(&args)?;
    let settings = TableSettings::default();

    let page_tables = |page: &pdfplumber::Page| -> Value {
        let rows_2d: Vec<Vec<Vec<Option<String>>>> = page
            .find_tables(&settings)
            .iter()
            .map(|t| {
                t.rows
                    .iter()
                    .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
                    .collect()
            })
            .collect();
        json!(rows_2d)
    };

    if let Some(idx) = args["page"].as_u64().map(|n| n as usize) {
        let page = pdf.page(idx).map_err(|e| format!("page {idx}: {e}"))?;
        return Ok(json_pretty(&page_tables(&page)));
    }

    let all: Vec<Value> = pdf
        .pages_iter()
        .enumerate()
        .filter_map(|(i, result)| {
            let page = result.ok()?;
            let tables = page_tables(&page);
            if tables.as_array()?.is_empty() {
                return None;
            }
            Some(json!({ "page": i, "tables": tables }))
        })
        .collect();

    Ok(json_pretty(&json!(all)))
}

fn extract_chars(args: Value) -> Result<Vec<Value>, String> {
    let idx = require_page_idx(&args)?;
    let pdf = open(&args)?;
    let limit = args["limit"].as_u64().unwrap_or(500) as usize;
    let page = pdf.page(idx).map_err(|e| format!("page {idx}: {e}"))?;

    let chars: Vec<Value> = page
        .chars()
        .iter()
        .take(limit)
        .map(|c| {
            json!({
                "text":     c.text,
                "x0":       c.bbox.x0,
                "top":      c.bbox.top,
                "x1":       c.bbox.x1,
                "bottom":   c.bbox.bottom,
                "fontname": c.fontname,
                "size":     c.size,
            })
        })
        .collect();

    Ok(json_pretty(&json!({
        "page":     idx,
        "total":    page.chars().len(),
        "returned": chars.len(),
        "chars":    chars,
    })))
}

fn extract_words(args: Value) -> Result<Vec<Value>, String> {
    let idx = require_page_idx(&args)?;
    let pdf = open(&args)?;
    let page = pdf.page(idx).map_err(|e| format!("page {idx}: {e}"))?;

    let words: Vec<Value> = page
        .extract_words(&WordOptions::default())
        .iter()
        .map(|w| {
            json!({
                "text":   w.text,
                "x0":     w.bbox.x0,
                "top":    w.bbox.top,
                "x1":     w.bbox.x1,
                "bottom": w.bbox.bottom,
            })
        })
        .collect();

    Ok(json_pretty(
        &json!({ "page": idx, "word_count": words.len(), "words": words }),
    ))
}

#[cfg(feature = "layout")]
fn layout(args: Value) -> Result<Vec<Value>, String> {
    use pdfplumber_layout::Document;

    let pdf = open(&args)?;
    let doc = Document::from_pdf(&pdf);
    let st = doc.stats();

    let sections: Vec<Value> = doc
        .sections()
        .iter()
        .map(|s| {
            let heading = s
                .heading()
                .map(|h| json!({ "level": h.level().as_int(), "text": h.text() }));
            let paras: Vec<&str> = s.paragraphs().map(|p| p.text()).collect();
            json!({ "heading": heading, "paragraph_count": paras.len(), "paragraphs": paras })
        })
        .collect();

    Ok(json_pretty(&json!({
        "page_count":    st.page_count,
        "section_count": sections.len(),
        "heading_count": st.heading_count,
        "word_count":    doc.text().split_whitespace().count(),
        "body_font_pt":  st.body_font_size,
        "sections":      sections,
    })))
}

#[cfg(not(feature = "layout"))]
fn layout(_: Value) -> Result<Vec<Value>, String> {
    Err("layout feature not compiled in — rebuild with --features layout".into())
}

#[cfg(feature = "layout")]
fn to_markdown(args: Value) -> Result<Vec<Value>, String> {
    use pdfplumber_layout::Document;
    let pdf = open(&args)?;
    Ok(text(Document::from_pdf(&pdf).to_markdown()))
}

#[cfg(not(feature = "layout"))]
fn to_markdown(_: Value) -> Result<Vec<Value>, String> {
    Err("layout feature not compiled in — rebuild with --features layout".into())
}

#[cfg(feature = "raster")]
fn render_page(args: Value) -> Result<Vec<Value>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use pdfplumber_raster::{RasterOptions, Rasterizer};

    let pdf = open(&args)?;
    let idx = args["page"].as_u64().unwrap_or(0) as usize;
    let scale = args["scale"].as_f64().unwrap_or(2.0) as f32;
    let page = pdf.page(idx).map_err(|e| format!("page {idx}: {e}"))?;

    let result = Rasterizer::new(RasterOptions {
        scale,
        ..Default::default()
    })
    .render_page(&page)
    .map_err(|e| format!("render: {e}"))?;

    Ok(vec![
        json!({ "type": "text", "text": format!("Page {idx}: {}×{} px @ {:.1}×", result.width_px, result.height_px, result.scale) }),
        json!({ "type": "image", "data": STANDARD.encode(&result.png), "mimeType": "image/png" }),
    ])
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_satisfy_mcp_schema() {
        for tool in definitions() {
            let name = tool["name"].as_str().unwrap_or("?");
            assert!(tool.get("name").is_some(), "{name}: missing name");
            assert!(
                tool.get("description").is_some(),
                "{name}: missing description"
            );
            let schema = tool
                .get("inputSchema")
                .expect(&format!("{name}: missing inputSchema"));
            assert_eq!(
                schema["type"], "object",
                "{name}: schema type must be object"
            );
            assert!(
                schema.get("properties").is_some(),
                "{name}: schema missing properties"
            );
        }
    }

    #[test]
    fn definitions_cover_all_core_tools() {
        let defs = definitions();
        let names: Vec<&str> = defs.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for t in &[
            "pdf.metadata",
            "pdf.extract_text",
            "pdf.extract_tables",
            "pdf.extract_chars",
            "pdf.extract_words",
            "pdf.layout",
            "pdf.to_markdown",
        ] {
            assert!(names.contains(t), "missing tool {t}");
        }
    }

    #[test]
    fn unknown_tool_is_err() {
        assert!(call("pdf.nope", json!({})).is_err());
        assert!(call("", json!({})).is_err());
    }

    #[test]
    fn missing_path_is_err() {
        for tool in &[
            "pdf.metadata",
            "pdf.extract_text",
            "pdf.extract_tables",
            "pdf.layout",
            "pdf.to_markdown",
        ] {
            let r = call(tool, json!({}));
            assert!(r.is_err(), "{tool}: should error on missing path");
            assert!(
                r.unwrap_err().contains("path"),
                "{tool}: error must mention 'path'"
            );
        }
    }

    #[test]
    fn nonexistent_file_is_err() {
        let r = call(
            "pdf.metadata",
            json!({ "path": "/tmp/__pdfplumber_mcp_absent__.pdf" }),
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("failed to open"));
    }

    #[test]
    fn extract_chars_requires_page() {
        let r = call("pdf.extract_chars", json!({ "path": "/tmp/x.pdf" }));
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("page"));
    }

    #[test]
    fn extract_words_requires_page() {
        let r = call("pdf.extract_words", json!({ "path": "/tmp/x.pdf" }));
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("page"));
    }

    #[test]
    fn text_helper_shape() {
        let c = text("hello");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0]["type"], "text");
        assert_eq!(c[0]["text"], "hello");
    }

    #[test]
    fn json_pretty_round_trips() {
        let v = json!({ "x": 99 });
        let c = json_pretty(&v);
        let back: Value = serde_json::from_str(c[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(back["x"], 99);
    }
}
