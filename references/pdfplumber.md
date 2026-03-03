# pdfplumber (Python)

- **URL:** https://github.com/jsvine/pdfplumber
- **Stars:** ~9.8k | **License:** MIT | **Latest:** v0.11.9 (2026-01-05)
- **Status:** Actively maintained

## Architecture

Built on **pdfminer.six** (handles PDF parsing, font metrics, char extraction). pdfplumber adds spatial APIs and table detection on top.

**Key source files:**
- `pdf.py` — PDF container, open/close
- `page.py` — Page API, crop, filter, `.chars`/`.lines`/`.rects`/`.curves`
- `table.py` — `TableFinder` class (edge collection → snap → join → intersect → cells → tables)
- `ctm.py` — Coordinate transform matrices
- `utils/` — Geometry helpers, text algorithms, clustering

## Table Detection Pipeline (`table.py`)

1. Collect edges based on strategy: `"lines"`, `"lines_strict"`, `"text"`, `"explicit"`
2. `snap_edges` — merge parallel lines within `snap_tolerance`
3. `join_edges` — connect collinear segments within `join_tolerance`
4. Find intersections within `intersection_tolerance`
5. Construct granular rectangular cells from intersection vertices
6. Group contiguous cells into tables

**Configurable via `TableSettings`:** snap tolerances, edge strategies, `min_words_vertical`/`min_words_horizontal`, `edge_min_length`.

## Key Design Decisions

- Font metrics delegated entirely to pdfminer.six
- Each page object carries full metadata (font, size, position, color)
- Visual debugging via `.to_image()` — renders overlays for chars, lines, rects, table cells
- No PDF parsing of its own — purely spatial analysis layer
