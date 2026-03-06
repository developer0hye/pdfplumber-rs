# pdfplumber-parse — PDF parsing backend and content stream interpreter

Layer 1 (PDF format parsing via pluggable backends) and Layer 2 (PostScript content stream interpretation) for pdfplumber-rs. Translates raw PDF bytes into the `pdfplumber-core` type system.

## Quick Start

```rust
use pdfplumber_parse::{LopdfBackend, PdfBackend, ContentHandler};

// Open a PDF through the lopdf backend
let doc = LopdfBackend::open_file("document.pdf").unwrap();
let page_geom = doc.page_geometry(0).unwrap();
println!("Page size: {}×{}", page_geom.width, page_geom.height);

// Interpret content stream — implement ContentHandler to receive events
struct MyHandler { chars: Vec<pdfplumber_core::Char> }
impl ContentHandler for MyHandler {
    fn on_char(&mut self, ev: pdfplumber_parse::CharEvent) {
        self.chars.push(ev.into_char());
    }
    // ... other event handlers
}
```

## API Overview

| Type | Description |
|------|-------------|
| `PdfBackend` | Trait for pluggable PDF parsing backends |
| `LopdfBackend` | Default backend using the `lopdf` crate |
| `ContentHandler` | Trait receiving events from content stream interpretation |
| `TextState` | PDF text state machine (Tf, Tm, Td, font matrices) |
| `CMap` | Character code → Unicode mapping (ToUnicode CMaps) |
| `FontMetrics` | Per-glyph width metrics for character positioning |

## Architecture

- **Two-layer design.** Backend layer (Layer 1) handles PDF object model, cross-reference tables, and page resources. Interpreter layer (Layer 2) executes content streams against a `TextState` machine.
- **Pluggable backend.** `LopdfBackend` is the current implementation. Swapping backends (e.g., for `pdf-rs`) requires only a new `PdfBackend` impl.
- **Event-driven extraction.** The interpreter emits `CharEvent`, `ImageEvent`, `PathEvent`, and `PaintOp` — callers implement `ContentHandler` to collect what they need.
- **Font encoding is deep.** Supports Standard 14, Type1, TrueType, CIDFont/Type0, ToUnicode CMaps, predefined CMaps for CJK (Adobe-CNS1, Adobe-GB1, Adobe-Japan1, Adobe-Korea1), and CFF embedded fonts.

## Feature Flags

None beyond workspace defaults.

## Changelog

### v0.1.0-dev
- `lopdf`-based backend with structure tree, metadata, bookmark, annotation, form, hyperlink, and image extraction
- Full content stream interpreter covering all text, path, color, and graphics operators
- CJK encoding: predefined CMaps for all four Adobe character collections
- TrueType glyph width extraction for accurate character bbox computation
