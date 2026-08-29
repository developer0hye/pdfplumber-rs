# Roadmap

This roadmap is the public, outcome-level sequence for `pdfplumber-rs`. Horizons describe priorities, not release-date promises. Work moves forward only when its tests and evidence gates pass.

For capabilities available in release `0.3.0`, use the versioned [“What is ready today?” snapshot](docs/readiness/v0.3.0.md). Exact maturity, tested platforms, versions, features, and limitations are in the [generated support matrix](docs/support.md).

The detailed [PRD](PRD.md) remains the authoritative task registry and evidence ledger. This page deliberately selects only the work that best explains the product direction; it is not a copy of the full compatibility backlog. Completing a linked task does not automatically change a surface's maturity.

## Now

The current focus is making the `0.3.x` alpha release easy to evaluate and hard to misunderstand.

### Turn compatibility results into a public scorecard

The versioned [machine-readable scorecard](docs/compatibility/scorecard-v0.3.0.json) exposes API, option, fixture-class, page, platform, and artifact results. Its generated [human workflow view](docs/compatibility/workflows-v0.3.0.md) keeps exact, approved-delta, unsupported, failure, and untested outcomes distinct across eleven common evaluation paths.

Completed foundations: `SCORE-010`, `SCORE-011`, and `SCORE-012`.

### Establish comparable benchmark inputs

Define a redistributable workload corpus and reject timing comparisons unless
the requested outputs and semantics are materially equivalent. Performance
evidence will follow the correctness gate, not substitute for it.

Completed foundations: `SCORE-001`, `SCORE-002`, `SCORE-003`, `SCORE-004`, `SCORE-005`, `SCORE-006`, `SCORE-007`, `SCORE-008`, `SCORE-009`, `SCORE-013`, and `SCORE-014`.

The versioned [benchmark result index](docs/benchmarks/results-v0.3.0.md) links the published exact-tag raw JSON, concise human report, and checksum assets.

The [benchmark regression policy](docs/benchmarks/regressions-v0.3.0.md) documents paired-run ordering, pinned host controls, noise qualification, and semantic-first failure behavior.

The [release-candidate history](docs/scorecards/release-candidate-history-v0.3.md) is the chronological view for repeated competitor and compatibility scorecards. It begins with the first verified candidate run; later candidates append instead of replacing earlier observations.

Next release-distribution task: [`DIST-004`](PRD.md#824-p1--distribution-and-installation).

## Next

After the trust reset, Rust and Python advance toward beta independently. Neither surface inherits the other's maturity.

### Define a credible Python migration beta

Provide a migration guide, prove installed-wheel behavior rather than source-tree behavior, and test the declared operating-system, architecture, and Python-version matrix.

Detailed tasks: [`DOC-003`](PRD.md#821-p2--documentation-migration-and-ecosystem-quality), [`PYAPI-017`](PRD.md#82-p0--python-packaging-and-import-architecture), [`CI-007`](PRD.md#819-p1--continuous-integration-and-release-engineering).

### Ship verifiable automation artifacts

Execute every prebuilt Command-Line Interface binary on its native target, publish checksums and provenance, and test installation from public registries after publication.

Detailed tasks: [`DIST-005`](PRD.md#824-p1--distribution-and-installation), [`DIST-007`](PRD.md#824-p1--distribution-and-installation).

## Later

Later work expands distribution and ecosystem reach only after the underlying surface contracts are ready.

### Prove the browser workflow

Build, type-check, install, and execute the WebAssembly package in Node and a maintained browser runner, then maintain a local-file example that does not upload the user's PDF.

Detailed tasks: [`DIST-013`](PRD.md#824-p1--distribution-and-installation), [`ECOSYS-006`](PRD.md#826-p1--ecosystem-integration-and-example-quality).

### Earn the `1.0` claim

Publish a security and private-reporting process, then complete documented production pilots with independent users across multiple workload classes before calling the project stable.

Detailed tasks: [`GOV-003`](PRD.md#827-p1--community-governance-and-external-validation), [`GOV-016`](PRD.md#827-p1--community-governance-and-external-validation).
