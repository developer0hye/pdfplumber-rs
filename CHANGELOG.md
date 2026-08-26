# Changelog

All notable user-visible changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Release notes are derived from this canonical record rather than from an uncurated
commit list.

## [Unreleased]

### Added

- **Documentation:** The primary Rust quick start is a complete, fallible program
  of no more than fifteen lines that opens a PDF, propagates file and page errors,
  prints extracted text, and is executed against a fixture in Continuous Integration.
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
  remain source-compatible and serialized schemas remain deferred to DX-006.
- **API:** `Table::cells` is now deterministically row-major (top-to-bottom,
  then left-to-right), independent of detection or caller input order.
- **Platform:** Versioned readiness and generated support pages now distinguish the
  alpha Rust, Python, and CLI surfaces from the experimental WebAssembly surface.

### Changed

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
