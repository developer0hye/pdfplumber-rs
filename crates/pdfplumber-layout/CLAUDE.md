# pdfplumber-layout — Agent Working Memory

```bash
cargo test -p pdfplumber-layout
cargo check -p pdfplumber-layout --features serde
cargo doc -p pdfplumber-layout --no-deps
```

**~80 tests (unit + integration) | ~2,800 lines | 10 files | 2026-03-06**

---

## Project State

Semantic layout inference on top of pdfplumber extraction output. All classification is rule-based: geometric and typographic signals only — font size ratios, font names, bounding boxes, indentation, bullet prefixes. No ML, no new mandatory deps.

### What's Built

| Module | Description |
|--------|-------------|
| `classifier` | `BlockClassifier` — assigns semantic role to each extraction block |
| `extractor` | `extract_page_layout` — column-aware, header/footer-suppressing layout pass |
| `sections` | `Section` — heading + body block grouping |
| `headings` | `Heading`, `HeadingLevel` (H1–H4) from font-size ratio |
| `paragraphs` | `Paragraph` with `is_list_item` / `is_caption` flags |
| `figures` | `Figure`, `FigureKind` (Image / PathDense) |
| `lists` | `List`, `ListItem`, `ListKind`, `extract_lists_from_section` |
| `markdown` | `Document::to_markdown()` + per-block helpers |
| `document` | `Document`, `DocumentStats` — root type, full-text, word_count, page_text |

### What's Not Done

- `benches/layout.rs` — criterion benchmarks for `extract_page_layout` on multi-page PDFs (Tier 3)
- `examples/extract_layout.rs` — pending Agent D
- `examples/to_markdown.rs` — pending Agent D
- `proptest` round-trips on list detection (Tier 3)

---

## How This Fits in the Workspace

| Dependency | What it gives us | Location |
|------------|-----------------|----------|
| `pdfplumber` | `Pdf`, `Page`, extraction API | `crates/pdfplumber` |
| `pdfplumber-core` | `BBox`, `Char`, `Table`, `StructElement` | `crates/pdfplumber-core` |
| Used by: `pdfplumber-mcp` | `pdf.layout`, `pdf.to_markdown` tools | `crates/pdfplumber-mcp` |

---

## Architecture Rules

1. **Rule-based only.** No tokenizers, no embeddings, no external model calls.
2. **`extract_lists_from_section` is public API.** It takes `&Section` → `Vec<List>`. Callers can use it independently of `Document`.
3. **`LayoutBlock` is `#[non_exhaustive]`** — do not match against it without a wildcard arm.
4. **`flush_list_run` is a standalone fn**, not a closure. This was required to avoid borrow conflicts in the loop — do not refactor back to closure.
5. **Column detection is `ColumnMode::Auto` by default.** Override with `LayoutOptions`. Never hardcode column splits in the classifier.

---

## Decisions Log

- **2026-03-06 (L6 lane)**: `extract_lists_from_section` added. `flush_list_run` extracted from closure to fix borrow checker. `Document::word_count()`, `page_text()`, `impl From<Document> for String` added. 9 new document tests.
- **2026-03-06**: Added CLAUDE.md and README.md.
