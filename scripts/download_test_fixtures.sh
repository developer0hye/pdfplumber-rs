#!/usr/bin/env bash
# Download test PDF fixtures from multiple sources for cross-validation testing.
#
# Sources:
#   1. jsvine/pdfplumber (stable branch) - MIT license
#   2. mozilla/pdf.js (master branch) - Apache 2.0 license
#   3. apache/pdfbox (trunk branch) - Apache 2.0 license
#   4. poppler/test (GitLab, master branch) - GPL license
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

# Download CJK/multilingual test PDFs from apache/pdfbox (trunk branch)
download_pdfbox() {
    echo "=== Downloading from apache/pdfbox (trunk) ==="
    local dest_dir="$PDF_DIR/pdfbox"
    mkdir -p "$dest_dir"

    local base_url="https://raw.githubusercontent.com/apache/pdfbox/trunk"

    # Map: source_path -> local_filename (shortened for readability)
    local -a pdfbox_mappings=(
        "pdfbox/src/test/resources/input/PDFBOX-5350-JX57O5E5YG6XM4FZABPULQGTW4OXPCWA-p1-reduced.pdf|pdfbox-5350-korean-reduced.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-3833-reduced.pdf|pdfbox-3833-japanese-reduced.pdf"
        "pdfbox/src/test/resources/org/apache/pdfbox/text/BidiSample.pdf|BidiSample.pdf"
        "pdfbox/src/test/resources/input/FC60_Times.pdf|FC60_Times.pdf"
        "pdfbox/src/test/resources/input/hello3.pdf|hello3.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-4531-bidi-ligature-1.pdf|pdfbox-4531-bidi-ligature-1.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-4531-bidi-ligature-2.pdf|pdfbox-4531-bidi-ligature-2.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-3127-RAU4G6QMOVRYBISJU7R6MOVZCRFUO7P4-VFont.pdf|pdfbox-3127-vfont-reduced.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-4322-Empty-ToUnicode-reduced.pdf|pdfbox-4322-empty-tounicode-reduced.pdf"
        "pdfbox/src/test/resources/input/PDFBOX-5747-unicode-surrogate-with-diacritic-reduced.pdf|pdfbox-5747-surrogate-diacritic-reduced.pdf"
    )

    local count=0
    for mapping in "${pdfbox_mappings[@]}"; do
        local src_path="${mapping%%|*}"
        local local_name="${mapping##*|}"
        download_file "$base_url/$src_path" "$dest_dir/$local_name" || true
        count=$((count + 1))
    done
    echo "  Processed $count files from PDFBox"
    echo ""
}

# Download CJK/multilingual test PDFs from poppler test data (GitLab)
download_poppler() {
    echo "=== Downloading from poppler/test (GitLab, master) ==="
    local dest_dir="$PDF_DIR/poppler"
    mkdir -p "$dest_dir"

    local base_url="https://gitlab.freedesktop.org/poppler/test/-/raw/master"

    # Target PDFs for multilingual testing
    local -a poppler_files=(
        "unittestcases/pdf20-utf8-test.pdf"
        "unittestcases/russian.pdf"
        "unittestcases/deseret.pdf"
    )

    local count=0
    for src_path in "${poppler_files[@]}"; do
        local filename
        filename="$(basename "$src_path")"
        download_file "$base_url/$src_path" "$dest_dir/$filename" || true
        count=$((count + 1))
    done
    echo "  Processed $count files from poppler"
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
    download_pdfbox
    download_poppler

    print_summary
}

main
