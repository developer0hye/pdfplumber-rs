#!/usr/bin/env python3
"""Generate golden JSON data from test PDFs using Python pdfplumber.

Usage:
    # With the .venv-golden virtualenv activated:
    python scripts/generate_golden.py

Reads PDFs from crates/pdfplumber/tests/fixtures/pdfs/
Writes JSON to crates/pdfplumber/tests/fixtures/golden/
"""

import json
import os
import sys

import pdfplumber

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(SCRIPT_DIR)
PDF_DIR = os.path.join(REPO_ROOT, "crates", "pdfplumber", "tests", "fixtures", "pdfs")
GOLDEN_DIR = os.path.join(REPO_ROOT, "crates", "pdfplumber", "tests", "fixtures", "golden")

COORD_DECIMALS = 4


def round_coord(v):
    """Round a coordinate value to COORD_DECIMALS places."""
    if v is None:
        return None
    return round(float(v), COORD_DECIMALS)


def extract_char(c):
    """Extract a char dict with only the fields we care about."""
    return {
        "text": c.get("text", ""),
        "x0": round_coord(c.get("x0")),
        "top": round_coord(c.get("top")),
        "x1": round_coord(c.get("x1")),
        "bottom": round_coord(c.get("bottom")),
        "fontname": c.get("fontname", ""),
        "size": round_coord(c.get("size")),
        "doctop": round_coord(c.get("doctop")),
        "upright": c.get("upright", True),
    }


def extract_word(w):
    """Extract a word dict."""
    return {
        "text": w.get("text", ""),
        "x0": round_coord(w.get("x0")),
        "top": round_coord(w.get("top")),
        "x1": round_coord(w.get("x1")),
        "bottom": round_coord(w.get("bottom")),
        "doctop": round_coord(w.get("doctop")),
    }


def extract_line(obj):
    """Extract a line dict."""
    return {
        "x0": round_coord(obj.get("x0")),
        "top": round_coord(obj.get("top")),
        "x1": round_coord(obj.get("x1")),
        "bottom": round_coord(obj.get("bottom")),
        "linewidth": round_coord(obj.get("linewidth", obj.get("line_width"))),
    }


def extract_rect(obj):
    """Extract a rect dict."""
    return {
        "x0": round_coord(obj.get("x0")),
        "top": round_coord(obj.get("top")),
        "x1": round_coord(obj.get("x1")),
        "bottom": round_coord(obj.get("bottom")),
        "linewidth": round_coord(obj.get("linewidth", obj.get("line_width"))),
        "stroke": obj.get("stroke", obj.get("stroking_color") is not None),
        "fill": obj.get("fill", obj.get("non_stroking_color") is not None),
    }


def extract_table(table):
    """Extract table as list of rows (list of cell strings)."""
    rows = []
    for row in table.cells if hasattr(table, "cells") else []:
        rows.append(row)

    # Use the extract() method which gives us a list of rows
    extracted = table.extract()
    bbox = table.bbox  # (x0, top, x1, bottom)
    return {
        "bbox": {
            "x0": round_coord(bbox[0]),
            "top": round_coord(bbox[1]),
            "x1": round_coord(bbox[2]),
            "bottom": round_coord(bbox[3]),
        },
        "rows": [
            [cell if cell is not None else "" for cell in row]
            for row in extracted
        ],
    }


def process_page(page):
    """Process a single page and return its golden data."""
    chars = [extract_char(c) for c in page.chars]
    words = [extract_word(w) for w in page.extract_words()]
    text = page.extract_text() or ""
    lines = [extract_line(l) for l in page.lines]
    rects = [extract_rect(r) for r in page.rects]

    tables_data = []
    try:
        tables = page.find_tables()
        tables_data = [extract_table(t) for t in tables]
    except Exception as e:
        print(f"  Warning: table extraction failed on page {page.page_number}: {e}")

    return {
        "page_number": page.page_number - 1,  # 0-indexed
        "width": round_coord(page.width),
        "height": round_coord(page.height),
        "chars": chars,
        "words": words,
        "text": text,
        "lines": lines,
        "rects": rects,
        "tables": tables_data,
    }


def process_pdf(pdf_path):
    """Process an entire PDF and return golden data."""
    filename = os.path.basename(pdf_path)
    print(f"Processing: {filename}")

    with pdfplumber.open(pdf_path) as pdf:
        pages = []
        for page in pdf.pages:
            page_data = process_page(page)
            n_chars = len(page_data["chars"])
            n_words = len(page_data["words"])
            n_tables = len(page_data["tables"])
            print(
                f"  Page {page_data['page_number']}: "
                f"{n_chars} chars, {n_words} words, "
                f"{len(page_data['lines'])} lines, "
                f"{len(page_data['rects'])} rects, "
                f"{n_tables} tables"
            )
            pages.append(page_data)

    return {
        "source": filename,
        "pdfplumber_version": pdfplumber.__version__,
        "pages": pages,
    }


def main():
    os.makedirs(GOLDEN_DIR, exist_ok=True)

    pdf_files = sorted(f for f in os.listdir(PDF_DIR) if f.endswith(".pdf"))
    if not pdf_files:
        print(f"No PDF files found in {PDF_DIR}")
        print("Run scripts/download_fixtures.sh first.")
        sys.exit(1)

    print(f"Python pdfplumber version: {pdfplumber.__version__}")
    print(f"Found {len(pdf_files)} PDF files\n")

    for pdf_file in pdf_files:
        pdf_path = os.path.join(PDF_DIR, pdf_file)
        golden = process_pdf(pdf_path)

        json_file = pdf_file.replace(".pdf", ".json")
        json_path = os.path.join(GOLDEN_DIR, json_file)

        with open(json_path, "w") as f:
            json.dump(golden, f, indent=2, ensure_ascii=False)

        print(f"  -> Written: {json_file}\n")

    print("Done! Golden data written to:", GOLDEN_DIR)


if __name__ == "__main__":
    main()
