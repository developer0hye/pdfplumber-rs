# Rust deprecation references

Rechecked: 2026-08-28.

## Rust Reference

Source: [Diagnostic attributes](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-deprecated-attribute).

- `#[deprecated]` makes rustc warn on use and rustdoc display the deprecation.
- `since` records the version where deprecation began; tools may validate it.
- `note` should explain the deprecation and preferred alternative.
- The attribute applies to items, trait items, variants, fields, external items,
  and macros, but not trait implementation items.

## Cargo SemVer Compatibility

Source: [Cargo SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html).

- Removing or moving a public item is a major change; deprecating it first is a
  mitigation, not permission to remove it in a compatible release.
- Adding a deprecation warning is normally compatible, though direct users that
  deny warnings can still be affected.
- Removing a public Cargo feature is breaking; retaining a documented no-op
  feature can provide a migration period.
- Before 1.0, Cargo treats a change to the left-most non-zero component as the
  compatibility boundary.

## Application to DX-015

Facade deprecations carry both fields, stay available through two subsequent
published minor releases, and are removed only through the project's SemVer and
migration-note gates. Only urgent safety or demonstrated unsoundness can shorten
that window.
