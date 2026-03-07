# pdfplumber — Agent Working Memory

```bash
cargo test -p pdfplumber
cargo check -p pdfplumber --features serde,parallel
cargo doc -p pdfplumber --no-deps
```

**~150 tests (unit + integration) | ~5,200 lines | 4 files | 2026-03-06**

---

## Project State

Public API facade for the pdfplumber-rs workspace. Ties together `pdfplumber-core` and `pdfplumber-parse` into a single ergonomic API surface. Re-exports the entire `pdfplumber-core` type system — callers import only this crate.

### What's Built

| Module | Description |
|--------|-------------|
| `pdf.rs` | `Pdf` struct — open, pages_iter, page, metadata, page_count, form_fields, bookmarks, signatures. **2,435 lines — NEEDS SPLITTING** |
| `page.rs` | `Page` struct — extract_text, chars, words, lines, rects, curves, images, find_tables, crop, filter, structure_elements, hyperlinks, annotations. **1,716 lines — NEEDS SPLITTING** |
| `cropped_page.rs` | `CroppedPage` / `FilteredPage` — same API as `Page` on subregions. **886 lines — borderline** |

### What's Not Done

- **File splitting** (Agent B owns this):
  - `pdf.rs` (2,435 lines) → `pdf/mod.rs`, `pdf/pages.rs`, `pdf/metadata.rs`
  - `page.rs` (1,716 lines) → `page/mod.rs`, `page/text.rs`, `page/geometry.rs`, `page/tables.rs`
- Benches for `find_tables` on complex documents (Tier 3)

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| `pdfplumber-core` | All shared types (re-exported) |
| `pdfplumber-parse` | `LopdfBackend`, content handler traits (re-exported) |
| Used by: `pdfplumber-cli` | CLI commands call `Pdf::open_file` |
| Used by: `pdfplumber-layout` | `Document::from_pdf(&pdf)` |
| Used by: `pdfplumber-raster` | `Rasterizer::render_page(&page)` |
| Used by: `pdfplumber-a11y` | `A11yAnalyzer::analyze(&pdf)` |
| Used by: `pdfplumber-py` | Python bindings via PyO3 |
| Used by: `pdfplumber-wasm` | WASM bindings via wasm-bindgen |
| Used by: `pdfplumber-mcp` | MCP server tools |

---

## Architecture Rules

1. **This crate is a facade.** All logic lives in `-core` or `-parse`. `pdfplumber` just composes and re-exports.
2. **`pages_iter()` yields `Result<Page, PdfError>`.** It does NOT panic. Callers handle errors.
3. **`Page::new(page_number, width, height, chars)`** — synthetic page constructor for tests. Exists and is public.
4. **`Page::with_geometry(page_number, width, height, chars, lines, rects, curves)`** — full synthetic constructor.
5. **`img.bbox()` is a method, not a field.** This has bitten agents before — `Image` uses `bbox()` not `.bbox`.

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md and README.md. Flagged 2 files for Agent B splitting.
- **`pdf.rs` split note**: When splitting, ensure `PagesIter` stays in `pdf/mod.rs` or is re-exported cleanly — it's in `pub use pdf::{PagesIter, Pdf}` at the crate root.
