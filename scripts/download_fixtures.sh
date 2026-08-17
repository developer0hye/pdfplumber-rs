#!/usr/bin/env bash
# Backward-compatible entry point for the pinned, path-preserving import.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec python3 "$SCRIPT_DIR/import_upstream_fixtures.py" "$@"
