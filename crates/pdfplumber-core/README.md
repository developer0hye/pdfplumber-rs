# pdfplumber-core — Coordinate-accurate PDF data types and algorithms

Backend-independent data types, geometric primitives, and table detection algorithms used across the pdfplumber-rs workspace. No mandatory external dependencies — all pure Rust.

## Quick Start

```rust
use pdfplumber_core::{BBox, Char, Word, WordOptions, WordExtractor};

// BBox arithmetic
let a = BBox::new(0.0, 0.0, 100.0, 50.0);
let b = BBox::new(50.0, 25.0, 150.0, 75.0);
let union = a.union(&b);           // BBox { x0: 0, top: 0, x1: 150, bottom: 75 }
let overlap = a.overlap_fraction(&b); // fraction of 'a' overlapped by 'b'

// Word extraction from chars
let words = WordExtractor::new(chars, &WordOptions::default());
```

## API Overview

| Type | Description |
|------|-------------|
| `BBox` | Axis-aligned bounding box with PDF coordinate system (origin top-left) |
| `Char` | Single extracted character with bbox, font, size, direction, and color |
| `Word` | Sequence of `Char` values forming a whitespace-delimited word |
| `Line` | Geometric line segment (not a text line) |
| `Rect` | Filled or stroked rectangle |
| `Table` | Detected table with rows and cells |
| `Color` | Gray, RGB, or CMYK color value |
| `StructElement` | Node in a tagged PDF structure tree |
| `PdfError` | Typed error enum (no `Internal(String)` variants) |

## Modules

| Module | Contents |
|--------|----------|
| `geometry` | `BBox`, `Point`, `Ctm`, `Orientation` |
| `text` | `Char`, `TextDirection`, CJK detection |
| `words` | `Word`, `WordExtractor`, `WordOptions` |
| `layout` | `TextLine`, `TextBlock`, reading-order sort |
| `shapes` | `Line`, `Rect`, `Curve` from painted paths |
| `table` | `Table`, `TableFinder`, `TableSettings` |
| `edges` | `Edge`, `EdgeSource` for table lattice detection |
| `images` | `Image`, `ImageMetadata`, export helpers |
| `painting` | `Color`, `GraphicsState`, `PaintedPath` |
| `struct_tree` | `StructElement` for tagged PDF access |
| `search` | `SearchMatch`, `search_chars` |
| `error` | `PdfError`, `ExtractWarning`, `ExtractOptions` |

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `serde` | Off | `Serialize`/`Deserialize` on all public data types |

## Architecture

- **No parsing dependencies.** This crate has zero knowledge of `lopdf` or any PDF format specifics. It receives already-extracted data from `pdfplumber-parse`.
- **Coordinate system.** PDF coordinate origin is bottom-left; this crate normalises to top-left (`top` < `bottom` in pixel terms) matching Python pdfplumber's convention.
- **Table detection is two-strategy.** Lattice strategy derives edges from lines/rects; stream strategy derives edges from character spacing. Both produce the same `Table` type.
- **`BBox` is the unit of everything.** Every extracted object carries a `BBox`. Spatial queries (overlap, containment, nearest-neighbour) all operate on `BBox`.

## Changelog

### v0.1.0-dev
- Full type surface: 25+ public types covering text, shapes, tables, images, structure, search
- Column-aware reading order sort (`sort_blocks_column_order`)
- `#![deny(missing_docs)]` enforced
