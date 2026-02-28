#!/usr/bin/env bash
# Download test PDF fixtures from multiple sources for cross-validation testing.
#
# Sources:
#   1. jsvine/pdfplumber (stable branch) - MIT license
#   2. mozilla/pdf.js (master branch) - Apache 2.0 license
#   3. (Future: PDFBox, poppler - added by subsequent stories)
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

# Download CJK/encoding test PDFs from mozilla/pdf.js (master branch)
download_pdfjs() {
    echo "=== Downloading from mozilla/pdf.js (master) ==="
    local dest_dir="$PDF_DIR/pdfjs"
    mkdir -p "$dest_dir"

    local base_url="https://raw.githubusercontent.com/mozilla/pdf.js/master/test/pdfs"

    # Target PDFs for CJK and encoding testing
    local pdfjs_files=(
        "issue3521.pdf"
        "noembed-eucjp.pdf"
        "noembed-sjis.pdf"
        "noembed-jis7.pdf"
        "noembed-identity.pdf"
        "noembed-identity-2.pdf"
        "vertical.pdf"
        "issue8570.pdf"
        "ArabicCIDTrueType.pdf"
        "cid_cff.pdf"
        "text_clip_cff_cid.pdf"
        "issue7696.pdf"
        "issue4875.pdf"
        "issue14117.pdf"
        "issue9262_reduced.pdf"
    )

    local count=0
    for filename in "${pdfjs_files[@]}"; do
        download_file "$base_url/$filename" "$dest_dir/$filename" || true
        count=$((count + 1))
    done
    echo "  Processed $count files from pdf.js"
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
    download_pdfjs

    print_summary
}

main
