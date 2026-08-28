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

The [Rust feature policy](rust-features.md) defines the default, additive
integration boundary, package-specific flags, and the representative
combination matrix. Parser-only and packaging-only flags remain outside the
stable facade.

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
with all features on the current stable Rust toolchain. Ignored rustdoc snippets
remain outside that compilation proof.

## Compile-time diagnostics

Compile-fail doctests cover three facade errors whose ordinary diagnostics can
hide the intended ownership or API pattern. `Pages` uses `IntoIterator` rather
than `Iterator`, so adapters start after an explicit `into_iter()`. A borrowed
`Pages<'_>` cannot outlive its source `Pdf`; return an owned `Page` instead.
The opaque `PdfError` has no matchable payload variants; branch on
`PdfError::kind()` and keep a wildcard arm for the non-exhaustive
`PdfErrorKind`.

Each intentional failure is adjacent to a compiling alternative in the public
rustdoc. Continuous Integration runs the all-feature doctest suite on current
stable Rust, so a negative example fails the gate if it unexpectedly starts
compiling and a positive replacement fails if it drifts from the API.

## Release SemVer gate

Every pull request is checked for a coordinated version change across the four
Rust packages published by the release workflow. Ordinary pull requests stop
after that inexpensive detection. A release pull request must advance
`pdfplumber-core`, `pdfplumber-parse`, `pdfplumber`, and `pdfplumber-cli`
together.

Release candidates run `cargo-semver-checks` for the three packages that expose
Rust library APIs, using the latest normal, non-yanked crates.io versions as
their baselines. The first pass forces patch compatibility, so any detected
public API break is visible even when the candidate version would allow it. If
that strict pass fails, a second version-aware pass approves the break only when
the candidate's SemVer increment permits it. The CLI remains in coordinated
version detection, but a binary-only command interface has no rustdoc library
API for `cargo-semver-checks` to compare.

An approved break must also have an entry in the candidate version's changelog
using `- **Migration:** Breaking: ...`. The note must name the removed behavior
and give a concrete replacement; placeholders do not satisfy the gate. One note
may describe several related diagnostics, but it must cover every intentional
break reported in the pull request.

The gate uses the action's stable-Rust and default-feature heuristic, which
enables ordinary public features such as `serde` and `parallel`. It cannot prove
all SemVer properties: the tool documents gaps around some type, generic,
lifetime, and partial-feature changes. Maintainer review and the migration-note
contract remain required rather than treating a green tool result as a complete
compatibility proof.

## Deprecation lifecycle

Stable facade items follow the [Rust deprecation policy](rust-deprecation-policy.md).
Ordinary deprecations provide complete `since` and replacement metadata, remain
callable through two subsequent published minor releases, and are removed only
in a SemVer-incompatible release after the existing migration-note gate passes.
Only an urgent safety issue or demonstrated unsoundness can shorten that window.
Hidden aliases are not implicitly deprecated.

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

The all-feature facade doctest command separately compiles the ordinary
examples and requires every `compile_fail` block to remain a compilation
failure on the current stable Rust toolchain.

The source-backed contract in
[`compat/tests/test_rust_rustdoc.py`](../compat/tests/test_rust_rustdoc.py)
guards the boundary, commands, lint policy, and known cross-item documentation
regressions. The official lint sources and the mapping from those sources to
this policy are recorded in [`references/rust-rustdoc.md`](../references/rust-rustdoc.md).
The release detector and workflow contract live in
[`compat/tests/test_rust_semver_release.py`](../compat/tests/test_rust_semver_release.py),
with upstream behavior recorded in
[`references/rust-semver-checks.md`](../references/rust-semver-checks.md).
