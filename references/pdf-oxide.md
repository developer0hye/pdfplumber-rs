# pdf_oxide (Rust)

- **URL:** https://github.com/yfedoseev/pdf_oxide
- **Observed source:** [`3be1951b171edb9d69a10f42ef72ee73f52e51bf`](https://github.com/yfedoseev/pdf_oxide/blob/3be1951b171edb9d69a10f42ef72ee73f52e51bf/README.md) on 2026-08-26
- **Stars at observation:** 975 | **License:** MIT/Apache-2.0
- **Repository status at observation:** Public, active, and not archived

## What It Does

Full-stack PDF processing: text, image, table, form, and layout extraction; Markdown and HTML conversion; PDF creation and editing. The observed README documents a Rust core, nineteen language bindings, WebAssembly, a Command-Line Interface, and a Model Context Protocol server.

## Self-Published Performance Claims

The observed README reports a 3,830-PDF corpus assembled from veraPDF, Mozilla pdf.js, and DARPA SafeDocs, together with speed, pass-rate, and text-parity results. These are `pdf_oxide` project claims. `pdfplumber-rs` has not independently reproduced them and must not present them as its own measurements.

## Architecture

- Custom PDF parser (not built on lopdf or pdf-rs)
- Character-level positioning data
- Multi-language bindings over the Rust core
- Extraction, creation, editing, Command-Line Interface, and local Model Context Protocol surfaces

## Relevance to pdfplumber-rs

- A broad Rust-core competitor, but not a like-for-like Python `pdfplumber` compatibility project
- Its current feature breadth includes table detection, so tables alone are not an honest differentiator
- Its corpus description is useful input for `SCORE-001` through `SCORE-009`, but only materially equivalent outputs and independently reproducible raw results can support a `pdfplumber-rs` performance claim
- Re-audit the live repository before reusing status, adoption, or feature observations
