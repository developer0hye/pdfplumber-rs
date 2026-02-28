#!/usr/bin/env python3
"""Generate golden reference JSON from Python pdfplumber for accuracy benchmarking.

Usage:
    pip install pdfplumber
    python3 tests/fixtures/generate_golden.py

Outputs one JSON per fixture PDF to tests/fixtures/golden/
"""

import json
import os
import sys

try:
    import pdfplumber
except ImportError:
    print("pdfplumber is required: pip install pdfplumber", file=sys.stderr)
    sys.exit(1)

FIXTURES_DIR = os.path.dirname(__file__)
GOLDEN_DIR = os.path.join(FIXTURES_DIR, "golden")
SOURCE_DIRS = [
    os.path.join(FIXTURES_DIR, "generated"),
    os.path.join(FIXTURES_DIR, "downloaded"),
]


def extract_page_data(page):
    """Extract chars, words, and tables from a single page."""
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
        })

    # Tables (default settings)
    tables = []
    for t in page.find_tables():
        bbox = [round(v, 4) for v in t.bbox]
        rows = []
        for row in t.extract():
            rows.append([cell if cell is not None else None for cell in row])
        tables.append({
            "bbox": bbox,
            "rows": rows,
        })

    return {
        "page_number": page.page_number,
        "width": round(float(page.width), 4),
        "height": round(float(page.height), 4),
        "chars": chars,
        "words": words,
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
        pages.append(extract_page_data(page))
    pdf.close()

    return {
        "source": pdf_name,
        "pages": pages,
    }


def main():
    os.makedirs(GOLDEN_DIR, exist_ok=True)
    count = 0

    for src_dir in SOURCE_DIRS:
        if not os.path.isdir(src_dir):
            print(f"  Directory not found: {src_dir}", file=sys.stderr)
            continue

        for fname in sorted(os.listdir(src_dir)):
            if not fname.endswith(".pdf"):
                continue

            pdf_path = os.path.join(src_dir, fname)
            stem = os.path.splitext(fname)[0]
            out_path = os.path.join(GOLDEN_DIR, f"{stem}.json")

            data = process_pdf(pdf_path, fname)
            if data is None:
                continue

            with open(out_path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2, ensure_ascii=False)

            total_chars = sum(len(p["chars"]) for p in data["pages"])
            total_words = sum(len(p["words"]) for p in data["pages"])
            total_tables = sum(len(p["tables"]) for p in data["pages"])
            print(f"  {stem}.json  ({len(data['pages'])} pages, "
                  f"{total_chars} chars, {total_words} words, {total_tables} tables)")
            count += 1

    print(f"Done! Generated {count} golden JSON files in {GOLDEN_DIR}")


if __name__ == "__main__":
    main()
