#!/usr/bin/env bash
# Download test PDF fixtures from multiple sources for cross-validation testing.
#
# Sources:
#   1. jsvine/pdfplumber (stable branch) - MIT license
#   2. (Future: pdf.js, PDFBox, poppler - added by subsequent stories)
#
# Usage: ./scripts/download_test_fixtures.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PDF_DIR="$REPO_ROOT/crates/pdfplumber/tests/fixtures/pdfs"

DOWNLOADED=0
SKIPPED=0
FAILED=0

# Download a single file. Args: url, dest_path
download_file() {
    local url="$1"
    local dest="$2"
    local name
    name="$(basename "$dest")"

    if [[ -f "$dest" ]]; then
        SKIPPED=$((SKIPPED + 1))
        return 0
    fi

    if curl -fsSL "$url" -o "$dest" 2>/dev/null; then
        local size
        size=$(wc -c < "$dest" | tr -d ' ')
        if [[ "$size" -eq 0 ]]; then
            echo "  WARNING: $name is 0 bytes, removing"
            rm -f "$dest"
            FAILED=$((FAILED + 1))
            return 1
        fi
        echo "  Downloaded: $name ($size bytes)"
        DOWNLOADED=$((DOWNLOADED + 1))
    else
        echo "  FAILED: $name"
        rm -f "$dest"
        FAILED=$((FAILED + 1))
        return 1
    fi
}

# Download PDFs from jsvine/pdfplumber GitHub repo (stable branch)
download_pdfplumber_python() {
    echo "=== Downloading from jsvine/pdfplumber (stable) ==="
    local dest_dir="$PDF_DIR"
    mkdir -p "$dest_dir"

    local base_url="https://raw.githubusercontent.com/jsvine/pdfplumber/stable/tests/pdfs"

    # List all files via gh api, filter to .pdf files only (skip .py, .zip, directories)
    local files
    files=$(gh api repos/jsvine/pdfplumber/contents/tests/pdfs \
        --jq '.[] | select(.type == "file") | select(.name | endswith(".pdf")) | .name')

    local count=0
    while IFS= read -r filename; do
        [[ -z "$filename" ]] && continue
        download_file "$base_url/$filename" "$dest_dir/$filename" || true
        count=$((count + 1))
    done <<< "$files"
    echo "  Processed $count files from tests/pdfs/"

    # Download from-oss-fuzz/load/ subdirectory
    echo ""
    echo "  --- from-oss-fuzz/load/ ---"
    local oss_dir="$dest_dir/oss-fuzz"
    mkdir -p "$oss_dir"

    local oss_files
    oss_files=$(gh api repos/jsvine/pdfplumber/contents/tests/pdfs/from-oss-fuzz/load \
        --jq '.[] | select(.type == "file") | select(.name | endswith(".pdf")) | .name')

    local oss_count=0
    while IFS= read -r filename; do
        [[ -z "$filename" ]] && continue
        download_file "$base_url/from-oss-fuzz/load/$filename" "$oss_dir/$filename" || true
        oss_count=$((oss_count + 1))
    done <<< "$oss_files"
    echo "  Processed $oss_count files from from-oss-fuzz/load/"
    echo ""
}

# Print summary
print_summary() {
    echo "=== Summary ==="
    echo "  Downloaded: $DOWNLOADED files"
    echo "  Skipped:    $SKIPPED files (already exist)"
    echo "  Failed:     $FAILED files"
    echo ""
    echo "Files in $PDF_DIR:"
    find "$PDF_DIR" -name '*.pdf' | wc -l | tr -d ' '
    echo ""
}

main() {
    echo "PDF Test Fixture Downloader"
    echo "==========================="
    echo ""

    download_pdfplumber_python

    print_summary
}

main
