# pdfplumber-layout — Semantic layout inference for PDF documents

Takes raw extraction output from `pdfplumber` and returns a structured `Document` with headings, paragraphs, sections, lists, tables, and figures — ready for LLM context or markdown export.

**Rule-based only. No ML. No new mandatory dependencies.**

## Quick Start

```rust
use pdfplumber::Pdf;
use pdfplumber_layout::Document;

let pdf = Pdf::open_file("report.pdf", None)?;
let doc = Document::from_pdf(&pdf);

// Markdown for LLM context windows
println!("{}", doc.to_markdown());

// Structured access
for section in doc.sections() {
    if let Some(heading) = section.heading() {
        println!("## {}", heading.text());
    }
    for para in section.paragraphs() {
        println!("  {} words", para.text().split_whitespace().count());
    }
}

// Statistics
let stats = doc.stats();
println!("{} sections, {} headings, {} paragraphs, {} tables",
    stats.section_count, stats.heading_count,
    stats.paragraph_count, stats.table_count);
```

## API Overview

| Type | Description |
|------|-------------|
| `Document` | Root type — sections, stats, full-text, markdown, word count |
| `Section` | Heading + body blocks (paragraphs, tables, figures, lists) |
| `Heading` | Text + level (H1–H4) + bbox + font metadata |
| `HeadingLevel` | `H1`/`H2`/`H3`/`H4` inferred from font-size ratio |
| `Paragraph` | Text block with line count, font metadata, list/caption flags |
| `LayoutTable` | Detected table with 2D cell array and bbox |
| `Figure` | Image or path-dense region with no meaningful text |
| `List` | Contiguous run of `ListItem` values (`Ordered`/`Unordered`) |
| `ListItem` | Text, prefix, depth, bbox |
| `LayoutOptions` | Column mode, header/footer zones, font-size thresholds |

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `serde` | Off | `Serialize`/`Deserialize` on all layout types |

## What Makes This Different

- **Header/footer suppression.** Two-pass algorithm detects repeating elements across pages and removes them before building the document model — page numbers and chapter titles don't pollute body text.
- **Column-aware.** `ColumnMode::Auto` detects multi-column layouts. Override with `ColumnMode::Explicit(vec![306.0])` for known split points.
- **List detection.** Bullet chars (•, -, *, ◦, ▪, ▸, ›, —), numeric (1., (1)), and alpha (a), a.) prefixes. Nesting depth inferred from x0 indentation.
- **Markdown export.** `Document::to_markdown()` produces GitHub-Flavored Markdown: ATX headings, pipe tables, fenced code blocks for figures.
- **Zero inference deps.** All classification is geometric/typographic — no tokenizers, no models.

## Inference Signals

| Signal | Used for |
|--------|----------|
| Font size ratio vs body baseline | Heading level assignment |
| Font name contains "Bold"/"Black" | Bold paragraph detection |
| Short line count, centred bbox | Caption detection |
| Bullet/ordinal prefix | List item detection |
| x0 indentation delta | List nesting depth |
| Image bbox overlapping chars | Figure boundary |
| Path density (lines/area) | Figure vs table disambiguation |

## Changelog

### v0.1.0-dev (L6 lane)
- Full layout inference pipeline: classifier → extractor → sections → document
- Column-aware reading order with `ColumnMode::Auto`
- Header/footer zone suppression (two-pass)
- List detection with kind splitting and nesting depth
- Markdown export with GFM tables
- `Document::word_count()`, `Document::page_text()`, `impl From<Document> for String`
- `extract_lists_from_section()` public API
