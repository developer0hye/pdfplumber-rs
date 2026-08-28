# Rust time-to-first-value sources

Official Cargo documentation used to define the clean-project measurement.

## Project creation

- [cargo new](https://doc.rust-lang.org/cargo/commands/cargo-new.html) creates a
  new package with a manifest and sample binary source. The measurement invokes
  the binary form explicitly and disables version-control initialization.
- [Specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
  defines the crates.io version requirement used by the rendered installation
  block. The generated lock file records the exact resolved release.

## Cache isolation

- [Cargo home](https://doc.rust-lang.org/cargo/guide/cargo-home.html) identifies
  `CARGO_HOME` as Cargo's download and source cache. The measurement points it
  at a newly created empty temporary directory.
- [Cargo environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html)
  defines `CARGO_HOME`, `CARGO_TARGET_DIR`, and compiler-wrapper overrides. The
  measurement removes target and wrapper overrides so build output stays in the
  new project's default `target` directory and no ambient compiler cache is used.
- [Build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html)
  documents the default project-local `target` directory. The measurement
  verifies that directory is absent before execution.

## One execution command

- [cargo run](https://doc.rust-lang.org/cargo/commands/cargo-run.html) builds and
  runs the selected binary with the project directory as its working directory.
  One `cargo run --quiet` therefore covers dependency resolution, download,
  compilation, and execution without adding separate `cargo fetch` or
  `cargo build` setup steps.

These sources define Cargo behavior, not a universal latency promise. Registry,
network, host, and toolchain observations remain in each versioned result.
