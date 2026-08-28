# pdfplumber-rs

Alpha Python migration path for `pdfplumber` workflows, powered by evidence-driven PDF extraction in Rust.

`pdfplumber-rs` is an alpha migration implementation targeting scoped [Python `pdfplumber`](https://github.com/jsvine/pdfplumber) workflows via [PyO3](https://pyo3.rs). Compatibility is incomplete, so validate your workflow against the published gaps instead of treating it as a complete drop-in replacement. The Rust extension is installed as the private `pdfplumber._native` submodule so the public package can provide a Python compatibility layer separately.

Distribution `pdfplumber-rs` installs import package `pdfplumber` and native module `pdfplumber._native`. Release `0.3.0` is alpha, uses the `Apache-2.0` license, and comes from `https://github.com/developer0hye/pdfplumber-rs`.

## Installation

```bash
pip install pdfplumber-rs
```

## Distribution and import names

The installable distribution is `pdfplumber-rs`. The Python import package is
`pdfplumber`. The private native module is `pdfplumber._native`.

Do not install `pdfplumber-rs` and Python `pdfplumber` in the same environment.
Both distributions write files under `pdfplumber/`. `pip` treats their distribution
names as different, so it does not resolve this shared-file conflict; `pip check`
can still succeed while installation order can silently select a mixed package.

Before installing, inspect the environment with `python -m pip show pdfplumber`
and `python -m pip show pdfplumber-rs`. Use a new, dedicated virtual environment
that contains exactly one of these distributions. Uninstalling only one
distribution is not a repair: either uninstaller can remove shared files recorded
by the other distribution. If both distributions have ever been installed in
one environment, discard that environment and create a new one, then install
only the distribution you intend to import.

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

Passing `laparams={}` to `pdfplumber.open` enables native horizontal layout analysis. The resulting `textboxhorizontal` and `textlinehorizontal` lists participate in the same page/document object caches and serialization as ordinary objects, and are also available through `.textboxhorizontals` and `.textlinehorizontals`. Horizontal grouping honors `line_overlap`, `char_margin`, `word_margin`, and `line_margin`; vertical layout and embedded-figure hierarchy support remain in progress.

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
| `.textboxhorizontals` / `.textlinehorizontals` | Aggregated horizontal layout objects when `laparams` is supplied |
| `.rust` | Explicit namespace for Rust-native document extensions |

### Page

| Method / Property | Description |
|---|---|
| `.page_number` | 1-based compatibility page number |
| `.width` / `.height` | Page dimensions in points |
| `.rotation` | Inherited page rotation, normalized to 0 through 359 degrees |
| `.bbox` | Original-page bounding box in rotation-aware, top-origin coordinates |
| `.mediabox` | Rotation-aware MediaBox in top-origin page coordinates |
| `.cropbox` | Inherited rotation-aware CropBox, falling back to MediaBox |
| `.trimbox` | Direct rotation-aware TrimBox when present; absent otherwise |
| `.bleedbox` | Direct rotation-aware BleedBox when present; absent otherwise |
| `.artbox` | Direct rotation-aware ArtBox when present; absent otherwise |
| `.initial_doctop` | Cumulative height of preceding pages in the current page view |
| `.point2coord(pt)` | Convert a PDF-space point to top-origin page coordinates |
| `repr(page)` | Return `<Page:N>` using the 1-based document page number |
| `.objects` | Cached mutable dictionary of present page objects keyed by type |
| `.textboxhorizontals` / `.textlinehorizontals` | Cached horizontal layout objects when `laparams` is supplied |
| `.flush_cache(properties=None)` | Discard all or selected cached page properties |
| `.close()` | Discard this original or derived page's cached objects |
| `.to_dict(object_types=None)` | Page geometry and requested object dictionaries |
| `.to_json(...)` | JSON string or text-stream serialization of `.to_dict()` |
| `.to_csv(...)` | CSV string or text-stream serialization of page objects |
| `.extract_text(layout=False)` | Extract all text |
| `.extract_words(x_tolerance=3.0, y_tolerance=3.0)` | Extract words with bounding boxes, width, height, and doctop |
| `.chars` | Character-level data with font info and complete bounding-box geometry |
| `.find_tables()` | Detect tables, returns `Table` objects |
| `.extract_tables()` | Extract all table content |
| `.lines` / `.rects` / `.curves` / `.images` | Geometric objects with top- and bottom-origin coordinates |
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

Supports the same content surface as `Page`: `chars`, `lines`, `rects`, `curves`, `images`, `extract_text()`, `extract_words()`, `find_tables()`, `extract_tables()`, and `point2coord()`, plus further `crop()`, `within_bbox()`, and `outside_bbox()`.

## Rust-Native Extensions

Python `pdfplumber` v0.11.10 does not define high-level document APIs for
bookmarks, forms, signatures, structural validation, or image-byte extraction.
`pdfplumber-rs` exposes these only through `document.rust`, so they cannot be
mistaken for compatibility behavior or silently collide with future upstream
methods. Page indexes and bookmark destinations in this namespace retain the
Rust API's 0-based convention.

| Method | Description |
|---|---|
| `document.rust.bookmarks()` | Outline entries and 0-based destinations |
| `document.rust.form_fields()` | AcroForm fields and 0-based page indexes |
| `document.rust.signatures()` | Signature-field metadata; no cryptographic verification |
| `document.rust.validate()` | Native structural validation issues |
| `document.rust.extract_images(page_index)` | Image metadata and raw bytes for a 0-based page index |

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

### Performance evidence

No text-extraction, table-detection, or memory advantage is currently claimed for
the Python surface. The compatibility corpus measures behavior, not a fair timing
comparison. The [versioned benchmark corpus](../../docs/benchmarks/corpus-v0.3.0.md)
defines shared inputs only; cross-project results remain deferred until `SCORE-002`
through `SCORE-009` satisfy the [comparison policy](../../docs/comparison.md).

## License

Licensed under the [Apache License, Version 2.0](../../LICENSE).
