//! pdfplumber-mcp — MCP server binary.
//!
//! Reads JSON-RPC 2.0 requests from stdin (newline-delimited),
//! dispatches to tool handlers, and writes responses to stdout.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p pdfplumber-mcp
//! ```
//!
//! # Claude Desktop / Cursor configuration
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "pdfplumber": {
//!       "command": "cargo",
//!       "args": ["run", "--release", "-p", "pdfplumber-mcp"],
//!       "cwd": "/path/to/pdfplumber-rs"
//!     }
//!   }
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut server = pdfplumber_mcp::Server::new();

    for line in stdin.lock().lines() {
        match line {
            Ok(l) if l.is_empty() => continue,
            Ok(l) => {
                let response = server.handle(&l);
                writeln!(out, "{response}").expect("stdout write failed");
                out.flush().expect("stdout flush failed");
            }
            Err(e) => {
                eprintln!("stdin error: {e}");
                break;
            }
        }
    }
}
