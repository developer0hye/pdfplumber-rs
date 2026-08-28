# Benchmark resource and artifact metrics 0.3.0

Suite `pdfplumber-rs-metrics-v0.3.0` extends stage suite `pdfplumber-rs-stages-v0.3.0` with resource, binary-size, WebAssembly bundle-size, and startup contracts.

## Measurement passes

Wall time remains an un-instrumented in-adapter component pass. CPU and allocation observations run in a separate instrumented invocation and are retained only when canonical output still exactly matches the untimed preflight result.

CPU time is process CPU consumed inside the stage boundary. Peak resident memory is the adapter process-lifetime high-water mark, so it includes interpreter/runtime startup and the declared setup operations and is not described as stage-local memory.

The resource adapters currently support Linux and macOS hosts. Other hosts fail closed instead of guessing peak-resident-memory units or APIs.

| Runtime | Method | Scope | Reported fields |
|---|---|---|---|
| `python` | `python-tracemalloc` | `in-adapter-stage-only` | `retained_allocation_count`, `retained_bytes`, `peak_traced_bytes` |
| `rust` | `rust-counting-global-allocator` | `in-adapter-stage-only` | `gross_allocation_count`, `gross_allocated_bytes` |

Python and Rust allocation fields are not equivalent: `tracemalloc` sees Python-traced retained blocks and peak traced bytes, while the Rust global allocator counts gross allocations and requested bytes. The report keeps the method and field names attached to every sample.

## Candidate artifact costs

| Artifact | Kind | Measured paths |
|---|---|---|
| `native-cli` | `native-executable` | `target/release/pdfplumber` |
| `wasm-node-package` | `wasm-package` | `crates/pdfplumber-wasm/pkg-benchmark/pdfplumber_wasm_bg.wasm`, `crates/pdfplumber-wasm/pkg-benchmark/pdfplumber_wasm.js` |

The native executable and WebAssembly runtime files are candidate-owned outputs. The combined Rust competitor adapter is deliberately excluded because its size cannot be attributed to one implementation.

Each WebAssembly startup sample launches a fresh Node.js process, then an in-process monotonic clock covers synchronous module load and WebAssembly instantiation. Node.js process launch is outside the clock.

```console
python3 scripts/run_benchmark_metrics.py --check
python3 scripts/run_benchmark_metrics.py --build
python3 scripts/run_benchmark_metrics.py --run --output /tmp/pdfplumber-rs-metrics.json
```

SCORE-005 component results are not published independently. SCORE-006 and SCORE-007 add scenario separation, complete environment capture, five raw repetitions, and statistical summaries. SCORE-008 publishes only the complete exact-tag result bundle; the result-removal policy remains SCORE-009.
