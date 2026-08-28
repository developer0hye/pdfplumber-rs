# Versioned Benchmark Results v0.3.0

The dedicated `benchmark-results-v0.3.0` evidence release is the only publication path for `pdfplumber-rs-provenance-v0.3.0`. Its tag target must be the exact clean source revision recorded inside the result.

## Release assets

- [Complete machine-readable result](https://github.com/developer0hye/pdfplumber-rs/releases/download/benchmark-results-v0.3.0/pdfplumber-rs-benchmark-results-v0.3.0.json) — semantic records, every preflight decision, all raw repetitions, statistical summaries, host/tool/build/lock/fixture provenance, and exact commands.
- [Concise human report](https://github.com/developer0hye/pdfplumber-rs/releases/download/benchmark-results-v0.3.0/pdfplumber-rs-benchmark-results-v0.3.0.md) — coverage, explicit rejections, recorded environment, and descriptive wall-time summaries derived from the raw result.
- [SHA-256 checksums](https://github.com/developer0hye/pdfplumber-rs/releases/download/benchmark-results-v0.3.0/pdfplumber-rs-benchmark-results-v0.3.0.sha256) — digest bindings for both result assets.

The `macos-14` tag workflow rebuilds the pinned Python reference, release-mode candidate wheel, and locked release competitor adapter before it runs five round-robin repetitions. It publishes only when the exact semantic gate passes for a comparison; rejected comparisons remain in the raw and human assets without timings.

These artifacts are one host observation, not a confidence interval, regression threshold, product ranking, or broad performance claim. Archived competitor revisions are historical comparison points and do not imply anything about current maintenance activity.

## Reproduction boundary

Use the source revision, tool versions, dependency-lock hashes, fixture hashes, build flags, and argument arrays inside the raw asset. The release tag is separate from package tags such as `v0.3.0`, so publishing benchmark evidence cannot trigger package registries.
