# pdf-extract (Rust)

- **URL:** https://github.com/jrmuizel/pdf-extract
- **Stars:** 571 | **License:** MIT | **Latest:** 0.10.0
- **Status:** Active (282 commits, 17 contributors)

## What It Does

Text extraction from PDF files, built on lopdf.

## Dependency Stack (maps sub-problems of PDF text extraction)

- `lopdf 0.39` — PDF parsing backend
- `adobe-cmap-parser` — CMap file parsing (CID mappings)
- `cff-parser` — CFF/Type1C font parsing
- `postscript` — PostScript font handling
- `type1-encoding-parser` — Type1 font encoding
- `encoding_rs` — Text encodings
- `unicode-normalization` — Unicode NFC/NFD

## Architecture

- Layered on lopdf
- Each font encoding type has a dedicated parser crate (clean modular design)
- Simple API: `extract_text_from_mem()` returns plain text

## Key Gap vs pdfplumber-rs

Returns **flat text only** — no character positions, no bounding boxes, no glyph metrics, no table detection. This is exactly what pdfplumber-rs adds.

## Relevance

- The dependency crates (`adobe-cmap-parser`, `cff-parser`, etc.) could be reused or referenced for correctness validation
- Shows the modular decomposition of font/encoding sub-problems in Rust
