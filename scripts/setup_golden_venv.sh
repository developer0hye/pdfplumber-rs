#!/usr/bin/env bash
# Build the pinned reference environment for golden-data generation (PARITY-003).
#
# Everything installed here is fixed by compat/requirements-golden.txt and
# verified by hash, so the environment is the same on every machine and in CI.
# The previous behavior -- `pip install pdfplumber` against whatever PyPI served
# that day -- could not be reproduced after the fact, which made every golden
# artifact untraceable.
#
# Usage:
#   bash scripts/setup_golden_venv.sh
#   .venv-reference/bin/python scripts/generate_golden.py
#
# The reference interpreter is pinned in compat/upstream.toml. Override it only
# when you accept that output may differ from CI:
#   PDFPLUMBER_RS_REFERENCE_PYTHON=python3.12 bash scripts/setup_golden_venv.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# tomllib, used to read compat/upstream.toml, landed in Python 3.11.
BOOTSTRAP_PYTHON="${PYTHON:-python3}"
if ! "$BOOTSTRAP_PYTHON" -c 'import tomllib' 2>/dev/null; then
    echo "error: $BOOTSTRAP_PYTHON cannot import tomllib; the harness needs Python 3.11+" >&2
    echo "       set PYTHON=/path/to/python3.11-or-newer and re-run" >&2
    exit 1
fi

read_setting() {
    "$BOOTSTRAP_PYTHON" -c "
import tomllib
with open('compat/upstream.toml', 'rb') as handle:
    settings = tomllib.load(handle)
print(settings['$1']['$2'])
"
}

TARGET_VERSION="$(read_setting target version)"
PINNED_PYTHON_VERSION="$(read_setting environment python_version)"
LOCKFILE="$(read_setting environment lockfile)"
VENV_DIR="$REPO_ROOT/$(read_setting environment reference_venv)"

# Prefer an explicit override, then the pinned version by name, then plain
# python3 -- which is what a version manager or actions/setup-python leaves on
# PATH. The name is only a guess either way; the version check below is what
# actually decides.
if [ -n "${PDFPLUMBER_RS_REFERENCE_PYTHON:-}" ]; then
    REFERENCE_PYTHON="$PDFPLUMBER_RS_REFERENCE_PYTHON"
elif command -v "python${PINNED_PYTHON_VERSION}" >/dev/null 2>&1; then
    REFERENCE_PYTHON="python${PINNED_PYTHON_VERSION}"
else
    REFERENCE_PYTHON="python3"
fi

if ! command -v "$REFERENCE_PYTHON" >/dev/null 2>&1; then
    echo "error: reference interpreter '$REFERENCE_PYTHON' is not on PATH" >&2
    exit 1
fi

ACTUAL_PYTHON_VERSION="$("$REFERENCE_PYTHON" -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
if [ "$ACTUAL_PYTHON_VERSION" != "$PINNED_PYTHON_VERSION" ]; then
    # CI sets this so a drifting interpreter fails the build instead of quietly
    # producing golden data nobody can reproduce.
    if [ -n "${PDFPLUMBER_RS_REQUIRE_PINNED_PYTHON:-}" ]; then
        echo "error: '$REFERENCE_PYTHON' is Python $ACTUAL_PYTHON_VERSION, but the pinned" >&2
        echo "       reference interpreter is Python $PINNED_PYTHON_VERSION" >&2
        exit 1
    fi
    echo "warning: '$REFERENCE_PYTHON' is Python $ACTUAL_PYTHON_VERSION, not the pinned"
    echo "         Python $PINNED_PYTHON_VERSION; golden output may differ from CI."
    echo "         The interpreter version is recorded in each provenance block."
    echo "         Install Python $PINNED_PYTHON_VERSION for reproducible artifacts."
fi

echo "Rebuilding $VENV_DIR from $LOCKFILE ..."
rm -rf "$VENV_DIR"
"$REFERENCE_PYTHON" -m venv "$VENV_DIR"

# --require-hashes rejects the whole file if any requirement is unpinned or has
# no matching hash, which is what makes the lock a gate rather than a suggestion.
"$VENV_DIR/bin/pip" install --quiet --require-hashes --requirement "$LOCKFILE"

"$VENV_DIR/bin/python" "$SCRIPT_DIR/verify_compat_env.py" --reference --expect-root "$VENV_DIR"

echo ""
echo "Reference environment ready (pdfplumber $TARGET_VERSION)."
echo "Generate golden data with:"
echo "  $VENV_DIR/bin/python scripts/generate_golden.py"
