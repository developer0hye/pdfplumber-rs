//! MCP `$/progress` notification helpers.
//!
//! Constructs progress notification JSON for long-running operations.
//! Owned by Agent F (feat/mcp-resources-prompts-transport).

use serde_json::{Value, json};

/// Build a `$/progress` notification payload for a token + value pair.
///
/// Callers write this to stdout as a newline-delimited JSON notification.
pub fn notification(token: &Value, value: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method":  "$/progress",
        "params":  { "progressToken": token, "value": value }
    })
}

/// Convenience: begin-progress notification with optional total.
pub fn begin(token: &Value, title: &str, total: Option<u64>) -> Value {
    let mut v = json!({ "kind": "begin", "title": title });
    if let Some(t) = total {
        v["total"] = json!(t);
    }
    notification(token, v)
}

/// Convenience: end-progress notification.
pub fn end(token: &Value, message: &str) -> Value {
    notification(token, json!({ "kind": "end", "message": message }))
}
