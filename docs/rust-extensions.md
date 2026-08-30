# Rust-native extensions

This guide inventories capabilities that are useful in `pdfplumber-rs` but are
not claims of Python `pdfplumber` compatibility. It describes the source tree at
merge commit `fba0fc9edf850dd07e78128a8f2787b3b239203b` for release `0.3.0`.
The inventory is deliberately current-state documentation: it does not turn an
implemented method into a stabilized cross-surface promise.

## How to read this guide

“Extension” has the project-wide meaning defined by the
[compatibility terminology contract](compatibility/terms.md): useful behavior
outside the pinned upstream contract. By definition, extension output is neither parity
evidence nor an approved deviation. A matching method name on two surfaces does
not make their indexing, schema, errors, or maturity interchangeable.

The compatibility reference is pinned CPython 3.13 with
`pdfplumber==0.11.10`. A live attribute probe on 2026-08-30 confirmed that
pinned `pdfplumber==0.11.10` exposes none of these five methods on `PDF`:
`bookmarks`, `form_fields`, `signatures`, `validate`, and `extract_images`. The
same probe found no `to_html`, `to_svg`, `export_images`, `semantic_chars`, or
`extract_text_body` method on the pinned `Page` class. That absence establishes
the classification; it does not prove the current implementation correct or
stable.

Read the exposure cells literally:

- **Rust** means the high-level `pdfplumber` facade unless a lower-level helper
  is named explicitly.
- **Python `document.rust`** means the explicit native-extension namespace, not
  the upstream-compatible `PDF` or `Page` API.
- **Command-Line Interface** means the alpha `pdfplumber` executable installed
  by `pdfplumber-cli`; it does not implement the upstream Python command-line
  contract.
- **WebAssembly** means the experimental `pdfplumber-wasm` package and its
  generated JavaScript/TypeScript boundary.

No extension family is promised on every surface. “Not exposed” is an
intentional statement of current source, not a request to infer a hidden route
through another adapter.

## Surface and maturity matrix

| Family | Rust | Python `document.rust` | Command-Line Interface | WebAssembly |
|---|---|---|---|---|
| Image extraction and export | `Page::export_images`, `Pdf::extract_image_content`, `Pdf::extract_images_with_content` | `document.rust.extract_images(page_index)` | `images`, with `--extract` for bytes | Not exposed |
| Bookmarks and outlines | `Pdf::bookmarks` | `document.rust.bookmarks()` | `bookmarks` | Not exposed |
| AcroForm fields | `Pdf::form_fields`, `Page::form_fields` | `document.rust.form_fields()` | `forms` | Not exposed |
| Digital-signature inspection | `Pdf::signatures` | `document.rust.signatures()` | Included by `info`; no dedicated verifier | Not exposed |
| HTML export | `Page::to_html`, `HtmlRenderer` | Not exposed | `text --format html` | Not exposed |
| SVG and table-debug SVG | `Page::to_svg`, `Page::debug_tablefinder_svg`, `SvgRenderer` | Not exposed | `debug`, optionally `--tables` | Not exposed |
| Semantic and structure traversal | `Pdf::structure_tree`, `Page::structure_tree`, `structure_elements`, `chars_by_mcid`, `semantic_chars` | Not in `document.rust`; separate compatibility properties exist | Not exposed | Not exposed |
| Validation and extraction warnings | `Pdf::validate`, `Page::warnings` | `document.rust.validate()`; warnings not exposed | `validate`; warnings not exposed | Not exposed |
| Table quality and merged-content normalization | `Table::quality`, `Table::accuracy`, `min_accuracy`, `duplicate_merged_content` | Not in `document.rust`; `Table.accuracy` is directly exposed | JSON table output includes quality | Not exposed |
| Multi-column and page-region reading order | Layout options, `Pdf::detect_page_regions`, `Page::extract_text_body` | Not in `document.rust` | `text --layout`; no page-region API | `extractText(layout?)`; no page-region API |
| Parallel page processing | `Pdf::pages_parallel` with the `parallel` feature | Not exposed | Not exposed | Not exposed |
| WebAssembly bindings | The adapter uses the Rust facade with bytes and Serde | Not applicable | Not applicable | `WasmPdf` and `WasmPage` subset |

All stabilization and exposure work remains separately tracked:

| Task | Current responsibility |
|---|---|
| `EXT-001` | Image extraction and export API stabilization remains open. |
| `EXT-002` | Bookmark and outline API stabilization remains open. |
| `EXT-003` | AcroForm API stabilization remains open. |
| `EXT-004` | Digital-signature inspection API stabilization remains open. |
| `EXT-005` | HTML export stabilization remains open. |
| `EXT-006` | SVG and table-debug SVG stabilization remains open. |
| `EXT-007` | Semantic character and structure traversal stabilization remains open. |
| `EXT-008` | Validation and warning API stabilization remains open. |
| `EXT-009` | Table quality and merged-content normalization stabilization remains open. |
| `EXT-010` | Automatic and explicit multi-column reading-order stabilization remains open. |
| `EXT-011` | Deterministic parallel page-processing stabilization remains open. |
| `EXT-012` | WebAssembly and TypeScript API stabilization remains open. |
| `EXT-013` | Intentional exposure across Rust, Python, Command-Line Interface, and WebAssembly remains open. |
| `EXT-014` | Collision-resistant namespacing across all extensions remains open. |
| `EXT-015` | Compatibility implications of enabling extensions remain open beyond this current-state guide. |

## Document inspection and assets

### Image bytes and deterministic export

The facade contains two distinct paths, and callers should choose deliberately:

1. `Page::export_images` packages bytes already present on extracted `Image`
   values. `Page::export_images` skips images without populated `Image.data` and
   therefore requires `ExtractOptions::extract_image_data`. Its default filename
   is `page{page}_img{index}.{ext}`: `{page}` is one-based while `{index}` is
   zero-based. Optional deduplication reuses a filename for identical content.
2. `Pdf::extract_image_content(page_index, image_name)` locates a named XObject
   and decodes it on demand. `Pdf::extract_images_with_content` applies that path
   to a page. `Pdf::extract_images_with_content` decodes named image XObjects on
   demand and skips an image whose bytes cannot be extracted. Inline or otherwise
   undecodable images can therefore be absent from its result without making page
   extraction fail.

The Python namespace wraps the second path. Its image results contain an
`image` dictionary, raw `data` bytes, `format`, `width`, and `height`. The
Command-Line Interface can list metadata without writing bytes; `images
--extract` writes the on-demand results to a selected directory. WebAssembly has
no image-byte method.

Source: [`Page::export_images`](../crates/pdfplumber/src/page.rs), the
[`Pdf` image methods](../crates/pdfplumber/src/pdf.rs), and the
[image models and naming helpers](../crates/pdfplumber-core/src/images.rs).

### Bookmarks, fields, and signatures

`Pdf::bookmarks` returns a cached slice. The bookmarks are a flattened outline
with `level` retaining nesting depth; a destination page uses the Rust-native
zero-based index. An absent outline produces an empty slice.

`Pdf::form_fields` reads the document AcroForm, while `Page::form_fields`
contains the fields associated with an extracted page. Form and signature
fields share one boundary: form and signature readers return an empty collection when the relevant AcroForm data is absent;
malformed AcroForm structures can instead return `PdfError`.

`Pdf::signatures` inspects signed and unsigned signature fields. For this
method, signature inspection reports field metadata and `is_signed`; it does not perform
cryptographic verification. In source, signature dates are retained as PDF strings rather
than validated timestamps. Do not use this API to claim certificate validity,
trust-chain status, revocation status, document integrity, or signer identity.

Source: the facade [`Pdf` methods](../crates/pdfplumber/src/pdf.rs),
[`Bookmark`](../crates/pdfplumber-core/src/bookmark.rs),
[`FormField`](../crates/pdfplumber-core/src/form_field.rs), and
[`SignatureInfo`](../crates/pdfplumber-core/src/signature.rs).

### Validation and extraction warnings

`Pdf::validate` checks a finite set of structural conditions and reports
`ValidationIssue { severity, code, message, location }`. `Pdf::validate` is a
bounded native structural checker, not a full ISO conformance certificate. An
empty result means only that the implemented checks found no issue.

Parser/interpreter recovery can also record `ExtractWarning` values on an
extracted page. `Page::warnings()` is available only when warning collection was
enabled before extraction. Validation issues and extraction warnings are
different types, are created at different phases, and must not be merged into a
single compatibility result. The Python namespace and Command-Line Interface
expose structural validation, but none of the adapters expose the page-warning
collection today.

Source: [`Pdf::validate`](../crates/pdfplumber/src/pdf.rs), the
[validation models](../crates/pdfplumber-core/src/validation.rs), and the
[warning models](../crates/pdfplumber-core/src/error.rs). Parser recovery and
warning boundaries are detailed in the
[parser and font limitations guide](parser-and-font-limitations.md).

## Rendering and semantic traversal

### HTML

`Page::to_html` detects tables with default settings and passes the page's
characters and tables to `HtmlRenderer`. HTML headings, emphasis, lists, blocks,
columns, and table placement are heuristic. Heading levels depend on font size
relative to the page median; bold and italic depend on font-name spelling; list
recognition uses text prefixes; table and text elements are then sorted by
their boxes. The output is semantic conversion, not preservation of the PDF's
appearance or tagged structure.

The Command-Line Interface `text --format html` uses default `HtmlOptions`.
Neither Python surface nor WebAssembly exposes HTML conversion.

Source: [`Page::to_html`](../crates/pdfplumber/src/page.rs),
[`HtmlRenderer`](../crates/pdfplumber-core/src/html.rs), and the
[Command-Line Interface text command](../crates/pdfplumber-cli/src/text_cmd.rs).

### SVG debugging

`SvgRenderer` can draw page geometry and overlays. The convenience methods have
narrower behavior: `Page::to_svg` draws only the page boundary with default
options, while `Page::debug_tablefinder_svg` selects table-finder edges,
intersections, cells, and table boxes according to `SvgDebugOptions`. The
Command-Line Interface `debug` command uses the renderer for object overlays or
the table-debug path with `--tables`.

This is a white-canvas diagnostic projection, not the raster `PageImage`
contract from Python `pdfplumber`. The exact drawing boundary is in the
[visual-debugging guide](visual-debugging.md). Python and WebAssembly expose no
SVG method.

Source: the facade [page methods](../crates/pdfplumber/src/page.rs),
[SVG renderer](../crates/pdfplumber-core/src/svg.rs), and
[debug command](../crates/pdfplumber-cli/src/debug_cmd.rs).

### Tagged structure and semantic character order

The Rust facade exposes cached document/page structure trees, flattened page
elements, characters grouped by marked-content identifier, and
`Page::semantic_chars`. In this surface, structure traversal requires tagged content and MCID
associations. An untagged document can produce empty/absent structure results,
and untagged characters are excluded from MCID groups. `semantic_chars` walks
the structure tree first and appends characters not reached by that traversal;
it is not a general semantic-understanding engine.

Python has separate `structure_tree` compatibility properties. Those are not
members of `document.rust`, and their Python dictionaries follow the pinned
compatibility contract rather than Rust model serialization. The Command-Line
Interface and WebAssembly expose no structure traversal today.

Source: [`Pdf::structure_tree`](../crates/pdfplumber/src/pdf.rs), the
[`Page` structure methods](../crates/pdfplumber/src/page.rs), and
[`StructElement`](../crates/pdfplumber-core/src/struct_tree.rs).

## Tables, layout, and concurrency

### Quality and merged content

Rust tables calculate filled-cell accuracy and average whitespace. Two settings
act after detection: `min_accuracy = None` disables filtering and
`duplicate_merged_content = false` leaves merged-cell placeholders unchanged.
When enabled, `min_accuracy` removes tables below the requested computed ratio;
`duplicate_merged_content` copies the real merged-cell text into placeholder
grid positions. Those operations change Rust extension results and do not
redefine pinned Python table semantics.

The current Python `Table` directly exposes `.accuracy`, and Command-Line
Interface JSON table output includes `accuracy` and `whitespace`. WebAssembly
serializes the detected table model but has no quality control or setting
argument. Detailed algorithms and compatibility differences are in the
[table-setting guide](table-settings.md).

Source: the facade [table pipeline](../crates/pdfplumber/src/page.rs),
[table models and helpers](../crates/pdfplumber-core/src/table.rs), and
[Command-Line Interface JSON projection](../crates/pdfplumber-cli/src/tables_cmd.rs).

### Multi-column and page regions

Rust layout extraction can cluster words into lines and blocks, detect column
boundaries, and sort blocks in reading order. Separately,
`Pdf::detect_page_regions` looks for repeating headers and footers across pages;
`Page::extract_text_body` then crops extraction to the derived body box.
The heuristic's page-region detection masks digit runs, requires a minimum page count, and
scans configurable header and footer margins. It is heuristic: variable text
outside digit runs, sparse pages, or content outside the scanned margins can
change the result.

The Command-Line Interface `text --layout` and WebAssembly
`extractText(layout?)` expose the layout toggle but not explicit page-region
detection. Python `document.rust` exposes neither route.

Source: [`Page` layout methods](../crates/pdfplumber/src/page.rs),
[`Pdf::detect_page_regions`](../crates/pdfplumber/src/pdf.rs), and the
[page-region algorithm](../crates/pdfplumber-core/src/page_regions.rs).

### Parallel pages

With the `parallel` feature, `Pdf::pages_parallel` maps every zero-based page
slot through Rayon and returns one `Result<Page, PdfError>` per slot. The ordering contract is that parallel
results retain page-index order, but shared resource budgets can make the first
reported limit failure scheduling-dependent. One page error does not cancel
the remaining slots. The method uses the caller's current Rayon pool and does
not select a worker count.

Python, the Command-Line Interface, and WebAssembly do not expose this method.
WebAssembly cannot enable the facade's Rayon path. See the
[Rust concurrency guide](rust-concurrency.md) for ownership, oversubscription,
and error-collection rules.

Source: [`Pdf::pages_parallel`](../crates/pdfplumber/src/pdf.rs) and the
[feature declaration](../crates/pdfplumber/Cargo.toml).

## Python `document.rust` namespace

`RustPDF` is implemented by the private `pdfplumber._native` module. Each
compatibility `PDF` instance exposes a `document.rust` property backed by the
same native document. The namespace has exactly five methods today:

| Method | Native result boundary |
|---|---|
| `document.rust.bookmarks()` | List of title, level, zero-based destination, and optional destination-top dictionaries |
| `document.rust.form_fields()` | List of native field dictionaries with zero-based optional `page_index` |
| `document.rust.signatures()` | List of metadata dictionaries; inspection only |
| `document.rust.validate()` | List of severity/code/message/location dictionaries |
| `document.rust.extract_images(page_index)` | List of image metadata and raw-byte dictionaries for a zero-based page |

Within that namespace, page indexes, bookmark destinations, and form-field page indexes remain
zero-based. At the boundary, every call returns native-shape dictionaries rather than
compatibility dictionaries. In source, compatibility `PDF`, `Page`, `PDF.objects`,
`to_dict`, `to_json`, and `to_csv` do not call this namespace, so invoking an
extension does not insert native-only fields into those compatibility outputs.

The current namespace does not prove the broader namespacing task complete.
`Table.accuracy` remains directly exposed on the current Python `Table` class
rather than under `document.rust`; this unnamespaced exception is a collision
risk, not a completed namespacing contract. Future upstream additions and other
direct extension properties still require review under `EXT-014`.

Source: the [PyO3 implementation](../crates/pdfplumber-py/src/lib.rs), packaged
[type stub](../crates/pdfplumber-py/python/pdfplumber/_native.pyi), and installed
[namespace regression](../crates/pdfplumber-py/tests/test_native_layout.py).

## Command-Line Interface and WebAssembly boundaries

The alpha Command-Line Interface exposes extension functionality through
ordinary subcommands rather than a separate executable namespace:

- `images --extract`, `bookmarks`, and `forms` expose document assets;
- `info` includes signature metadata but does not verify signatures;
- `validate` reports native structural issues;
- `text --format html` renders heuristic HTML;
- `debug` and `debug --tables` write SVG diagnostics;
- `tables --format json` includes native quality values.

Page selection flags are one-based at the command boundary even when the Rust
models underneath carry zero-based indexes. The executable has no structured
page-warning output, semantic-tree command, explicit page-region command, or
parallel switch.

WebAssembly exposes bytes-only open, metadata, page access, characters, text,
words, tables, and search. WebAssembly does not expose the document inspection,
image-byte export, HTML, SVG, validation, warning, structure, or parallel APIs
listed above. Its `layout` boolean reaches Rust text layout, but table calls use
default settings and accept no extension controls. The generated package is an
experimental adapter subset, not proof that all Rust exports work in a browser.

Source: the [Command-Line Interface declaration](../crates/pdfplumber-cli/src/cli.rs),
[WebAssembly binding](../crates/pdfplumber-wasm/src/lib.rs), and checked
[TypeScript declaration](../crates/pdfplumber-wasm/pdfplumber-wasm.d.ts).

## Compatibility implications

A Cargo feature is a compile-time capability switch, not an enhanced
compatibility mode. `std` adds path constructors, `serde` adds trait
implementations, and `parallel` adds `Pdf::pages_parallel`. The contract is that enabling a feature
must not change sequential extraction values or strict Python output. Runtime
options can change Rust extension output, but cannot silently alter the
compatibility facade's defaults or schemas.

Use these review rules when adopting or changing an extension:

1. Compare compatible operations against the pinned reference before looking at
   extension output. An extension cannot compensate for a failed or untested
   compatibility call.
2. Keep native page indexes and schemas at the extension boundary. Convert a
   page identity exactly once when crossing to a one-based Python page number or
   Command-Line Interface selector.
3. Do not add extension fields to compatibility dictionaries, JSON, CSV,
   scorecards, or golden artifacts. Version any independently serialized native
   schema under its own contract.
4. Treat missing adapter exposure as missing, not as implicit support. Add a
   deliberate surface and test it if an application requires that route.
5. Preserve default-off extension controls when they change output, resource
   use, or execution order; document their effects and test their union with
   other Cargo features.

The full Cargo matrix and additive-semantics rule are in the
[Rust feature policy](rust-features.md). Python/Rust page-number conversions are
in the [page-numbering guide](page-numbering.md), and typed-versus-compatible
schemas are in the [Rust data-model](rust-data-models.md) and
[object-dictionary](object-dictionary-schemas.md) guides.

## Validation and source map

The current inventory is bound to these implementation authorities:

| Boundary | Source |
|---|---|
| High-level document and page APIs | [`crates/pdfplumber/src/pdf.rs`](../crates/pdfplumber/src/pdf.rs), [`crates/pdfplumber/src/page.rs`](../crates/pdfplumber/src/page.rs) |
| Image bytes/export | [`crates/pdfplumber-core/src/images.rs`](../crates/pdfplumber-core/src/images.rs) |
| HTML | [`crates/pdfplumber-core/src/html.rs`](../crates/pdfplumber-core/src/html.rs) |
| SVG | [`crates/pdfplumber-core/src/svg.rs`](../crates/pdfplumber-core/src/svg.rs) |
| Tables | [`crates/pdfplumber-core/src/table.rs`](../crates/pdfplumber-core/src/table.rs) |
| Page regions | [`crates/pdfplumber-core/src/page_regions.rs`](../crates/pdfplumber-core/src/page_regions.rs) |
| Validation | [`crates/pdfplumber-core/src/validation.rs`](../crates/pdfplumber-core/src/validation.rs) |
| Signatures | [`crates/pdfplumber-core/src/signature.rs`](../crates/pdfplumber-core/src/signature.rs) |
| Python namespace | [`crates/pdfplumber-py/src/lib.rs`](../crates/pdfplumber-py/src/lib.rs), [`_native.pyi`](../crates/pdfplumber-py/python/pdfplumber/_native.pyi) |
| Command-Line Interface | [`crates/pdfplumber-cli/src/cli.rs`](../crates/pdfplumber-cli/src/cli.rs) |
| WebAssembly | [`crates/pdfplumber-wasm/src/lib.rs`](../crates/pdfplumber-wasm/src/lib.rs) |

Targeted unit and installed-package regressions remain the behavioral evidence;
this guide is not a substitute for them. The Python namespace fixture exercises
all five native dictionary families. Facade/core tests cover image export,
bookmarks, fields, signatures, validation, HTML, SVG, structure traversal,
table quality, page regions, and parallel ordering. Command-Line Interface and
WebAssembly suites separately cover their public adapter subsets.

Maturity remains surface-specific: Rust release `0.3.0` is alpha, the Python
distribution is alpha, the Command-Line Interface is alpha, and the WebAssembly
package is experimental. These labels come from the generated
[support matrix](support.md) and versioned [readiness page](readiness/v0.3.0.md);
the guide does not update either generated artifact.

## Claim boundary

DOC-015 changes documentation only. It does not stabilize any extension API and
does not change runtime behavior, fixtures, thresholds, tolerances, generated
support/readiness artifacts, or compatibility results. It adds no cross-surface
exposure and approves no compatibility deviation.

`PDF-035`, `EXT-001` through `EXT-015`, and strict section 10 remain open. The
open tasks still own API stability, naming, output schema, error behavior,
resource behavior, adapter exposure, future upstream-collision handling, and
remote release evidence. A consumer should pin the exact `0.3.x` release,
enable only required features, and test the extension results it depends on.
