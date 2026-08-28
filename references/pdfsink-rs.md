# pdfsink-rs (Rust)

- **URL:** https://github.com/clark-labs-inc/pdfsink-rs
- **Observed source:** [`980d9f7b8ec44456f3d54427f4ced747b6eb6154`](https://github.com/clark-labs-inc/pdfsink-rs/blob/980d9f7b8ec44456f3d54427f4ced747b6eb6154/README.md) on 2026-08-28
- **License:** MIT
- **Repository status at observation:** Public and not archived

## Relevant API shape

The pinned library opens a document through `PdfDocument::open`, materializes
ordered pages, and exposes document- and page-level text and word extraction.
Its `TextOptions` defaults match the SCORE-002 word request on 3-point x/y
tolerances, no layout, no blank characters, no text flow, and expanded
ligatures. Its table settings also expose line strategies and the corresponding
snap, join, intersection, and minimum-word controls.

The pinned command-line interface opens the full document but selects only one
page for text, word, and table commands. SCORE-003 therefore uses a thin library
adapter for page-preserving full-document work instead of repeatedly reopening
the same PDF through that interface.

## Relevance to pdfplumber-rs

- Pure-Rust, `pdfplumber`-inspired extraction competitor
- Useful reference for materially equivalent text and geometry options
- Self-published performance claims are not evidence for this project
- Only independently run cases that pass exact output preflight may be timed
