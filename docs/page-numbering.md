# Page numbers and page indexes

`pdfplumber-rs` has host-language boundaries with deliberately different page
bases. The Python compatibility surface follows Python `pdfplumber` and exposes
one-based document page numbers. The Rust and WebAssembly surfaces expose
zero-based indexes. Python collection positions are also zero-based even though
the `Page` values stored in the collection have one-based `page_number` values.

Use these terms consistently:

| Term | Meaning | Base |
|---|---|---|
| Python page-list position | Position used by `pdf.pages[...]` | Zero-based |
| Python document page number | `Page.page_number`, `pages=`, object dictionaries, and compatibility serialization | One-based |
| Rust page index | `Pages::get`, `Pdf::page`, `Page::page_number`, errors, and model fields | Zero-based |
| WebAssembly page index | `WasmPdf.page()` and `WasmPage.pageNumber` | Zero-based |
| Rust-extension page index | Bookmark destinations, form-field indexes, and image-extraction input under `document.rust` | Zero-based |

Do not infer the base from the word “number” alone. Read it from the API surface
and record it explicitly at application boundaries.

## Python compatibility surface

`pdf.pages` is a Python list, so its positions are zero-based. The first element
is `pdf.pages[0]`, but its compatibility identity is one-based:
`pdf.pages[0].page_number == 1`.

`pages=` accepts one-based document page numbers. It is a membership filter,
not a list of zero-based offsets. For example:

```python
import pdfplumber

with pdfplumber.open(path, pages=(3, 5)) as pdf:
    assert [page.page_number for page in pdf.pages] == [3, 5]
    third_document_page = pdf.pages[0]
    fifth_document_page = pdf.pages[1]
```

The filtering rule means selection is deduplicated and returned in document
order; the order or repeated values in the selector do not reorder or duplicate
pages. Using no values means an empty selection produces an empty page list.
Values that are not valid one-based document page numbers select no corresponding
page, and a non-iterable selector fails when the page list is materialized.

After filtering, selected pages preserve their original document page numbers.
In the example, the selected list has positions zero and one while the page
values keep numbers three and five. They are not renumbered to one and two.

Crop, `within_bbox`, and `outside_bbox` create derived pages. Those operations
mean derived pages copy the immediate parent's current `page_number`, including
an application mutation made before deriving the child. That value identifies
the source page; it is not the derived page's list position or a new crop
sequence number.

Page objects, character and geometry dictionaries, annotations, structure
elements, `to_dict()`, JSON, and CSV stay within the Python compatibility
surface. Therefore, object dictionaries and serialized Python output use the
same one-based `page_number` as their source page.

## Rust facade

Rust collection access, returned `Page` identity, iterator order, error context,
and native models all use zero-based indexes. Rust `Page::page_number()` is a
zero-based index despite its historical name. Direct access makes the alignment
visible:

```rust,ignore
let first = pdf.pages().get(0)?;
assert_eq!(first.page_number(), 0);

let index = 4;
let fifth = pdf.page(index)?;
assert_eq!(fifth.page_number(), index);
```

`pdf.page(index)` and `pdf.pages().get(index)` have the same base. Iteration
preserves that identity:

```rust,ignore
for (index, page) in pdf.pages().into_iter().enumerate() {
    let page = page?;
    assert_eq!(index, page.page_number());
}
```

The same zero-based rule applies to `pages_parallel()` result order, page
locations carried by `PdfErrorContext`, bookmark destinations, warnings,
search matches, structure-tree page indexes, and native serialization fields.

## WebAssembly surface

WebAssembly follows the Rust convention. Both selection and the returned
property use a zero-based index:

```javascript
const first = pdf.page(0);
console.assert(first.pageNumber === 0);
```

Do not add one when moving between Rust and WebAssembly. Convert only when data
crosses into or out of the Python compatibility contract.

## Rust-only extensions from Python

The Python package exposes native-only capabilities under `document.rust` so
they cannot be confused with upstream-compatible Python behavior. For mixed
Python calls, the compatibility facade and the Rust-only namespace use different
bases:

The complete exposure and schema boundary is in the
[Rust-native extensions](rust-extensions.md) guide.

- `document.rust.bookmarks()` destinations are zero-based;
- `document.rust.form_fields()` page indexes are zero-based;
- `document.rust.extract_images(page_index)` accepts a zero-based index.

These extension values do not become one-based merely because Python calls the
methods. A Python `Page.page_number` and a bookmark destination that identify
the same first document page are respectively one and zero.

## Convert at one boundary

Applications must convert exactly once at the surface boundary. The arithmetic
is:

```text
python_page_number = rust_page_index + 1
rust_page_index = python_page_number - 1
```

Validate the source domain before conversion:

```text
python_page_number >= 1
rust_page_index < pdf.pages().len()
```

For Python-to-Rust conversion, validate before subtracting so zero and negative
values do not become valid or underflowing indexes. For Rust-to-Python
conversion, check the Rust index against the document page count before adding
one. `Pages::get` and `Pdf::page` are fallible: an out-of-range Rust index
returns `PdfError`. The WebAssembly adapter maps that failure to `JsError`, so
an out-of-range WebAssembly index throws a JavaScript error.

Keep the conversion beside the boundary type or adapter. At that boundary, do
not apply a blanket increment or decrement to an entire Python response: its
compatibility fields and `document.rust` extension fields can coexist with
different bases. Do not convert a value twice after it has already entered the
destination contract.

PDF page labels are separate metadata. A displayed Roman numeral, section
prefix, or custom label does not change the list position, document page number,
or Rust index described here. Do not feed a page label into numeric conversion.

## Logs, storage, and messages

Do not persist or log a cross-surface field named only `page`. The same rule
applies to a newly designed field named only `page_number` when both surfaces
can reach that schema. Always record the base in the field name or schema. For
example:

```json
{
  "page_number_one_based": 5,
  "page_index_zero_based": 4,
  "bookmark_page_index_zero_based": 4
}
```

If an existing database stores the legacy pre-parity value, changing an index
base is a data migration. Version the schema, transform old rows once, and keep
the source base in migration metadata. Do not silently reinterpret stored
values on every read.

User-visible messages normally use the Python-style one-based document number;
internal Rust diagnostics retain their zero-based page index. Label either one
when logs from both paths are aggregated.

## Evidence and compatibility boundary

This page documents the current API contracts; page-number documentation is not
compatibility evidence for an untested workflow, release, artifact, or platform.
It does not approve a compatibility deviation, change a scorecard result, or
turn a Rust-only extension into Python parity. Use the
[compatibility terminology](compatibility/terms.md),
[workflow scorecard](compatibility/workflows-v0.3.0.md), and
[Python-release matrix](compatibility/python-release-matrix-v0.3.0.md) for
claim-scoped evidence.

## Sources

- Pinned upstream Python behavior: `pdfplumber` v0.11.10
  [`pdf.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/pdf.py)
  and
  [`page.py`](https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py).
- Current Rust collection and direct-access contract:
  [`crates/pdfplumber/src/pdf.rs`](../crates/pdfplumber/src/pdf.rs).
- Current zero-based Rust page identity:
  [`crates/pdfplumber/src/page.rs`](../crates/pdfplumber/src/page.rs).
- Current Python compatibility translation:
  [`crates/pdfplumber-py/src/lib.rs`](../crates/pdfplumber-py/src/lib.rs).
- Current WebAssembly translation:
  [`crates/pdfplumber-wasm/src/lib.rs`](../crates/pdfplumber-wasm/src/lib.rs).
