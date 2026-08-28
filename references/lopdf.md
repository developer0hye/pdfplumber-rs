# lopdf (Rust)

- **URL:** https://github.com/J-F-Liu/lopdf
- **Stars:** 2.2k | **License:** MIT | **Release:** 0.44.0
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

## Input Loading Pattern

Verified against lopdf 0.44.0 source commit
[`8c454dd93d9c37e608c552a2b304d1d31d1cb2e1`](https://github.com/J-F-Liu/lopdf/blob/8c454dd93d9c37e608c552a2b304d1d31d1cb2e1/src/reader.rs),
the version resolved by this workspace:

- `Document::load` opens a filesystem path and reads the file into an owned buffer.
- `Document::load_from<R: Read>` accepts a synchronous reader without requiring
  `Seek` and reads from its current position through end-of-file.
- `Document::load_mem` parses a borrowed byte slice into an owned `Document`.

This supports a source-named high-level family while keeping backend types private:
`Pdf::open_path`, `Pdf::open_bytes`, and `Pdf::open_reader`. The facade applies its
own input budget before parsing so path and reader sources share the public
`PdfError` boundary rather than leaking lopdf errors.

## Limitations

- No text-level semantics — purely structural
- No font metrics extraction from embedded font programs
- No CMap parsing
