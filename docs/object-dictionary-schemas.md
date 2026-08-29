# Object-dictionary schema guide

This guide is the complete page-object dictionary reference for applications
comparing Python `pdfplumber` v0.11.10 with the current `pdfplumber-rs`
surfaces. It starts from the pinned Python runtime contract, including exact
key order and fields that the shorter upstream README does not list, then
separates the narrower current Python adapter, typed Rust, and WebAssembly
boundaries. A documented field name does not establish object-schema
compatibility.

The Python examples use the pinned upstream package. Keep it in an environment
separate from `pdfplumber-rs`: both distributions import as `pdfplumber`.

## Access objects without losing their family

`Page.objects` maps singular family names to lists of dictionaries. Without
`laparams`, it contains the low-level families encountered on the page. Passing
layout parameters also exposes higher-level `pdfminer.six` layout families.
`.objects` omits empty families, so consumers must use `.get(family, [])` when
an absent family and an empty list have the same application meaning.

```python
import pdfplumber

with pdfplumber.open(
    "document.pdf",
    laparams={"detect_vertical": True},
) as pdf:
    page = pdf.pages[0]
    objects = page.objects
    chars = page.chars
    horizontal_boxes = page.textboxhorizontals
    print(list(objects))
    print(list(chars[0]))
```

The ordinary properties `.chars`, `.lines`, `.rects`, `.curves`, `.images`,
`.textboxhorizontals`, `.textboxverticals`, `.textlinehorizontals`, and
`.textlineverticals` read their corresponding `.objects` list. The page cache
preserves the list and dictionary insertion order; mutation is observable
through other accessors that reuse that cache. For these dictionaries,
insertion order is observable, so do not sort dictionary keys when comparing
or persisting compatibility evidence.

Within pinned Python, annotations and hyperlinks are not members of `.objects`;
use `page.annots` and `page.hyperlinks`. Likewise, derived edges are not members
of `.objects`; use their dedicated properties.

```python
annots = page.annots
links = page.hyperlinks
rect_edges = page.rect_edges
curve_edges = page.curve_edges
all_edges = page.edges
horizontal = page.horizontal_edges
vertical = page.vertical_edges
```

`PDF.objects` aggregates each family in selected-page order. It does not add
annotations, hyperlinks, or derived edges. The equivalent explicit spellings
are useful when auditing what crosses a document boundary:

```python
document_objects = pdf.objects
document_dict = pdf.to_dict(object_types=["char"])
assert document_objects is pdf.objects
assert "pages" in document_dict
```

## Pinned ordered schemas

The committed reference snapshot found twelve observed object families and one
invariant ordered key schema per observed family. These are runtime schemas,
not a transcription of the upstream README. In particular, the README has
placeholder character pattern names that were not present in the pinned
runtime, while the graphical runtime dictionaries contain several fields not
listed in their short tables.

| Family or accessor projection | Pinned key order |
| --- | --- |
| `annot` | `page_number` → `object_type` → `x0` → `y0` → `x1` → `y1` → `doctop` → `top` → `bottom` → `width` → `height` → `uri` → `title` → `contents` → `data` |
| `char` | `matrix` → `fontname` → `adv` → `upright` → `x0` → `y0` → `x1` → `y1` → `width` → `height` → `size` → `mcid` → `tag` → `object_type` → `page_number` → `ncs` → `text` → `stroking_color` → `non_stroking_color` → `top` → `bottom` → `doctop` |
| `curve` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `pts` → `linewidth` → `stroke` → `fill` → `evenodd` → `stroking_color` → `non_stroking_color` → `mcid` → `tag` → `object_type` → `page_number` → `path` → `dash` → `top` → `bottom` → `doctop` |
| `figure` | `name` → `matrix` → `x0` → `y0` → `x1` → `y1` → `width` → `height` → `object_type` → `page_number` → `top` → `bottom` → `doctop` |
| `hyperlink` | `page_number` → `object_type` → `x0` → `y0` → `x1` → `y1` → `doctop` → `top` → `bottom` → `width` → `height` → `uri` → `title` → `contents` → `data` |
| `image` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `name` → `stream` → `srcsize` → `imagemask` → `bits` → `colorspace` → `mcid` → `tag` → `object_type` → `page_number` → `top` → `bottom` → `doctop` |
| `line` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `pts` → `linewidth` → `stroke` → `fill` → `evenodd` → `stroking_color` → `non_stroking_color` → `mcid` → `tag` → `object_type` → `page_number` → `path` → `dash` → `top` → `bottom` → `doctop` |
| `rect` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `pts` → `linewidth` → `stroke` → `fill` → `evenodd` → `stroking_color` → `non_stroking_color` → `mcid` → `tag` → `object_type` → `page_number` → `path` → `dash` → `top` → `bottom` → `doctop` |
| `textboxhorizontal` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `object_type` → `page_number` → `text` → `top` → `bottom` → `doctop` |
| `textboxvertical` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `object_type` → `page_number` → `text` → `top` → `bottom` → `doctop` |
| `textlinehorizontal` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `object_type` → `page_number` → `text` → `top` → `bottom` → `doctop` |
| `textlinevertical` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `object_type` → `page_number` → `text` → `top` → `bottom` → `doctop` |

The `hyperlink` row names the accessor projection used by the snapshot. A
hyperlink is an annotation filtered to a non-`None` URI; a hyperlink dictionary
keeps `object_type == "annot"` rather than changing the discriminator to
`"hyperlink"`.

The snapshot proves keys and order on the scanned corpus, not one universal
runtime type for every value. Across the scan, optional fields remain present
with `None` in the observed families. Numeric `int` versus `float` provenance,
color shapes, stream state, nested annotation values, and exact values are
separate parts of the compatibility contract.

## Common identity and geometry fields

`object_type` is the singular family discriminator stored in the dictionary.
`page_number` is one-based and retains the original document page identity,
including selected and derived pages. Specifically, word and search-result
dictionaries do not have `object_type` or `page_number`; they are method
results rather than members of the page-object cache.

`x0`, `x1`, `top`, and `bottom` use top-origin page space: x is measured from
the left edge and the vertical fields are measured downward from the top.
`y0` and `y1` use bottom-origin page space. `width` is `x1 - x0`, `height` is
`bottom - top`, and `doctop` is measured from the top of the document. The
[coordinate-system guide](coordinate-systems.md) covers page rotation,
non-zero origins, boxes, and conversions. The
[crop-semantics guide](crop-semantics.md) covers which of these fields are
clipped or preserved on derived pages.

## Family-specific fields and value shapes

### Characters

- `text`, `fontname`, `size`, `adv`, and `upright` describe the decoded glyph,
  displayed font, nominal size, text-space advance, and orientation.
- `matrix` is a six-value tuple `(a, b, c, d, e, f)` carrying the glyph's
  affine transform. It is not another bounding box.
- `ncs` names the non-stroking color space when pdfminer exposes it.
- `stroking_color` and `non_stroking_color` preserve the runtime color value
  shape; gray, device colors, patterns, and unavailable colors do not all have
  one common tuple length.
- `mcid` and `tag` describe marked content and remain `None` when no marked
  content value applies.

### Lines, rectangles, and curves

The three graphical families share one ordered schema because they descend
from the same pdfminer curve model. Values still distinguish the families:

- `pts` retains top-origin points. For a line or rectangle it describes the
  recognized path vertices; for a curve it includes the curve points exposed
  by pdfminer.
- `path` retains drawing commands and control points as command-first tuples.
- `linewidth`, `stroke`, `fill`, and `evenodd` carry paint state and the fill
  rule.
- `stroking_color` and `non_stroking_color` carry outline and fill color
  values.
- `dash` is a `([dash_array], dash_phase)` tuple when a dashing style is
  available, otherwise it may be `None`.
- `mcid` and `tag` describe marked content, as for characters.

Do not infer that every point is an endpoint: Bezier control points matter in
`path`. Do not infer that a zero-height or zero-width graphical object is a
specific family: pdfminer's classification determines the discriminator.

### Images

`name` is the image XObject resource name. `srcsize` is a `(width, height)`
tuple in source pixels, while the ordinary `width` and `height` fields are
displayed page-space extents. `bits`, `colorspace`, and `imagemask` describe
the source sample representation. `stream` is the pdfminer PDF stream proxy,
not decoded image bytes. `mcid` and `tag` carry marked-content values.

The Rust-only image export API separately exposes decoded or encoded bytes,
format, dimensions, filter, and MIME information. Those extension dictionaries
are not Python page-object dictionaries.

### Annotations and hyperlinks

`uri`, `title`, and `contents` are decoded public annotation values and may be
`None`. `data` is the resolved annotation dictionary, including nested PDF
values; its page reference is replaced with the current `Page` when present.
Malformed text can warn, omit the decoded public value, or raise according to
`raise_unicode_errors`. `.hyperlinks` filters annotations to a present URI but
does not rewrite their keys or discriminator.

### Figures and higher-level layout

`figure` dictionaries contain `name`, `matrix`, identity, and geometry fields.
Text boxes and text lines contain geometry, identity, and `text`. Horizontal
and vertical families appear only when the chosen layout analysis emits them;
`detect_vertical=True` enables vertical recognition.

Pinned v0.11.10 has a public trap: the figure family has no `.figures`
accessor. A figure can therefore appear in `page.objects`, but serializing a
discovered figure through `to_dict` raises `AttributeError`, whether the family
was selected explicitly or reached by the default family list. Preserve that
failure in a differential test instead of inventing an accessor.

## Derived edge dictionaries

Derived edges are computed views, not additional page-object families.
`line_to_edge` shallow-copies a line and appends `orientation`. Rectangle edges
shallow-copy the rectangle, replace the relevant side geometry and
`object_type`, then append `orientation`. Curve edges are new compact
dictionaries for each adjacent point pair and do not retain the curve's paint
or path fields.

| Derived family | Pinned key order |
| --- | --- |
| `line-derived` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `pts` → `linewidth` → `stroke` → `fill` → `evenodd` → `stroking_color` → `non_stroking_color` → `mcid` → `tag` → `object_type` → `page_number` → `path` → `dash` → `top` → `bottom` → `doctop` → `orientation` |
| `rectangle-derived` | `x0` → `y0` → `x1` → `y1` → `width` → `height` → `pts` → `linewidth` → `stroke` → `fill` → `evenodd` → `stroking_color` → `non_stroking_color` → `mcid` → `tag` → `object_type` → `page_number` → `path` → `dash` → `top` → `bottom` → `doctop` → `orientation` |
| `curve-derived` | `object_type` → `x0` → `x1` → `top` → `doctop` → `bottom` → `width` → `height` → `orientation` |

For derived edges, `orientation` is `"h"`, `"v"`, or `None`; the last value
is possible for a diagonal curve segment. `.edges` concatenates line-derived,
rectangle-derived, and curve-derived edges in that order.
`.horizontal_edges` and `.vertical_edges` filter `.edges` to `"h"` and `"v"`
respectively.

```python
for edge in page.edges:
    assert edge["orientation"] in ("h", "v", None)

assert page.edges == (
    [dict(edge) for edge in map(pdfplumber.utils.line_to_edge, page.lines)]
    + page.rect_edges
    + page.curve_edges
)
```

The copied rectangle and line fields can contain nested mutable values shared
with their source dictionaries. Treat derived edges as cached views rather
than independent deep copies.

## Containers and serialization

`Page.to_dict(object_types=None)` adds page metadata, then pluralizes every
selected singular family name. By default it selects the families present in
`.objects` and then `annot`; it does not add `hyperlink` or derived edges.
`PDF.to_dict` wraps selected page dictionaries with document metadata.

```python
page_payload = page.to_dict(object_types=["char", "annot"])
page_json = page.to_json(object_types=["char", "image"])
document_payload = pdf.to_dict(object_types=["char"])

assert "chars" in page_payload
assert "annots" in page_payload
assert isinstance(page_payload["chars"][0]["matrix"], tuple)
```

On this surface, direct dictionaries retain tuples and opaque stream objects.
JSON serialization converts tuples to arrays and recursively serializes
supported PDF literals, streams, bytes, lists, and nested dictionaries.
`include_attrs` and `exclude_attrs` filter serialized output; they do not
mutate the source dictionaries or change the pinned unfiltered schema.
`precision` rounds serialized floats rather than the cached direct values. CSV
builds a union of non-dictionary fields and ignores nested dictionary columns.

Schema comparisons must name the surface: `page.objects`, `to_dict`, JSON, and
CSV have intentionally different value representations even when they begin
from the same cached dictionary.

## Current surface boundaries

The current `pdfplumber-rs` Python adapter exposes several compatible names,
but its page-object schemas are incomplete relative to the pinned table:

| Family | Current Python-adapter boundary |
| --- | --- |
| `char` | `char` is missing `ncs` and adds `direction`; its insertion order also differs. |
| `line` | `line` is missing nine pinned fields and adds `orientation`: `pts`, `stroke`, `fill`, `evenodd`, `non_stroking_color`, `mcid`, `tag`, `path`, and `dash` are absent. The added value uses `"horizontal"`, `"vertical"`, or `"diagonal"`, not the derived-edge letters. |
| `rect` | `rect` is missing six pinned fields: `pts`, `evenodd`, `mcid`, `tag`, `path`, and `dash`. |
| `curve` | `curve` is missing five pinned fields: `evenodd`, `mcid`, `tag`, `path`, and `dash`. |
| `image` | `image` is missing `stream`, `imagemask`, `mcid`, and `tag`. |
| `annot` / `hyperlink` | The annotation key order matches while `data` remains incomplete; the adapter currently emits an empty dictionary there. |
| higher-level layout | Horizontal and vertical text-box/text-line dictionaries use the pinned twelve keys, subject to separate value and ordering tasks. |
| `figure` | At present, the current Python adapter does not emit `figure`. |
| derived edges | At present, the current Python adapter does not expose derived-edge properties. |

These gaps remain assigned to the OBJ-CHAR, OBJ-GFX, OBJ-IMG, annotation,
layout, serialization, and strict-gate tasks. Matching common field names or
one fixture does not establish object-schema compatibility.

Rust uses typed structs and idiomatic field names. `Char` and `Annotation` use
nested `BBox` values; `Line`, `Rect`, `Curve`, `Image`, and `Edge` retain typed
coordinate fields. Other Rust names include `advance`, `ctm`, `line_width`,
`stroke_color`, `fill_color`, `src_width`, and `bits_per_component`. Only the
families named by the curated model table belong to that stable `0.3.x`
boundary; `Image`, `Annotation`, and `Edge` are not silently promoted by this
guide. The stable model and Serde contracts are documented in
[Rust data models](rust-data-models.md) and
[Rust Serde schemas](rust-serde-schema.md); neither is the flat Python schema.
Rust page indexes are zero-based.

WebAssembly exposes only serialized `Char` values through `chars()` among the
page-object families in this guide. Its word, search, and table results use
their Rust Serde models, and it has no general `.objects`, annotation, figure,
image, geometry-family, or derived-edge accessor. WebAssembly page indexes are
zero-based. The Command-Line Interface likewise has no Python-style public
page-object dictionary surface.

## Validation and provenance

The artifact
`compat/snapshots/pdfplumber-v0.11.10-object-schemas.json` is bound to pinned
CPython 3.13, `pdfplumber==0.11.10`, upstream tag `v0.11.10`, and commit
`7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`. It attempted all 223 indexed
fixtures in four collection classes, scanning the first three pages with
`laparams={"detect_vertical": True}`. It records every completed fixture and
all 32 recorded failure observations rather than silently dropping unreadable
inputs. Collection counts are evidence context, not schema assertions.

Set up the pinned reference environment, then regenerate rather than editing
the JSON by hand:

```bash
bash scripts/setup_golden_venv.sh
. .venv-reference/bin/activate
python scripts/generate_object_schema_snapshot.py --check
python -m unittest compat.tests.test_object_schema_snapshot
```

The guide and executable contract are anchored to official upstream source at
that commit:

- README blob `f6f1ce3e0e546b854787aff946601af44fcc6f69` documents the public object
  families and human field summaries.
- `pdfplumber/page.py` blob
  `286e7e158c12da8305520ecc1f550f3bd8f1a906` defines parsed object keys,
  annotation dictionaries, page transforms, and serialization selection.
- `pdfplumber/container.py` blob
  `2589a4a36278f5e04d9092ec1a27faf2c4336883` defines accessors, document
  aggregation, derived-edge containers, JSON, and CSV.
- `pdfplumber/utils/geometry.py` blob
  `93f5f3b83002deb04c7c8c7462b7dab1c4d245a7` defines the three derived-edge
  projections and orientation values.

Primary and current implementation links:

- Pinned upstream [README object reference](https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md),
  [`Page` object processing](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py),
  [`Container` access and serialization](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/container.py),
  and [derived-edge geometry](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/utils/geometry.py).
- Repository [reference snapshot](../compat/snapshots/pdfplumber-v0.11.10-object-schemas.json),
  [snapshot builder](../compat/harness/object_schema_snapshot.py), and
  [snapshot contract](../compat/tests/test_object_schema_snapshot.py).
- Current [Python adapter](../crates/pdfplumber-py/src/lib.rs), [Rust character
  model](../crates/pdfplumber-core/src/text.rs), [Rust graphical
  models](../crates/pdfplumber-core/src/shapes.rs), [Rust edge
  model](../crates/pdfplumber-core/src/edges.rs), and [WebAssembly
  adapter](../crates/pdfplumber-wasm/src/lib.rs).

The committed snapshot remains the authority when the README summary and
observed runtime schema differ. Any future target release needs its own
regenerated artifact and source audit.

## Claim boundary

This guide documents pinned reference behavior and identifies current surface
gaps. It does not change runtime extraction, schema order, numeric types,
serialization, cache behavior, candidate results, scorecard outcomes, or
approved deviations. Documentation is not compatibility evidence. Only an
exact, named reference/candidate observation can support a compatibility claim
for that operation, input, options, environment, and artifact. In this task,
documentation is not compatibility evidence.
