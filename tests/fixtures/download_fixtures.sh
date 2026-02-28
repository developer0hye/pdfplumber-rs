#!/usr/bin/env bash
# Download real-world PDF fixtures from pdfplumber's test suite.
#
# Sources: https://github.com/jsvine/pdfplumber (MIT License)
# All government documents are US public domain.
#
# Usage:
#   bash tests/fixtures/download_fixtures.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTDIR="$SCRIPT_DIR/downloaded"
mkdir -p "$OUTDIR"

BASE_URL="https://raw.githubusercontent.com/jsvine/pdfplumber/stable/tests/pdfs"

echo "Downloading PDF fixtures..."

download() {
    local name="$1"
    local url="$2"
    local dest="$OUTDIR/$name"
    if [ -f "$dest" ]; then
        echo "  $name (already exists, skipping)"
    else
        echo "  $name <- $url"
        curl -fsSL -o "$dest" "$url"
    fi
}

download "pdffill-demo.pdf" "$BASE_URL/pdffill-demo.pdf"
download "nics-firearm-checks.pdf" "$BASE_URL/nics-background-checks-2015-11.pdf"
download "scotus-transcript-p1.pdf" "$BASE_URL/scotus-transcript-p1.pdf"

echo ""
echo "Verifying checksums..."

CHECKSUM_FILE="$SCRIPT_DIR/checksums.sha256"
if [ -f "$CHECKSUM_FILE" ]; then
    cd "$SCRIPT_DIR"
    if command -v shasum &>/dev/null; then
        shasum -a 256 -c checksums.sha256
    elif command -v sha256sum &>/dev/null; then
        sha256sum -c checksums.sha256
    else
        echo "  WARNING: No sha256 tool found, skipping verification"
    fi
    cd - >/dev/null
else
    echo "  No checksums.sha256 found, generating..."
    cd "$SCRIPT_DIR"
    if command -v shasum &>/dev/null; then
        shasum -a 256 downloaded/*.pdf generated/*.pdf 2>/dev/null > checksums.sha256 || \
        shasum -a 256 downloaded/*.pdf > checksums.sha256
    elif command -v sha256sum &>/dev/null; then
        sha256sum downloaded/*.pdf generated/*.pdf 2>/dev/null > checksums.sha256 || \
        sha256sum downloaded/*.pdf > checksums.sha256
    fi
    echo "  Generated checksums.sha256"
    cd - >/dev/null
fi

echo ""
echo "Done! Downloaded fixtures to $OUTDIR/"
