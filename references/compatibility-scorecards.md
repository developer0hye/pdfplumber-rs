# wpt.fyi and EARL compatibility-result patterns

## Sources

- [wpt.fyi API](https://github.com/web-platform-tests/wpt.fyi/blob/main/api/README.md)
- [EARL 1.0 Schema](https://www.w3.org/TR/EARL10-Schema/)

## Relevant patterns

- wpt.fyi keeps execution metadata (`TestRun`) separate from individually
  addressable test results.
- A run identifies the tested revision, product, version, operating system, and
  raw result location; result records retain stable test identities.
- EARL models the tested subject, assertion, test criterion, mode, and outcome
  separately so results remain attributable and reusable.
- EARL distinguishes an unexecuted test (`untested`) from a failed test and
  from a result that cannot be determined.

## Applied here

- The compatibility scorecard has explicit run records for platform and
  artifact provenance, while observations retain API, option, fixture class,
  fixture, and page identities.
- Required platform/artifact cells without a parity run are published as
  `not_tested`; they are never inferred from package-build or smoke-test jobs.
- The project keeps its domain-specific exact/delta/failure vocabulary instead
  of adopting EARL's accessibility-focused serialization.
