# Rust deprecation policy

This policy applies to the stable public `pdfplumber` facade described by the
[Rust API contract](rust-api.md). It does not promote `pdfplumber-core` or
`pdfplumber-parse` internals into that facade. Python compatibility, the
Command-Line Interface, and the `serde-json-v1` schema have separate contracts.
A `#[doc(hidden)]` compatibility alias is not deprecated merely because it is
hidden; removal requires the same explicit lifecycle as any other facade item.

## Declaring a deprecation

The replacement should be available and documented before the old item is
deprecated. The declaration uses complete compiler and rustdoc metadata:

```rust
#[deprecated(since = "X.Y.Z", note = "Use replacement")]
```

`since` is the first published project version carrying the warning. `note`
names the replacement path and the concrete migration action. Bare
`#[deprecated]` attributes and notes that only say an item is obsolete are not
accepted. If no safe replacement can exist, the note explains why and names the
smallest safe response.

The introducing release must update all of these places:

- rustdoc, including `since`, `note`, and a replacement link;
- the changelog under `### Deprecated`;
- generated or versioned release notes;
- tests that keep the old path working and the replacement working.

## Minimum window

A deprecated item remains callable for at least two subsequent, separately
published minor releases after the release that first deprecates it. Patch
releases do not count. Skipped minor version numbers do not count; the window is
measured by published releases, not arithmetic on a version string.

For the pre-1.0 line, an item deprecated in `0.3.x` remains available throughout
published `0.4.x` and `0.5.x` releases; its earliest ordinary removal is
`0.6.0`. Cargo treats each pre-1.0 minor line as a compatibility boundary, but
this project deliberately provides the longer migration window.

After 1.0, removal is allowed only in a SemVer major release. An item deprecated
in `1.2.x` remains available through published `1.3.x` and `1.4.x` releases and,
even after that window, its earliest ordinary removal is `2.0.0`.

## Removal gate

Removal needs a focused pull request that identifies the first deprecating
release and both subsequent published minor releases. It must run
`cargo-semver-checks` before removal and use a SemVer-incompatible release even
when the minimum window has elapsed. Rustdoc, the changelog, and release notes
must describe the removal and replacement together. The release changelog uses
the existing actionable form:

```markdown
- **Migration:** Breaking: Replace the removed path with its documented replacement.
```

Feature names and public re-exports follow the same gate because removing them
can break downstream builds. Deprecation is not permission to weaken tests or
silently change the old path while it remains available.

## Safety and unsoundness exception

Only an urgent safety issue or demonstrated unsoundness can shorten the window.
Maintenance burden, design preference, convenience, or cleanup are not an
exception. The pull request and release record must provide a public rationale
and migration path, identify the affected versions, and explain why waiting
would expose users to harm.

The exception makes the smallest possible break: preserve a safe compatibility
shim where possible, disable only the unsafe operation when a shim is
impossible, and still choose the strongest SemVer signal feasible. Private
security details may remain embargoed until coordinated disclosure, but the
published release must state that the exception was used.

The official Rust and Cargo behavior behind this policy is recorded in the
[source note](../references/rust-deprecation.md).
