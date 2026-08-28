#!/usr/bin/env bash
set -euo pipefail

readonly expected_rust_version="1.98.0"
readonly observed_rust_version="$(rustc --version | awk '{print $2}')"
readonly observed_cargo_version="$(cargo --version | awk '{print $2}')"

if [[ "${observed_rust_version}" != "${expected_rust_version}" ]]; then
    echo "expected rustc ${expected_rust_version}, observed ${observed_rust_version}" >&2
    exit 1
fi
if [[ "${observed_cargo_version}" != "${expected_rust_version}" ]]; then
    echo "expected cargo ${expected_rust_version}, observed ${observed_cargo_version}" >&2
    exit 1
fi

python3 --version
python3 scripts/check_doc_quickstarts.py --rust
cargo test -p pdfplumber --test feature_semantics
cargo test -p pdfplumber --features parallel --test concurrency
cargo check -p pdfplumber --examples --all-features
