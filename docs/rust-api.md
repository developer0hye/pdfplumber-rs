# Public Rust API documentation contract

This page defines which `pdfplumber` Rust items form the stable generated
facade during the `0.3.x` line and what “useful rustdoc” means for that facade.
It is a documentation contract, not a promise that every public workspace or
crate-root symbol has the same compatibility status.

## Stable facade

The canonical entry point and page traversal types are `Pdf`, `Pages`, and
`PagesIter`. Extracted page values and their spatial views are `Page`,
`PageObjectKind`, `CroppedPage`, and the `FilteredPage` alias. The curated
character, word, geometry, table, metadata, warning, and extraction-option
types live under `pdfplumber::models`. The supporting stable diagnostic types
are `PdfError`, `PdfErrorKind`, `PdfErrorContext`, `PdfObjectId`, and
`PdfResourceLimit`.

Every visible associated item on those types is part of this documented
facade. Feature-gated items are included when their feature is enabled:
`Pdf::open_path` under the default `std` feature, the curated Serde contract
under `serde`, and `Pdf::pages_parallel` under `parallel`.

Compatibility aliases marked `#[doc(hidden)]` remain callable during the alpha
line but are not a second generated facade. Other types and algorithms
re-exported at the crate root remain source compatible for now, but only the
families re-exported through `pdfplumber::models` have the stable `0.3.x` model
commitment.

## Usefulness requirements

Rustdoc for each stable item must say what the item returns or represents and
record the observable details a caller needs to use it correctly. Depending on
the item, those details include zero-based indexing, top-left page coordinates,
ownership and borrowing, ordering, option effects, empty or absent results,
resource accounting, and feature availability. Accessor documentation may
delegate shared units and field semantics to the
[data-model contract](rust-data-models.md) instead of repeating it.

Every stable public function returning `Result` must include a `# Errors`
section. Every stable public function that Clippy can prove may panic must
either remove that panic or include a `# Panics` section describing the
condition. Intra-doc links must resolve in every enabled feature combination
covered by the build. A summary belonging to one method must not bleed into
the next method's rustdoc.

The [task-oriented Rust examples](rust-examples.md) are complete programs built
with all features on both supported Continuous Integration toolchains. Ignored
rustdoc snippets remain outside that compilation proof.

## Enforced gates

The facade crate carries `#![deny(missing_docs)]` and
`#![deny(rustdoc::broken_intra_doc_links)]`. Continuous Integration builds the
facade with all features and `RUSTDOCFLAGS=-D warnings`, so `std`, `serde`, and
`parallel` documentation must render together without a warning.

Continuous Integration also runs facade-only Clippy with all features,
`clippy::missing_errors_doc`, and `clippy::missing_panics_doc` denied. The
ordinary workspace Clippy run remains separate so this facade policy does not
silently promote every advanced `pdfplumber-core` algorithm into the stable
high-level contract.

The source-backed contract in
[`compat/tests/test_rust_rustdoc.py`](../compat/tests/test_rust_rustdoc.py)
guards the boundary, commands, lint policy, and known cross-item documentation
regressions. The official lint sources and the mapping from those sources to
this policy are recorded in [`references/rust-rustdoc.md`](../references/rust-rustdoc.md).
