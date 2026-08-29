# Cargo packaging and dry-run sources

Official Cargo documentation used by the crates.io candidate gate.

- [`cargo package`](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
  creates a normalized `.crate`, records best-effort Git information, extracts
  it, and builds it from scratch. `--no-verify` skips that build, so the gate
  does not use the flag.
- [`cargo publish`](https://doc.rust-lang.org/cargo/commands/cargo-publish.html)
  packages, verifies, and uploads a crate. `--dry-run` performs the checks
  without uploading.
- [`cargo info`](https://doc.rust-lang.org/cargo/commands/cargo-info.html)
  accepts an exact `package@version` specification and a named registry. The
  release gate polls this Cargo boundary so a dependent is not published until
  its exact predecessor version resolves from crates.io.
- [Specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#multiple-locations)
  explains that a local path is used during workspace development but the
  versioned registry location is used after publication.
- [Overriding dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html)
  permits a `[patch]` override through Cargo's `--config` option. The gate uses
  that external override to verify coordinated candidates before their new
  versions exist in the index; the published manifests remain registry-based.

Candidate verification is followed by ordinary, unpatched publication in
dependency order. A later public-registry install test is a separate boundary.
