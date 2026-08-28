# Rust toolchain policy references

Rechecked: 2026-08-28.

## Rust stable channel

Source: [Rust channel layout](https://forge.rust-lang.org/infra/channel-layout.html#channel-rust-stable)

- The stable channel advances when the Rust project publishes a stable release.
- On 2026-08-28, the current release was
  [Rust 1.98.0](https://blog.rust-lang.org/2026/08/20/Rust-1.98.0/).
- `rustup update stable` updates a local installation to that moving channel.

## Cargo package metadata

Source: [Cargo `rust-version`](https://doc.rust-lang.org/cargo/reference/rust-version.html)

- `package.rust-version` declares the oldest supported compiler and affects
  dependency resolution.
- Omitting it makes no Minimum Supported Rust Version promise.

## Application to DX-013

`pdfplumber-rs` follows current stable Rust rather than a fixed compiler floor.
Required CI installs `stable`, and published manifests intentionally omit
`rust-version`. Dependency upgrades may therefore raise the effective compiler
requirement without a separate MSRV-preservation task; current stable CI remains
the compatibility gate.

DIST-015 additionally pins the current release and a complete official-image
digest for a reproducible contributor snapshot. That snapshot is updated when
rolling stable advances; it does not create a fixed compiler support floor.
