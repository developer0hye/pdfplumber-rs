#!/usr/bin/env bash
# Download test PDF fixtures from jsvine/pdfplumber GitHub repository.
# Usage: ./scripts/download_fixtures.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PDF_DIR="$REPO_ROOT/crates/pdfplumber/tests/fixtures/pdfs"

BASE_URL="https://raw.githubusercontent.com/jsvine/pdfplumber/stable/tests/pdfs"

PDFS=(
    "pdffill-demo.pdf"
    "scotus-transcript-p1.pdf"
    "nics-background-checks-2015-11.pdf"
    "issue-33-lorem-ipsum.pdf"
)

mkdir -p "$PDF_DIR"

for pdf in "${PDFS[@]}"; do
    dest="$PDF_DIR/$pdf"
    if [[ -f "$dest" ]]; then
        echo "Already exists: $pdf"
    else
        echo "Downloading: $pdf"
        curl -fsSL "$BASE_URL/$pdf" -o "$dest"
        echo "  -> OK ($(wc -c < "$dest" | tr -d ' ') bytes)"
    fi
done

echo ""
echo "All fixtures downloaded to: $PDF_DIR"
ls -lh "$PDF_DIR"
