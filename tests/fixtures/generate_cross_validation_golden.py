#!/usr/bin/env python3
"""Generate golden reference JSON for cross-validation tests.

Uses the rich schema required by cross_validation.rs:
- chars: text, x0, top, x1, bottom, fontname, size, doctop, upright
- words: text, x0, top, x1, bottom, doctop
- text: full page text
- lines: x0, top, x1, bottom, linewidth
- rects: x0, top, x1, bottom, linewidth, stroke, fill
- tables: bbox (dict), rows

Usage:
    pip install pdfplumber==0.11.9
    python3 tests/fixtures/generate_cross_validation_golden.py
"""

import json
import os
import sys

try:
    import pdfplumber
except ImportError:
    print("pdfplumber is required: pip install pdfplumber==0.11.9", file=sys.stderr)
    sys.exit(1)

FIXTURES_DIR = os.path.dirname(__file__)
REPO_ROOT = os.path.join(FIXTURES_DIR, "..", "..")
PDF_DIR = os.path.join(REPO_ROOT, "crates", "pdfplumber", "tests", "fixtures", "pdfs")
GOLDEN_DIR = os.path.join(REPO_ROOT, "crates", "pdfplumber", "tests", "fixtures", "golden")

# PDFs that need special handling
SKIP_PDFS = {
    "password-example.pdf",  # Password-protected
    "empty.pdf",             # Empty file (0 bytes)
}


def extract_page_data(page):
    """Extract full cross-validation data from a page."""
    # Chars
    chars = []
    for c in page.chars:
        chars.append({
            "text": c["text"],
            "x0": round(c["x0"], 4),
            "top": round(c["top"], 4),
            "x1": round(c["x1"], 4),
            "bottom": round(c["bottom"], 4),
            "fontname": c.get("fontname", ""),
            "size": round(c.get("size", 0.0), 4),
            "doctop": round(c.get("doctop", c["top"]), 4),
            "upright": c.get("upright", True),
        })

    # Words (default settings)
    words = []
    for w in page.extract_words():
        words.append({
            "text": w["text"],
            "x0": round(w["x0"], 4),
            "top": round(w["top"], 4),
            "x1": round(w["x1"], 4),
            "bottom": round(w["bottom"], 4),
            "doctop": round(w.get("doctop", w["top"]), 4),
        })

    # Full text
    text = page.extract_text() or ""

    # Lines
    lines = []
    for ln in page.lines:
        lines.append({
            "x0": round(ln["x0"], 4),
            "top": round(ln["top"], 4),
            "x1": round(ln["x1"], 4),
            "bottom": round(ln["bottom"], 4),
            "linewidth": round(ln.get("linewidth", ln.get("lw", 0.0)) or 0.0, 4),
        })

    # Rects
    rects = []
    for r in page.rects:
        rects.append({
            "x0": round(r["x0"], 4),
            "top": round(r["top"], 4),
            "x1": round(r["x1"], 4),
            "bottom": round(r["bottom"], 4),
            "linewidth": round(r.get("linewidth", r.get("lw", 0.0)) or 0.0, 4),
            "stroke": bool(r.get("stroke", True)),
            "fill": bool(r.get("fill", False)),
        })

    # Tables (default settings)
    tables = []
    for t in page.find_tables():
        bbox = {
            "x0": round(t.bbox[0], 4),
            "top": round(t.bbox[1], 4),
            "x1": round(t.bbox[2], 4),
            "bottom": round(t.bbox[3], 4),
        }
        rows = []
        for row in t.extract():
            rows.append([cell if cell is not None else "" for cell in row])
        tables.append({
            "bbox": bbox,
            "rows": rows,
        })

    return {
        "page_number": page.page_number - 1,  # Convert to 0-based for Rust
        "width": round(float(page.width), 4),
        "height": round(float(page.height), 4),
        "chars": chars,
        "words": words,
        "text": text,
        "lines": lines,
        "rects": rects,
        "tables": tables,
    }


def process_pdf(pdf_path, pdf_name):
    """Process a single PDF and return golden data dict."""
    try:
        pdf = pdfplumber.open(pdf_path)
    except Exception as e:
        print(f"  SKIP {pdf_name}: {e}", file=sys.stderr)
        return None

    pages = []
    for page in pdf.pages:
        try:
            pages.append(extract_page_data(page))
        except Exception as e:
            print(f"  WARN {pdf_name} page {page.page_number}: {e}", file=sys.stderr)
    pdf.close()

    return {
        "source": pdf_name,
        "pdfplumber_version": pdfplumber.__version__,
        "pages": pages,
    }


def main():
    os.makedirs(GOLDEN_DIR, exist_ok=True)
    count = 0
    skipped = 0
    failed = 0

    if not os.path.isdir(PDF_DIR):
        print(f"PDF directory not found: {PDF_DIR}", file=sys.stderr)
        print("Run download_all_fixtures.sh first.", file=sys.stderr)
        sys.exit(1)

    pdf_files = sorted(f for f in os.listdir(PDF_DIR) if f.endswith(".pdf"))
    print(f"Processing {len(pdf_files)} PDFs from {PDF_DIR}")
    print(f"Using pdfplumber {pdfplumber.__version__}")
    print()

    for fname in pdf_files:
        if fname in SKIP_PDFS:
            print(f"  SKIP {fname} (in skip list)")
            skipped += 1
            continue

        pdf_path = os.path.join(PDF_DIR, fname)
        stem = os.path.splitext(fname)[0]
        out_path = os.path.join(GOLDEN_DIR, f"{stem}.json")

        data = process_pdf(pdf_path, fname)
        if data is None:
            failed += 1
            continue

        with open(out_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

        total_chars = sum(len(p["chars"]) for p in data["pages"])
        total_words = sum(len(p["words"]) for p in data["pages"])
        total_tables = sum(len(p["tables"]) for p in data["pages"])
        total_lines = sum(len(p["lines"]) for p in data["pages"])
        total_rects = sum(len(p["rects"]) for p in data["pages"])
        print(f"  {stem}.json  ({len(data['pages'])} pages, "
              f"{total_chars} chars, {total_words} words, "
              f"{total_lines} lines, {total_rects} rects, {total_tables} tables)")
        count += 1

    print()
    print(f"Done! Generated: {count}, Skipped: {skipped}, Failed: {failed}")
    print(f"Output: {GOLDEN_DIR}")


if __name__ == "__main__":
    main()
