# pdfplumber-rs

[![CI](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pdfplumber.svg)](https://crates.io/crates/pdfplumber)
[![docs.rs](https://docs.rs/pdfplumber/badge.svg)](https://docs.rs/pdfplumber)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/developer0hye/pdfplumber-rs)
[![License](https://img.shields.io/crates/l/pdfplumber.svg)](https://github.com/developer0hye/pdfplumber-rs/blob/main/LICENSE)

**Evidence-driven PDF extraction for Rust, with a Python `pdfplumber` migration path under active development.**

Use the Rust crate to extract text, words, coordinates, graphics, images, and tables from machine-generated PDFs. The Python package uses the same Rust core and targets ordinary [Python `pdfplumber`](https://github.com/jsvine/pdfplumber) v0.11.10 workflows.

**Maturity: `0.3.x` alpha.** The Rust extraction API is available today. Python compatibility is incomplete and is not yet a full drop-in replacement.

**Release `0.3.0`.** Rust crate `pdfplumber` (import `pdfplumber`) is alpha. Python distribution `pdfplumber-rs` (import `pdfplumber`) is alpha. CLI crate `pdfplumber-cli` installs `pdfplumber` and is alpha. The npm package `pdfplumber-wasm` is experimental. Every surface uses the `Apache-2.0` license and the canonical repository `https://github.com/developer0hye/pdfplumber-rs`; see the [versioned release metadata](docs/releases/v0.3.0.md).

Compatibility work is checked against the pinned upstream release on an indexed corpus of 223 PDFs. Product direction is in the [public roadmap](ROADMAP.md); exact results and remaining gaps stay in the [detailed evidence ledger](PRD.md#13-evidence-ledger).

## Choose `pdfplumber-rs` when…

- You need a native Rust library for structured PDF text extraction.
- You are evaluating a scoped migration from Python `pdfplumber` v0.11.10 and can verify your workflow against the current alpha support boundary.
- Tables, bounding boxes, and coordinate-rich page geometry matter to your application.
- You are building local services, batch pipelines, or command-line automation around structured extraction.

`pdfplumber-rs` does not perform Optical Character Recognition (OCR). For scanned or image-only PDFs, run an OCR tool first and process the resulting searchable PDF.

For tradeoffs against other Rust and Python choices, see the [evidence-separated comparison guide](docs/comparison.md). Current maturity, verified platforms, versions, and limitations are in the [generated support matrix](docs/support.md). The versioned [“What is ready today?” snapshot](docs/readiness/v0.3.0.md) is generated from checked task state and named test contracts.

## Features

- **Text extraction** with spatial grouping into words, lines, and text blocks
- **Table detection** using lattice (line-based), stream (text-alignment), and explicit strategies, choosable per axis
- **Spatial filtering** via `crop`, `within_bbox`, and `outside_bbox`
- **Embedded font programs** parsed for glyph widths and encodings: CFF/Type1C, TrueType (`hmtx`/`vmtx`), Type1, and the 14 standard fonts
- **CJK support** including CID fonts, Identity-H/V and predefined CMaps, the Adobe-Japan1/GB1/CNS1/Korea1 CID→Unicode tables, EUC-JP/Shift-JIS/JIS7, and vertical writing (`WMode 1`)
- **Right-to-left text** via the Unicode BiDi algorithm with Arabic shaping
- **Rotated pages** handled for text, word order, and tables at 90/180/270 degrees
- **Tagged PDF** structure trees, with characters addressable by MCID for semantic reading order
- **Reading order** for multi-column layouts, plus header/footer detection
- **Images** located on the page and exported as raw stream data
- **Resource budgets** bounding input size, page count, object count, and image bytes for untrusted input
- **Page iteration** with caller-controlled page-at-a-time processing
- **WASM support** via `wasm32-unknown-unknown` target
- **Optional serde** serialization for all data types
- **Optional parallel** processing via rayon

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pdfplumber = "0.3"
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

### Extract Text Lines

Each line carries its bounding box and the characters behind it. Grouping is
vertical, so columns printed side by side share a line — crop the page to read
them separately.

```rust,no_run
use pdfplumber::{Pdf, TextOptions};

fn main() {
    let pdf = Pdf::open_file("document.pdf", None).unwrap();
    let page = pdf.page(0).unwrap();
    for line in page.extract_text_lines(&TextOptions::default()) {
        println!("{} at top={:.1}", line.text(), line.bbox.top);
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

A cell's text is `None` only where another cell spans that position; a cell left
blank reads as an empty string.

For a table ruled between rows but not between columns, give each axis its own
strategy:

```rust,no_run
use pdfplumber::{Strategy, TableSettings};

let settings = TableSettings {
    vertical_strategy: Some(Strategy::Stream),    // columns from text alignment
    horizontal_strategy: Some(Strategy::Lattice), // rows from ruled lines
    ..TableSettings::default()
};
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
pdfplumber = { version = "0.3", default-features = false }
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

Licensed under the [Apache License, Version 2.0](LICENSE).

Releases up to and including `0.2.0` were published as `MIT OR Apache-2.0`; that
grant still stands for those versions. See the repository-wide
[license policy](docs/license.md) for source and package-artifact requirements.
