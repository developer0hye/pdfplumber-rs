# Rust public data-model design references

Pinned/retrieved: 2026-08-27. Used for `DX-005` only; these notes record design
inputs rather than claiming API equivalence with another project.

## Cargo SemVer Compatibility

Source: [Cargo SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)

- Cargo treats `0.3.x` releases as compatible with one another and `0.4.0` as
  the next incompatible line.
- Adding a public field to a struct whose fields are all public can break
  construction, and changing a field type is a major change.
- Adding an enum variant to an exhaustive enum can break downstream matches.
- These constraints are why the curated `pdfplumber::models` set explicitly
  commits to field names/types/meaning and exhaustive variants for `0.3.x`.

## Rust Reference: `non_exhaustive`

Source: [The Rust Reference: `non_exhaustive`](https://doc.rust-lang.org/reference/attributes/type_system.html#the-non_exhaustive-attribute)

- `#[non_exhaustive]` can reserve room for fields or variants, but applying it
  to existing externally constructed structs would itself restrict downstream
  construction and pattern matching.
- The current public models therefore remain exhaustive and use reviewed
  version transitions rather than retroactively changing their construction
  rules.

## pdfplumber v0.11.10

Source: [pdfplumber v0.11.10 README](https://github.com/jsvine/pdfplumber/blob/v0.11.10/README.md)

- The pinned upstream describes page-object coordinates through `x0`, `x1`,
  `top`, `bottom`, and `doctop`, and documents word extraction's optional
  content-flow ordering.
- It exposes table rows/cells and distinguishes page-relative from
  document-relative positions. Those semantics inform the Rust contract's
  coordinate and ordering vocabulary.
- The Rust types are not a serialized clone of Python dictionaries. Schema
  compatibility is intentionally deferred to DX-006.
