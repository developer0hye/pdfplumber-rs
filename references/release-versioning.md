# Release-version source references

Sources used by the repository's single-version contract.

- [Cargo workspaces: the `package` table](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table)
  defines inheritable workspace package keys, including `version`, and the
  member-side `{key}.workspace = true` form.
- [Cargo manifest package metadata](https://doc.rust-lang.org/cargo/reference/manifest.html#the-package-section)
  defines the package version consumed by Cargo packaging and exposed to builds.
- [Maturin project layout](https://www.maturin.rs/project_layout.html)
  documents Cargo-based project metadata and mixed Rust/Python layouts.
- [`wasm-pack build`](https://rustwasm.github.io/docs/wasm-pack/commands/build.html)
  documents the generated npm package boundary checked before publication.

Repository policy is stricter than inheritance alone: all six member versions
must inherit, publishable path-dependency requirements must equal the root
version, generated documentation selectors must match, and a release tag must
be exactly `v` plus that version.
