# Visual debugging

This guide defines the visual-debugging dependencies and observable behavior
of pinned Python `pdfplumber` v0.11.10, then keeps that raster API separate
from this project's current Rust SVG extension, Command-Line Interface,
Python adapter, and WebAssembly surfaces. It is a migration and operations
reference, not a parity declaration.

## Choose the surface first

| Surface | Output | PDF page background | Overlay API |
| --- | --- | --- | --- |
| Pinned Python `pdfplumber` | Pillow raster image, normally saved as PNG | Rendered by PDFium | Mutable `PageImage` drawing methods |
| Current Rust library | SVG text | White page-coordinate canvas only | `SvgRenderer` and typed draw styles |
| Current Command-Line Interface | SVG file | White page-coordinate canvas only | Fixed ordinary or table-pipeline overlays |
| Current Python adapter | None | None | Not exposed |
| Current WebAssembly adapter | None | None | Not exposed |

Pinned Python `pdfplumber` produces a raster page image with raster overlays.
Rust produces SVG markup on a white page-coordinate canvas. These results are
useful for similar investigations, but they are not byte-, pixel-, dependency-,
or API-compatible artifacts.

## Pinned Python dependencies

`pdfplumber==0.11.10` requires `Pillow>=12.2.0` and `pypdfium2>=5.9.0`.
For evidence, the pinned environment resolved Pillow 12.3.0 and pypdfium2
5.13.0. Pillow owns the image and drawing API;
PDFium renders the PDF page.

`Wand`, ImageMagick, and Ghostscript are not dependencies of ordinary
`.to_image()`. Ghostscript is used only by the separate `repair=True` path.
The normal visualization path does not start Ghostscript, ImageMagick, or an
image-viewer process while rendering. `PageImage.show()` is different: it asks
Pillow to open the completed image in a local viewer.

The reference dependencies are ordinary package requirements, not optional
visualization extras:

```console
python -m pip install "pdfplumber==0.11.10"
python -c "from importlib.metadata import requires; print(requires('pdfplumber'))"
```

An application that removes either Pillow or pypdfium2 has an incomplete
upstream installation. A headless application can render and save without a
desktop viewer, but should not call `show()` unless its Pillow environment has
a suitable viewer integration.

## Creating the pinned Python `PageImage`

Call `Page.to_image(...)` on an original or cropped page:

```python
import pdfplumber

with pdfplumber.open("document.pdf") as pdf:
    image = pdf.pages[0].to_image(resolution=150, antialias=True)
    image.save("page-1.png")
```

In one call, only one of `resolution`, `width`, and `height` may be supplied.

| Argument | Pinned default and behavior |
| --- | --- |
| `resolution` | `resolution` defaults to 72 pixels per inch. It is passed to PDFium as a scale relative to 72. |
| `width` | Unset by default. `width` computes `resolution = 72 * width / page.width`. |
| `height` | Unset by default. `height` computes `resolution = 72 * height / page.height`. |
| `antialias` | `antialias=False`. Text, path, and image smoothing are disabled; `True` enables all three. |
| `force_mediabox` | `force_mediabox=False`. An original page displays its crop box unless this flag selects the media box. |

The implementation uses `resolution or DEFAULT_RESOLUTION`, so
`resolution=0` falls back to 72. A positive floating-point value is accepted
even though the upstream README describes integer inputs. Width and height
target one dimension; PDFium rounding determines the other dimension.

On the committed
`nics-background-checks-2015-11.pdf` first page, whose displayed size is
`(1008, 612)` points:

```python
page.to_image(width=503).original.size       # (503, 306)
page.to_image(height=805).original.size      # (1326, 805)
page.to_image(resolution=150).original.size  # (2100, 1275)
page.to_image(resolution=0).resolution       # 72
```

Therefore `width=503` produces `(503, 306)`, `height=805` produces
`(1326, 805)`, and `resolution=150` produces `(2100, 1275)` on that fixture.

Supplying two sizing controls raises `ValueError` before PDFium is called:

```python
page.to_image(resolution=72, height=100)
# ValueError: Only one of these arguments can be provided: resolution, width, height. You provided 2
```

### Rendering pipeline

The renderer opens the same path or rewinds the in-memory stream to byte zero.
It forwards the PDF password to `pypdfium2.PdfDocument`, selects
`page_number - 1`, renders at `scale=resolution / 72`, and sets
`prefer_bgrx=True`. It disables text, path, and image smoothing unless
`antialias=True`, then converts the rendered bitmap to `RGB`.

The path-backed and byte-stream forms therefore render the same selected PDF
page, subject to the same PDFium version and arguments:

```python
from io import BytesIO
import pdfplumber

payload = open("document.pdf", "rb").read()
with pdfplumber.open(BytesIO(payload)) as pdf:
    image = pdf.pages[0].to_image()
```

For page calls, every call to `.to_image()` creates a new `PageImage`; image generation does
not mutate the page object cache. It does read the page, document path or
stream, password, page boxes, and page number.

## Coordinate projection, crop boxes, and filters

`PageImage.bbox` is the page-unit rectangle displayed by the bitmap. Its
`scale` is the rendered pixel width divided by the page crop-box width. For
every overlay, page coordinates are translated by the displayed bounding box,
multiplied by `scale`, and truncated with `int`.

In the pinned pipeline, cropped pages render the root PDF page and then crop the bitmap. For example:

```python
cropped = page.crop((10, 20, 30, 50))
image = cropped.to_image()
assert image.original.size == (20, 30)
assert image.bbox == (10, 20, 30, 50)
```

At 72 pixels per inch, a crop box `(10, 20, 30, 50)` produces a `(20, 30)`
image at 72 pixels per inch. Coordinates supplied to drawing methods remain in
the root page frame; the `PageImage` projection subtracts the crop origin.

`force_mediabox=True` affects an original page whose media box differs from its
crop box. On `issue-1054-example.pdf`, the default bitmap is `(596, 842)` and
the forced-media-box bitmap is `(2227, 2923)`. The flag chooses between page
boxes only for the original-page case; it does not override an explicit
`CroppedPage` bounding box.

In that model, filtered pages cannot remove or alter content in the underlying raster because
PDFium renders the original PDF page. Even so, their filtered objects can still be drawn
as overlays:

```python
filtered = page.filter(lambda obj: obj.get("object_type") == "char")
image = filtered.to_image()          # raster still contains every PDF mark
image.draw_rects(filtered.chars)     # overlay uses the filtered collection
```

This is why upstream documents `Page.crop(...)` as supported for rendering but
warns that `Page.filter(...)` changes cannot be incorporated into the base
image.

## `PageImage` state

`original` is the unannotated RGB bitmap. `annotated` is a separate RGB working
image, and `draw` is a Pillow `ImageDraw` handle configured for RGBA overlays.
All drawing methods mutate `annotated` and return the same `PageImage`, so calls can
be chained.

| Method | Pinned state behavior |
| --- | --- |
| `reset()` | `reset()` discards every overlay by rebuilding `annotated` from `original`; returns the same image object. |
| `copy()` | Starts with a clean overlay. It constructs with default arguments, so `copy()` does not preserve a non-default resolution value. Original and cropped pages differ as described below. |
| `show()` | `show()` delegates to Pillow and may launch an external local viewer; it returns `None`. |
| `_repr_png_()` | Serializes the annotated image to default PNG bytes for notebook display. |
| `save()` | Writes the current annotated image to a path or writable binary file object. |

Specifically, on an original page, `copy()` shares the same original image
object but starts with a clean overlay. In contrast, copying an already cropped
`PageImage` reapplies the crop and can produce an empty bitmap; pinned
v0.11.10 produced `(0, 0)` after copying the committed `(20, 30)` crop.

The `copy()` resolution detail affects saved DPI metadata: copying a
150-pixel-per-inch `PageImage` produces a clean image whose `resolution` field
is 72 unless the class behavior changes upstream. Do not use `copy()` as a
metadata-preserving clone without testing the pinned release.

## Drawing methods

All drawing coordinates use the displayed page's top-origin
`(x0, top, x1, bottom)` frame before projection to integer pixels. In these
calls, the default fill is blue with alpha 50, the default stroke is red with alpha 200, and the
default stroke width is one image pixel. Colors may be Pillow color strings,
RGB tuples, or RGBA tuples.

| Single method | Batch method | Accepted shape and behavior |
| --- | --- | --- |
| `draw_line()` | `draw_lines()` | A point tuple/list or object dictionary; for object inputs, line objects prefer `pts` and otherwise use `x0`, `top`, `x1`, and `bottom`. |
| `draw_vline()` | `draw_vlines()` | One or more x-coordinates spanning the displayed bounding box. |
| `draw_hline()` | `draw_hlines()` | One or more top-coordinates spanning the displayed bounding box. |
| `draw_rect()` | `draw_rects()` | A four-value box or object dictionary; for object inputs, rectangle objects use `x0`, `top`, `x1`, and `bottom`. |
| `draw_circle()` | `draw_circles()` | A point or object dictionary; for object inputs, circle objects use the center of that object box. |

The batch forms accept raw point sequences, object dictionaries, and pandas
collections through the upstream `utils.to_list` adapter. Pandas is referenced
only for optional accepted collection types; it is not a base dependency of
`pdfplumber`.

```python
image = page.to_image()
assert image.draw_rects(page.rects) is image
image.draw_lines(page.edges, stroke="orange", stroke_width=2)
image.draw_circles([(100, 100)], radius=4, fill=(0, 0, 0, 0))
image.draw_vline(72).draw_hline(72)
```

Rectangles inset their fill and border by half the stroke width so the outline
stays within the requested box. A non-positive rectangle stroke width suppresses
its four border segments. Circles pass fill and outline colors to Pillow; there
is no separate circle stroke-width argument.

Higher-level helpers use those primitives:

- `outline_words()` extracts words using its x/y tolerances, then calls
  `draw_rects()`.
- `outline_chars()` draws every current `page.chars` box with an opaque red
  stroke and quarter-alpha red fill by default.
- `debug_table()` draws every cell box belonging to one `Table`.
- `debug_tablefinder()` runs or accepts a table finder and draws the complete
  debug overlay.

## Saving and displaying output

`save()` writes `annotated`, not `original`. Its defaults are PNG,
`quantize=True`, 256 colors, and 8 bits. On default save, quantization uses Pillow's fast octree
method and palette mode; `quantize=False` keeps the RGB annotated image. For either path, the
saved DPI is `(resolution, resolution)`. Additional keyword arguments are
forwarded to Pillow's image saver.

```python
from io import BytesIO

palette_png = BytesIO()
image.save(palette_png)  # PNG, quantized palette, 8-bit request

rgb_png = BytesIO()
image.save(rgb_png, format="PNG", quantize=False)
assert palette_png.getvalue().startswith(b"\x89PNG\r\n\x1a\n")
```

`_repr_png_()` performs the default save into a `BytesIO` and returns those
bytes. Jupyter calls `_repr_png_()` and receives PNG bytes. `show()` is intended
for an interactive script or read-eval-print loop; it has host-specific viewer
side effects and no headless-display guarantee.

## Table debugging

`Page.debug_tablefinder()` returns data and does not draw.
`PageImage.debug_tablefinder()` draws and returns the image. The image method
accepts a `TableFinder`, a `TableSettings`, a dictionary, or `None`. Settings,
dictionaries, and `None` are passed through the page method; an existing finder
is reused.

```python
settings = {"horizontal_strategy": "text", "intersection_tolerance": 5}
finder = page.debug_tablefinder(settings)

image = page.to_image()
image.debug_tablefinder(finder).save("table-debug.png")
```

In overlay order, tables are drawn first, then edges, then intersections. There,
table cells use the default blue fill and red stroke. Edges use the default red
stroke with width one. Finally, intersections are transparent circles with a
blue stroke and radius 3.
The default pinned `nics-background-checks-2015-11.pdf` first-page finder
observed 46 edges, 358 intersections, 313 cells, and one table in the stated
reference environment.

An invalid visual finder value has this exact current failure, including the
missing space between `TableFinder` and `or`:

```text
ValueError: Argument must be instance of TableFinderor a TableFinder settings dict.
```

The complete settings and table-pipeline rules remain in the
[table-setting guide](table-settings.md). A visual overlay helps inspect that
pipeline; it does not prove that extracted cells or values are correct.
The [Rust-native extensions](rust-extensions.md) guide places this SVG surface
beside the other current native-only families and records its Python and
WebAssembly exposure gaps.

## Rendering failures and resource behavior

A PDFium document-open failure is wrapped at the rendering boundary. The
invalid-byte probe produces:

```text
MalformedPDFException: Failed to load document (PDFium: Data format error).
```

Earlier parser, password, page-selection, filesystem, Pillow, and viewer
failures can retain their own exception types. The exact error above is not a
universal replacement for every visualization failure.

Raster memory grows with rendered pixel area, not only PDF byte size. Higher
resolution, a large media box, antialiasing, multiple live images, and an RGB
unquantized save can increase time or memory. Quantization occurs while saving;
it does not shrink the live RGB `original` and `annotated` images. Close the PDF
when finished and release large `PageImage` instances in long-running jobs.

Neither rendering nor drawing uploads the document. `show()` may launch a host
viewer, and `save()` writes to the caller's destination. Treat those as explicit
local side effects under the repository's [privacy guide](privacy.md).

## Current project surfaces

### Rust library: SVG extension

For native use, the current extension is self-contained: the Rust SVG path
requires no Pillow, PDFium, ImageMagick, Wand, or Ghostscript runtime. It
formats extracted geometry as SVG text. The Rust SVG
is deliberately different: the Rust SVG contains no rendered PDF background
and starts with a white page rectangle.

`Page::to_svg` adds only the page boundary unless the caller uses `SvgRenderer`
drawing methods. `SvgOptions` controls output width, height, and scale without
changing the view box. An explicit width or height controls that output
dimension; `scale` supplies dimensions that remain unset.

```rust,no_run
use pdfplumber::{Pdf, SvgDebugOptions, SvgOptions, TableSettings};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pdf = Pdf::open_path("document.pdf", None)?;
    let page = pdf.pages().get(0)?;

    let boundary_only = page.to_svg(&SvgOptions::default());
    std::fs::write("page.svg", boundary_only)?;

    let tables = page.debug_tablefinder_svg(
        &TableSettings::default(),
        &SvgDebugOptions::default(),
    );
    std::fs::write("table-debug.svg", tables)?;
    Ok(())
}
```

`SvgRenderer` exposes typed calls for chars, lines, rectangles, edges,
intersections, cells, and tables. Its built-in defaults differ visibly from
pinned `PageImage`: chars are blue outlines, ordinary lines are red, rectangles
are green, edges are orange, table cells are light blue with steel-blue borders,
intersections are red, and candidate cell boundaries are dashed magenta.

`SvgDebugOptions` independently controls edges, intersections, cells, and
tables; all four SVG debug flags default to `true`. Omitting a stage removes
only that stage from the generated SVG. This selective SVG option object has no
equivalent in pinned `PageImage.debug_tablefinder()`.

### Rust-native Command-Line Interface

For files, the `pdfplumber debug` Command-Line Interface writes SVG:

```console
pdfplumber debug document.pdf --pages 1 --output debug.svg
pdfplumber debug document.pdf --pages 1 --output tables.svg --tables
```

By default, ordinary debug mode draws chars, lines, rects, edges, and table cells. Curves,
PDF raster content, and embedded images are not painted as a page background.
`--tables` draws table-pipeline edges, intersections, cells, and tables using
all four default SVG debug flags.

For multiple selected pages, the output stem and extension are reused:

```console
pdfplumber debug document.pdf --pages 1-2 --output debug.custom
# writes debug_page1.custom and debug_page2.custom
```

For that mode, multiple selected pages append `_pageN` before the requested extension. In every case,
the extension does not change the SVG content format: `debug.custom` still contains
SVG markup. A single page uses the requested path exactly. The command reports
each written path on standard error and exits nonzero for page selection,
opening, extraction, or write failures.

### Python and WebAssembly adapters

At present, the current Python adapter does not expose `Page.to_image`, `PageImage`, or
visual-debug drawing methods. Its current table surface also lacks
`Page.debug_tablefinder`. Installing Pillow or pypdfium2 beside
`pdfplumber-rs` does not add those missing adapter methods.

At present, the current WebAssembly adapter exposes no raster or SVG visual-debugging
method. It exposes extraction data such as characters and tables; applications
may build their own browser visualization from those values, but that is
application code rather than this package's visual-debug contract.

## Validation and provenance

The pinned reference observations in this guide used CPython 3.13,
`pdfplumber==0.11.10`, Pillow 12.3.0, pypdfium2 5.13.0, and the committed
upstream fixture copies. They exercised path and byte-stream rendering,
resolution/width/height selection, crop and media boxes, filtered pages,
state mutation and reset, copy behavior, every drawing family, table debugging,
quantized and RGB PNG output, and exact invalid-input failures.

Native validation ran the 40 focused SVG unit tests and executed both CLI debug
modes on `nics-background-checks-2015-11.pdf`. The ordinary candidate SVG
contained 4,887 rectangle elements and 1,400 line elements; table mode contained
627 rectangle, 46 line, and 358 circle elements. Neither SVG contained an
`image`, data-URI, or external raster reference. A two-page CLI check on
`pdffill-demo.pdf` verified the suffix and extension behavior.

Authoritative sources and current implementations:

- Upstream v0.11.10 [visual-debugging documentation](https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md).
- Upstream v0.11.10 [`display.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/display.py), Git blob `4b915da24aa7cc7066bdec0e8aebc0457fd1783c`.
- Upstream v0.11.10 [`page.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py), Git blob `286e7e158c12da8305520ecc1f550f3bd8f1a906`.
- Upstream v0.11.10 README blob `f6f1ce3e0e546b854787aff946601af44fcc6f69` and tag commit `7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`.
- Repository Rust SVG implementation: [`../crates/pdfplumber-core/src/svg.rs`](../crates/pdfplumber-core/src/svg.rs).
- Repository `Page` SVG facade: [`../crates/pdfplumber/src/page.rs`](../crates/pdfplumber/src/page.rs).
- Repository CLI writer: [`../crates/pdfplumber-cli/src/debug_cmd.rs`](../crates/pdfplumber-cli/src/debug_cmd.rs).
- Current Python declaration boundary: [`../crates/pdfplumber-py/python/pdfplumber/_native.pyi`](../crates/pdfplumber-py/python/pdfplumber/_native.pyi).
- Current WebAssembly boundary: [`../crates/pdfplumber-wasm/src/lib.rs`](../crates/pdfplumber-wasm/src/lib.rs).

## Claim boundary

This guide does not establish visual-debugging compatibility. Under this policy,
visual-debugging documentation is not compatibility evidence and does not approve a
compatibility deviation. DOC-011 changes no runtime behavior.

The Python raster result, Pillow state model, drawing calls, save behavior,
errors, viewer effects, Rust SVG extension, fixed CLI overlays, and absent
adapter surfaces remain distinct. Rendering, visual-debug, Command-Line
Interface, table, crop, filter, error, packaging, platform, and strict-gate
tasks remain open wherever the PRD leaves them unchecked. Matching page size,
geometry, color intent, or a useful picture does not convert an untested or
different surface into a compatible result.
