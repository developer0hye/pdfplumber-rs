# pdfplumber-parse — Agent Working Memory

```bash
cargo test -p pdfplumber-parse
cargo check -p pdfplumber-parse
cargo doc -p pdfplumber-parse --no-deps
```

**~200 tests (unit + integration) | ~16,000 lines | 22 files | 2026-03-06**

---

## Project State

PDF format parsing (Layer 1) and PostScript content stream interpretation (Layer 2) for pdfplumber-rs. Receives raw PDF bytes, produces `pdfplumber-core` types. The `lopdf` crate handles PDF object model parsing; this crate handles everything above that.

### What's Built

| Module | Description |
|--------|-------------|
| `lopdf_backend` | PDF structure: pages, resources, fonts, images, metadata, structure tree, annotations, forms, hyperlinks, bookmarks, signatures. **5,251 lines — NEEDS SPLITTING** |
| `interpreter` | PostScript content stream executor — all text, path, color, graphics operators. **3,265 lines — NEEDS SPLITTING** |
| `text_state` | PDF text state machine: Tf, Tm, Td, Tj matrices. **856 lines — borderline** |
| `text_renderer` | Character event production from text-showing operators |
| `tokenizer` | PDF content stream tokenizer. **1,457 lines — NEEDS SPLITTING** |
| `font_metrics` | Per-glyph width tables. **1,444 lines — DATA TABLE, exempt from split** |
| `interpreter_state` | Graphics state stack (q/Q) |
| `handler` | `ContentHandler` trait — `CharEvent`, `ImageEvent`, `PathEvent`, `PaintOp` |
| `backend` | `PdfBackend` trait definition |
| `cmap` | ToUnicode CMap parsing and lookup |
| `cid_font` | CIDFont/Type0 font metrics and encoding |
| `cjk_encoding` | CJK glyph-to-unicode mapping |
| `char_extraction` | `char_from_event` — content handler to Char converter |
| `truetype` | TrueType glyph width and vertical metrics extraction |
| `page_geometry` | Page size, rotation, and CTM computation |
| `standard_fonts` | Standard 14 PDF fonts — metrics and encoding |
| `color_space` | PDF color space parsing and conversion |
| `error` | `BackendError` |
| `cff` | CFF (Compact Font Format) embedded font parsing |
| `adobe_*_ucs2` | Predefined CMap tables for CJK (CNS1, GB1, Japan1, Korea1) |

### What's Not Done

- **File splitting** (Agent B owns this):
  - `lopdf_backend.rs` (5,251 lines) → `backend/` module directory
  - `interpreter.rs` (3,265 lines) → `interpreter/` module directory
  - `tokenizer.rs` (1,457 lines) → `tokenizer/` module directory
- `proptest` round-trip tests for tokenizer (Tier 3)
- Fuzz target on tokenizer (Tier 3) — this is the attack surface

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| `pdfplumber-core` | All shared data types |
| `lopdf` | PDF object model parsing (external) |
| Used by: `pdfplumber` | re-exports `LopdfBackend`, `PdfBackend`, content handler traits |

---

## Architecture Rules

1. **`lopdf_backend` is Layer 1; `interpreter` is Layer 2.** They are different abstraction levels — don't mix concerns.
2. **`ContentHandler` is the seam.** All extraction flows through the trait — don't add direct coupling between interpreter and consumers.
3. **`font_metrics.rs` is a data table.** It's 1,444 lines of lookup data and is explicitly exempt from the 800-line file cap.
4. **CJK encoding is complete.** All four Adobe predefined CMap collections are present. Do not add workarounds — the tables are the source of truth.

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md and README.md. Flagged 3 files for Agent B splitting.
- **font_metrics exemption**: Data lookup table. No functional code. Exempt from 800-line cap. Documented here as the authoritative exemption record.
