#!/usr/bin/env bash
set -euo pipefail

readonly repository_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly image_name="${PDFPLUMBER_RS_DEV_IMAGE:-pdfplumber-rs-dev:local}"

if ! command -v docker >/dev/null 2>&1; then
    echo "Docker is required to run the reproducible Rust environment." >&2
    exit 1
fi

docker build \
    --pull \
    --file "${repository_root}/.devcontainer/Dockerfile" \
    --tag "${image_name}" \
    "${repository_root}"

docker run \
    --rm \
    --volume "${repository_root}:/workspaces/pdfplumber-rs:ro" \
    --workdir /workspaces/pdfplumber-rs \
    --env CARGO_HOME=/tmp/pdfplumber-cargo \
    --env CARGO_TARGET_DIR=/tmp/pdfplumber-target \
    "${image_name}" \
    bash scripts/check_rust_dev_environment.sh
