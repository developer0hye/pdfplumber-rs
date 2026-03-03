# allsorts (Rust)

- **URL:** https://github.com/yeslogic/allsorts
- **Stars:** 793 | **License:** Apache-2.0 | **Latest:** 0.16.1
- **Status:** Active

## What It Does

OpenType font parser, shaper, and subsetter. Parses glyf, CFF, CFF2, WOFF, WOFF2.

## CFF Parsing (`src/cff/`)

- CFF INDEX structures
- Top DICT, Private DICT parsing
- CharString interpreter (width extraction)
- `nominalWidthX` + charstring width operand, or `defaultWidthX` if no width operand
- CFF subsetting (eliminates unused operators)

## Other Capabilities

- Font shaping for complex scripts (Arabic, Indic, Thai)
- TrueType table parsing (head, hhea, maxp, hmtx, cmap, etc.)
- Font subsetting for PDF embedding

## Origin

Extracted from **Prince** (HTML/CSS to PDF tool) — well-tested with real PDF workflows.

## Relevance to pdfplumber-rs

**Strongest Rust reference for CFF parsing architecture.** Handles the same CFF structures pdfplumber-rs needs: Top DICT, Private DICT, nominalWidthX, defaultWidthX, CharStrings. The CharString parser implementation is particularly relevant for `cff.rs`.
