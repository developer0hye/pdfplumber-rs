# Changelog — pdfplumber-mcp

All notable changes to this crate follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.0] — 2026-03-06

### Added
- MCP 2024-11-05 JSON-RPC 2.0 server over stdio (`Server::handle`)
- 9 tools: `pdf.extract_text`, `pdf.extract_tables`, `pdf.extract_chars`,
  `pdf.extract_words`, `pdf.metadata`, `pdf.layout`, `pdf.to_markdown`,
  `pdf.render_page` (feature-gated), `pdf.accessibility`, `pdf.infer_tags`
- Path allowlist via `PDFPLUMBER_ALLOWED_PATHS` environment variable —
  colon-separated directory prefixes; unset = permit all (dev mode)
- Path traversal prevention via `canonicalize()` before allowlist check
- Feature flags: `layout` (default), `a11y` (default), `raster`, `full`
- 21 unit tests covering protocol compliance, tool dispatch, and path security
