# Frequently Asked Questions

This page answers recurring questions about the current `0.3.x` alpha. The
[support matrix](support.md) is the source of truth for versions, maturity,
platform evidence, and known limitations; the versioned
[readiness snapshot](readiness/v0.3.0.md) names the workflows exercised in required
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

The Rust API accepts user or owner passwords through `Pdf::open_with_password` and
`Pdf::open_file_with_password`. The Python facade accepts `password=`, and applicable
Command-Line Interface commands accept `--password`. A missing password produces a
password-required error and a wrong password produces an invalid-password error.
The current high-level WebAssembly wrapper exposes only passwordless `WasmPdf.open`.

## What happens with malformed PDFs?

Normal opening returns an error when it cannot parse the input; it does not enable
repair implicitly. The Rust API offers best-effort native repair through
`Pdf::open_with_repair`, and applicable Command-Line Interface commands expose
`--repair`. In the Python compatibility
facade, `repair=True` invokes the external Ghostscript executable selected by
`gs_path` or found on the system path. Repair may still fail for severely damaged
files, so retain the original input and treat repaired output as data to verify. The
[privacy statement](privacy.md) documents the child-process and data boundary.

## Which coordinate system does the project use?

Public Rust bounding boxes use a top-left origin and PDF points. The tuple
`(x0, top, x1, bottom)` records the left edge, distance from the page top, right edge,
and distance from the page top to the lower edge. `doctop` adds the heights of earlier
pages to the page-local `top` value. The Python compatibility dictionaries also expose
bottom-origin `y0` and `y1` fields. Check the surface-specific geometry before mixing
these values, especially for rotated or cropped pages.

## Is the Python package a drop-in replacement for pdfplumber?

No. Compatibility with Python `pdfplumber` v0.11.10 is incomplete, and this alpha is
not a complete drop-in replacement. The installable distribution is `pdfplumber-rs`,
while its import package is `pdfplumber`; those files conflict with the separate
Python `pdfplumber` distribution. Use a fresh environment containing exactly one of
the two distributions, follow the [Python guide](../crates/pdfplumber-py/README.md),
and validate the APIs your application uses against the published gaps.

## Is the WebAssembly package ready for browser production use?

Treat it as experimental. Repository source is `0.3.0`.
The observed npm release is `0.2.0`. Required Continuous Integration builds bundler
and Node.js packages and executes the rendered Node.js Quick Start.
The browser end-to-end behavior is not gated. Browser compatibility, bundle size,
startup time, memory use, and installation from the published npm package remain
unverified; see the
[WebAssembly guide](../crates/pdfplumber-wasm/README.md) for the current API.
