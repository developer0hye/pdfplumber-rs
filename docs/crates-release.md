# Verified crates.io release candidates

Package versions follow the repository's
[single release-version source](release-versioning.md).

Every publishable Rust workspace package crosses the same package boundary
before a tag can publish it:

1. Discover crates.io packages and their dependency order from `cargo metadata`.
2. Require a clean checkout at the exact release commit.
3. Run verified `cargo package` for each candidate.
4. Run `cargo publish --dry-run` for each candidate.
5. Inspect each `.crate` archive and require its Cargo Git provenance to name
   that exact clean commit.

The current ordered set is `pdfplumber-core`, `pdfplumber-parse`, `pdfplumber`,
and `pdfplumber-cli`. The checker discovers this set instead of relying on a
second hand-maintained package list; packages with `publish = false` are
excluded.

## Coordinated unpublished dependencies

Cargo removes path locations from a published manifest and resolves its versioned
dependencies from crates.io. Before a coordinated release is public, that would
make parser, facade, and Command-Line Interface verification compile against the
previous registry release. The candidate gate therefore supplies a command-line `[patch.crates-io]`
override for the exact local predecessor source candidates while Cargo
packages and verifies each current source candidate. It does not edit or preserve
a patch in any published manifest.

This is deliberately narrower than registry-backed verification. The later
ordinary `cargo publish` commands do not receive the patch: each dependent is
verified against predecessors that the release workflow has already published.
The separate post-publication install and execution gate remains open as
[`DIST-007`](../PRD.md#824-p1--distribution-and-installation).

## Registry resolution between dependent publishes

After each predecessor upload, the release workflow runs
`cargo info <crate>@<version>` against crates.io. A successful command proves
that Cargo can resolve the exact expected version before the workflow publishes
its dependent; seeing only a crate name or a different version is insufficient.
The core crate must resolve before parser publication, parser before facade
publication, and facade before Command-Line Interface publication.

Each probe is time-bounded and unsuccessful probes use exponential backoff from
five to at most thirty seconds. A five-minute deadline bounds the entire gate,
while every Cargo invocation also has its own thirty-second timeout. Probe
output is not copied into workflow logs because registry diagnostics may include
sensitive configuration. Exhausting the deadline stops publication and directs
maintainers to the [release recovery runbook](release-recovery.md); it never
skips ahead to a dependent package. This resolution gate does not install or
execute the published package, which remains the separate [`DIST-007`](../PRD.md#824-p1--distribution-and-installation)
boundary.

## Commands

Continuous Integration packages the exact checkout and leaves the four archives
for license, metadata, and [release integrity](release-integrity.md) inspection:

```bash
python scripts/check_crates_release.py \
  --expected-commit "$(git rev-parse HEAD)" --package-only
```

A tagged release runs the complete preflight before any crates.io upload:

```bash
python scripts/check_crates_release.py --release-tag v0.3.0
```

To reproduce the exact registry-resolution probe and bounded retry policy:

```bash
python scripts/wait_for_crate_resolution.py pdfplumber-core \
  --release-tag v0.3.0 --timeout-seconds 300
```

`cargo publish --dry-run` performs Cargo's publish checks but does not upload.
The checker also refuses a dirty tree, a SHA mismatch, a tag/version mismatch,
a missing package, a failed package build, or an archive whose
`.cargo_vcs_info.json` does not bind it to the selected commit. Registry and
network behavior can still change after preflight, so successful candidate
verification is not a claim that public installation has passed.

If a registry, credential, package, or published claim fails after upload starts,
follow the [release recovery runbook](release-recovery.md). It records each
registry independently and avoids re-running already published immutable
versions. Ordinary uploads receive a temporary crates.io credential through
the repository's [trusted publishing contract](trusted-publishing.md), never a
stored registry token.
