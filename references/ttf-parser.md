# ttf-parser (Rust)

- **URL:** https://github.com/harfbuzz/ttf-parser
- **Stars:** 753 | **License:** MIT/Apache-2.0 | **Latest:** 0.25.1
- **Status:** Stable

## What It Does

Zero-allocation, zero-unsafe TrueType/OpenType/AAT parser. Parses 47+ table types.

## Key Tables for pdfplumber-rs

- **`hmtx`** (`src/tables/hmtx.rs`): `Metrics { advance, side_bearing }`. GIDs >= `num_h_metrics` reuse last advance width.
- **`head`**: Units per em, bbox, flags
- **`hhea`**: Number of h-metrics, ascender/descender
- **`maxp`**: Number of glyphs
- **`cmap`**: Character-to-glyph mapping
- **CFF/CFF2**: Outline support

## API Levels

- **High-level:** `Face::glyph_hor_advance(glyph_id)` → advance width
- **Low-level:** Direct table access for custom processing

## Architecture

- Many libraries built on top: rusttype, ab-glyph, fontdue
- C API available for FFI usage
- Pure Rust, no system dependencies

## Relevance to pdfplumber-rs

Best Rust reference for TrueType hmtx table parsing. Clean, safe implementation of glyph width extraction. The hmtx table structure (`num_h_metrics` entries of `{advance_width, lsb}` + extra `lsb` values) directly applies to `truetype.rs`.
