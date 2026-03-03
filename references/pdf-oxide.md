# pdf_oxide (Rust)

- **URL:** https://github.com/yfedoseev/pdf_oxide
- **Stars:** 197 | **License:** MIT/Apache-2.0
- **Status:** Active (Jan 2025 commits)

## What It Does

Full-stack PDF processing: text extraction, image extraction, markdown conversion, PDF creation/editing. Bindings for Python, JS/WASM, CLI (22 commands), MCP server.

## Performance Claims

- 0.8ms mean per document
- 5x faster than PyMuPDF, 15x than pypdf, 29x than pdfplumber
- 100% pass rate on 3,830 real-world PDFs (veraPDF, pdf.js, DARPA SafeDocs)
- 99.5% text parity vs PyMuPDF and pypdfium2

## Architecture

- Custom PDF parser (not built on lopdf or pdf-rs)
- Character-level positioning data
- Multi-language bindings: Python (PyPI), JS (WASM), CLI
- Form fields, annotations, bookmarks

## Relevance to pdfplumber-rs

- **Most direct competitor** in the Rust PDF ecosystem
- Provides character positions but **no table detection** (pdfplumber-rs's differentiator)
- Benchmarking methodology (3,830 PDFs, text parity metrics) worth adopting
- Performance numbers useful as competitive reference
