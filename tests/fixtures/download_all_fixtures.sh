#!/usr/bin/env bash
# Download real-world PDF fixtures from pdfplumber's test suite and examples.
#
# Sources: https://github.com/jsvine/pdfplumber (MIT License)
# All government documents are US public domain.
#
# Usage:
#   bash tests/fixtures/download_all_fixtures.sh
#
# PDFs are saved to crates/pdfplumber/tests/fixtures/pdfs/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTDIR="$REPO_ROOT/crates/pdfplumber/tests/fixtures/pdfs"
mkdir -p "$OUTDIR"

BASE_URL="https://raw.githubusercontent.com/jsvine/pdfplumber/stable"
TESTS_URL="$BASE_URL/tests/pdfs"
EXAMPLES_URL="$BASE_URL/examples/pdfs"

DOWNLOADED=0
SKIPPED=0
FAILED=0

download() {
    local name="$1"
    local url="$2"
    local dest="$OUTDIR/$name"
    if [ -f "$dest" ]; then
        SKIPPED=$((SKIPPED + 1))
        return
    fi
    if curl -fsSL -o "$dest" "$url" 2>/dev/null; then
        DOWNLOADED=$((DOWNLOADED + 1))
    else
        echo "  WARN: failed to download $name"
        FAILED=$((FAILED + 1))
    fi
}

echo "Downloading real-world PDF fixtures to $OUTDIR/"
echo ""

# ── Category 1: Government/Tabular ──────────────────────────────────────────
echo "Government/Tabular documents..."
download "ag-energy-round-up-2017-02-24.pdf"           "$EXAMPLES_URL/ag-energy-round-up-2017-02-24.pdf"
download "background-checks.pdf"                       "$EXAMPLES_URL/background-checks.pdf"
download "ca-warn-report.pdf"                          "$EXAMPLES_URL/ca-warn-report.pdf"
download "san-jose-pd-firearm-sample.pdf"              "$EXAMPLES_URL/san-jose-pd-firearm-sample.pdf"
download "federal-register-2020-17221.pdf"             "$TESTS_URL/federal-register-2020-17221.pdf"
download "WARN-Report-for-7-1-2015-to-03-25-2016.pdf" "$TESTS_URL/WARN-Report-for-7-1-2015-to-03-25-2016.pdf"
download "senate-expenditures.pdf"                     "$TESTS_URL/senate-expenditures.pdf"
download "la-precinct-bulletin-2014-p1.pdf"            "$TESTS_URL/la-precinct-bulletin-2014-p1.pdf"
download "cupertino_usd_4-6-16.pdf"                    "$TESTS_URL/cupertino_usd_4-6-16.pdf"

# ── Category 2: Complex Layouts ─────────────────────────────────────────────
echo "Complex layouts..."
download "150109DSP-Milw-505-90D.pdf"                  "$TESTS_URL/150109DSP-Milw-505-90D.pdf"
download "2023-06-20-PV.pdf"                           "$TESTS_URL/2023-06-20-PV.pdf"
download "chelsea_pdta.pdf"                            "$TESTS_URL/chelsea_pdta.pdf"
download "issue-13-151201DSP-Fond-581-90D.pdf"         "$TESTS_URL/issue-13-151201DSP-Fond-581-90D.pdf"

# ── Category 3: Table Edge Cases ────────────────────────────────────────────
echo "Table edge cases..."
download "table-curves-example.pdf"                    "$TESTS_URL/table-curves-example.pdf"
download "pr-136-example.pdf"                          "$TESTS_URL/pr-136-example.pdf"
download "pr-138-example.pdf"                          "$TESTS_URL/pr-138-example.pdf"
download "pr-88-example.pdf"                           "$TESTS_URL/pr-88-example.pdf"

# ── Category 4: Annotations ─────────────────────────────────────────────────
echo "Annotations..."
download "annotations.pdf"                             "$TESTS_URL/annotations.pdf"
download "annotations-rotated-90.pdf"                  "$TESTS_URL/annotations-rotated-90.pdf"
download "annotations-rotated-180.pdf"                 "$TESTS_URL/annotations-rotated-180.pdf"
download "annotations-rotated-270.pdf"                 "$TESTS_URL/annotations-rotated-270.pdf"
download "annotations-unicode-issues.pdf"              "$TESTS_URL/annotations-unicode-issues.pdf"

# ── Category 5: Character Handling ──────────────────────────────────────────
echo "Character handling..."
download "issue-71-duplicate-chars.pdf"                "$TESTS_URL/issue-71-duplicate-chars.pdf"
download "issue-71-duplicate-chars-2.pdf"              "$TESTS_URL/issue-71-duplicate-chars-2.pdf"
download "issue-1114-dedupe-chars.pdf"                 "$TESTS_URL/issue-1114-dedupe-chars.pdf"
download "line-char-render-example.pdf"                "$TESTS_URL/line-char-render-example.pdf"
download "test-punkt.pdf"                              "$TESTS_URL/test-punkt.pdf"
download "issue-203-decimalize.pdf"                    "$TESTS_URL/issue-203-decimalize.pdf"

# ── Category 6: Structure/Tagged PDFs ───────────────────────────────────────
echo "Structure/Tagged PDFs..."
download "figure_structure.pdf"                        "$TESTS_URL/figure_structure.pdf"
download "hello_structure.pdf"                         "$TESTS_URL/hello_structure.pdf"
download "image_structure.pdf"                         "$TESTS_URL/image_structure.pdf"
download "pdf_structure.pdf"                           "$TESTS_URL/pdf_structure.pdf"
download "word365_structure.pdf"                       "$TESTS_URL/word365_structure.pdf"

# ── Category 7: Edge Cases ──────────────────────────────────────────────────
echo "Edge cases..."
download "empty.pdf"                                   "$TESTS_URL/empty.pdf"
download "password-example.pdf"                        "$TESTS_URL/password-example.pdf"
download "malformed-from-issue-932.pdf"                "$TESTS_URL/malformed-from-issue-932.pdf"
download "page-boxes-example.pdf"                      "$TESTS_URL/page-boxes-example.pdf"
download "extra-attrs-example.pdf"                     "$TESTS_URL/extra-attrs-example.pdf"
download "nics-background-checks-2015-11-rotated.pdf"  "$TESTS_URL/nics-background-checks-2015-11-rotated.pdf"

# ── Category 8: Issue Regressions ───────────────────────────────────────────
echo "Issue regressions..."
download "issue-53-example.pdf"                        "$TESTS_URL/issue-53-example.pdf"
download "issue-67-example.pdf"                        "$TESTS_URL/issue-67-example.pdf"
download "issue-90-example.pdf"                        "$TESTS_URL/issue-90-example.pdf"
download "issue-140-example.pdf"                       "$TESTS_URL/issue-140-example.pdf"
download "issue-297-example.pdf"                       "$TESTS_URL/issue-297-example.pdf"
download "issue-316-example.pdf"                       "$TESTS_URL/issue-316-example.pdf"
download "issue-461-example.pdf"                       "$TESTS_URL/issue-461-example.pdf"

echo ""
echo "Done! Downloaded: $DOWNLOADED, Skipped: $SKIPPED, Failed: $FAILED"
echo "Total PDFs in $OUTDIR/: $(find "$OUTDIR" -name '*.pdf' | wc -l | tr -d ' ')"
