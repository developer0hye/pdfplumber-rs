# Rust time to first value

The current-source Rust quick start reached useful extracted text in **34.927
seconds**, below the five-minute activation gate. This is a dated, automated
clean-state observation rather than a universal build-time promise. The exact
machine-readable result is
[`rust-ttfv-workspace-2026-08-30.json`](measurements/rust-ttfv-workspace-2026-08-30.json).

## Scope

The measured outcome is the first README example printing known text from a
searchable PDF. A preinstalled rolling stable Rust toolchain with Cargo is a
prerequisite. Installing Rust itself is outside the clock because the product
gate starts from a supported development environment, not a machine without a
toolchain.

The harness copies the rendered README installation block and primary Rust
program without maintaining a second example. It substitutes the current
workspace candidate through Cargo's local patch mechanism; that substitution is
measurement machinery, not a documented user setup step. Transitive dependencies
still resolve through crates.io with an empty download cache.

## Clock boundary

The clock starts before `cargo new`. It covers these ordered phases:

1. Create a new binary Cargo project with no repository, lock file, or target
   directory.
2. Declare the rendered `pdfplumber = "0.4.0"` dependency.
3. Copy the rendered program and the searchable `document.pdf` input.
4. Run one `cargo run --quiet`, which resolves and downloads dependencies,
   builds the candidate and consumer, then executes the program.
5. Interpret the first result by requiring the known extracted-text marker.

The clock stops after that interpretation succeeds. There is no separate
`cargo fetch`, `cargo build`, feature selection, parser/core dependency, async
runtime, serialization setup, or parallel-runtime setup on the activation path.

The script automates code copy and interpretation, so this result is a
reproducible machine lower bound. It does not measure human reading, typing, or
PDF-selection time. Network and registry latency vary, as do CPU, storage, and
future compiler or dependency costs; the five-minute comparison applies only to
the recorded environment and inputs.

## Isolation

- The consumer is a new temporary project.
- `CARGO_HOME` is an empty temporary directory, isolating Cargo's registry and
  source caches.
- Ambient target-directory, compiler-wrapper, offline, and Rust flag overrides
  are removed.
- Build output uses the new project's default, initially absent `target`
  directory.
- The generated `Cargo.lock`, rendered snippets, fixture, and current facade,
  parser, and core source tree are bound by SHA-256 in the result.

Cargo's definitions of project creation, caches, dependency requirements, and
`cargo run` are mapped in the [source record](../references/rust-ttfv.md).

## Observed result

| Phase | Seconds |
|---|---:|
| Project creation | 0.082 |
| Dependency declaration | 0.001 |
| Code and fixture copy | 0.001 |
| Resolve, download, build, and execute | 34.844 |
| Interpret useful output | 0.000 |
| **Total clock** | **34.927** |
| Product gate | 300.000 |

Environment: macOS Darwin 25.6.0 on arm64, `rustc 1.98.0`, and `cargo
1.98.0`. The result records the complete version strings rather than deriving a
cross-machine expectation from this single observation.

## Artifact boundary

This result proves the current workspace candidate, not installation of the
published registry artifact. A cold trial of published release 0.3.0 on
2026-08-28 reached compilation but failed because `Pdf::open_path` is absent
from that release; the current README uses the newer canonical constructor. The
failed trial is explicit in the JSON and is not counted as a passing TTFV.

Exact clean-commit package-boundary proof is now enforced under `DIST-001`.
Post-publication installation and execution remain open under `DIST-007`. A
passing current-source result must not be used to claim that the current public
registry release runs the same example.

## Reproduce

From the repository root on a machine with current stable Rust and network
access:

```bash
python3 scripts/measure_rust_ttfv.py \
  --measure \
  --expected-version 0.4.0 \
  --output docs/measurements/rust-ttfv-workspace-2026-08-30.json
python3 scripts/measure_rust_ttfv.py \
  --check docs/measurements/rust-ttfv-workspace-2026-08-30.json
```

Continuous Integration validates the checked-in result against the current
rendered snippets, fixture, source-tree digest, schema, threshold, and explicit
registry limitation. The existing rendered-quick-start job separately compiles
and executes the current candidate on every change.

Remeasure for every Rust release, primary installation or quick-start change,
rolling-stable toolchain transition, or material dependency/build change. A new
result must preserve prior observations rather than silently rewriting them.
