# Changelog

All notable user-visible changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Release notes are derived from this canonical record rather than from an uncurated
commit list.

## [Unreleased]

### Added

- **Performance:** Defined a versioned, redistributable ten-document benchmark
  corpus covering text, word geometry, tables, graphics, CJK, right-to-left,
  images, encryption, malformed inputs, and measured small/large sizes. Exact
  fixture digests and access metadata are checked against the licensed corpus.
  Added exact-output-gated cold-process, warmed-process, live-page cache-hit,
  single-page, full-document, and four-worker ordered parallel page scenarios;
  operating-system filesystem cache state remains explicitly uncontrolled.
  A fail-closed preflight now requires identical fixture digests, request
  semantics, output schemas, and canonical JSON results before a case is
  eligible for timing. A source-pinned local suite now exercises overlapping
  document-open and page-preserving text workloads against Python `pdfplumber`,
  `pdf_oxide`, and `pdfsink-rs`; rejected cases remain untimed and the
  one-sample results are not published as a comparison. A second local suite
  now separates seven component clocks for document open, page materialization,
  character extraction, word grouping, table detection, canonical JSON
  serialization, and installed-candidate PyO3 conversion. Process launch and
  named setup remain outside every clock; exact-output failures and fused APIs
  retain explicit untimed outcomes. A separate instrumented pass now reports
  stage-only process CPU and method-specific Python/Rust allocation fields while
  labeling peak resident memory as a process-lifetime high-water mark. Candidate
  Command-Line Interface size, WebAssembly runtime bundle size, and fresh-process
  Node module startup are measured without attributing the combined competitor
  adapter binary to any one implementation. A clean-source run contract now
  records host hardware and operating system, Python/Rust/build-tool versions,
  explicit release build flags, dependency-lock and built-artifact digests, fixture hashes, and
  exact argument arrays. It retains five round-robin raw repetitions per eligible
  key and emits sample-bound minimum, median, mean, maximum, sample-deviation, and
  relative-deviation summaries. Results remain local and unpublished.
- **Compatibility:** Published a versioned machine-readable scorecard with
  per-API, per-option, per-fixture-class, per-page, per-platform, and
  per-artifact results. Exact matches, approved deltas, unsupported behavior,
  reference failures, candidate failures, and untested coverage remain
  distinct countable outcomes rather than one percentage. A generated human
  report groups those observations into eleven common workflows and leaves
  absent crop, serialization, rendering, and Command-Line Interface parity
  evidence explicitly untested.
- **Documentation:** The primary Rust quick start is a complete, fallible program
  of no more than fifteen lines that opens a PDF, propagates file and page errors,
  prints extracted text, and is executed against a fixture in Continuous Integration.
- **Documentation:** Defined the stable Rust facade and its
  [public rustdoc contract](docs/rust-api.md). Continuous Integration now builds
  all-feature rustdoc with warnings denied and rejects undocumented fallible or
  panicking public methods.
- **Documentation:** Added complete compiled Rust examples for text, word
  bounding boxes, tables, page geometry, metadata, encrypted and malformed
  inputs, Serde JSON, and ordered parallel page processing.
- **Documentation:** Added compile-fail documentation tests with compiling
  alternatives for page-view adapters, borrowed page-view lifetimes, and
  opaque error classification on current stable Rust.
- **Platform:** Rust release pull requests now run `cargo-semver-checks` against
  the published library baselines. Detected breaks require both a compatible
  release-version increment and actionable migration notes in the release
  changelog.
- **Platform:** Every crates.io candidate now runs verified `cargo package` and
  `cargo publish --dry-run` commands from one exact clean commit before tagged
  publication. Coordinated workspace dependencies are patched only during the
  candidate preflight; ordinary publication still verifies registry-resolvable
  predecessors.
- **Platform:** Added a reproducible Rust development container pinned to the
  official Rust 1.98.0 Bookworm image digest. One command runs the rendered Rust
  quick starts, focused extraction and concurrency tests, and all-feature
  contributor examples in an isolated, read-only checkout.
- **Platform:** Added an additive Rust feature policy and a Continuous
  Integration matrix for no-default, default, Serde, parallel, all-feature,
  and parser-tracing builds. Exact fixture fingerprints ensure optional
  integrations do not silently change text, geometry, or table extraction.
- **Documentation:** Added a Rust deprecation policy that retains stable facade
  items through at least two subsequent published minor releases, requires
  compiler-visible replacement guidance, and limits shorter windows to urgent
  safety issues or demonstrated unsoundness.
- **Documentation:** Added a source-backed architecture guide for all six
  workspace crates, including dependency direction, one extraction request,
  cache lifetimes, extension boundaries, and contributor ownership.
- **Documentation:** Added required API-design review criteria for stable Rust
  facade changes, covering ownership, allocations, iterator behavior,
  determinism, error composition, extension traits, and future compatibility.
- **Documentation:** Measured the current-source Rust time to first value from an
  empty Cargo project and cache at under five minutes, including dependency
  resolution, code copy, execution, and interpretation. Optional features and
  internal crates now follow rather than interrupt the first useful extraction;
  public-registry installation remains a separate release gate.
- **Documentation:** Major README and versioned release-note claims now carry
  adjacent links to repository tests, readiness scorecards, benchmark artifacts, or
  generated support entries, with a contract test that rejects missing or stale
  targets.
- **Documentation:** Added a dated adoption baseline for registry downloads, public
  dependent repositories, documentation traffic, activation-failure observability,
  issues, and confirmed external adopters before defining growth targets.
- **Documentation:** Added a privacy and local-processing statement covering local
  extraction, document uploads, usage telemetry, host applications, and the optional
  Python Ghostscript repair process.
- **Documentation:** Added a maintained Frequently Asked Questions page for scanned
  documents, tables, passwords, malformed files, coordinates, Python migration, and
  WebAssembly readiness.
- **API:** Python character dictionaries now expose the six-value PDF transformation
  `matrix` used for each glyph.
- **API:** Added the coherent `Pdf::open_path`, `Pdf::open_bytes`, and
  `Pdf::open_reader` Rust input family, with matching password methods and
  documented ownership and error behavior. Existing open methods remain
  source-compatible aliases during the alpha line.
- **API:** Added the borrowed `Pdf::pages` collection view. `Pages::get`
  selects one zero-based page directly, while exact-sized, double-ended
  iteration extracts owned pages on demand without cloning the document.
  Existing `Pdf::page` and `Pdf::pages_iter` methods remain source-compatible.
- **API:** Added `pdfplumber::models` as the curated stable `0.3.x` data-model
  boundary for characters, words, geometry, tables, metadata, warnings, and
  extraction options. Its contract defines units, coordinate origins,
  deterministic ordering, and optional-field meaning; existing root imports
  remain source-compatible, while serialized schemas use the separate policy
  below.
- **API:** `Table::cells` is now deterministically row-major (top-to-bottom,
  then left-to-right), independent of detection or caller input order.
- **API:** Defined the `serde-json-v1` compatibility policy for every curated
  `pdfplumber::models` type. Frozen producer and legacy-consumer fixtures guard
  field names, JSON value shapes, and enum encodings across `0.3.x`;
  `ExtractOptions`, `TextOptions`, and `WordOptions` now implement the optional
  Serde traits promised by that boundary.
- **API:** Added opaque, machine-readable Rust errors through `PdfErrorKind`,
  `PdfErrorContext`, `PdfObjectId`, and `PdfResourceLimit`. The facade preserves
  underlying causes through `std::error::Error::source`, attaches available
  page and indirect-object context, and keeps source messages and document
  content out of default `Display` and `Debug` output.
- **API:** Documented and tested Rust thread-safety and concurrency guarantees.
  `Pdf`, borrowed page views, `Page`, and `CroppedPage` are `Send + Sync`;
  document resource budgets are shared across attempts; parallel results retain
  page-index order; and the Python GIL/cache boundary is stated separately.
- **Platform:** Versioned readiness and generated support pages now distinguish the
  alpha Rust, Python, and CLI surfaces from the experimental WebAssembly surface.

### Changed

- **Platform:** Replaced the fixed Rust 1.85 Minimum Supported Rust Version
  contract with a rolling stable Rust policy. Package manifests no longer
  publish `rust-version`, and required Continuous Integration follows the
  current stable channel. Production code has also been migrated to the lints
  enforced by Rust 1.98 Clippy.
- **Dependencies:** Updated the PDF parser from two lopdf 0.34/0.39 copies to
  lopdf 0.44.0, removed the object-dense compatibility conversion path, and
  moved the WebAssembly entropy bridge to getrandom 0.4.
- **API:** Replaced the public `PdfError` string-payload variants with an opaque
  alpha error type. Rust callers should inspect `PdfError::kind`,
  `PdfError::context`, and `PdfError::resource_limit`; source-chain diagnostics
  are explicit and may contain sensitive parser or operating-system details.
- **API:** `Pdf` is now the canonical high-level entry point from the root
  `pdfplumber` crate. Parser backends and content-event types are no longer
  re-exported at that root; advanced parser consumers can depend on the separate
  `pdfplumber-parse` crate explicitly.
- **Platform:** Search and package-registry descriptions now share the same
  evidence-driven Rust extraction position, label each surface's maturity, and keep
  the Python migration claim explicitly alpha and incomplete.
- **Performance:** Public guidance no longer repeats unverified cross-project speed or
  memory ranges. No cross-project performance advantage is claimed until the
  benchmark and artifact gates in the detailed roadmap are complete.
- **Migration:** Python installation guidance now calls out that `pdfplumber-rs` and
  Python `pdfplumber` provide the same `pdfplumber` import. Use separate environments
  when comparing them, and reinstall in a fresh environment after an accidental
  co-install instead of treating package-manager order as a supported configuration.

### Fixed

- **Tables:** Object-dense, unencrypted PDFs no longer stall in parsing before
  table discovery; PDFium-generated documents with tens of thousands of indirect
  objects now reach the existing table extractor.
- **Compatibility:** Python character dictionaries now match the pinned
  `pdfplumber` v0.11.10 behavior for font names, reported size and advance, upright
  state, and transformation matrices. Mirrored-text word grouping and stable source
  ordering for tied word clusters were also restored.

## [0.3.0] - 2026-08-22

### Added

- **API:** Added the Python `pdfplumber` facade with `open`, lazy `PDF.pages`, page
  selection and geometry, crop relationships, cache flushing, object dictionaries,
  annotations, hyperlinks, structure trees, and JSON/CSV serialization. The Python
  extension remains alpha and is not yet a complete drop-in replacement.
- **Platform:** Versioned the Rust crate, Python distribution, CLI crate, and
  WebAssembly source at `0.3.0` under Apache-2.0. Rust, Python, and CLI are alpha;
  WebAssembly is experimental, and its npm package was not published with this
  release and therefore remained at `0.2.0`.

### Changed

- **Performance:** Changed word-to-line clustering to use y-coordinate buckets so it
  avoids repeatedly comparing words from unrelated rows. No cross-project performance
  result is claimed by this release.
- **Migration:** Breaking: `UnicodeNorm::default()` changed from no normalization to
  NFC. Select `UnicodeNorm::None` explicitly when byte-for-byte preservation of
  decomposed text is required. The Rust `MarkdownOptions`, `MarkdownRenderer`, and
  `Page::to_markdown` APIs, plus CLI Markdown output, were removed; render Markdown in
  a downstream integration instead.

### Fixed

- **Compatibility:** Expanded text, word, table, page-geometry, font, CJK, RTL,
  rotation, tagged-PDF, and malformed-content handling against Python
  `pdfplumber`. The Python compatibility target is pinned to v0.11.10 and remains
  evidence-scoped rather than a blanket parity claim.

[Unreleased]: https://github.com/developer0hye/pdfplumber-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/developer0hye/pdfplumber-rs/compare/v0.2.0...v0.3.0
