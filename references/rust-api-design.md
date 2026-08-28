# Rust API-design sources

Reviewed on 2026-08-28 for the `pdfplumber` stable-facade design criteria.
These sources guide project judgment; they are not substituted for review of
the repository's actual ownership, runtime behavior, or compatibility policy.

## Rust API Guidelines

- Source: <https://rust-lang.github.io/api-guidelines/checklist.html>
- Source: <https://rust-lang.github.io/api-guidelines/flexibility.html>
- Applied here: iterator naming; meaningful error types; caller control over
  copying and placement; exposing useful intermediate results; generic-input
  tradeoffs; dyn-compatible traits when trait objects are intended; sealed
  traits and private fields for future evolution.
- Qualification: the project treats the document as review guidance, not an
  automatic pass/fail mandate. A PDF extraction API may intentionally allocate
  an owned model or expose a stable data field when that observable contract is
  more useful than maximal abstraction.

## Standard-library collections

- Source: <https://doc.rust-lang.org/stable/std/collections/struct.HashMap.html>
- Applied here: `HashMap` uses a randomized default state and its iterators
  visit entries in arbitrary order. It may back keyed lookup, but its iteration
  cannot define deterministic extracted or serialized sequence order.

## Standard-library errors

- Source: <https://doc.rust-lang.org/std/error/trait.Error.html#method.source>
- Applied here: `Error::source` crosses abstraction boundaries while preserving
  a lower-level cause. A wrapped cause should be exposed through the source
  chain or rendered by the outer `Display`, not duplicated through both.

## Rust Reference: non-exhaustive types

- Source: <https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute>
- Applied here: `#[non_exhaustive]` preserves room for fields or variants by
  restricting downstream construction and exhaustive matching. That caller
  restriction is itself part of the API contract.

## Rust RFC 1105: API evolution

- Source: <https://rust-lang.github.io/rfcs/1105-api-evolution.html>
- Applied here: public fields, enum variants, function arity, trait items,
  bounds, and method additions carry different downstream break risks;
  behavioral contracts require human review beyond compilation compatibility.
- Qualification: the RFC is an early baseline rather than a complete modern
  Cargo SemVer specification. The repository's pinned `cargo-semver-checks`
  workflow and migration-note policy remain the executable release gate.
