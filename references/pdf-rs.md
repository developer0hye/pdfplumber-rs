# pdf-rs (Rust)

- **URL:** https://github.com/pdf-rs/pdf
- **Stars:** 1.6k | **License:** MIT
- **Status:** Actively maintained (818 commits, 37 contributors)

## What It Does

Read, manipulate, write PDFs with a strongly typed object model.

## Architecture

- Workspace: `pdf` (core) + `pdf_derive` (proc macros) + `pdf_text` (text extraction)
- PDF dictionaries auto-map to Rust structs via derive macros — catches structure errors at compile time
- Resolver pattern for indirect object references
- Typed enums for PDF value variants

## Key Patterns Worth Studying

- **Derive macros** for PDF object types reduce parsing boilerplate
- **`pdf_text`** subcrate: CMap decoding, font encoding, Unicode mapping
- **`pdf-rs/font`** (separate repo): glyf + CFF outlines, CMap formats, kerning

## Relevance to pdfplumber-rs

- Most "Rustic" approach to modeling PDF structures
- If ever migrating from lopdf, pdf-rs offers a more type-safe foundation
- The `pdf_text` CMap implementation is a useful reference
