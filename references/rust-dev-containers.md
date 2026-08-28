# Rust development-container references

Rechecked: 2026-08-28.

## Development Container Specification

Sources: [Development Containers](https://containers.dev/) and
[Create a Dev Container](https://code.visualstudio.com/docs/devcontainers/create-dev-container)

- `.devcontainer/devcontainer.json` is the portable editor entry point.
- A referenced Dockerfile keeps image construction usable outside one editor.

## Docker reproducibility

Sources: [Docker image pull](https://docs.docker.com/reference/cli/docker/image/pull/#pull-an-image-by-digest-immutable-identifier)
and [Docker build policy examples](https://docs.docker.com/build/policies/examples/#pin-base-images-to-digests)

- Tags can move; a `sha256` digest identifies immutable image content.
- A multi-platform manifest-list digest retains native platform selection.

## Official Rust image

Source: [Rust Official Image](https://hub.docker.com/_/rust)

- The versioned Bookworm variant supplies Rust and common build tools on a
  named Debian release.
- The repository pins both `1.98.0-bookworm` and its complete manifest digest.

## Application to DIST-015

The checked-in Dockerfile is shared by the editor configuration and the
command-line verifier. The verifier mounts source read-only and isolates Cargo
downloads and targets under the disposable container filesystem.
