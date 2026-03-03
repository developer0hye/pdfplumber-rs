# lopdf (Rust)

- **URL:** https://github.com/J-F-Liu/lopdf
- **Stars:** 2.1k | **License:** MIT | **MSRV:** 1.85
- **Status:** Actively maintained (current backend for pdfplumber-rs)

## What It Does

Low-level PDF document manipulation: read, create, modify PDF files at the object level.

## Architecture

- `Document` contains numbered objects (`ObjectId`)
- Pages in tree structure, accessed via `get_pages()`
- Content streams decoded to `Vec<Operation>` (operator + operands)
- Two parser backends: `nom_parser` (fast, recommended) and `pom_parser`
- Handles xref tables, object resolution, stream decompression
- `dictionary!` macro for building PDF structures

## Key Capabilities for pdfplumber-rs

- Raw content stream access (`Tj`, `TJ`, `Tm`, `Td` operators)
- Object/stream decompression
- PDF 1.5+ object streams and xref streams
- Font dictionary access (but no font-program-level parsing)

## Limitations

- No text-level semantics — purely structural
- No font metrics extraction from embedded font programs
- No CMap parsing
