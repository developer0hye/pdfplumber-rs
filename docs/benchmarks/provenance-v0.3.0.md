# Benchmark Run Provenance v0.3.0

Suite `pdfplumber-rs-provenance-v0.3.0` binds `pdfplumber-rs-scenarios-v0.3.0` to complete run metadata, 5 raw repetitions per eligible implementation/case, and deterministic statistical summaries.

## Run identity

A run is accepted only from a clean Git worktree. It records the exact repository revision, UTC capture time, operating-system name and release, architecture, Central Processing Unit model and logical count, physical memory, and the complete fixture identifier/SHA-256 set. The built Rust adapter and installed candidate native extension are also size- and digest-bound.

Compiler and interpreter evidence includes the harness, reference, and candidate Python versions plus verbose Rust compiler, Cargo, and Maturin versions. Commands are retained as argument arrays from repository root rather than shell strings.

## Build and dependency inputs

| Artifact | Command | Material flags |
|---|---|---|
| `candidate-python-wheel` | `python3.13 scripts/setup_candidate_venv.py --python python3.13` | `maturin=1.14.1`, `profile=release`, `pip=--no-deps` |
| `rust-benchmark-adapter` | `cargo build --manifest-path benchmarks/adapters/rust/Cargo.toml --target-dir benchmarks/adapters/rust/target --release --locked` | `--release`, `--locked`, `features=parallel` |

| Dependency role | Lock | Run record |
|---|---|---|
| `python-reference` | `compat/requirements-golden.txt` | SHA-256 recorded at run time |
| `rust-benchmark-adapter` | `benchmarks/adapters/rust/Cargo.lock` | SHA-256 recorded at run time |
| `rust-workspace` | `Cargo.lock` | SHA-256 recorded at run time |

The pinned Python reference environment is rebuilt from `compat/requirements-golden.txt` with hashes required. The candidate setup enforces Maturin 1.14.1, builds its local wheel in the release profile, and installs it with `--no-deps`; the Rust adapter uses its committed lock with release mode and the candidate's `parallel` feature.

## Repetitions and statistics

Each exact-output-eligible key runs 5 times in `round-robin-by-repetition` order. Every raw sample retains its repetition index, exact adapter argv, semantic-output digest, scenario state, and fixture digest. A summary is emitted only when repetitions 1 through 5 are all present and every non-time field remains identical.

Summaries report sample size, minimum, median, arithmetic mean, maximum, sample standard deviation, and relative standard deviation for monotonic wall time. The ordered raw-sample array is SHA-256-bound into each summary. These descriptive statistics estimate observed run noise; they are not a confidence interval, regression threshold, or winner declaration.

```console
python3 scripts/run_benchmark_provenance.py --check
python3 scripts/run_benchmark_provenance.py --build
python3 scripts/run_benchmark_provenance.py --run --output /tmp/pdfplumber-rs-provenance.json
```

SCORE-008 promotes a complete run only through the exact `benchmark-results-v0.3.0` tag target and retains the raw JSON, concise human report, and checksums as release assets. SCORE-009 re-audits the immutable tag and withdraws all three result assets if semantic reproduction or output equivalence fails while retaining the tag and audit tombstone. These descriptive observations do not create a broad product performance claim.
