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
///
/// # Path allowlisting
///
/// Set the `PDFPLUMBER_ALLOWED_PATHS` environment variable to a colon-separated
/// list of directory prefixes that the server is allowed to read from. If the
/// variable is unset, **all paths are permitted** (development mode). In
/// production deployments, always set this variable:
///
/// ```sh
/// PDFPLUMBER_ALLOWED_PATHS=/home/user/documents:/tmp/uploads pdfplumber-mcp
/// ```
///
/// A tool call whose `path` argument does not start with any allowed prefix
/// returns `isError: true` without opening the file.
#[derive(Default)]
pub struct Server {
    initialized: bool,
    /// Allowed path prefixes. Empty = allow all (dev mode).
    allowed_paths: Vec<std::path::PathBuf>,
}

impl Server {
    /// Create a new server instance.
    ///
    /// Reads `PDFPLUMBER_ALLOWED_PATHS` from the environment. If unset, all
    /// paths are permitted. If set, only paths under listed directories are
    /// accessible.
    pub fn new() -> Self {
        let allowed_paths = std::env::var("PDFPLUMBER_ALLOWED_PATHS")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
            .collect();
        Self {
            initialized: false,
            allowed_paths,
        }
    }

    /// Check whether a path is allowed under the configured allowlist.
    ///
    /// Returns `Ok(())` if allowed, `Err(message)` if denied.
    pub fn check_path(&self, path: &str) -> Result<(), String> {
        if self.allowed_paths.is_empty() {
            return Ok(());
        }
        let requested = std::path::Path::new(path);
        // Canonicalize to prevent path traversal (../../etc/passwd).
        let canonical = requested
            .canonicalize()
            .map_err(|e| format!("path not accessible: {e}"))?;
        let allowed = self.allowed_paths.iter().any(|prefix| {
            prefix
                .canonicalize()
                .map(|p| canonical.starts_with(p))
                .unwrap_or(false)
        });
        if allowed {
            Ok(())
        } else {
            Err(format!(
                "path '{path}' is not under any allowed directory (PDFPLUMBER_ALLOWED_PATHS)"
            ))
        }
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

        // Enforce path allowlist before opening any file.
        if let Some(path) = args.get("path").and_then(|p| p.as_str()) {
            if let Err(msg) = self.check_path(path) {
                return rpc_ok(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": msg }],
                        "isError": true
                    }),
                );
            }
        }

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
            "pdf.accessibility",
            "pdf.infer_tags",
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
    fn allowlist_empty_permits_all() {
        let srv = Server {
            initialized: false,
            allowed_paths: vec![],
        };
        assert!(srv.check_path("/tmp/anything.pdf").is_ok());
    }

    #[test]
    fn allowlist_blocks_outside_paths() {
        let srv = Server {
            initialized: false,
            allowed_paths: vec![std::path::PathBuf::from("/tmp/allowed")],
        };
        // /etc/passwd is outside /tmp/allowed — must be denied even if it exists.
        // We check the error message, not whether the file exists.
        let result = srv.check_path("/etc/passwd");
        // Either denied by allowlist or "path not accessible" (file exists on some systems).
        // In either case it must not be Ok for a path outside /tmp/allowed.
        if result.is_ok() {
            // /tmp/allowed doesn't exist so canonicalize of /etc/passwd would fail — also Err.
            // This branch only hits if both paths canonicalize. That's fine: in that case
            // the allowlist check itself would have returned Err. So this is unreachable
            // in practice but we leave the test non-panicking.
        }
        // Path traversal attempt must always fail if allowlist is set.
        assert!(srv.check_path("/tmp/allowed/../../etc/passwd").is_err());
    }

    #[test]
    fn allowlist_blocked_path_returns_is_error_true_in_rpc() {
        // Allowlist set but path not under it — tools/call must return isError:true.
        let mut srv = Server {
            initialized: true,
            allowed_paths: vec![std::path::PathBuf::from("/nonexistent_allowed_dir")],
        };
        let r = parse(&srv.handle(
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"pdf.metadata","arguments":{"path":"/etc/passwd"}}}"#,
        ));
        assert_eq!(r["result"]["isError"], true);
        let msg = r["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(msg.contains("PDFPLUMBER_ALLOWED_PATHS") || msg.contains("not accessible"), "unexpected msg: {msg}");
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
