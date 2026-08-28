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

## Validity and withdrawal

Status: **retained**

The initial [confirmed reproduction and publication run](https://github.com/developer0hye/pdfplumber-rs/actions/runs/33194166728) completed the committed exact-tag harness and exact output-equivalence gate. The registry binds the raw, report, and checksum assets to SHA-256 values `07e2f5face4b35d88a68243214c76407d7c4ff716608785968130056bc04a0b0`, `7354f174ce506fd939d10775e85c53be5e1e2b9fe00a96f46b5f513972ba922c`, and `db56fde739c1a9556c48c0be2336ab4e869ea90ac103a76ded093ff119945bda`.

A scheduled read-only audit reruns the immutable tag and compares semantic records, preflight decisions, timed keys, fixture bindings, and semantic digests. Host identity and timing values may differ. A completed audit that changes any semantic result produces a machine-readable withdrawal decision; transient setup or network failures are inconclusive and cannot remove evidence.

Withdrawal removes the three result-bearing assets and replaces the Release body with a tombstone under the verified `developer0hye` identity. It never deletes the source tag or the audit decision.

## Regression alerts

The separate [SCORE-013 regression policy](regressions-v0.3.0.md) uses this retained tag as its immutable baseline. It checks exact semantic and timing-eligibility identities before applying its paired-run noise rule; alert decisions do not alter these Release assets.
