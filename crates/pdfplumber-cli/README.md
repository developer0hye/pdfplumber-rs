# pdfplumber-cli

Command-line tool to extract text, characters, words, and tables from PDF documents.

**pdfplumber-cli** is the CLI frontend for [pdfplumber-rs](https://github.com/developer0hye/pdfplumber-rs), a Rust port of Python's [pdfplumber](https://github.com/jsvine/pdfplumber).

## Installation

```bash
cargo install pdfplumber-cli
```

## Usage

```bash
pdfplumber <COMMAND> [OPTIONS] <FILE>
```

### Subcommands

| Command  | Description                                      |
|----------|--------------------------------------------------|
| `text`   | Extract text from PDF pages                      |
| `chars`  | Extract individual characters with coordinates   |
| `words`  | Extract words with bounding box coordinates      |
| `tables` | Detect and extract tables from PDF pages         |
| `info`   | Display PDF metadata and page information        |

### Global Options

| Option      | Description                              |
|-------------|------------------------------------------|
| `--version` | Print version number                     |
| `--help`    | Print help information                   |

### Extract Text

```bash
# Extract all text
pdfplumber text document.pdf

# Extract text from specific pages
pdfplumber text document.pdf --pages 1,3-5

# Layout-preserving extraction
pdfplumber text document.pdf --layout

# JSON output (one object per page)
pdfplumber text document.pdf --format json
```

### Extract Characters

```bash
# Tab-separated output (default)
pdfplumber chars document.pdf

# JSON output with all fields (text, fontname, size, bbox, etc.)
pdfplumber chars document.pdf --format json

# CSV output
pdfplumber chars document.pdf --format csv --pages 1
```

Example CSV output:

```
page,text,x0,top,x1,bottom,fontname,size
1,H,72.00,72.00,84.00,84.00,Helvetica,12.00
1,e,84.00,72.00,90.72,84.00,Helvetica,12.00
```

### Extract Words

```bash
# Tab-separated output (default)
pdfplumber words document.pdf

# JSON output
pdfplumber words document.pdf --format json

# CSV output with custom tolerances
pdfplumber words document.pdf --format csv --x-tolerance 5.0 --y-tolerance 2.5
```

Example CSV output:

```
page,text,x0,top,x1,bottom
1,Hello,72.00,72.00,108.00,84.00
1,World,112.00,72.00,148.00,84.00
```

### Extract Tables

```bash
# Human-readable grid format (default)
pdfplumber tables document.pdf

# JSON output
pdfplumber tables document.pdf --format json

# CSV output
pdfplumber tables document.pdf --format csv

# Use stream strategy instead of lattice
pdfplumber tables document.pdf --strategy stream

# Tune detection parameters
pdfplumber tables document.pdf --snap-tolerance 5.0 --join-tolerance 4.0 --text-tolerance 2.0
```

Example grid output:

```
--- Table 1 (page 1, bbox: [72.00, 100.00, 540.00, 300.00]) ---
Name   | Age | City
Alice  | 30  | New York
Bob    | 25  | London
```

### Inspect PDF Info

```bash
# Text summary
pdfplumber info document.pdf

# JSON output
pdfplumber info document.pdf --format json

# Specific pages only
pdfplumber info document.pdf --pages 1-3
```

Example text output:

```
=== PDF Info ===
Pages: 3

--- Page 1 (612.00 x 792.00, rotation: 0°) ---
  Chars:  1250
  Lines:  45
  Rects:  12
  Curves: 0
  Images: 2

=== Summary ===
Total chars:  3200
Total tables: 1
```

## Output Formats

| Subcommand | text (default) | json | csv |
|------------|---------------|------|-----|
| `text`     | Plain text    | JSON lines | — |
| `chars`    | TSV           | JSON array | CSV |
| `words`    | TSV           | JSON array | CSV |
| `tables`   | Grid          | JSON array | CSV |
| `info`     | Summary       | JSON       | — |

## Page Selection

Use `--pages` to select specific pages (1-indexed):

- `--pages 1` — single page
- `--pages 1-5` — range
- `--pages 1,3,5` — list
- `--pages 1-3,7,10-12` — mixed

Omit `--pages` to process all pages.

## License

MIT OR Apache-2.0
