//! # pdfplumber-mcp
//!
//! [Model Context Protocol](https://modelcontextprotocol.io) server for
//! [pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs).
//!
//! Exposes PDF extraction as agent-callable tools via JSON-RPC 2.0 over stdio.
//! One request per line in, one response per line out — no state between calls.
//!
//! ## Protocol
//!
//! MCP 2024-11-05 · JSON-RPC 2.0 · newline-delimited stdio
//!
//! Supported methods: `initialize`, `initialized`, `ping`, `tools/list`, `tools/call`
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use pdfplumber_mcp::Server;
//!
//! let mut srv = Server::new();
//! let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;
//! println!("{}", srv.handle(init));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod tools;
pub mod types;

use serde_json::{Value, json};

/// MCP server. Create once; call [`Server::handle`] for each stdin line.
///
/// Stateless across tool calls — no file handles, caches, or sessions are kept.
#[derive(Default)]
pub struct Server {
    initialized: bool,
}

impl Server {
    /// Create a new server instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process one JSON-RPC 2.0 message and return the serialized response.
    ///
    /// Always returns valid JSON. Never panics.
    pub fn handle(&mut self, raw: &str) -> String {
        let response = serde_json::from_str::<Value>(raw)
            .map(|msg| {
                let id = msg.get("id").cloned().unwrap_or(Value::Null);
                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let params = msg.get("params").cloned().unwrap_or_default();
                self.dispatch(id, method, params)
            })
            .unwrap_or_else(|_| rpc_error(Value::Null, -32700, "Parse error"));

        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}"#
                .into()
        })
    }

    fn dispatch(&mut self, id: Value, method: &str, params: Value) -> Value {
        match method {
            "initialize" => self.on_initialize(id, params),
            "initialized" => rpc_ok(id, json!({})),
            "ping" => rpc_ok(id, json!({})),
            "tools/list" => self.on_tools_list(id),
            "tools/call" => self.on_tools_call(id, params),
            _ => rpc_error(id, -32601, "Method not found"),
        }
    }

    fn on_initialize(&mut self, id: Value, _params: Value) -> Value {
        self.initialized = true;
        rpc_ok(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities":    { "tools": {} },
                "serverInfo":      { "name": "pdfplumber-mcp", "version": env!("CARGO_PKG_VERSION") }
            }),
        )
    }

    fn on_tools_list(&self, id: Value) -> Value {
        rpc_ok(id, json!({ "tools": tools::definitions() }))
    }

    fn on_tools_call(&self, id: Value, params: Value) -> Value {
        let Some(name) = params.get("name").and_then(|n| n.as_str()) else {
            return rpc_error(id, -32602, "Missing tool name");
        };
        let args = params.get("arguments").cloned().unwrap_or_default();

        match tools::call(name, args) {
            Ok(content) => rpc_ok(id, json!({ "content": content, "isError": false })),
            Err(msg) => rpc_ok(
                id,
                json!({
                    "content": [{ "type": "text", "text": msg }],
                    "isError": true
                }),
            ),
        }
    }
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srv() -> Server {
        Server::new()
    }
    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#;

    #[test]
    fn initialize_returns_protocol_version() {
        let r = parse(&srv().handle(INIT));
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["name"], "pdfplumber-mcp");
    }

    #[test]
    fn ping_returns_ok() {
        let r = parse(&srv().handle(r#"{"jsonrpc":"2.0","id":2,"method":"ping","params":{}}"#));
        assert!(r.get("error").is_none());
        assert_eq!(r["id"], 2);
    }

    #[test]
    fn tools_list_has_all_expected_tools() {
        let mut s = srv();
        s.handle(INIT);
        let r = parse(&s.handle(r#"{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}"#));
        let tools = r["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in &[
            "pdf.metadata",
            "pdf.extract_text",
            "pdf.extract_tables",
            "pdf.extract_chars",
            "pdf.extract_words",
            "pdf.layout",
            "pdf.to_markdown",
        ] {
            assert!(names.contains(expected), "missing tool '{expected}'");
        }
    }

    #[test]
    fn unknown_method_is_method_not_found() {
        let r = parse(&srv().handle(r#"{"jsonrpc":"2.0","id":4,"method":"???","params":{}}"#));
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn malformed_json_is_parse_error() {
        let r = parse(&srv().handle("not { json"));
        assert_eq!(r["error"]["code"], -32700);
    }

    #[test]
    fn missing_tool_name_is_invalid_params() {
        let r =
            parse(&srv().handle(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{}}"#));
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn unknown_tool_returns_is_error_true() {
        let r = parse(&srv().handle(
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"pdf.unknown","arguments":{}}}"#,
        ));
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn initialized_notification_is_ok() {
        let r =
            parse(&srv().handle(r#"{"jsonrpc":"2.0","id":7,"method":"initialized","params":{}}"#));
        assert!(r.get("error").is_none());
    }

    #[test]
    fn handle_always_returns_valid_json() {
        for input in &["", "   ", "{}", "null", "[]"] {
            let out = srv().handle(input);
            assert!(
                serde_json::from_str::<Value>(&out).is_ok(),
                "not JSON for {input:?}: {out}"
            );
        }
    }
}
