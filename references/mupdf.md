# MuPDF (C)

- **URL:** https://github.com/ArtifexSoftware/mupdf
- **Stars:** 2.6k | **License:** AGPL-3.0
- **Status:** Actively maintained (Artifex Software)

## Text Hierarchy

4-level structure built during sequential content stream parsing:

**Block** (paragraph) → **Line** → **Span** (same font) → **Character**

### Key Files
- `source/fitz/stext-device.c` — Text extraction device
- `include/mupdf/fitz/structured-text.h` — Structure definitions

## Algorithm

- Hierarchy built sequentially as content stream is parsed (not post-processing like pdfminer.six)
- Heuristics consider: font size, font name, horizontal char distance, char width, vertical distances, writing direction/angle
- Stream-order-dependent — produces different results from spatial-clustering approaches

## Reading Order

- `sort=True` reorders blocks top-left to bottom-right
- Multi-column detection uses text background colors and H/V lines as column border hints

## Characteristics

- Very fast (used by Sumatrapdf, PyMuPDF)
- Stream-order approach is simpler than spatial clustering but less robust for complex layouts
- AGPL license restricts commercial use without Artifex license

## Relevance to pdfplumber-rs

Stream-order approach is an alternative to pdfminer.six's spatial clustering. The Block→Line→Span→Char hierarchy is a clean model. Worth comparing behavior on complex multi-column documents.
