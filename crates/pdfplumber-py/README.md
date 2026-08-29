# pdfplumber-rs

Alpha Python migration path for `pdfplumber` workflows, powered by evidence-driven PDF extraction in Rust.

`pdfplumber-rs` is an alpha migration implementation targeting scoped [Python `pdfplumber`](https://github.com/jsvine/pdfplumber) workflows via [PyO3](https://pyo3.rs). Compatibility is incomplete, so validate your workflow against the published gaps instead of treating it as a complete drop-in replacement. The Rust extension is installed as the private `pdfplumber._native` submodule so the public package can provide a Python compatibility layer separately.

Distribution `pdfplumber-rs` installs import package `pdfplumber` and native module `pdfplumber._native`. Release `0.3.0` is alpha, uses the `Apache-2.0` license, and comes from `https://github.com/developer0hye/pdfplumber-rs`.

The current-source policy for the next release supports exactly CPython 3.13.
Both the wheel and source distribution are installed and executed in required
Continuous Integration. Python 3.14 is excluded by `Requires-Python`, and PyPy
is not supported; see the [Python support policy](../../docs/python-support.md).
Use the evidence-scoped [migration guide](../../docs/python-migration.md) to
inventory an upstream application, build isolated reference and candidate
environments, compare the same workload, and make a reversible cutover decision.
If upgrading from this project's 0.2.0 pre-parity Python API, use the
[pre-parity binding guide](../../docs/pre-parity-python-migration.md) for the
method-to-property, page-number, extension-namespace, and rollback changes.

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

Page and object geometry crosses top- and bottom-origin conventions. The
[coordinate-system guide](../../docs/coordinate-systems.md) diagrams the
transform and distinguishes `mediabox`, `cropbox`, `bbox`, `top`/`bottom`,
`y0`/`y1`, and `doctop` before these values leave the Python surface.
The [crop-semantics guide](../../docs/crop-semantics.md) documents the current
mixed migration state: object-list properties apply upstream-style transforms,
while extraction methods still consume the legacy Rust cropped view.
The [text-option guide](../../docs/text-options.md) lists every pinned v0.11.10
text keyword and example, then distinguishes the options this alpha actually
accepts.
The [table-setting guide](../../docs/table-settings.md) does the same for all
pinned table settings and records that this alpha's two table methods currently
accept no settings argument.
The [object-dictionary schema guide](../../docs/object-dictionary-schemas.md)
lists the pinned ordered schemas, accessors, derived edges, and serialization
rules before identifying every current Python-adapter schema gap.
The [visual-debugging guide](../../docs/visual-debugging.md) records the pinned
PDFium/Pillow raster contract and makes explicit that this alpha does not expose
`Page.to_image`, `PageImage`, or the visual drawing helpers.
The [error and resource-limit guide](../../docs/errors-and-resource-limits.md)
separates pinned exception/warning behavior from the adapter's private native
error classes and its currently absent resource-budget controls.

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
Rust API's 0-based convention. The [page-numbering guide](../../docs/page-numbering.md)
shows how this differs from one-based compatibility page numbers and zero-based
Python list positions.

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
defines shared inputs, and the [output-equivalence preflight](../../docs/benchmarks/equivalence-v0.3.0.md)
rejects mismatched semantics or canonical results before timing. The
[separated stage suite](../../docs/benchmarks/stages-v0.3.0.md) isolates seven
component clocks, including installed-candidate PyO3 conversion. The
[resource metric suite](../../docs/benchmarks/metrics-v0.3.0.md) repeats only
exact-output-eligible cases for separately instrumented CPU, process-lifetime
peak memory, and explicitly `tracemalloc`-scoped Python allocation fields.
The [workload-scenario suite](../../docs/benchmarks/scenarios-v0.3.0.md)
separates warmed opens, same-live-page cache hits, and matched page scopes.
The [run-provenance contract](../../docs/benchmarks/provenance-v0.3.0.md)
records environment/build inputs and retains five raw repetitions plus descriptive summaries.
The versioned `SCORE-008` [benchmark result assets](../../docs/benchmarks/results-v0.3.0.md)
retain that complete exact-tag run. The `SCORE-009` retention audit and
[comparison policy](../../docs/comparison.md) withdraw those assets if semantic
reproduction or output equivalence fails, without promoting a ranking.
The `SCORE-013` [regression alert policy](../../docs/benchmarks/regressions-v0.3.0.md)
compares paired baseline/current runs only after the same exact semantic gate and
records noisy overlap without promoting it into a regression.

## License

Licensed under the [Apache License, Version 2.0](../../LICENSE).
