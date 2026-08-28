# MLPerf Inference Rules

- **URL:** https://github.com/mlcommons/inference_policies
- **Observed source:** [`d3eba2f21026d868ad65cdcad2bb81e4a17ce3d3`](https://github.com/mlcommons/inference_policies/blob/d3eba2f21026d868ad65cdcad2bb81e4a17ce3d3/inference_rules.adoc) on 2026-08-28
- **License:** Apache-2.0
- **Repository status at observation:** Public, active, and not archived

## Relevant Benchmark Pattern

- The reference implementation is the canonical benchmark definition, and valid
  implementations must be equivalent to it.
- Quality or accuracy and performance are separate phases; a run must meet its
  quality requirement before its performance result is valid.
- The performance data set is a subset of the accuracy data set, and committed
  verification scripts bind data to checksums.
- Preprocessing and postprocessing semantics are explicit so a faster but
  materially different task is not treated as the same benchmark.
- Replicability is mandatory. A materially failed post-publication audit can
  move or remove the invalid result, while the audit record is retained.

## Relevance to pdfplumber-rs

`SCORE-002` applies the same separation to PDF extraction: adapters first emit
untimed canonical outputs for the same digest-bound fixture and semantic request.
Only exact equivalent output is eligible for later timing. MLPerf thresholds and
machine-learning-specific scenarios are not copied into this project.
`SCORE-009` also adopts the audit-lifecycle shape: confirmed semantic drift
withdraws the result-bearing assets while retaining the immutable source tag,
Release tombstone, and machine-readable audit decision. Infrastructure failures
remain inconclusive rather than causing destructive removal.
