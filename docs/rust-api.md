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

The [workspace and extraction architecture guide](architecture.md) explains
how the facade composes the parser and core crates, which state is cached, how
one text request flows through those layers, and where advanced parser or
binding extensions stop. Architecture visibility does not promote those lower
or host-specific APIs into this stable facade.

Proposals that add or change stable facade items must complete the
[Rust API-design review](rust-api-design.md). The record makes ownership,
allocation, iterator, ordering, error, extension-trait, and future-compatibility
decisions explicit before implementation and merge.

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
The [page-numbering guide](page-numbering.md) is the cross-surface authority for
Rust indexes, Python list positions and page numbers, WebAssembly indexes, and
safe conversions between them.
The [coordinate-system guide](coordinate-systems.md) is the cross-surface
authority for displayed `BBox` values, source page-box arrays, rotations,
Python bottom-origin companions, and document-top coordinates.
The [crop-semantics guide](crop-semantics.md) distinguishes the current Rust
center-selection and rebased-view contract from pinned Python intersection,
clipping, root-coordinate, nesting, and validation behavior.
The [text-option guide](text-options.md) distinguishes typed Rust word, layout,
and search options from the complete pinned Python option pipeline and its
method-specific result controls.
The [table-setting guide](table-settings.md) distinguishes typed Rust table
strategies and extensions from the complete pinned Python settings, resolution,
validation, and cell-text forwarding pipeline.
The [object-dictionary schema guide](object-dictionary-schemas.md) separates
typed Rust and Serde models from pinned Python's flat, ordered page-object and
derived-edge dictionaries.
The [visual-debugging guide](visual-debugging.md) distinguishes pinned Python's
raster `PageImage` contract from Rust's white-canvas `SvgRenderer`, selective
table-debug SVG, fixed Command-Line Interface overlays, and adapter gaps.
The [error and resource-limit guide](errors-and-resource-limits.md) is the
cross-surface authority for Python exceptions and warnings, Rust typed
diagnostics, wired versus declarative extraction controls, and host isolation.
The [encryption and repair guide](encryption-and-repair.md) is the authority for
password-aware inputs, verified revision limits, permission policy, native
repair semantics, Ghostscript compatibility, and adapter exposure.
The [parser and font limitations](parser-and-font-limitations.md) guide records
the structural and font-resolution behavior beneath this facade, including the
current Rust-only warning surface and measured compatibility residuals.
The [Rust-native extensions](rust-extensions.md) guide inventories the facade's
non-upstream inspection, export, rendering, semantic, table, layout, and
concurrency families together with their adapter and stability boundaries.

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
The source-backed design-review contract lives in
[`compat/tests/test_rust_api_design.py`](../compat/tests/test_rust_api_design.py),
with its primary-source mapping in
[`references/rust-api-design.md`](../references/rust-api-design.md).
