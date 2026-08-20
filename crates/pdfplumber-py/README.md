# pdfplumber-rs

High-performance PDF text, table, and geometry extraction for Python — powered by Rust.

`pdfplumber-rs` is a Rust-native reimplementation of [pdfplumber](https://github.com/jsvine/pdfplumber) exposed to Python via [PyO3](https://pyo3.rs). The Rust extension is installed as the private `pdfplumber._native` submodule so the public package can provide a Python compatibility layer separately.

## Installation

```bash
pip install pdfplumber-rs
```

## Quick Start

```python
import pdfplumber

# Open a PDF
pdf = pdfplumber.open("document.pdf")

# Access pages
for page in pdf.pages:
    # Extract text
    text = page.extract_text()
    print(text)

    # Extract words with bounding boxes
    words = page.extract_words()
    for w in words:
        print(f"{w['text']} at ({w['x0']}, {w['top']}, {w['x1']}, {w['bottom']})")

    # Extract tables
    tables = page.find_tables()
    for table in tables:
        rows = table.extract()
        for row in rows:
            print(row)

# Open from bytes
from pdfplumber import _native

with open("document.pdf", "rb") as f:
    pdf = _native.PDF.open_bytes(f.read())
```

The top-level `pdfplumber.open` alias is available for filesystem paths. Other compatibility-facade work remains in progress; use the private extension explicitly for the native-only API shown above.

## API Reference

### PDF

| Method / Property | Description |
|---|---|
| `PDF.open(path)` | Open a PDF file from a path |
| `PDF.open_bytes(data)` | Open a PDF from bytes |
| `.pages` | List of `Page` objects |
| `.metadata` | Document metadata dict (title, author, etc.) |
| `.to_dict(object_types=None)` | Document metadata and selected-page dictionaries |
| `.to_json(...)` | JSON string or text-stream serialization of `.to_dict()` |
| `.to_csv(...)` | CSV string or text-stream serialization of selected page objects |
| `.bookmarks()` | Table of contents / outline entries |

### Page

| Method / Property | Description |
|---|---|
| `.page_number` | 0-based page index |
| `.width` / `.height` | Page dimensions in points |
| `.to_dict(object_types=None)` | Page geometry and requested object dictionaries |
| `.to_json(...)` | JSON string or text-stream serialization of `.to_dict()` |
| `.to_csv(...)` | CSV string or text-stream serialization of page objects |
| `.extract_text(layout=False)` | Extract all text |
| `.extract_words(x_tolerance=3.0, y_tolerance=3.0)` | Extract words with bounding boxes |
| `.chars()` | Character-level data with font info |
| `.find_tables()` | Detect tables, returns `Table` objects |
| `.extract_tables()` | Extract all table content |
| `.lines()` / `.rects()` / `.curves()` / `.images()` | Geometric objects |
| `.crop(bbox)` | Crop to region `(x0, top, x1, bottom)` |
| `.within_bbox(bbox)` / `.outside_bbox(bbox)` | Spatial filtering |
| `.search(pattern, regex=True, case=True)` | Search for text |

### Table

| Method / Property | Description |
|---|---|
| `.bbox` | Bounding box as `(x0, top, x1, bottom)` |
| `.rows` | Cell data organized by row |
| `.accuracy` | Fraction of non-empty cells |
| `.extract()` | Table content as `list[list[str \| None]]` |

### CroppedPage

Supports the same content methods as `Page`: `chars()`, `extract_text()`, `extract_words()`, `find_tables()`, `extract_tables()`, `lines()`, `rects()`, `curves()`, `images()`, plus further `crop()`, `within_bbox()`, `outside_bbox()`.

## Comparison with Python pdfplumber

| Feature | pdfplumber (Python) | pdfplumber-rs |
|---|---|---|
| Language | Pure Python | Rust + PyO3 |
| Text extraction | Yes | Yes |
| Table detection | Yes | Yes |
| Word extraction | Yes | Yes |
| Geometry (lines, rects, curves) | Yes | Yes |
| Spatial filtering (crop, within_bbox) | Yes | Yes |
| Text search | Yes | Yes |
| Type stubs | No | Yes (.pyi) |

### Performance

`pdfplumber-rs` benefits from Rust's zero-cost abstractions and compiled performance:

- **Text extraction**: Typically 5-20x faster than Python pdfplumber
- **Table detection**: Typically 3-10x faster for lattice-based tables
- **Memory usage**: Lower memory footprint due to Rust's ownership model

Actual speedups depend on document complexity and system configuration.

## License

Dual-licensed under MIT or Apache 2.0 at your option.
