# pdfplumber-core — Agent Working Memory

```bash
cargo test -p pdfplumber-core
cargo check -p pdfplumber-core --features serde
cargo doc -p pdfplumber-core --no-deps
```

**~400 tests (unit) | ~8,000 lines | 25 files | 2026-03-06**

---

## Project State

Foundation crate for the entire pdfplumber-rs workspace. Provides all shared data types, geometric primitives, text grouping algorithms, table detection (lattice + stream strategies), and the `PdfError` type. No PDF parsing knowledge — receives already-extracted data from `pdfplumber-parse`.

### What's Built

| Module | Description |
|--------|-------------|
| `geometry` | `BBox`, `Point`, `Ctm`, `Orientation` — coordinate arithmetic |
| `text` | `Char`, `TextDirection`, CJK detection (`is_cjk`, `is_cjk_text`) |
| `words` | `Word`, `WordExtractor`, `WordOptions` — char → word grouping |
| `layout` | `TextLine`, `TextBlock`, `TextOptions` — word → line → block grouping |
| `shapes` | `Line`, `Rect`, `Curve` — geometric shapes from painted paths |
| `table` | `Table`, `TableFinder`, `TableSettings` — full lattice+stream detection |
| `edges` | `Edge`, `EdgeSource` — table lattice edge derivation |
| `images` | `Image`, `ImageMetadata`, export helpers |
| `painting` | `Color`, `GraphicsState`, `PaintedPath`, `DashPattern` |
| `path` | `Path`, `PathBuilder`, `PathSegment` |
| `struct_tree` | `StructElement` — tagged PDF structure nodes |
| `encoding` | `FontEncoding`, `EncodingResolver` |
| `error` | `PdfError`, `ExtractWarning`, `ExtractOptions` |
| `search` | `SearchMatch`, `SearchOptions`, `search_chars` |
| `annotation` | `Annotation`, `AnnotationType` |
| `bookmark` | `Bookmark` — PDF outline/TOC |
| `hyperlink` | `Hyperlink` |
| `form_field` | `FormField`, `FieldType` |
| `signature` | `SignatureInfo` |
| `metadata` | `DocumentMetadata` |
| `page_regions` | `PageRegions`, `PageRegionOptions` — header/footer zone detection |
| `html` | `HtmlRenderer`, `HtmlOptions` |
| `svg` | `SvgRenderer`, `SvgOptions`, `SvgDebugOptions` |
| `bidi` | Unicode BiDi text direction analysis |
| `dedupe` | Duplicate character deduplication |
| `unicode_norm` | `UnicodeNorm`, `normalize_chars` |

### What's Not Done

- `proptest` round-trip tests on BBox / table detection (Tier 3)
- Fuzz target on table detection inputs (Tier 3)

---

## How This Fits in the Workspace

| Dependency | What it gives us |
|------------|-----------------|
| (none — this is the base) | — |
| Used by: `pdfplumber-parse` | receives type definitions |
| Used by: `pdfplumber` | re-exports the full type surface |
| Used by: `pdfplumber-layout` | `BBox`, `Char`, `Table`, `StructElement` |
| Used by: `pdfplumber-raster` | `Color`, `BBox`, `Char`, `Rect`, `Line` |
| Used by: `pdfplumber-a11y` | `StructElement`, `BBox`, `Hyperlink` |

---

## Architecture Rules

1. **No parsing.** This crate has no knowledge of `lopdf`, PDF object model, or content stream operators.
2. **`BBox` origin is top-left.** `top < bottom` in pixel terms. This matches Python pdfplumber's convention, not raw PDF coordinates.
3. **`PdfError` has no `Internal(String)` variant.** Every error variant is typed.
4. **All public items have rustdoc.** `#![deny(missing_docs)]` is enforced.
5. **`serde` is feature-gated.** Do not add serde derives without the feature flag.

---

## Decisions Log

- **2026-03-06**: Added CLAUDE.md and README.md as part of Shippable Crate Standard pass.
- **font_metrics exemption**: `font_metrics.rs` in `pdfplumber-parse` (not here) is a data lookup table — exempt from 800-line file cap.
