# Benchmark regression alerts v0.3.0

Policy: `pdfplumber-rs-regressions-v0.3.0`. Baseline: `benchmark-results-v0.3.0`. Runner: `macos-14`.

## Measurement protocol

The workflow builds the retained baseline and current revision on one runner, then executes `baseline → candidate → candidate → baseline`. Each run retains 5 round-robin samples, giving 10 samples per revision and timing group. This ABBA order balances first-versus-last drift; it does not claim to eliminate hosted-runner noise.

Targets are `pdfplumber-rs`, `pdfplumber-rs-python`. Pinned controls are `pdfplumber-python`, `pdf-oxide`, `pdfsink-rs`. The median current-to-baseline control ratio across at least 50 common groups normalizes shared host movement.

## Alert rule

A target group alerts only when all three conditions hold after control normalization: median slowdown is at least 20%; slowdown is at least 3 times the larger relative median absolute deviation; and the current 25% quantile is above the baseline 75% quantile. Otherwise the group is recorded as within policy or noise-overlap, not promoted into a regression claim.

## Semantic gate

Untimed records, output-equivalence decisions, eligible target group identities, fixture bindings, and semantic output digests are compared before wall time. Any target semantic or eligibility drift is `semantic-failure`; thresholds are never consulted, and making output checks weaker cannot turn it into a performance pass.

A missing run, changed host/toolchain identity, malformed summary, insufficient controls, or non-finite normalization is `inconclusive`. `regression`, `semantic-failure`, and `inconclusive` all fail the workflow after its machine-readable decision artifact is uploaded. The alert is a guard for investigation, not a ranking, confidence interval, or broad product performance claim.
