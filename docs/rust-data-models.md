# Rust data-model contract

`pdfplumber::models` is the curated, stable data-model boundary for ordinary
Rust extraction workflows. The module gathers the character, word, geometry,
table, metadata, warning, and option types used with `Pdf` and `Page`; the same
items remain available at the crate root for source compatibility.

## Compatibility scope

For the `0.3.x` line, each curated model's public field name, Rust type, and
documented meaning is a compatibility commitment. Removing or changing a
public field is breaking, and adding a field to a struct that callers can
construct is also breaking. Removing, renaming, or adding an enum variant is breaking
for these exhaustive enums. Such changes require the next incompatible crate
line or an explicitly documented migration.

This commitment does not make every current root re-export a stable model. The
root continues to expose advanced algorithms and lower-level types for alpha
source compatibility, while `pdfplumber::models` is the deliberately reviewed
set. It also does not promise that extraction algorithms will never improve or
that every PDF produces byte-for-byte identical output; semantic changes still
require tests and changelog entries.

Serialized representations are not covered by this contract. Field names and
enum encodings produced with the optional `serde` feature need a separately
versioned schema or compatibility policy; that work remains tracked by DX-006.

## Curated families

| Family | Stable models |
|---|---|
| Characters and words | `Char`, `Word`, `TextDirection`, `TextLine`, `TextBlock` |
| Geometry and paint | `BBox`, `Line`, `Rect`, `Curve`, `Color`, `Orientation` |
| Tables | `Cell`, `Table`, `TableQuality` |
| Metadata | `DocumentMetadata`, `RawDocumentMetadata`, `MetadataEntry`, `MetadataValue`, `MetadataReference` |
| Warnings | `ExtractWarning`, `ExtractWarningCode` |
| Extraction options | `ExtractOptions`, `DedupeOptions`, `UnicodeNorm`, `WordOptions`, `TextOptions`, `ColumnMode`, `TableSettings`, `Strategy`, `ExplicitLines` |

Types outside this table are not silently promoted into the stable model
boundary merely because another curated type uses them internally or the crate
root currently re-exports them.

## Units and coordinate systems

Displayed page geometry uses PDF points: 72 points nominally equal one inch.
`BBox`, `Line`, `Rect`, `Curve`, table geometry, character boxes, word boxes,
and point-valued tolerances all use displayed page space after page rotation.

The displayed page-space origin is top-left; x increases to the right and y increases down.
A `BBox` is `(x0, top, x1, bottom)`, so width is `x1 - x0` and
height is `bottom - top`. Page indices are zero-based, as are warning page
indices and operator indices.

`Char::doctop` is a document-space vertical position: the sum of the displayed
heights of preceding pages plus the character's page-local `bbox.top`.
`Word::doctop` is the minimum `doctop` of its constituent characters. In
contrast, the text-space `advance` is not measured in page-space points.
`Char::ctm` is the six-value affine matrix active when the glyph was
rendered; it is not another top-left bounding box.

## Ordering

Page object slices such as characters, lines, rectangles, and curves retain
their content-stream encounter order within each object family. Default word
extraction uses spatial reading order; `WordOptions::use_text_flow = true`
preserves character content-flow order when grouping words.

Detected tables are ordered top-to-bottom, then left-to-right. `Table::cells`
has row-major order: top-to-bottom, then left-to-right. `Table::rows` uses that
same row order, while `Table::columns` is left-to-right with cells
top-to-bottom. A table grid may contain placeholder cells for spans, as
described below.

Raw metadata entries and nested dictionaries retain metadata source order.
Page warnings retain extraction encounter order. Consumers may therefore
stream or display either collection without an additional sort, but should not
infer ordering across different page-object families.

## Optional and empty values

`DocumentMetadata` fields are `None` when the corresponding information entry
is absent. An empty string is a present value and is not converted to absence.
The `resolution_error` field on `MetadataEntry` records a failed
indirect-reference resolution while `value` retains the unresolved source value.

`ExtractWarning` context fields are `None` when the page, element, operator, or
font context was unavailable or not applicable. The `collect_warnings` field
on `ExtractOptions` controls whether non-fatal warnings are retained; strict
mode can instead escalate them.

In fully extracted tables returned by `Page` or `CroppedPage`, `Some("")` in
`Cell::text` means a real cell was present but contained no extracted text.
`None` means the rectangular grid position is a placeholder where no cell
starts, usually because another cell spans it. Lower-level table-pipeline
callers may also observe `None` before calling a text-population function.

Option fields use `None` to mean disabled, unbounded, or inherited according
to the field: resource limits are unbounded, deduplication and punctuation
splitting are disabled, ratio tolerances use their fixed values, and per-axis
table strategies inherit `TableSettings::strategy`. `min_accuracy = None`
disables quality filtering, while `explicit_lines = None` supplies no
caller-defined boundaries.

## Using the boundary

Applications may import the families together without depending directly on a
workspace-internal crate:

```rust
use pdfplumber::models::{BBox, Char, ExtractOptions, Table, WordOptions};

fn accepts_models(
    chars: &[Char],
    tables: &[Table],
    _region: BBox,
    _extract: &ExtractOptions,
    _words: &WordOptions,
) -> usize {
    chars.len() + tables.len()
}
```

Continue to use the root `pdfplumber` crate as the only dependency for normal
extraction. The module is an organization and compatibility boundary, not a
second crate or a required conversion layer.
