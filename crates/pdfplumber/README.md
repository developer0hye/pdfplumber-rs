# pdfplumber — Extract text, tables, and geometry from PDF documents

High-level Rust API for coordinate-accurate PDF extraction. Port of Python's [pdfplumber](https://github.com/jsvine/pdfplumber) — same spatial model, same extraction quality, ~10× faster.

## Quick Start

```rust
use pdfplumber::{Pdf, TextOptions, TableSettings};

let pdf = Pdf::open_file("document.pdf", None)?;

// Text extraction
for page_result in pdf.pages_iter() {
    let page = page_result?;
    let text = page.extract_text(&TextOptions::default());
    println!("Page {}: {}", page.page_number(), text);
}

// Table extraction
let page = pdf.page(0)?;
let tables = page.find_tables(&TableSettings::default());
for table in &tables {
    for row in &table.rows {
        let cells: Vec<&str> = row.iter()
            .map(|c| c.text.as_deref().unwrap_or(""))
            .collect();
        println!("{}", cells.join(" | "));
    }
}

// Character-level with metadata
let chars = page.chars();
for ch in chars.iter().take(5) {
    println!("{:?} at ({:.1}, {:.1}) font={} size={:.1}",
        ch.text, ch.bbox.x0, ch.bbox.top, ch.fontname, ch.size);
}
```

## API Overview

| Type | Key Methods |
|------|-------------|
| `Pdf` | `open_file`, `open`, `page`, `pages_iter`, `page_count`, `metadata` |
| `Page` | `extract_text`, `chars`, `words`, `lines`, `rects`, `images`, `find_tables`, `filter`, `crop` |
| `CroppedPage` | Same as `Page` — all extraction methods available on subregions |
| `Table` | `rows: Vec<Vec<Cell>>`, `bbox` |
| `Cell` | `text: Option<String>`, `bbox` |
| `Char` | `text`, `bbox`, `fontname`, `size`, `color`, `direction` |
| `Word` | `text`, `bbox`, `chars` |

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `std` | Yes | File-path APIs (`Pdf::open_file`). Disable for WASM. |
| `serde` | Off | `Serialize`/`Deserialize` on all public types |
| `parallel` | Off | `Pdf::pages_parallel()` via rayon. Not WASM-compatible. |

## What Makes This Different

- **Coordinate accuracy.** Every character carries its exact `x0/top/x1/bottom` bbox derived from the PDF text matrix — not estimated from font metrics.
- **Column-aware reading order.** Multi-column PDFs are sorted correctly by detected column boundaries, not naively top-to-bottom.
- **Two-strategy table detection.** Lattice (line-defined tables) and stream (whitespace-defined tables) — same API, different `TableSettings::strategy`.
- **Full geometry access.** Lines, rects, curves, and images are first-class — not just text.
- **Spatial filtering.** `page.crop(bbox)` and `page.filter(predicate)` return a `CroppedPage` that supports all the same extraction methods.
- **WASM-compatible.** Disable `std` feature for `wasm32-unknown-unknown` targets.

## Architecture

```
Pdf::open_file()
  └── LopdfBackend (pdfplumber-parse)
        └── ContentInterpreter → CharEvent / PathEvent / ImageEvent
              └── Page (pdfplumber)
                    ├── extract_text()  ← TextOptions
                    ├── find_tables()   ← TableSettings (lattice | stream)
                    ├── chars / words / lines / rects / images
                    └── crop() / filter() → CroppedPage
```

- **No rendering.** This crate extracts; it doesn't render. For PNG output see `pdfplumber-raster`.
- **No layout inference.** For headings/paragraphs/sections/markdown see `pdfplumber-layout`.

## Changelog

### v0.1.0-dev
- Full extraction API: text, chars, words, lines, rects, curves, images, tables
- Structure tree access (`page.structure_elements()`)
- Hyperlink extraction (`page.hyperlinks()`)
- Form field extraction (`pdf.form_fields()`)
- Digital signature info (`pdf.signatures()`)
- `CroppedPage` with full filter/crop composability
