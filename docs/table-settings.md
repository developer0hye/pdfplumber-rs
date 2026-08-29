# Table-setting guide

This guide is the complete table-setting reference for applications comparing
Python `pdfplumber` v0.11.10 with the current `pdfplumber-rs` surfaces. It
documents the pinned Python contract first, including interactions that are
easy to miss in the shorter upstream README, then names the narrower Rust,
Python-adapter, Command-Line Interface, and WebAssembly boundaries. A setting
with the same name or default on two surfaces is not evidence of equal output.

The Python examples use the pinned upstream package. Keep it in an environment
separate from `pdfplumber-rs`: both distributions import as `pdfplumber`.

```python
import pdfplumber

with pdfplumber.open("document.pdf") as pdf:
    page = pdf.pages[0]
    table_settings = {}
    tables = page.extract_tables(table_settings)
```

## Methods and accepted setting containers

All five pinned `Page` table methods accept `table_settings=None`. The value may
be `None`, a dictionary, or an already constructed `TableSettings` instance.

| Pinned method | Result |
| --- | --- |
| `Page.find_tables` | Every discovered `Table`, with `.bbox`, `.cells`, `.rows`, `.columns`, and `.extract(...)` |
| `Page.find_table` | The largest discovered `Table`, or `None` |
| `Page.extract_tables` | All tables as `table -> row -> cell` text arrays |
| `Page.extract_table` | The largest table as `row -> cell`, or `None` |
| `Page.debug_tablefinder` | A `TableFinder` exposing `.edges`, `.intersections`, `.cells`, and `.tables` |

`PageImage.debug_tablefinder` is the visual counterpart. It draws the selected
edges, intersections, and table regions on an image rather than returning the
pipeline object.
The [visual-debugging guide](visual-debugging.md) defines that image's pinned
dependencies, rendering, overlay order, colors, mutation, saving, and error
behavior and keeps it separate from this project's current SVG extension.

```python
from pdfplumber.table import TableSettings

default_settings = TableSettings.resolve(None)
settings_instance = TableSettings(
    vertical_strategy="text",
    horizontal_strategy="lines",
    min_words_vertical=2,
    text_settings={"x_tolerance": 1, "y_tolerance": 1},
)
assert TableSettings.resolve(settings_instance) is settings_instance

table_settings = {
    "vertical_strategy": "lines",
    "horizontal_strategy": "lines",
}
tables = page.find_tables(table_settings)
largest = page.find_table(table_settings)
all_text = page.extract_tables(table_settings)
largest_text = page.extract_table(table_settings)
finder = page.debug_tablefinder(table_settings)
im = page.to_image()
annotated = im.debug_tablefinder(table_settings)
```

Dictionary resolution separates table discovery from cell-text options:
dictionary resolution strips the `text_` prefix and stores the remainder in
`TableSettings.text_settings`. Constructing `TableSettings` directly instead
uses the nested `text_settings={...}` field, as above. Passing an instance
returns that same object; passing `None` constructs defaults. Do not put a
nested `"text_settings"` key in the dictionary form: its `text_` prefix is
removed and it does not mean the constructor field.

## Pipeline and coordinate units

All distances and coordinates below use PDF points in the page's top-origin
coordinate space. The [coordinate-system guide](coordinate-systems.md) covers
page boxes, rotation, and the other coordinate spaces.

The pinned pipeline has a fixed order: each axis selects edges independently;
explicit lines are then added; edges are snapped, joined, and filtered by final
length; intersections become cells; contiguous cells become tables; and text
is extracted per cell. This order explains why changing two settings together
can differ from changing either one alone.

## Table-specific settings

The default column shows the effective value after `TableSettings.__post_init__`
has run. The non-default examples are suitable starting points for differential
tests, not universal recommendations.

| Setting | Effective default | Stage and meaning | Non-default example |
| --- | --- | --- | --- |
| `vertical_strategy` | `"lines"` | Select vertical, column-boundary edges | `"text"` |
| `horizontal_strategy` | `"lines"` | Select horizontal, row-boundary edges | `"lines_strict"` |
| `explicit_vertical_lines` | `None` | Add numeric x-coordinates or vertical edges derived from objects | `[x0, x1]` |
| `explicit_horizontal_lines` | `None` | Add numeric y-coordinates or horizontal edges derived from objects | `[top, bottom]` |
| `snap_tolerance` | `3` | Fallback for omitted x/y snapping tolerances | `1` |
| `snap_x_tolerance` | `3` | Snap nearby vertical edges to a common x-position | `2` |
| `snap_y_tolerance` | `3` | Snap nearby horizontal edges to a common top-position | `2` |
| `join_tolerance` | `3` | Fallback for omitted x/y joining tolerances | `1` |
| `join_x_tolerance` | `3` | Join horizontal segments separated along x | `2` |
| `join_y_tolerance` | `3` | Join vertical segments separated along y | `2` |
| `edge_min_length` | `3` | Discard merged edges shorter than this length | `1` |
| `edge_min_length_prefilter` | `1` | Discard short page-derived edges before merging | `0.5` |
| `min_words_vertical` | `3` | Required aligned words for vertical `text` edges | `2` |
| `min_words_horizontal` | `1` | Required aligned words for horizontal `text` edges | `2` |
| `intersection_tolerance` | `3` | Fallback for omitted x/y intersection tolerances | `1` |
| `intersection_x_tolerance` | `3` | Horizontal proximity allowed at an orthogonal crossing | `2` |
| `intersection_y_tolerance` | `3` | Vertical proximity allowed at an orthogonal crossing | `2` |
| `text_tolerance` | `3` | Legacy fallback for missing cell-text x/y tolerances | `1` |
| `text_x_tolerance` | `3` | Word grouping along x for discovery and cell text | `1` |
| `text_y_tolerance` | `3` | Word grouping along y for discovery and cell text | `1` |

### Raw sentinels and effective fallbacks

The dataclass signature displays the six axis-specific fallback fields with a
special float sentinel whose representation looks like zero. Their raw defaults
are `snap_x_tolerance=UNSET`, `snap_y_tolerance=UNSET`,
`join_x_tolerance=UNSET`, `join_y_tolerance=UNSET`,
`intersection_x_tolerance=UNSET`, and `intersection_y_tolerance=UNSET`.
Post-initialization replaces each sentinel with its general setting, producing
the effective `3` values in the table.

The check is identity against `UNSET`, not a truthiness test: an omitted axis
tolerance inherits its general tolerance, but an explicit zero remains zero.
Changing `snap_tolerance` to `1` therefore changes both axes only when their
specific keys are omitted. An explicit `snap_x_tolerance=2` still wins.

## Strategies and mixed axes

Both axes accept the same four names:

| Strategy | Pinned behavior |
| --- | --- |
| `lines` | Use graphical line objects and rectangle sides |
| `lines_strict` | Use graphical line objects, excluding rectangle sides |
| `text` | Create imaginary boundaries from aligned words |
| `explicit` | Use only caller-supplied lines for that axis |

In source terms, `lines` includes line objects and rectangle edges, while
`lines_strict` keeps only line-object edges. `text` synthesizes edges from
aligned words. `explicit` suppresses detected edges only on that axis. Because
the axes resolve independently, a ruled-row, borderless-column table can use
horizontal lines and vertical text alignment.

```python
line_and_strict = page.extract_table(
    {
        "vertical_strategy": "lines",
        "horizontal_strategy": "lines_strict",
    }
)

text_both = page.extract_table(
    {
        "vertical_strategy": "text",
        "horizontal_strategy": "text",
        "min_words_vertical": 2,
        "min_words_horizontal": 2,
    }
)
```

`min_words_vertical` applies only to vertical `text` discovery and
`min_words_horizontal` only to horizontal `text` discovery. Neither threshold
changes a line-based axis.

## Explicit lines and objects

For numeric inputs, numeric vertical coordinates span the page height and
numeric horizontal coordinates span the page width. A dictionary object is
expanded into its component edges: object inputs are expanded with
`obj_to_edges`, then only edges with the requested orientation are retained.
The documented object families are line-, rectangle-, and curve-like page
objects.

Pinned behavior has another exact requirement: an explicit strategy requires
at least two entries for its axis. The reliable form supplies the list or tuple
explicitly; in pinned v0.11.10, an empty list
raises the intended `ValueError`, while leaving the field as `None` reaches a
`len(None)` `TypeError`. Preserve the exact observed failure in compatibility
tests rather than normalizing the two cases.

```python
x0, top, x1, bottom = page.bbox
numeric_grid = page.extract_table(
    {
        "vertical_strategy": "explicit",
        "horizontal_strategy": "explicit",
        "explicit_vertical_lines": [x0, x1],
        "explicit_horizontal_lines": [top, bottom],
    }
)
```

Explicit inputs also augment detected edges. In particular, explicit lines
augment `lines`, `lines_strict`, and `text`; they are not limited to the
`explicit` strategy.

```python
object_edges = page.debug_tablefinder(
    {
        "explicit_vertical_lines": [page.rects[0]],
        "explicit_horizontal_lines": [page.rects[0]],
    }
)
```

## Edge normalization and cell construction

Snapping groups almost-parallel boundaries before joining reconnects collinear
segments. `snap_x_tolerance` groups vertical edges by x-position, while
`snap_y_tolerance` groups horizontal edges by top-position.
`join_x_tolerance` closes horizontal endpoint gaps, while `join_y_tolerance`
closes vertical endpoint gaps.

`edge_min_length_prefilter` acts before strategy edges are merged and
`edge_min_length` acts after snapping and joining. The two stages deliberately
allow several short, aligned fragments to survive the prefilter, join into one
rule, and pass the final length gate. The prefilter applies to page-derived
`lines`/`lines_strict` edges; text-derived and explicit edges enter through
their own paths before the final filter.

After normalization, `intersection_x_tolerance` and
`intersection_y_tolerance` decide whether orthogonal edges meet closely enough
to form cell vertices. The general intersection setting only supplies omitted
axis values; it is not a third intersection pass.

```python
finder = page.debug_tablefinder(
    {
        "snap_tolerance": 1,
        "snap_x_tolerance": 2,
        "snap_y_tolerance": 2,
        "join_tolerance": 1,
        "join_x_tolerance": 2,
        "join_y_tolerance": 2,
        "edge_min_length": 1,
        "edge_min_length_prefilter": 0.5,
        "intersection_tolerance": 1,
        "intersection_x_tolerance": 2,
        "intersection_y_tolerance": 2,
    }
)
```

## Forwarded `text_*` settings

The dictionary interface accepts the pinned text-extraction options with a
`text_` prefix. The defaults below are the underlying WordExtractor/TextMap
defaults. `text_x_tolerance` and `text_y_tolerance` also appear in the table
settings above because they influence word-derived edges as well as cell text.
The [text-option guide](text-options.md) gives the full option semantics,
direction validation, layout rules, and method scopes.

| Setting | Forwarded default | Main role in table extraction | Non-default example |
| --- | --- | --- | --- |
| `text_x_tolerance` | `3` | Fixed horizontal word gap | `1` |
| `text_y_tolerance` | `3` | Fixed vertical word gap | `1` |
| `text_x_tolerance_ratio` | `None` | Dynamic horizontal gap from prior character size | `0.4` |
| `text_y_tolerance_ratio` | `None` | Dynamic vertical gap from prior character size | `0.4` |
| `text_keep_blank_chars` | `False` | Keep blanks inside grouped words | `True` |
| `text_use_text_flow` | `False` | Use content-flow order for word grouping | `True` |
| `text_vertical_ttb` | `True` | Deprecated vertical ordering switch | `False` |
| `text_horizontal_ltr` | `True` | Deprecated horizontal ordering switch | `False` |
| `text_line_dir` | `"ttb"` | Upright line progression | `"btt"` |
| `text_char_dir` | `"ltr"` | Upright character progression | `"rtl"` |
| `text_line_dir_rotated` | `None` | Rotated line progression | `"rtl"` |
| `text_char_dir_rotated` | `None` | Rotated character progression | `"btt"` |
| `text_extra_attrs` | `None` | Split words when named character attributes differ | `["fontname"]` |
| `text_split_at_punctuation` | `False` | Split punctuation into separate words | `True` |
| `text_expand_ligatures` | `True` | Expand known presentation ligatures | `False` |
| `text_layout` | `False` | Render cell text into a spatial TextMap | `True` |
| `text_layout_width` | `0` | Point width before table-cell override | `500` |
| `text_layout_height` | `0` | Point height before table-cell override | `700` |
| `text_layout_width_chars` | `0` | Requested output width in character cells | `70` |
| `text_layout_height_chars` | `0` | Requested output height in character rows | `50` |
| `text_layout_bbox` | `(0, 0, 0, 0)` | Placement rectangle before table-cell override | `page.bbox` |
| `text_x_density` | `7.25` | Points represented by one output column | `8` |
| `text_y_density` | `13` | Points represented by one output row | `14` |
| `text_x_shift` | `0` | Horizontal layout-origin shift | `2` |
| `text_y_shift` | `0` | Vertical layout-origin shift | `2` |
| `text_char_dir_render` | `None` | Final TextMap character direction | `"rtl"` |
| `text_line_dir_render` | `None` | Final TextMap line direction | `"btt"` |
| `text_presorted` | `False` | Utility-level TextMap ordering hint | `True` |

Together, the twenty table-specific names and twenty-eight forwarded names form
forty-six unique dictionary settings because the x/y text tolerances belong to
both groups. The reference option matrix uses fifty cases: the extra cases are
the additional non-default values for the two strategy settings, not hidden
keywords.

### Word options during text-based discovery

The forwarding sequence matters: when either strategy is `text`, the complete
forwarded dictionary is first passed to `Page.extract_words`.
WordExtractor-compatible options can therefore change both the imaginary edges
and the eventual text extracted from cells.

```python
word_driven = page.extract_tables(
    {
        "vertical_strategy": "text",
        "horizontal_strategy": "text",
        "min_words_vertical": 2,
        "min_words_horizontal": 2,
        "text_x_tolerance": 1,
        "text_y_tolerance": 1,
        "text_x_tolerance_ratio": 0.4,
        "text_y_tolerance_ratio": 0.4,
        "text_keep_blank_chars": True,
        "text_use_text_flow": True,
        "text_line_dir": "btt",
        "text_char_dir": "rtl",
        "text_line_dir_rotated": "rtl",
        "text_char_dir_rotated": "btt",
        "text_extra_attrs": ["fontname"],
        "text_split_at_punctuation": True,
        "text_expand_ligatures": False,
    }
)
```

The legacy direction switches are also accepted and preserve their upstream
logging behavior:

```python
legacy_direction = page.extract_tables(
    {
        "vertical_strategy": "text",
        "horizontal_strategy": "text",
        "text_vertical_ttb": False,
        "text_horizontal_ltr": False,
    }
)
```

TextMap-only keys such as `layout`, density, shift, and render direction are
not WordExtractor constructor options. TextMap-only keys combined with a
`text` strategy therefore raise the pinned `WordExtractor` unexpected-keyword
`TypeError`. Use text-based discovery with WordExtractor-compatible settings,
or keep TextMap controls on a line/explicit-discovered table.

### Cell text, layout, and the legacy tolerance

After discovery, the same forwarded dictionary is later passed to
`Table.extract` for every discovered cell. `text_tolerance` is a legacy
shorthand that fills missing text x/y tolerances and is then removed. A
specific `text_x_tolerance` or `text_y_tolerance` overrides the shorthand for
its axis.

For layout, when the `layout` key is present, `Table.extract` replaces layout
width, height, and bounding box with each cell's geometry. This happens based
on key presence, not whether its value is true. Caller-supplied `text_layout_width`,
`text_layout_height`, and `text_layout_bbox` are therefore replaced for each
cell in the active layout path.

```python
layout_cells = page.extract_tables(
    {
        "text_tolerance": 1,
        "text_x_tolerance": 1,
        "text_y_tolerance": 1,
        "text_layout": True,
        "text_layout_width": 500,
        "text_layout_height": 700,
        "text_layout_bbox": page.bbox,
        "text_x_density": 8,
        "text_y_density": 14,
        "text_x_shift": 2,
        "text_y_shift": 2,
        "text_char_dir_render": "rtl",
        "text_line_dir_render": "btt",
        "text_presorted": True,
    }
)
```

One limitation follows: active character-count layout dimensions conflict with
the point dimensions injected by `Table.extract`. In pinned v0.11.10, adding `"text_layout": True`
to the next call raises `ValueError`; without active layout, the two dimensions
are accepted but do not affect cell strings.

```python
inactive_character_dimensions = page.extract_tables(
    {
        "text_layout_width_chars": 70,
        "text_layout_height_chars": 50,
    }
)
```

## Resolution, validation, and exact failures

Strategies are validated against `lines`, `lines_strict`, `text`, and
`explicit`. The pinned non-negative check covers snap, join, edge-length,
minimum-word, and intersection settings on their general and axis forms. Thus
non-negative validation covers edge tolerances and thresholds but does not
validate forwarded `text_*` values. Those values may fail later in word or text
extraction instead.

Unknown table dictionary keys reach the dataclass constructor and raise its
unexpected-keyword `TypeError`. Values that are neither `None`, dictionaries,
nor `TableSettings` instances raise `ValueError` from `TableSettings.resolve`.
Do not replace these with one generic configuration exception in a differential
test.

```python
TableSettings.resolve({"snap_tolerance": 0})

# Exact pinned failures to test when they matter:
TableSettings.resolve({"snap_tolerance": -1})
TableSettings.resolve({"unknown_setting": 1})
TableSettings.resolve(42)
page.extract_tables(
    {"vertical_strategy": "explicit", "explicit_vertical_lines": []}
)
```

After discovery, `find_table` chooses the most cells, then the topmost and
leftmost table. `extract_table` uses that same choice and calls `Table.extract`;
the plural extraction method preserves the discovered table order.

## Current `pdfplumber-rs` surface matrix

The compatible examples above are pinned Python examples. The current project
does not yet accept that complete option surface.

| Surface | Current controls | Important boundary |
| --- | --- | --- |
| Rust `Page`/`CroppedPage` | Typed `TableSettings` for `find_tables`, `extract_tables`, and `extract_table`; `Page` also has SVG debugging | Similar type names do not imply Python resolution, validation, extraction, or tie-breaking parity |
| Python adapter | Argument-free `find_tables()` and `extract_tables()` on page and cropped page | Always constructs Rust defaults; the other three pinned methods and `TableSettings` are absent |
| Command-Line Interface `tables` | `lattice`/`stream`, general snap/join/text tolerances | No per-axis strategy, explicit lines, thresholds, intersection settings, extensions, or forwarded text dictionary |
| WebAssembly `PdfPage` | Argument-free `findTables()` and `extractTables()` | Always constructs Rust defaults |

The current Python `Page.find_tables` and `Page.extract_tables` accept no
settings argument. The current Python adapter does not expose `TableSettings`,
`find_table`, `extract_table`, or `debug_tablefinder`. The cropped-page adapter
has the same two argument-free methods. The current WebAssembly `findTables()`
and `extractTables()` methods always use defaults and accept no settings.

The current Command-Line Interface exposes only `lattice`/`stream`,
`snap_tolerance`, `join_tolerance`, and `text_tolerance`. In that adapter, the
Command-Line Interface copies its general snap/join values into both axis
fields. It also
copies text tolerance into the Rust general and axis text fields, but the
current core pipeline does not consult the three Rust text-tolerance fields.

### Typed Rust settings

Rust `TableSettings.strategy` is a fallback for optional per-axis strategies.
Rust `Strategy::Lattice`, `LatticeStrict`, `Stream`, and `Explicit` correspond
by intent to the four pinned strategy names. Optional `vertical_strategy` and
`horizontal_strategy` override the Rust fallback independently.

```rust,no_run
use pdfplumber::{ExplicitLines, Strategy, TableSettings};

let settings = TableSettings {
    strategy: Strategy::Lattice,
    vertical_strategy: Some(Strategy::Stream),
    horizontal_strategy: Some(Strategy::LatticeStrict),
    explicit_lines: Some(ExplicitLines {
        vertical_lines: vec![36.0, 300.0, 576.0],
        horizontal_lines: vec![72.0, 144.0, 216.0],
    }),
    snap_x_tolerance: 2.0,
    snap_y_tolerance: 2.0,
    ..TableSettings::default()
};
```

Rust explicit inputs are numeric coordinates grouped under `ExplicitLines`;
upstream object descriptors are not accepted by that type. Rust general
snap/join fields do not dynamically populate axis fields after construction:
the current pipeline reads the already concrete x/y fields. Rust table
discovery and cell extraction use `WordOptions::default()` instead of
forwarding the pinned `text_*` dictionary.

Rust does not reproduce `TableSettings.resolve` or its pinned validation and
error behavior. It also has no Rust `find_table` method returning a `Table`.
Rust `extract_table` breaks an equal-cell-count tie by area, not by pinned
top/left order.

### Rust-only extensions

Two fields are product extensions and must stay out of Python parity claims:

- `min_accuracy` is a Rust-only post-detection filter. `None` leaves every
  detected table; `Some(value)` retains tables whose computed accuracy meets
  the value.
- `duplicate_merged_content` is a Rust-only cell-content normalization. When
  true, content from a spanning cell is copied into its covered grid positions.

These names do not exist in pinned Python `TableSettings`. Keep them in a
separate Rust configuration layer rather than passing them through a dictionary
that is meant to be upstream-compatible.

## Evidence and claim boundary

The pinned 50-case table option matrix records reference behavior; it is not
candidate parity evidence. It includes all forty-six unique setting names,
every non-default strategy value, dynamic page-derived explicit coordinates,
complete normalized results, warnings/logs, fixture hashes, and target
provenance. It does not prove that a Rust-backed package accepts or matches
those calls.

The versioned scorecard labels Tables as observed evidence rather than a
workflow-level pass. The cross-surface rule is that matching a setting name or
default does not establish matching output. Compare complete table ordering,
bounding boxes, cell grids,
text, `None` placement, runtime types, logs, warnings, and failures on the PDFs
and installed artifacts that matter to the application.

This boundary means table-setting documentation is not compatibility evidence
and does not approve a compatibility deviation. TABLE-001 through TABLE-020
remain open, as do the edge, cell, public-API, differential, rotation, crop,
dashed-line, merged-cell, text-option, ordering, and strict Continuous
Integration tasks. This guide changes no runtime behavior, scorecard
observation, approved-delta record, package metadata, or release claim.

## Sources

The pinned target is tag `v0.11.10`, commit
`7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`. The primary source blobs used for
this guide are README `f6f1ce3e0e546b854787aff946601af44fcc6f69`,
`table.py` `3c493ee6f7c2de3dfe1c644c07689f7983b6f5a5`, and `page.py`
`286e7e158c12da8305520ecc1f550f3bd8f1a906`.

- Pinned upstream [README table reference](https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md).
- Pinned upstream [`TableSettings`, `TableFinder`, and `Table` implementation](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/table.py).
- Pinned upstream [`Page` table methods](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py).
- Repository [text-option guide](text-options.md), [reference option
  catalog](../compat/harness/option_matrix.py), and [catalog
  contract](../compat/tests/test_option_matrix.py).
- Current Rust [table pipeline and settings](../crates/pdfplumber-core/src/table.rs)
  and [page table methods](../crates/pdfplumber/src/page.rs).
- Current [Python adapter](../crates/pdfplumber-py/src/lib.rs), [Command-Line
  Interface arguments](../crates/pdfplumber-cli/src/cli.rs), and [WebAssembly
  adapter](../crates/pdfplumber-wasm/src/lib.rs).
- Versioned [Tables workflow evidence](compatibility/workflows-v0.3.0.md#tables).
