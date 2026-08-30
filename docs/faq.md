# Frequently Asked Questions

This page answers recurring questions about the current `0.4.x` alpha. The
[support matrix](support.md) is the source of truth for versions, maturity,
platform evidence, and known limitations; the versioned
[readiness snapshot](readiness/v0.4.0.md) names the workflows exercised in required
Continuous Integration. Update this page and its contract test whenever those public
boundaries change.

## Does pdfplumber-rs support scanned or image-only PDFs?

Not by itself. `pdfplumber-rs` does not perform Optical Character Recognition (OCR);
it extracts text that is already encoded in PDF content streams. Run an OCR tool
before extraction so a scanned or image-only document becomes a searchable PDF,
then validate the resulting text and geometry for your workflow.

## How does table extraction work, and will it find every table?

Table detection can use `lattice` (drawn lines), `stream` (text alignment), or
`explicit` boundaries, with a separate strategy for each axis. Detection is
heuristic: a result from one layout is not a guarantee for another, and tolerances
or strategies may need adjustment. Validate the extracted cells instead of treating
detection as ground truth. Start with the [Rust examples](../README.md#quick-start)
or the [Command-Line Interface guide](../crates/pdfplumber-cli/README.md).

## Can it open password-protected PDFs?

The Rust API accepts passwords through the matching
`Pdf::open_bytes_with_password`, `Pdf::open_path_with_password`, and
`Pdf::open_reader_with_password` input family. The Python facade accepts
`password=`, and applicable Command-Line Interface commands accept `--password`.
A missing password produces a password-required error and a wrong password
produces an invalid-password error.
The current high-level WebAssembly wrapper exposes only passwordless `WasmPdf.open`.
The [encryption and repair guide](encryption-and-repair.md) gives the verified
revision matrix, including the current legacy owner-password extraction gap and
the empty-user-password edge case.
The [parser and font limitations](parser-and-font-limitations.md) guide records
what a successful open does and does not prove, plus current Unicode, metrics,
warning, and cross-surface limitations.

## What happens with malformed PDFs?

Normal opening returns an error when it cannot parse the input; it does not enable
repair implicitly. The Rust API offers best-effort native repair through
`Pdf::open_with_repair`, and applicable Command-Line Interface commands expose
`--repair`. In the Python compatibility
facade, `repair=True` invokes the external Ghostscript executable selected by
`gs_path` or found on the system path. Repair may still fail for severely damaged
files, so retain the original input and treat repaired output as data to verify. The
[privacy statement](privacy.md) documents the child-process and data boundary.
The [encryption and repair guide](encryption-and-repair.md) defines exact
Ghostscript arguments and ownership, native repair scope, and the non-composing
Command-Line Interface flags.

## Which coordinate system does the project use?

Public Rust bounding boxes use a top-left origin and PDF points. The tuple
`(x0, top, x1, bottom)` records the left edge, distance from the page top, right edge,
and distance from the page top to the lower edge. `doctop` adds the heights of earlier
pages to the page-local `top` value. The Python compatibility dictionaries also expose
bottom-origin `y0` and `y1` fields. Check the surface-specific geometry before mixing
these values, especially for rotated or cropped pages. The diagrammed
[coordinate-system guide](coordinate-systems.md) defines the transforms, page boxes,
surface matrix, formulas, and persistence names.

For cropping, pinned Python `pdfplumber` retains overlapping objects, clips partial
extent fields, and keeps coordinates in the root page frame. Current Rust crop
methods instead retain objects by center, keep their full extents, and rebase
coordinates to the crop origin. The Python alpha currently mixes transformed
object-list properties with extraction methods backed by that Rust view. The
[crop-semantics guide](crop-semantics.md) documents the exact inclusion,
clipping, nesting, validation, and surface boundaries. The compatibility
scorecard still marks crop as not tested.

Text option names are also surface-specific. The [text-option guide](text-options.md)
lists the complete pinned Python grouping, layout, line, and search controls,
their interactions and examples, and the narrower current Rust, Python, and
WebAssembly boundaries.

Table configuration is likewise surface-specific. The
[table-setting guide](table-settings.md) covers every pinned strategy,
explicit-line input, tolerance, threshold, and forwarded text option, including
their pipeline order and current Rust, Python, Command-Line Interface, and
WebAssembly boundaries.

Object field names, presence, and order are surface-specific too. The
[object-dictionary schema guide](object-dictionary-schemas.md) lists every
observed pinned family, derived-edge schema, serialization boundary, and known
current adapter gap without treating documentation as parity evidence.

Visual debugging is surface-specific as well. The
[visual-debugging guide](visual-debugging.md) documents pinned Python's
PDFium/Pillow raster and overlay behavior, its local viewer and file side
effects, the current Rust/Command-Line Interface SVG extension, and the absent
Python-adapter and WebAssembly visual APIs.

Error handling and resource protection are also surface-specific. The
[error and resource-limit guide](errors-and-resource-limits.md) lists exact
pinned Python exception/warning behavior, typed Rust errors, enforced and
declarative-only budgets, adapter gaps, and safe operational controls.

## Is the Python package a drop-in replacement for pdfplumber?

No. Compatibility with Python `pdfplumber` v0.11.10 is incomplete, and this alpha is
not a complete drop-in replacement. The installable distribution is `pdfplumber-rs`,
while its import package is `pdfplumber`; those files conflict with the separate
Python `pdfplumber` distribution. Use a fresh environment containing exactly one of
the two distributions, follow the [migration guide](python-migration.md), and
validate the APIs your application uses against the published gaps. The
[Python guide](../crates/pdfplumber-py/README.md) lists the current surface. If
your application already uses `pdfplumber-rs` 0.2.0, use the
[pre-parity binding guide](pre-parity-python-migration.md) instead of treating
the 0.3.x alpha as an in-place compatible upgrade.

## Is the WebAssembly package ready for browser production use?

Treat it as experimental. Repository source is `0.4.0`.
The observed npm release is `0.2.0`. Required Continuous Integration installs fresh
bundler and Node.js package archives, type-checks strict consumers, and requires exact
fixture output in Node.js 24.20.0 and Playwright 1.62.1 Chromium. This one maintained
Chromium build does not establish cross-browser or production support. Bundle size,
startup time, memory use, and installation from the published npm package remain
independent gates; see the [WebAssembly guide](../crates/pdfplumber-wasm/README.md)
and [prepublication test boundary](wasm-package-testing.md).
