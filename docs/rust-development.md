# Reproducible Rust development

The repository provides a development container for the Rust quick start and
the focused contributor paths used by source Continuous Integration. From a
Docker-capable checkout, build the pinned image and run all of those checks with
one command:

```bash
scripts/check_rust_dev_container.sh
```

The command mounts the checkout read-only. Cargo downloads and build products
live only in temporary container directories, so ambient host Rust versions,
Cargo registries, target directories, and compiler settings do not supply a
passing result. The first run requires network access to pull the image and the
locked Cargo dependencies.

## Included checks

The in-container `scripts/check_rust_dev_environment.sh` command verifies Rust
and Cargo 1.98.0, requires Python 3 for the repository harness, then runs:

- all five rendered README Rust quick starts against the current source;
- default-feature extraction semantics;
- parallel extraction ordering and resource behavior; and
- every task-oriented Rust example with all features.

These are focused contributor checks, not the complete workspace, compatibility,
packaging, WebAssembly, or Python suite. Pull requests still need every required
Continuous Integration job.

## Editor workflow

`.devcontainer/devcontainer.json` uses the same Dockerfile. In an editor that
implements the Development Container Specification, open the checkout in its
container and run:

```bash
scripts/check_rust_dev_environment.sh
```

The editor maps the non-root `rustdev` user to the checkout owner. The image's
multi-platform manifest selects a native Linux image on amd64 and arm64 hosts.

## Reproducibility and Rust support

The base is the official image
`rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922`.
The full manifest-list digest is immutable and keeps the operating-system and
toolchain input fixed across supported host architectures. The Dockerfile adds
no network-fetched operating-system packages.

This is a reproducible snapshot, not a Minimum Supported Rust Version promise.
Required Continuous Integration continues to follow rolling stable Rust. When
that channel advances, inspect the official image manifest, update both the
version tag and digest, run the container command on the new snapshot, and let
the exact-head Continuous Integration gates prove the transition before merge.

The official sources and the reason for the digest and Dev Container choices
are recorded in the [source mapping](../references/rust-dev-containers.md).
