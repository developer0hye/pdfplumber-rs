# Rust SemVer release references

Rechecked: 2026-08-27.

## cargo-semver-checks

Sources: [official project](https://github.com/obi1kenobi/cargo-semver-checks),
[official action](https://github.com/obi1kenobi/cargo-semver-checks-action).

- The action is designed for the pre-publish gate and compares with the latest
  normal, non-yanked crates.io release by default.
- `release-type: patch` is stricter than a version-derived check and therefore
  exposes breaks that a valid minor or major increment would approve.
- The default feature heuristic includes ordinary features while excluding
  names conventionally marked unstable, nightly, benchmark-only, or internal.
- The tool explicitly does not detect every Rust SemVer violation, including
  some type, generic, lifetime, and partial-feature changes.

## Cargo SemVer compatibility

Source: [Cargo SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html).

- Removing or narrowing public items is generally a breaking change.
- Compatibility applies to library APIs; a binary-only command interface is
  not represented by rustdoc API data.

## Application to DX-012

All four published Rust packages must version together. The three library
packages run a strict patch comparison first. A break proceeds only after the
normal version-aware comparison accepts the candidate increment and the
versioned changelog supplies actionable `**Migration:** Breaking:` guidance.
