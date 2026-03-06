//! MCP `prompts/list` and `prompts/get` handlers.
//!
//! Exposes canned prompts for common PDF analysis tasks.
//! Owned by Agent F (feat/mcp-resources-prompts-transport).

use serde_json::{Value, json};

/// Handle `prompts/list` — return all available prompt definitions.
pub fn handle_list() -> Value {
    json!({
        "prompts": [
            {
                "name":        "analyze_pdf",
                "description": "Comprehensive analysis of a PDF: structure, content summary, tables, and key findings.",
                "arguments": [
                    { "name": "path", "description": "Path to the PDF file.", "required": true }
                ]
            },
            {
                "name":        "audit_accessibility",
                "description": "PDF/UA accessibility audit: tag structure, alt text, reading order, and WCAG compliance.",
                "arguments": [
                    { "name": "path", "description": "Path to the PDF file.", "required": true }
                ]
            },
            {
                "name":        "extract_structured_data",
                "description": "Extract all tables and structured data from a PDF into clean JSON.",
                "arguments": [
                    { "name": "path",   "description": "Path to the PDF file.",        "required": true  },
                    { "name": "format", "description": "Output format: json or csv.",  "required": false }
                ]
            },
            {
                "name":        "summarize_layout",
                "description": "Summarize the layout and document structure using semantic inference.",
                "arguments": [
                    { "name": "path", "description": "Path to the PDF file.", "required": true }
                ]
            }
        ]
    })
}

/// Handle `prompts/get` — return the messages array for a named prompt.
pub fn handle_get(name: &str, args: &Value) -> Value {
    let path = args["path"].as_str().unwrap_or("<pdf-path>");
    let messages = match name {
        "analyze_pdf" => vec![
            json!({
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!(
                        "Please analyze the PDF at `{path}` comprehensively:\n\
                        1. Use `pdf.metadata` to get document info and page count.\n\
                        2. Use `pdf.to_markdown` to get the full text and structure.\n\
                        3. Use `pdf.extract_tables` on pages with tables.\n\
                        4. Summarize: purpose, key topics, data found, and notable structure."
                    )
                }
            })
        ],
        "audit_accessibility" => vec![
            json!({
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!(
                        "Please audit the PDF at `{path}` for PDF/UA accessibility:\n\
                        1. Use `pdf.metadata` — check if the document is tagged.\n\
                        2. Use `pdf.layout` — inspect heading hierarchy and reading order.\n\
                        3. Report: tagging status, heading structure, alt text coverage, \
                           reading order quality, and WCAG 2.1 compliance issues."
                    )
                }
            })
        ],
        "extract_structured_data" => {
            let format = args["format"].as_str().unwrap_or("json");
            vec![
                json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Extract all structured data from `{path}` in {format} format:\n\
                            1. Use `pdf.extract_tables` to get all tables.\n\
                            2. Use `pdf.layout` to find any structured lists or data sections.\n\
                            3. Return the data clean and machine-readable."
                        )
                    }
                })
            ]
        },
        "summarize_layout" => vec![
            json!({
                "role": "user",
                "content": {
                    "type": "text",
                    "text": format!(
                        "Summarize the layout and document structure of `{path}`:\n\
                        1. Use `pdf.layout` to get the full semantic structure.\n\
                        2. Describe: document type, heading hierarchy, section count, \
                           estimated reading time, and any multi-column layout detected."
                    )
                }
            })
        ],
        _ => vec![
            json!({
                "role": "user",
                "content": { "type": "text", "text": format!("Unknown prompt: {name}") }
            })
        ],
    };
    json!({ "messages": messages })
}
