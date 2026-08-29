# Text-option guide

This guide is the complete text-option reference for applications comparing
Python `pdfplumber` v0.11.10 with the current `pdfplumber-rs` surfaces. It
documents the pinned Python contract first, then names the narrower Rust,
Python-adapter, and WebAssembly boundaries. An option appearing on both sides
does not imply equal behavior.

The examples in the Python sections use the pinned upstream package. Keep it in
an environment separate from `pdfplumber-rs`: both distributions import as
`pdfplumber`.

```python
import pdfplumber

with pdfplumber.open("document.pdf") as pdf:
    page = pdf.pages[0]
    print(page.extract_text())
```

## Which method consumes which options?

Pinned v0.11.10 builds text in two stages. `WordExtractor` groups characters
into words. `WordMap.to_textmap` turns words and their characters into a
`TextMap`, optionally inserting layout whitespace. Page methods pass only the
options relevant to their stages.

| Pinned public method | Accepted option families | Result |
| --- | --- | --- |
| `Page.extract_words` | All word-grouping options, plus `return_chars` | Word dictionaries |
| `Page.extract_text` | All word-grouping and TextMap options except `presorted` | One string |
| `Page.extract_text_simple` | `x_tolerance`, `y_tolerance` | One string from the simpler algorithm |
| `Page.extract_text_lines` | All `Page.extract_text` options, plus `strip` and `return_chars` | Text-line dictionaries |
| `Page.search` | All `Page.extract_text` options, plus search controls | Match dictionaries |
| `utils.extract_words` | All word-grouping options, plus `return_chars` | Word dictionaries from supplied chars |
| `utils.extract_text` | Word-grouping and TextMap options, including `presorted` | One string from supplied chars |
| `utils.extract_text_simple` | `x_tolerance`, `y_tolerance` | One string from supplied chars |
| `utils.chars_to_textmap` | Word-grouping and TextMap options, including `presorted` | A `TextMap` from supplied chars |

`Page.extract_text`, `Page.extract_text_lines`, and `Page.search` use the same
option pipeline, but their method-specific defaults and return values still
matter. Do not forward one unreviewed dictionary to every method.

## Word-grouping options

The table lists the exact `WordExtractor` defaults. The example column uses a
non-default value suitable for an explicit compatibility test.

| Option | Default | Meaning | Non-default example |
| --- | --- | --- | --- |
| `x_tolerance` | `3` | Maximum intraline gap in PDF points when no ratio is set | `x_tolerance=1` |
| `y_tolerance` | `3` | Cross-line or vertical grouping tolerance in PDF points when no ratio is set | `y_tolerance=1` |
| `x_tolerance_ratio` | `None` | Dynamic horizontal tolerance based on the previous character size | `x_tolerance_ratio=0.4` |
| `y_tolerance_ratio` | `None` | Dynamic vertical tolerance based on the previous character size | `y_tolerance_ratio=0.4` |
| `keep_blank_chars` | `False` | Keep blank characters inside a word instead of using them as separators | `keep_blank_chars=True` |
| `use_text_flow` | `False` | Preserve content-stream flow instead of presorting characters spatially | `use_text_flow=True` |
| `vertical_ttb` | `True` | Legacy ordering switch for vertical words | `vertical_ttb=False` |
| `horizontal_ltr` | `True` | Legacy ordering switch for horizontal words | `horizontal_ltr=False` |
| `line_dir` | `"ttb"` | Expected direction in which upright lines advance | `line_dir="btt"` |
| `char_dir` | `"ltr"` | Expected direction in which characters advance within upright lines | `char_dir="rtl"` |
| `line_dir_rotated` | `None` | Line direction for rotated text; omitted means the upright character direction | `line_dir_rotated="rtl"` |
| `char_dir_rotated` | `None` | Character direction for rotated text; omitted means the upright line direction | `char_dir_rotated="btt"` |
| `extra_attrs` | `None` | Require equal character attributes within a word and copy them to the result | `extra_attrs=["fontname", "size"]` |
| `split_at_punctuation` | `False` | Split punctuation into standalone words | `split_at_punctuation=True` or `split_at_punctuation=",.;"` |
| `expand_ligatures` | `True` | Expand known presentation ligatures such as `ﬁ` to constituent letters | `expand_ligatures=False` |

### Tolerances, whitespace, and flow

`x_tolerance_ratio` replaces the fixed horizontal tolerance with the previous
character's `size` multiplied by the ratio. `y_tolerance_ratio` applies the
corresponding dynamic rule on the vertical axis. A ratio of zero is an active
zero tolerance; `None`, rather than falsiness, selects the fixed tolerance.

`keep_blank_chars=True` changes both token membership and resulting word
geometry. `use_text_flow=True` keeps the PDF content-stream order as the guide
for ordering and segmentation; it can resemble cursor selection order and is
not guaranteed to look like spatial reading order.

```python
words = page.extract_words(
    x_tolerance=1,
    y_tolerance=1,
    x_tolerance_ratio=0.4,
    y_tolerance_ratio=0.4,
    keep_blank_chars=True,
    use_text_flow=True,
)
```

### Directions and rotated text

Valid direction values are `"ttb"`, `"btt"`, `"ltr"`, and `"rtl"`.
`line_dir` and `char_dir` must be orthogonal: one must be vertical and the
other horizontal. The same validation applies to the rotated pair. For pinned
v0.11.10, omitted rotated directions cross-default:
`line_dir_rotated = char_dir` and `char_dir_rotated = line_dir`.

`vertical_ttb=False` and `horizontal_ltr=False` still affect ordering but emit
deprecation warnings. New code should express direction with the four explicit
direction options while retaining the legacy switches only when reproducing an
existing call and its warning behavior.

```python
right_to_left = page.extract_words(
    line_dir="btt",
    char_dir="rtl",
    line_dir_rotated="rtl",
    char_dir_rotated="btt",
)

with_deprecation_warnings = page.extract_words(
    vertical_ttb=False,
    horizontal_ltr=False,
)
```

### Attributes, punctuation, ligatures, and constituent chars

`extra_attrs` both separates words on attribute changes and copies those
attributes into each word dictionary. `split_at_punctuation=True` means
Python's complete `string.punctuation`; a string selects exactly the supplied
separators. `expand_ligatures=False` preserves a known ligature code point in
the word text. `return_chars` is a method/result option rather than a
`WordExtractor` constructor option.

```python
annotated_words = page.extract_words(
    extra_attrs=["fontname", "size"],
    split_at_punctuation=True,
    expand_ligatures=False,
    return_chars=True,
)

custom_punctuation = page.extract_words(split_at_punctuation=",.;")
```

## TextMap and layout options

These are the raw `WordMap.to_textmap` defaults. Page methods supply page-aware
values before calling it: `layout_bbox=page.bbox`, `layout_width=page.width`,
and `layout_height=page.height` unless the corresponding character-count size
was supplied. Thus the zero values below describe the public utility layer,
not an empty page rectangle silently substituted by `Page.extract_text`.

| Option | Default | Meaning | Non-default example |
| --- | --- | --- | --- |
| `layout` | `False` | Enable TextMap whitespace that mimics page layout | `layout=True` |
| `layout_width` | `0` | Target width in PDF points | `layout_width=500` |
| `layout_height` | `0` | Target height in PDF points | `layout_height=700` |
| `layout_width_chars` | `0` | Target width in output character cells | `layout_width_chars=70` |
| `layout_height_chars` | `0` | Target height in output character rows | `layout_height_chars=50` |
| `layout_bbox` | `(0, 0, 0, 0)` | Coordinate rectangle used to place words in the layout grid | `layout_bbox=page.bbox` |
| `x_density` | `7.25` | PDF points represented by one horizontal character cell | `x_density=8` |
| `y_density` | `13` | PDF points represented by one output row | `y_density=14` |
| `x_shift` | `0` | Horizontal origin shift subtracted before grid placement | `x_shift=2` |
| `y_shift` | `0` | Vertical origin shift subtracted before grid placement | `y_shift=2` |
| `char_dir_render` | `None` | Override the character direction of the rendered TextMap | `char_dir_render="rtl"` |
| `line_dir_render` | `None` | Override the line direction of the rendered TextMap | `line_dir_render="btt"` |
| `presorted` | `False` | Tell a public utility path that input words already have the required order | `presorted=True` |

`layout_width` and `layout_width_chars` are mutually exclusive.
`layout_height` and `layout_height_chars` are mutually exclusive. Supplying
both members of either pair raises `ValueError`. In both cases, layout
dimensions, density, bounding box, and shifts affect layout output only when
`layout=True`.

`line_dir_render` and `char_dir_render` transform the rendered TextMap after
grouping directions have been applied. They change how the final string is
transposed or reversed; they do not replace `line_dir`, `char_dir`, or their
rotated variants during word formation. Render directions must also be
orthogonal after defaults are resolved.

Use either point dimensions:

```python
fixed_points = page.extract_text(
    layout=True,
    layout_width=500,
    layout_height=700,
    layout_bbox=page.bbox,
    x_density=8,
    y_density=14,
    x_shift=2,
    y_shift=2,
    char_dir_render="rtl",
    line_dir_render="btt",
)
```

Or use character-count dimensions, never both forms for one axis:

```python
fixed_cells = page.extract_text(
    layout=True,
    layout_width_chars=70,
    layout_height_chars=50,
)
```

`presorted` belongs to the public utility path, not `Page.extract_text`.
`utils.chars_to_textmap` sets its internal word map as presorted after it has
handled the supplied character order. `utils.extract_text` accepts the keyword,
but its non-layout branch does not consult it and its layout branch reaches the
same forced-true helper. Treat the option as a `WordMap.to_textmap` control, not
as proof that either convenience utility preserved caller-selected ordering.

```python
from pdfplumber import utils

chars = page.chars
text = utils.extract_text(chars, presorted=True)
words = utils.extract_words(chars, return_chars=True)
simple = utils.extract_text_simple(chars, x_tolerance=1, y_tolerance=1)
textmap = utils.chars_to_textmap(chars, layout=True)
```

## Method-specific options

| Option | Default | Method scope | Effect |
| --- | --- | --- | --- |
| `return_chars` | `False` | `Page.extract_words`, `utils.extract_words` | Add constituent chars to each word dictionary when true |
| `return_chars` | `True` | `Page.extract_text_lines`, `Page.search` | Omit constituent or matched chars when false |
| `strip` | `True` | `Page.extract_text_lines` | Apply surrounding-whitespace stripping to line text |
| `regex` | `True` | `Page.search` | Interpret the pattern as regular expression syntax |
| `case` | `True` | `Page.search` | Match case-sensitively |
| `main_group` | `0` | `Page.search` | Select the whole match or one capturing group as the primary result |
| `return_groups` | `True` | `Page.search` | Include captured groups in each match dictionary |

### Simple text and text lines

`extract_text_simple` accepts only the two fixed tolerances and deliberately
uses a separate, less flexible algorithm. `extract_text_lines` defaults to
`strip=True` and `return_chars=True`. Its remaining keyword options are the same
options accepted by `Page.extract_text`.

```python
simple_text = page.extract_text_simple(x_tolerance=1, y_tolerance=1)

lines_without_chars = page.extract_text_lines(strip=False, return_chars=False)
layout_lines = page.extract_text_lines(layout=True, x_density=8, y_density=14)
```

### Search

The `pattern` argument can be a string or compiled regular expression;
compiled regular-expression patterns are accepted. With `regex=False`, a
string is treated literally. With `case=False`, matching is case-insensitive.
`main_group` selects the regex group whose text, characters, and bounding box
become the primary match. `return_groups=False` and `return_chars=False`
remove those fields from each result dictionary.

Pinned v0.11.10 follows a positional rule: zero-width and all-whitespace search
matches are discarded because they generally have no explicit page position.

```python
matches = page.search(
    pattern=r"(invoice)\s+(\d+)",
    regex=True,
    case=False,
    main_group=2,
    return_groups=False,
    return_chars=False,
    layout=True,
)
```

For a literal query, use `page.search("Invoice (final)", regex=False)`. If a
compiled pattern carries flags, preserve that compiled object in the
compatibility test instead of rebuilding it from its source string.

## Current `pdfplumber-rs` surface matrix

The compatible examples above are pinned Python examples. The current project
does not yet accept that complete option surface.

| Surface | Currently accepted controls | Important boundary |
| --- | --- | --- |
| Python `Page` | `extract_text(layout=False)`; `extract_words(x_tolerance=3, y_tolerance=3)`; `search(pattern, regex=True, case=True)` | Most pinned keywords and two text methods are absent |
| Python `CroppedPage` | `extract_text(layout=False)`; `extract_words(x_tolerance=3, y_tolerance=3)` | Extraction still uses the legacy Rust cropped view; search is absent |
| Rust `Page`/`CroppedPage` | Typed `WordOptions` and `TextOptions`; `SearchOptions` has regex and case controls | Types and names are not a Python-parity claim |
| WebAssembly `PdfPage` | `extractText(layout?)`; `extractWords(x_tolerance?, y_tolerance?)`; `search(pattern, regex?, case?)` | Other pinned text controls and text-line/simple-text methods are absent |

The current Python `Page.extract_text` accepts only `layout`. The current
Python `Page.extract_words` accepts only `x_tolerance` and `y_tolerance`. The
current Python `Page.search` accepts only `pattern`, `regex`, and `case`. The
current Python surface does not expose `extract_text_simple` or
`extract_text_lines`. The current WebAssembly surface exposes the same narrow
text, word, and search controls as the Python adapter.

Rust `WordOptions` exposes fixed and ratio tolerances, blank handling,
content-flow ordering, one `text_direction`, ligature expansion, and
punctuation splitting. It does not expose the pinned upright/rotated line and
character direction pairs, `extra_attrs`, or the Python result flag as
like-for-like fields. Rust words always retain typed constituent characters.

Rust `TextOptions` defaults to `x_density=10` and `y_density=10`, not the pinned
Python defaults. Rust `ColumnMode`, `min_column_gap`, and `max_columns` are
extensions, not Python-compatible text options. In current Rust text and
text-line extraction, only `y_tolerance` and `expand_ligatures` are forwarded
into word grouping; fixed/default values are used for the other word fields.
The Rust layout path is a block and column algorithm, not the pinned Python
TextMap grid algorithm.

The cross-surface rule is that matching an option name or default does not
establish matching output. Keep calls surface-specific and compare complete
returned strings, ordering, dictionaries, coordinates, runtime types,
warnings, and failures on the PDFs that matter to the application.

## Evidence and claim boundary

The pinned 111-case text option matrix records reference behavior; it is not
candidate parity evidence. It supplies at least one non-default observation for
each API-scoped keyword, fixture hashes, exact normalized results, logs, and
target provenance. It does not prove the current Rust-backed package accepts or
matches those calls.

The versioned scorecard labels Text, Words, and Search as observed evidence
rather than workflow-level passes. Review the exact result categories and
application-relevant records; do not turn them into a general compatibility
claim.

This boundary means text-option documentation is not compatibility evidence and
does not approve a compatibility deviation. TEXT-001 through TEXT-SEARCH-008
remain open, including the exact word, TextMap, extra-method, search, cache,
ordering, geometry, warning, and error work. This guide changes no runtime
behavior, scorecard observation, approved-delta record, package metadata, or
release claim.

## Sources

The pinned target is tag `v0.11.10`, commit
`7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62`. The source blobs used for this
guide are README `f6f1ce3e0e546b854787aff946601af44fcc6f69`, `page.py`
`286e7e158c12da8305520ecc1f550f3bd8f1a906`, and `text.py`
`1601f9c5097dc9ab06f37767849009c730155dac`.

- Pinned upstream [README text-method reference](https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md).
- Pinned upstream [`Page` implementation](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py).
- Pinned upstream [text utility implementation](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/utils/text.py).
- Repository [reference option catalog](../compat/harness/option_matrix.py) and
  [catalog contract](../compat/tests/test_option_matrix.py).
- Current Rust [word options](../crates/pdfplumber-core/src/words.rs),
  [layout options](../crates/pdfplumber-core/src/layout.rs), and
  [search options](../crates/pdfplumber-core/src/search.rs).
- Current [Python adapter](../crates/pdfplumber-py/src/lib.rs) and
  [WebAssembly adapter](../crates/pdfplumber-wasm/src/lib.rs).
- Versioned [Text workflow evidence](compatibility/workflows-v0.3.0.md#text),
  [Words workflow evidence](compatibility/workflows-v0.3.0.md#words), and
  [Search workflow evidence](compatibility/workflows-v0.3.0.md#search).
