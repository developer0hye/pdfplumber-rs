# Cargo feature references

Rechecked: 2026-08-28.

## Cargo Book

Source: [Features](https://doc.rust-lang.org/cargo/reference/features.html).

- Default features should make common use convenient; removing one from the
  default set can be a SemVer-incompatible change.
- Dependency feature unification takes the union of enabled features, so
  features should be additive and safe in any supported combination.
- Cargo recommends selecting a project-specific combination strategy because
  full coverage grows exponentially; common gates include no defaults,
  individual integrations, representative combinations, and all features.
- Resolver 2 and later make workspace package feature selection explicit and
  avoid several unwanted build/dev/target dependency unifications.
- Public features should be documented with their availability and effects.

## Application to DX-014

The public facade retains `std` as its default path capability and treats
`serde` and `parallel` as additive integrations. Continuous Integration runs
semantic fixture regressions under no defaults, defaults, each optional
integration without defaults, and the complete union. The parser's internal
`tracing` feature is tested separately rather than expanding the facade.
