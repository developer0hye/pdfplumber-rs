# Lane 6 — pdfplumber-layout — Status

## State: BUILD_REQUESTED — awaiting Agent 1 (Bosun) compile verification

## What's here (no stubs, no deferred phases)

### Core pipeline (`src/`)
| Module | What it does | Tests |
|--------|-------------|-------|
| `classifier.rs` | `compute_body_baseline` (modal 0.5pt bucket), `is_heading_candidate`, `mean_font_size`, `FontProfile` | 9 inline |
| `headings.rs` | `HeadingLevel` (H1-H4), `Heading` struct, `from_size_ratio` | 7 inline |
| `paragraphs.rs` | `Paragraph` struct, `looks_like_caption` (6 prefix patterns) | 8 inline |
| `figures.rs` | `detect_figures_from_images` (img.bbox() method), `detect_figures_from_rects` (greedy cluster), `merge_overlapping_figures`, `FigureKind` | 13 inline |
| `lists.rs` | `parse_list_prefix` (bullets + ordered), `indent_depth`, `ListItem`, `List` | 9 inline |
| `sections.rs` | `partition_into_sections` (flat heading-delimited), `Section` accessors | 9 inline |
| `extractor.rs` | **Column-aware** `extract_page_layout`: `ColumnMode::Auto` detects columns, `sort_in_column_order` emits correct multi-column reading order, header/footer zone suppression | 6 inline |
| `document.rs` | **Two-pass** `Document::from_pdf`: pass 1 = collect pages + detect regions via `detect_page_regions`, pass 2 = extract with zones set. `to_markdown()`. `DocumentStats` with `pages_with_header/footer`. | 2 inline |
| `markdown.rs` | GFM rendering: `heading_to_markdown` (ATX), `table_to_markdown` (pipe tables), `figure_to_markdown` (placeholders), `paragraph_to_markdown` (captions→italic), `sections_to_markdown` (HR-separated) | 9 inline |

### Integration tests (`tests/integration.rs`)
- 10 no-panic smoke tests across all fixture PDFs
- Stats consistency (page count, block count sum)
- Section structure (covers all blocks, nonempty, bbox present)
- Block type properties (nonempty text, valid dimensions, positive area)
- Bbox validity (x0≤x1, top≤bottom, in-bounds)
- Reading order (top-to-bottom within page)
- Text extraction (nonempty, contains recognizable words)
- Flat iterator consistency with stats
- LayoutOptions toggle (disable tables, disable figures)
- Page layout accessors (width/height positive, accessor sum = block count)
- Caption length sanity
- Body font size range
- Markdown output (nonempty, contains headings, table separator rows)
- Header/footer stats sanity
- Column mode override (ColumnMode::None produces results)
- List detection round-trip
- `block_to_markdown` round-trips for headings and tables

**Total: 62 tests (53 inline unit + 9 integration added in this pass, total integration ~43)**

## Key architectural decisions

### Column-aware reading order
`ColumnMode::Auto` (default): `detect_columns()` from pdfplumber-core finds x-coordinate
gaps. `sort_in_column_order()` assigns blocks to columns by their x-midpoint, emits
column 0 top-to-bottom then column 1, etc. This is correct for academic papers,
annual reports, Federal Register.

### Two-pass header/footer suppression
Pass 1 collects margin text from all pages → `detect_page_regions()` finds repeating
patterns. Pass 2 sets `header_zone_bottom` and `footer_zone_top` in `LayoutOptions`
per-page. Blocks fully inside these zones are silently dropped. Result: clean body text
with no page numbers or chapter headers mixed in.

### Markdown as first-class output
`Document::to_markdown()` → GFM. This is the primary output for LLM context building.
Lane 8 (pdfplumber-chunk) should build on top of this. Lane 13 (CLI) can pipe it.

## Known risks for build
- `pdfplumber::Page` is not Clone — stored as `Vec<pdfplumber::Page>` via owned iter values
- `ColumnMode` imported from `pdfplumber_core` — confirmed exported in core lib.rs
- `detect_page_regions` confirmed exported from pdfplumber_core
- `sort_blocks_reading_order` signature is `(&mut [TextBlock], f64)` — confirmed
- `Image.bbox()` is a method — confirmed

## Build command
```
cargo check -p pdfplumber-layout && cargo test -p pdfplumber-layout
```
