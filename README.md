# pdfplumber-rs

[![CI](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pdfplumber.svg)](https://crates.io/crates/pdfplumber)
[![docs.rs](https://docs.rs/pdfplumber/badge.svg)](https://docs.rs/pdfplumber)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/developer0hye/pdfplumber-rs)
[![License](https://img.shields.io/crates/l/pdfplumber.svg)](https://github.com/developer0hye/pdfplumber-rs/blob/main/LICENSE)

Extract chars, words, lines, rects, and tables from PDF documents with precise coordinates.

**pdfplumber-rs** is a Rust port of Python's [pdfplumber](https://github.com/jsvine/pdfplumber). It extracts structured content from PDF files with coordinate-accurate positioning, including characters, words, lines, rectangles, curves, images, and tables.

## Features

- **Text extraction** with spatial grouping into words, lines, and text blocks
- **Table detection** using lattice (line-based), stream (text-alignment), and explicit strategies
- **Spatial filtering** via `crop`, `within_bbox`, and `outside_bbox`
- **CJK support** including CID fonts, Identity-H/V CMaps, and CJK-aware word grouping
- **Page-level streaming** for memory-efficient processing of large documents
- **WASM support** via `wasm32-unknown-unknown` target
- **Optional serde** serialization for all data types
- **Optional parallel** processing via rayon

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pdfplumber = "0.1"
```

### Feature Flags

| Feature    | Default | Description                                                    |
|------------|---------|----------------------------------------------------------------|
| `std`      | Yes     | Enables file-path APIs (`Pdf::open_file`). Disable for WASM.  |
| `serde`    | No      | Adds `Serialize`/`Deserialize` to all public data types.       |
| `parallel` | No      | Enables `Pdf::pages_parallel()` via rayon. Not WASM-compatible.|

## Quick Start

### Extract Text

```rust,no_run
use pdfplumber::{Pdf, TextOptions};

fn main() {
    let pdf = Pdf::open_file("document.pdf", None).unwrap();
    for page_result in pdf.pages_iter() {
        let page = page_result.unwrap();
        let text = page.extract_text(&TextOptions::default());
        println!("Page {}: {}", page.page_number(), text);
    }
}
```

### Extract Tables

```rust,no_run
use pdfplumber::{Pdf, TableSettings};

fn main() {
    let pdf = Pdf::open_file("document.pdf", None).unwrap();
    let page = pdf.page(0).unwrap();
    let tables = page.find_tables(&TableSettings::default());
    for table in &tables {
        for row in &table.rows {
            let cells: Vec<&str> = row.iter()
                .map(|c| c.text.as_deref().unwrap_or(""))
                .collect();
            println!("{:?}", cells);
        }
    }
}
```

### Extract Characters

```rust,no_run
use pdfplumber::Pdf;

fn main() {
    let pdf = Pdf::open_file("document.pdf", None).unwrap();
    let page = pdf.page(0).unwrap();
    for ch in page.chars() {
        println!(
            "'{}' at ({:.1}, {:.1}) font={} size={:.1}",
            ch.text, ch.bbox.x0, ch.bbox.top, ch.fontname, ch.size
        );
    }
}
```

## WASM Support

For `wasm32-unknown-unknown` targets, disable the default `std` feature:

```toml
[dependencies]
pdfplumber = { version = "0.1", default-features = false }
```

Use the bytes-based API:

```rust,ignore
let pdf = Pdf::open(pdf_bytes, None)?;
let page = pdf.page(0)?;
let text = page.extract_text(&TextOptions::default());
```

## Architecture

```text
+--------------------------------------------------------------+
|  Layer 5: Table Detection (Lattice / Stream / Explicit)      |
+--------------------------------------------------------------+
|  Layer 4: Text Grouping & Reading Order                      |
|  Characters -> Words -> Lines -> TextBlocks                  |
+--------------------------------------------------------------+
|  Layer 3: Object Extraction                                  |
|  Chars (bbox/font/size/color), Paths (lines/rects/curves)    |
+--------------------------------------------------------------+
|  Layer 2: Content Stream Interpreter                         |
|  Text state, Graphics state, CTM, XObject Do                 |
+--------------------------------------------------------------+
|  Layer 1: PDF Parsing (pluggable backend via PdfBackend)     |
|  lopdf (default)                                             |
+--------------------------------------------------------------+
```

The library is split into three crates:

| Crate              | Description                                      |
|---------------------|--------------------------------------------------|
| `pdfplumber-core`   | Backend-independent data types and algorithms    |
| `pdfplumber-parse`  | PDF parsing and content stream interpretation    |
| `pdfplumber`        | Public API facade (this is what you depend on)   |

## Minimum Supported Rust Version

Rust 1.85 or later.

## License

Licensed under either of:

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT License](http://opensource.org/licenses/MIT)

at your option.
