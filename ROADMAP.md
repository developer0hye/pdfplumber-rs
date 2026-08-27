# Roadmap

This roadmap is the public, outcome-level sequence for `pdfplumber-rs`. Horizons describe priorities, not release-date promises. Work moves forward only when its tests and evidence gates pass.

For capabilities available in release `0.3.0`, use the versioned [“What is ready today?” snapshot](docs/readiness/v0.3.0.md). Exact maturity, tested platforms, versions, features, and limitations are in the [generated support matrix](docs/support.md).

The detailed [PRD](PRD.md) remains the authoritative task registry and evidence ledger. This page deliberately selects only the work that best explains the product direction; it is not a copy of the full compatibility backlog. Completing a linked task does not automatically change a surface's maturity.

## Now

The current focus is making the `0.3.x` alpha release easy to evaluate and hard to misunderstand.

### Hold the Minimum Supported Rust Version

Lock the Minimum Supported Rust Version so dependency updates cannot silently
raise it without a reviewed task and changelog entry.

Detailed task: [`DX-013`](PRD.md#823-p0--rust-developer-experience-and-api-stability).

### Turn compatibility results into a public scorecard

Publish machine-readable and workflow-oriented views of the compatibility harness. Results will distinguish exact matches, approved deltas, unsupported behavior, reference failures, candidate failures, and cases that have not been tested.

Detailed tasks: [`SCORE-010`](PRD.md#825-p1--public-benchmarks-and-compatibility-scorecards), [`SCORE-011`](PRD.md#825-p1--public-benchmarks-and-compatibility-scorecards), [`SCORE-012`](PRD.md#825-p1--public-benchmarks-and-compatibility-scorecards).

## Next

After the trust reset, Rust and Python advance toward beta independently. Neither surface inherits the other's maturity.

### Make feature combinations predictable

Audit default and optional features, test representative combinations, and
ensure integrations do not silently change extraction behavior.

Detailed task: [`DX-014`](PRD.md#823-p0--rust-developer-experience-and-api-stability).

### Define a credible Python migration beta

Provide a migration guide, prove installed-wheel behavior rather than source-tree behavior, and test the declared operating-system, architecture, and Python-version matrix.

Detailed tasks: [`DOC-003`](PRD.md#821-p2--documentation-migration-and-ecosystem-quality), [`PYAPI-017`](PRD.md#82-p0--python-packaging-and-import-architecture), [`CI-007`](PRD.md#819-p1--continuous-integration-and-release-engineering).

## Later

Later work expands distribution and ecosystem reach only after the underlying surface contracts are ready.

### Ship verifiable automation artifacts

Provide prebuilt Command-Line Interface binaries, publish checksums and provenance, and test installation from public registries after publication.

Detailed tasks: [`DIST-003`](PRD.md#824-p1--distribution-and-installation), [`DIST-005`](PRD.md#824-p1--distribution-and-installation), [`DIST-007`](PRD.md#824-p1--distribution-and-installation).

### Prove the browser workflow

Build, type-check, install, and execute the WebAssembly package in Node and a maintained browser runner, then maintain a local-file example that does not upload the user's PDF.

Detailed tasks: [`DIST-013`](PRD.md#824-p1--distribution-and-installation), [`ECOSYS-006`](PRD.md#826-p1--ecosystem-integration-and-example-quality).

### Earn the `1.0` claim

Publish a security and private-reporting process, then complete documented production pilots with independent users across multiple workload classes before calling the project stable.

Detailed tasks: [`GOV-003`](PRD.md#827-p1--community-governance-and-external-validation), [`GOV-016`](PRD.md#827-p1--community-governance-and-external-validation).
