# Release versioning

The root `Cargo.toml` entry at `[workspace.package].version` is the repository's
single release source of truth. Every workspace crate declares
`version.workspace = true`; changing a member version independently is an
invalid release state.

The repository keeps a few publishable selectors in formats that cannot inherit
Cargo workspace fields. The package metadata gate requires each one to equal the
root version:

| Consumer | Binding |
|---|---|
| Rust crates | Cargo resolves every member's inherited package version. Versioned internal dependencies must use the same requirement before publication. |
| Python | Maturin reads the inherited Cargo version because `pyproject.toml` keeps `version` dynamic. Installed tests require the `pdfplumber-rs` distribution metadata and native `_native.__version__` to match. |
| npm | `wasm-pack` reads the inherited `CARGO_PKG_VERSION`; the packaged `package.json` is checked before publication. |
| Native modules | Rust exposes `CARGO_PKG_VERSION` as Python and WebAssembly version data. |
| Documentation | `support-matrix.toml`, `readiness.toml`, `docs/releases/vX.Y.Z.md`, and `docs/readiness/vX.Y.Z.md` are validated against the root selector. |
| GitHub | The release tag must be exactly `vX.Y.Z` for the root version before publication starts. |

## Version-bump protocol

1. Change only `[workspace.package].version` as the Cargo package authority.
2. Update versioned internal dependency requirements needed by crates.io, plus
   the support, readiness, license-policy, changelog, and documentation selectors
   for the same version. Do not rewrite historical benchmark or scorecard data.
3. Regenerate the support matrix, readiness page, and release notes for the new
   selector.
4. Run `python3 scripts/check_package_metadata.py --source`,
   `python3 scripts/check_package_licenses.py --source`, and the complete test
   suite. Build and install the Python distributions and npm package so their
   emitted versions cross the artifact gates.
5. Open a release pull request. The Rust release detector treats the root-only
   version transition as a coordinated change for every publishable crate.
6. Create the release tag only after the exact merge commit and all required
   checks are verified. Never move a published tag or reuse a registry version.

The local implementation and the authoritative Cargo inheritance behavior are
summarized in the [release-versioning reference note](../references/release-versioning.md).
