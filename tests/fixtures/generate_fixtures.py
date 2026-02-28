#!/usr/bin/env python3
"""Generate realistic PDF test fixtures using fpdf2.

Usage:
    pip install fpdf2
    python3 tests/fixtures/generate_fixtures.py

Outputs 10 PDFs to tests/fixtures/generated/
"""

import os
import sys

try:
    from fpdf import FPDF
except ImportError:
    print("fpdf2 is required: pip install fpdf2", file=sys.stderr)
    sys.exit(1)

OUTDIR = os.path.join(os.path.dirname(__file__), "generated")
os.makedirs(OUTDIR, exist_ok=True)


def save(pdf: FPDF, name: str):
    path = os.path.join(OUTDIR, name)
    pdf.output(path)
    size = os.path.getsize(path)
    print(f"  {name} ({size:,} bytes)")


# ---------- 1. basic_text.pdf ----------
def gen_basic_text():
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)

    # Paragraph 1: pangram
    pdf.multi_cell(0, 6, "The quick brown fox jumps over the lazy dog.")
    pdf.ln(4)

    # Paragraph 2: special characters (latin-1 compatible)
    pdf.multi_cell(0, 6, 'Special chars: "quotes", copyright \u00a9, registered \u00ae, section \u00a7, degree \u00b0, plus-minus \u00b1')
    pdf.ln(4)

    # Paragraph 3: accented characters (latin-1)
    pdf.multi_cell(0, 6, "Accented: caf\u00e9, na\u00efve, r\u00e9sum\u00e9, \u00fcber, pi\u00f1ata, \u00e0 la carte")
    pdf.ln(4)

    # Paragraph 4: numbers and punctuation
    pdf.multi_cell(0, 6, "Numbers: 0 1 2 3 4 5 6 7 8 9. Price: $1,234.56. Ratio: 3:1. Percent: 99.9%")

    save(pdf, "basic_text.pdf")


# ---------- 2. multicolumn.pdf ----------
def gen_multicolumn():
    pdf = FPDF()

    # Page 1: 2-column layout
    pdf.add_page()
    pdf.set_font("Helvetica", size=10)
    col_w = (pdf.w - 20) / 2  # 2 columns with margins
    left_text = [
        "Left column line 1",
        "Left column line 2",
        "Left column line 3",
        "Left column line 4",
        "Left column line 5",
    ]
    right_text = [
        "Right column line 1",
        "Right column line 2",
        "Right column line 3",
        "Right column line 4",
        "Right column line 5",
    ]
    for i, (lt, rt) in enumerate(zip(left_text, right_text)):
        y = 20 + i * 8
        pdf.set_xy(10, y)
        pdf.cell(col_w, 6, lt)
        pdf.set_xy(10 + col_w, y)
        pdf.cell(col_w, 6, rt)

    # Page 2: 3-column layout
    pdf.add_page()
    col_w3 = (pdf.w - 20) / 3
    cols = [
        ["Col A line 1", "Col A line 2", "Col A line 3"],
        ["Col B line 1", "Col B line 2", "Col B line 3"],
        ["Col C line 1", "Col C line 2", "Col C line 3"],
    ]
    for i in range(3):
        for c in range(3):
            y = 20 + i * 8
            pdf.set_xy(10 + c * col_w3, y)
            pdf.cell(col_w3, 6, cols[c][i])

    save(pdf, "multicolumn.pdf")


# ---------- 3. table_lattice.pdf ----------
def gen_table_lattice():
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=10)

    headers = ["ID", "Name", "Category", "Price", "Stock"]
    data = [
        ["1", "Widget A", "Hardware", "$10.00", "100"],
        ["2", "Gadget B", "Electronics", "$25.50", ""],
        ["3", "Tool C", "Hardware", "$7.25", "50"],
        ["4", "Part D", "Components", "", "200"],
        ["5", "Device E", "Electronics", "$99.99", "12"],
        ["6", "Supply F", "Materials", "$3.00", "500"],
        ["7", "Item G", "Misc", "$15.75", ""],
    ]

    col_w = 35
    x_start = 15
    y_start = 30

    # Header row
    pdf.set_font("Helvetica", "B", 10)
    for j, h in enumerate(headers):
        x = x_start + j * col_w
        pdf.rect(x, y_start, col_w, 10)
        pdf.set_xy(x + 1, y_start + 2)
        pdf.cell(col_w - 2, 6, h)

    # Data rows
    pdf.set_font("Helvetica", size=10)
    for i, row in enumerate(data):
        y = y_start + 10 + i * 10
        for j, cell in enumerate(row):
            x = x_start + j * col_w
            pdf.rect(x, y, col_w, 10)
            pdf.set_xy(x + 1, y + 2)
            pdf.cell(col_w - 2, 6, cell)

    save(pdf, "table_lattice.pdf")


# ---------- 4. table_borderless.pdf ----------
def gen_table_borderless():
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=10)

    headers = ["ID", "Name", "Category", "Price", "Stock"]
    data = [
        ["1", "Widget A", "Hardware", "$10.00", "100"],
        ["2", "Gadget B", "Electronics", "$25.50", ""],
        ["3", "Tool C", "Hardware", "$7.25", "50"],
        ["4", "Part D", "Components", "", "200"],
        ["5", "Device E", "Electronics", "$99.99", "12"],
        ["6", "Supply F", "Materials", "$3.00", "500"],
        ["7", "Item G", "Misc", "$15.75", ""],
    ]

    col_w = 35
    x_start = 15
    y_start = 30

    # Header row (bold, no borders)
    pdf.set_font("Helvetica", "B", 10)
    for j, h in enumerate(headers):
        pdf.set_xy(x_start + j * col_w + 1, y_start + 2)
        pdf.cell(col_w - 2, 6, h)

    # Data rows (no borders)
    pdf.set_font("Helvetica", size=10)
    for i, row in enumerate(data):
        y = y_start + 10 + i * 10
        for j, cell in enumerate(row):
            pdf.set_xy(x_start + j * col_w + 1, y + 2)
            pdf.cell(col_w - 2, 6, cell)

    save(pdf, "table_borderless.pdf")


# ---------- 5. table_merged_cells.pdf ----------
def gen_table_merged_cells():
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=10)

    x0, y0 = 15, 30
    cw, rh = 40, 12

    # Row 0: merged header spanning 4 cols
    pdf.set_font("Helvetica", "B", 12)
    pdf.rect(x0, y0, cw * 4, rh)
    pdf.set_xy(x0 + 1, y0 + 2)
    pdf.cell(cw * 4 - 2, 8, "Quarterly Report")

    # Row 1: sub-headers
    pdf.set_font("Helvetica", "B", 10)
    sub_headers = ["Region", "Q1", "Q2", "Q3"]
    for j, h in enumerate(sub_headers):
        x = x0 + j * cw
        pdf.rect(x, y0 + rh, cw, rh)
        pdf.set_xy(x + 1, y0 + rh + 2)
        pdf.cell(cw - 2, 8, h)

    # Rows 2-4: data with one merged cell (Region spans 2 rows)
    pdf.set_font("Helvetica", size=10)
    # "North" spans rows 2-3
    pdf.rect(x0, y0 + 2 * rh, cw, 2 * rh)
    pdf.set_xy(x0 + 1, y0 + 2 * rh + 2)
    pdf.cell(cw - 2, 8, "North")

    row2_data = ["100", "150", "200"]
    for j, v in enumerate(row2_data):
        x = x0 + (j + 1) * cw
        pdf.rect(x, y0 + 2 * rh, cw, rh)
        pdf.set_xy(x + 1, y0 + 2 * rh + 2)
        pdf.cell(cw - 2, 8, v)

    row3_data = ["110", "160", "210"]
    for j, v in enumerate(row3_data):
        x = x0 + (j + 1) * cw
        pdf.rect(x, y0 + 3 * rh, cw, rh)
        pdf.set_xy(x + 1, y0 + 3 * rh + 2)
        pdf.cell(cw - 2, 8, v)

    # Row 4: "South"
    row4 = ["South", "200", "250", "300"]
    for j, v in enumerate(row4):
        x = x0 + j * cw
        pdf.rect(x, y0 + 4 * rh, cw, rh)
        pdf.set_xy(x + 1, y0 + 4 * rh + 2)
        pdf.cell(cw - 2, 8, v)

    save(pdf, "table_merged_cells.pdf")


# ---------- 6. cjk_mixed.pdf ----------
def gen_cjk_mixed():
    pdf = FPDF()
    pdf.add_page()
    pdf.set_font("Helvetica", size=14)

    # We use Latin text placeholders since Helvetica doesn't support CJK.
    # The test value is in having a multi-script-aware PDF structure.
    pdf.cell(0, 10, "Chinese: [CJK-Chinese-Placeholder]", new_x="LEFT", new_y="NEXT")
    pdf.cell(0, 10, "Japanese: [CJK-Japanese-Placeholder]", new_x="LEFT", new_y="NEXT")
    pdf.cell(0, 10, "Korean: [CJK-Korean-Placeholder]", new_x="LEFT", new_y="NEXT")
    pdf.cell(0, 10, "Mixed: Hello World CJK Test 123", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)
    pdf.cell(0, 10, "CJK placeholder - test file for Unicode extraction", new_x="LEFT", new_y="NEXT")

    save(pdf, "cjk_mixed.pdf")


# ---------- 7. rotated_pages.pdf ----------
def gen_rotated_pages():
    pdf = FPDF()

    rotations = [0, 90, 180, 270]
    for r in rotations:
        pdf.add_page()
        pdf.set_font("Helvetica", size=14)
        pdf.cell(0, 10, f"This page has rotation = {r} degrees")

    save(pdf, "rotated_pages.pdf")

    # Post-process to inject /Rotate into each page dictionary
    path = os.path.join(OUTDIR, "rotated_pages.pdf")
    _set_page_rotations(path, rotations)


def _set_page_rotations(path: str, rotations: list):
    """Inject /Rotate into page dictionaries using pypdf (or pikepdf)."""
    try:
        from pypdf import PdfReader, PdfWriter
        from pypdf.generic import NameObject, NumberObject
        reader = PdfReader(path)
        writer = PdfWriter()
        for i, page in enumerate(reader.pages):
            if i < len(rotations):
                page[NameObject("/Rotate")] = NumberObject(rotations[i])
            writer.add_page(page)
        with open(path, "wb") as f:
            writer.write(f)
        print(f"  -> Set /Rotate values: {rotations}")
    except ImportError:
        try:
            import pikepdf
            with pikepdf.open(path, allow_overwriting_input=True) as pdf_obj:
                for i, rot in enumerate(rotations):
                    if i < len(pdf_obj.pages):
                        pdf_obj.pages[i].Rotate = rot
                pdf_obj.save(path)
            print(f"  -> Set /Rotate values: {rotations}")
        except ImportError:
            print("  -> Neither pypdf nor pikepdf available; /Rotate not set (text indicates rotation)")


# ---------- 8. multi_font.pdf ----------
def gen_multi_font():
    pdf = FPDF()
    pdf.add_page()

    # Title: 24pt bold
    pdf.set_font("Helvetica", "B", 24)
    pdf.cell(0, 14, "Document Title", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)

    # Subtitle: 14pt italic
    pdf.set_font("Helvetica", "I", 14)
    pdf.cell(0, 10, "A subtitle in italic style", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)

    # Body: 12pt regular
    pdf.set_font("Helvetica", size=12)
    pdf.multi_cell(0, 6, "This is the body text in regular 12pt Helvetica. It contains multiple sentences to provide enough characters for font analysis. The quick brown fox jumps over the lazy dog.")
    pdf.ln(4)

    # Code: 10pt Courier
    pdf.set_font("Courier", size=10)
    pdf.multi_cell(0, 5, "def hello():\n    print('Hello, World!')\n    return 42")

    save(pdf, "multi_font.pdf")


# ---------- 9. long_document.pdf ----------
def gen_long_document():
    pdf = FPDF()
    pdf.set_auto_page_break(auto=False)

    for page_num in range(1, 6):
        pdf.add_page()

        # Header
        pdf.set_font("Helvetica", "B", 10)
        pdf.cell(0, 6, f"Long Document - Page {page_num} of 5", new_x="LEFT", new_y="NEXT")
        pdf.line(10, pdf.get_y(), pdf.w - 10, pdf.get_y())
        pdf.ln(4)

        # Body: 15 lines (fewer to avoid overflow)
        pdf.set_font("Helvetica", size=11)
        for line_num in range(1, 16):
            pdf.cell(0, 6, f"Page {page_num}, Line {line_num}: Lorem ipsum dolor sit amet.", new_x="LEFT", new_y="NEXT")

        # Footer at bottom
        pdf.set_y(-20)
        pdf.set_font("Helvetica", "I", 8)
        pdf.cell(0, 6, f"Footer - Page {page_num}", align="C")

    save(pdf, "long_document.pdf")


# ---------- 10. annotations_links.pdf ----------
def gen_annotations_links():
    pdf = FPDF()

    # Page 1: with a URI link
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)
    pdf.cell(0, 10, "Page 1: Links and Annotations", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)
    pdf.cell(0, 10, "Visit: https://example.com", new_x="LEFT", new_y="NEXT", link="https://example.com")
    pdf.ln(4)
    pdf.cell(0, 10, "This page has a hyperlink annotation above.", new_x="LEFT", new_y="NEXT")

    # Page 2: with text content
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)
    pdf.cell(0, 10, "Page 2: More content", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)
    pdf.multi_cell(0, 6, "This is the second page with body text. It serves as a target for internal navigation and bookmark testing.")

    # Page 3: additional content
    pdf.add_page()
    pdf.set_font("Helvetica", size=12)
    pdf.cell(0, 10, "Page 3: Final section", new_x="LEFT", new_y="NEXT")
    pdf.ln(4)
    pdf.multi_cell(0, 6, "The third page contains the conclusion. This PDF has metadata and link annotations for testing.")

    # Set document metadata
    pdf.set_title("Test Document with Annotations")
    pdf.set_author("pdfplumber-rs Test Suite")
    pdf.set_subject("PDF Fixture for Testing")
    pdf.set_creator("fpdf2 generate_fixtures.py")

    save(pdf, "annotations_links.pdf")


# ---------- Main ----------
def main():
    print("Generating PDF fixtures...")
    gen_basic_text()
    gen_multicolumn()
    gen_table_lattice()
    gen_table_borderless()
    gen_table_merged_cells()
    gen_cjk_mixed()
    gen_rotated_pages()
    gen_multi_font()
    gen_long_document()
    gen_annotations_links()
    print("Done! Generated 10 PDFs in tests/fixtures/generated/")


if __name__ == "__main__":
    main()
