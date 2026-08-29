# Python macOS wheel compatibility references

Observed: 2026-08-29.

## Primary sources

- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  lists `macos-15-intel` as Intel and `macos-15` as arm64 hardware.
- [Rust Apple Darwin targets](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)
  defines macOS 10.12 as the x86-64 floor and macOS 11.0 as the arm64 floor,
  and documents `MACOSX_DEPLOYMENT_TARGET` as the per-binary override.
- [Apple's SDK compatibility guide](https://developer.apple.com/library/archive/documentation/DeveloperTools/Conceptual/cross_development/Configuring/configuring.html)
  defines the deployment target as the earliest operating-system version on
  which software can run.
- [Maturin Action](https://github.com/PyO3/maturin-action) documents native
  host builds for macOS and supports pinning the Maturin release.

## Applied pattern

The release matrix uses one native runner per architecture, exports the Rust
deployment floor explicitly, and pins Maturin 1.14.1. The checked script binds
the exact wheel tag to `lipo` architecture and `otool` deployment metadata,
then retains a real isolated-wheel extraction probe. Native execution on
macOS 15 and a matching Mach-O floor are separate facts; neither is described
as a runtime test on macOS 10.12 or 11.0.
