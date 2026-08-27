# pdfplumber-rs

[![CI](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/developer0hye/pdfplumber-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/pdfplumber.svg)](https://crates.io/crates/pdfplumber)
[![docs.rs](https://docs.rs/pdfplumber/badge.svg)](https://docs.rs/pdfplumber)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/developer0hye/pdfplumber-rs)
[![License](https://img.shields.io/crates/l/pdfplumber.svg)](https://github.com/developer0hye/pdfplumber-rs/blob/main/LICENSE)

**Evidence-driven PDF extraction for Rust, with an alpha Python `pdfplumber` migration path.**
([evidence](docs/support.md#surface-summary))

Use the Rust crate to extract text, words, coordinates, graphics, images, and tables from machine-generated PDFs. The Python package uses the same Rust core and targets ordinary [Python `pdfplumber`](https://github.com/jsvine/pdfplumber) v0.11.10 workflows.
[Surface evidence](docs/support.md#features-and-known-limitations) records the exact features and boundaries behind those claims.

**Maturity: `0.3.x` alpha.** The Rust extraction API is available today. Python compatibility is incomplete and is not yet a full drop-in replacement. See the [maturity evidence](docs/support.md#surface-summary).

**Release `0.3.0`.** Rust crate `pdfplumber` (import `pdfplumber`) is alpha. Python distribution `pdfplumber-rs` (import `pdfplumber`) is alpha. CLI crate `pdfplumber-cli` installs `pdfplumber` and is alpha. The npm package `pdfplumber-wasm` is experimental. Every surface uses the `Apache-2.0` license and the canonical repository `https://github.com/developer0hye/pdfplumber-rs`; see the [versioned release notes](docs/releases/v0.3.0.md) and [support evidence](docs/support.md#surface-summary).

Compatibility work is checked against the pinned upstream release on an [indexed corpus of 223 PDFs](compat/tests/test_corpus_index.py). User-visible release changes are curated in the [changelog](CHANGELOG.md). Product direction is in the [public roadmap](ROADMAP.md); exact results and remaining gaps stay in the [detailed evidence ledger](PRD.md#13-evidence-ledger).

## Choose `pdfplumber-rs` when…

- You need a native Rust library for structured PDF text extraction. ([evidence](docs/support.md#rust))
- You are evaluating a scoped migration from Python `pdfplumber` v0.11.10 and can verify your workflow against the current alpha support boundary. ([evidence](docs/readiness/v0.3.0.md#ready-workflows))
- Tables, bounding boxes, and coordinate-rich page geometry matter to your application. ([evidence](docs/support.md#rust))
- You are building local services, batch pipelines, or command-line automation around structured extraction. ([evidence](docs/readiness/v0.3.0.md#ready-workflows))

`pdfplumber-rs` does not perform Optical Character Recognition (OCR). For scanned or image-only PDFs, run an OCR tool first and process the resulting searchable PDF. ([evidence](docs/support.md#rust))

For common extraction and migration questions, see the [Frequently Asked Questions](docs/faq.md). [Privacy and local processing](docs/privacy.md) documents the document-upload, telemetry, host-application, and optional-executable boundaries. The [dated adoption baseline](docs/adoption/baseline-2026-08-26.md) separates observed registry, traffic, issue, dependent, and evaluator signals from unavailable measurements and future targets. For tradeoffs against other Rust and Python choices, see the [evidence-separated comparison guide](docs/comparison.md). Current maturity, verified platforms, versions, and limitations are in the [generated support matrix](docs/support.md). The versioned [“What is ready today?” snapshot](docs/readiness/v0.3.0.md) is generated from checked task state and named test contracts.

## Features

- **Text extraction** with spatial grouping into words, lines, and text blocks ([evidence](docs/support.md#rust))
- **Table detection** using lattice (line-based), stream (text-alignment), and explicit strategies, choosable per axis ([evidence](docs/support.md#rust))
- **Spatial filtering** via `crop`, `within_bbox`, and `outside_bbox` ([evidence](docs/support.md#rust))
- **Embedded font programs** parsed for glyph widths and encodings: CFF/Type1C, TrueType (`hmtx`/`vmtx`), Type1, and the 14 standard fonts ([evidence](docs/support.md#rust))
- **CJK support** including CID fonts, Identity-H/V and predefined CMaps, the Adobe-Japan1/GB1/CNS1/Korea1 CID→Unicode tables, EUC-JP/Shift-JIS/JIS7, and vertical writing (`WMode 1`) ([evidence](docs/support.md#rust))
- **Right-to-left text** via the Unicode BiDi algorithm with Arabic shaping ([evidence](docs/support.md#rust))
- **Rotated pages** handled for text, word order, and tables at 90/180/270 degrees ([evidence](docs/support.md#rust))
- **Tagged PDF** structure trees, with characters addressable by MCID for semantic reading order ([evidence](docs/support.md#rust))
- **Reading order** for multi-column layouts, plus header/footer detection ([evidence](docs/support.md#rust))
- **Images** located on the page and exported as raw stream data ([evidence](docs/support.md#rust))
- **Resource budgets** bounding input size, page count, object count, and image bytes for untrusted input ([evidence](docs/support.md#rust))
- **Typed Rust errors** with preserved sources, safe default formatting, and available page/object context ([evidence](compat/tests/test_rust_errors.py))
- **Page iteration** with caller-controlled page-at-a-time processing ([evidence](docs/support.md#rust))
- **WASM support** via `wasm32-unknown-unknown` target ([evidence](docs/support.md#webassembly))
- **Optional serde** serialization for all curated models, with a frozen JSON compatibility [policy](docs/rust-serde-schema.md) ([evidence](compat/tests/test_rust_serde_schema.py))
- **Optional parallel** processing via rayon, with documented ordering and
  shared-state guarantees ([guide](docs/rust-concurrency.md),
  [evidence](compat/tests/test_rust_concurrency.py))

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pdfplumber = "0.3"
```

### Feature Flags

| Feature    | Default | Description                                                    |
|------------|---------|----------------------------------------------------------------|
| `std`      | Yes     | Enables file-path APIs (`Pdf::open_path`). Disable for WASM.  |
| `serde`    | No      | Adds `Serialize`/`Deserialize`; curated-model JSON follows [`serde-json-v1`](docs/rust-serde-schema.md). |
| `parallel` | No      | Enables `Pdf::pages_parallel()` via rayon. Not WASM-compatible.|

## Quick Start

### Extract Text

```rust,no_run
use pdfplumber::{Pdf, TextOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::open_path("document.pdf", None)?;
    for page in pdf.pages() {
        let page = page?;
        let text = page.extract_text(&TextOptions::default());
        println!("Page {}: {}", page.page_number(), text);
    }
    Ok(())
}
```

### Extract Text Lines

Each line carries its bounding box and the characters behind it. Grouping is
vertical, so columns printed side by side share a line — crop the page to read
them separately.

```rust,no_run
use pdfplumber::{Pdf, TextOptions};

fn main() {
    let pdf = Pdf::open_path("document.pdf", None).unwrap();
    let page = pdf.pages().get(0).unwrap();
    for line in page.extract_text_lines(&TextOptions::default()) {
        println!("{} at top={:.1}", line.text(), line.bbox.top);
    }
}
```

### Extract Tables

```rust,no_run
use pdfplumber::{Pdf, TableSettings};

fn main() {
    let pdf = Pdf::open_path("document.pdf", None).unwrap();
    let page = pdf.pages().get(0).unwrap();
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

Complete compiled programs for text, word bounding boxes, tables, page
geometry, metadata, encrypted and malformed inputs, Serde JSON, and parallel
page batches are listed in the [task-oriented Rust examples](docs/rust-examples.md).

### Extract Characters

```rust,no_run
use pdfplumber::Pdf;

fn main() {
    let pdf = Pdf::open_path("document.pdf", None).unwrap();
    let page = pdf.pages().get(0).unwrap();
    for ch in page.chars() {
        println!(
            "'{}' at ({:.1}, {:.1}) font={} size={:.1}",
            ch.text, ch.bbox.x0, ch.bbox.top, ch.fontname, ch.size
        );
    }
}
```

## Rust API Boundary

The root `pdfplumber` crate is the only dependency for ordinary extraction, and
`Pdf` is the canonical high-level entry point. Its page methods accept the
options and return the errors and extracted data re-exported at the same root,
as enforced by the [facade contract](compat/tests/test_rust_facade.py). The
[public rustdoc contract](docs/rust-api.md) names the stable generated surface,
its documentation quality rules, and the all-feature Continuous Integration
gates that reject missing or incomplete API documentation.
([evidence](compat/tests/test_rust_rustdoc.py))

For ordinary applications, do not add direct dependencies on `pdfplumber-core` or
`pdfplumber-parse`. Those workspace crates contain reusable algorithms and parser
internals for advanced contributors; they are not additional steps in the
high-level path.

## Rust Data Models

Import related extraction types from the curated `pdfplumber::models` module,
or continue using their source-compatible crate-root re-exports. The
[Rust data-model contract](docs/rust-data-models.md) defines the stable `0.3.x`
families and documents their units, top-left coordinate system, collection
ordering, optional fields, and the separate serialized-schema boundary.

With the optional `serde` feature, all curated models implement `Serialize`
and `Deserialize`. Their direct `serde_json` representation follows the
[`serde-json-v1` compatibility policy](docs/rust-serde-schema.md), which freezes
field names, JSON value shapes, and enum encodings across `0.3.x`.

## Rust Input API

The canonical constructors form one source-named family:

| Input | Constructor | Availability |
|-------|-------------|--------------|
| Filesystem path | `Pdf::open_path` | Default `std` feature |
| In-memory byte slice | `Pdf::open_bytes` | All supported targets, including WebAssembly |
| Synchronous reader | `Pdf::open_reader` | Any `std::io::Read`; `Seek` is not required |

Each constructor returns an owned `Pdf` that does not borrow its path, byte
slice, or reader. Path files are closed, byte slices need to live only for the
call, and reader input is consumed from the current reader position through
end-of-file without being retained. Reader and path I/O failures have
`PdfErrorKind::Io`; bytes that cannot be parsed as a PDF have
`PdfErrorKind::Parse`. Resource-limit and password errors use the same kinds
for every input kind. Encrypted inputs use the parallel
`Pdf::open_path_with_password`, `Pdf::open_bytes_with_password`, and
`Pdf::open_reader_with_password` family. Best-effort repair is currently
byte-only through `Pdf::open_bytes_with_repair`.

## Rust Errors

`PdfError` exposes a stable `PdfErrorKind`, safe `PdfErrorContext`, and typed
resource-limit details. Its ordinary `Display` and `Debug` output does not echo
input paths, parser details, passwords, or document content. The underlying
cause is preserved for explicit inspection through `std::error::Error::source`;
source-chain output may be sensitive and belongs only in a protected diagnostic
sink. The [Rust error guide](docs/rust-errors.md) documents classification,
page/object context, actionable messages, source handling, and migration from
the former string-payload variants.

## Rust Page API

`Pdf::pages()` returns a borrowed collection view. Creating that view does not
clone the document or interpret any page content. Select a zero-based page
directly with `pdf.pages().get(0)?`; only that page's content stream is
interpreted, so selecting a later page does not process the pages before it.

Use `for page in pdf.pages()` to process the document on demand, one independently
owned `Page` at a time. The iterator is exact-sized and double-ended, supports
normal iterator adapters, and does not retain pages after yielding them unless
the caller chooses to collect them. `Pdf::page` and `Pdf::pages_iter` remain
source-compatible shortcuts during the alpha line.

## Rust Concurrency

`Pdf`, its borrowed page views, and owned `Page`/`CroppedPage` results implement
`Send` and `Sync`. An opened document can be shared as `Arc<Pdf>` for concurrent
immutable reads. Document-wide object and image-byte budgets remain shared
across page extractions that reach resource accounting, including retries.

With the optional `parallel` feature, `Pdf::pages_parallel()` returns one
`Result` per page in page-index order through the current Rayon thread pool.
The Python binding has a separate GIL and cache-lock boundary and does not
promise equivalent CPU parallelism. See the complete
[Rust concurrency and thread-safety contract](docs/rust-concurrency.md).

## WASM Support

For `wasm32-unknown-unknown` targets, disable the default `std` feature. The [WebAssembly support entry](docs/support.md#webassembly) records the current build and execution boundary:

```toml
[dependencies]
pdfplumber = { version = "0.3", default-features = false }
```

Use the bytes-based API:

```rust,ignore
let pdf = Pdf::open_bytes(pdf_bytes, None)?;
let page = pdf.pages().get(0)?;
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
| `pdfplumber`        | High-level public API facade for applications    |

## Minimum Supported Rust Version

[Rust 1.85 or later](docs/support.md#rust).

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

Releases up to and including `0.2.0` were published as `MIT OR Apache-2.0`; that
grant still stands for those versions. See the repository-wide
[license policy](docs/license.md) for source and package-artifact requirements.
