#!/usr/bin/env bash
# Setup a Python virtual environment for golden data generation.
# Creates .venv-golden at the repo root and installs pdfplumber.
#
# Usage:
#   bash scripts/setup_golden_venv.sh
#   source .venv-golden/bin/activate
#   python scripts/generate_golden.py
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$REPO_ROOT/.venv-golden"

if [ -d "$VENV_DIR" ]; then
    echo "Virtual environment already exists at $VENV_DIR"
    echo "To recreate, remove it first: rm -rf $VENV_DIR"
else
    echo "Creating virtual environment at $VENV_DIR ..."
    python3 -m venv "$VENV_DIR"
    echo "Virtual environment created."
fi

echo "Installing pdfplumber ..."
"$VENV_DIR/bin/pip" install --upgrade pip --quiet
"$VENV_DIR/bin/pip" install pdfplumber --quiet
echo "pdfplumber installed: $("$VENV_DIR/bin/python" -c 'import pdfplumber; print(pdfplumber.__version__)')"

echo ""
echo "Setup complete. To generate golden data:"
echo "  source $VENV_DIR/bin/activate"
echo "  python scripts/generate_golden.py"
