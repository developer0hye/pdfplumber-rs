# pdfplumber-mcp

MCP ([Model Context Protocol](https://modelcontextprotocol.io)) server for
[pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs). Exposes PDF
extraction, layout inference, and page rasterization as agent-callable tools
via JSON-RPC 2.0 over stdio.

## Quick Start

```sh
# Build and run
cargo run --release -p pdfplumber-mcp

# With raster support (base64 PNG output)
cargo run --release -p pdfplumber-mcp --features raster
```

## Claude Desktop / Cursor

```json
{
  "mcpServers": {
    "pdfplumber": {
      "command": "cargo",
      "args": ["run", "--release", "-p", "pdfplumber-mcp"],
      "cwd": "/path/to/pdfplumber-rs"
    }
  }
}
```

## Tools

| Tool | Description |
|------|-------------|
| `pdf.metadata` | Title, author, page count, tagged/encrypted status |
| `pdf.extract_text` | Full text or single page, with optional layout preservation |
| `pdf.extract_tables` | 2-D cell arrays from detected tables |
| `pdf.extract_chars` | Character-level data: text, bbox, font, size |
| `pdf.extract_words` | Word-level data: text, bbox |
| `pdf.layout` | Semantic structure: headings, paragraphs, sections, lists, tables |
| `pdf.to_markdown` | Full document → GitHub-Flavored Markdown |
| `pdf.render_page` | Page → PNG as base64 (`--features raster` required) |

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `layout` | ✓ | Enables `pdf.layout` and `pdf.to_markdown` via `pdfplumber-layout` |
| `raster` | ✗ | Enables `pdf.render_page` via `pdfplumber-raster` (pure Rust, zero C deps) |
| `full` | ✗ | All features |

## Architecture

The server is a thin JSON-RPC 2.0 dispatch layer over the existing pdfplumber-rs
crate APIs. Each tool is a function: deserialize args → call pdfplumber → serialize
result. No session state, no caching, no threads.

The MCP transport is newline-delimited JSON-RPC over stdio — one request per line,
one response per line. This matches the MCP 2024-11-05 specification and is
compatible with all MCP clients (Claude Desktop, Cursor, Continue, etc.).

**Protocol**: MCP 2024-11-05  
**Transport**: JSON-RPC 2.0 over stdio (newline-delimited)  
**Thread model**: single-threaded, synchronous  
**Dependencies**: `pdfplumber`, `pdfplumber-layout` (optional), `pdfplumber-raster` (optional), `serde_json`, `base64` (optional)
