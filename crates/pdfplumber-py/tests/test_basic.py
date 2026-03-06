"""
Integration tests for pdfplumber-py.

These tests run against the compiled extension module installed via `maturin develop`.
They exercise the Python API surface end-to-end — not the Rust unit tests.

Run with:
    maturin develop --features extension-module
    pytest crates/pdfplumber-py/tests/ -v
"""
import pytest


# ---------------------------------------------------------------------------
# Import guard — skip entire suite gracefully if the extension isn't built
# ---------------------------------------------------------------------------

pdfplumber = pytest.importorskip(
    "pdfplumber",
    reason="pdfplumber extension not built — run `maturin develop` first",
)


# ---------------------------------------------------------------------------
# Module metadata
# ---------------------------------------------------------------------------


def test_version_is_present():
    assert hasattr(pdfplumber, "__version__")
    assert isinstance(pdfplumber.__version__, str)
    parts = pdfplumber.__version__.split(".")
    assert len(parts) == 3, f"Expected semver, got {pdfplumber.__version__!r}"


def test_classes_are_exported():
    assert hasattr(pdfplumber, "PDF")
    assert hasattr(pdfplumber, "Page")
    assert hasattr(pdfplumber, "Table")
    assert hasattr(pdfplumber, "CroppedPage")


def test_exception_classes_are_exported():
    for name in [
        "PdfParseError",
        "PdfIoError",
        "PdfFontError",
        "PdfInterpreterError",
        "PdfResourceLimitError",
        "PdfPasswordRequired",
        "PdfInvalidPassword",
    ]:
        assert hasattr(pdfplumber, name), f"Missing exception class: {name}"


# ---------------------------------------------------------------------------
# PDF.open_bytes
# ---------------------------------------------------------------------------


def test_open_bytes_returns_pdf(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    assert pdf is not None


def test_open_bytes_invalid_raises():
    with pytest.raises(Exception):
        pdfplumber.PDF.open_bytes(b"this is not a pdf")


# ---------------------------------------------------------------------------
# PDF.open (file path)
# ---------------------------------------------------------------------------


def test_open_file_returns_pdf(minimal_pdf_path):
    pdf = pdfplumber.PDF.open(minimal_pdf_path)
    assert pdf is not None


def test_open_nonexistent_file_raises():
    with pytest.raises(Exception):
        pdfplumber.PDF.open("/nonexistent/path/file.pdf")


# ---------------------------------------------------------------------------
# pages property
# ---------------------------------------------------------------------------


def test_pages_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    pages = pdf.pages
    assert isinstance(pages, list)
    assert len(pages) == 1


def test_page_is_page_instance(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    page = pdf.pages[0]
    assert isinstance(page, pdfplumber.Page)


# ---------------------------------------------------------------------------
# metadata
# ---------------------------------------------------------------------------


def test_metadata_is_dict(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    meta = pdf.metadata
    assert isinstance(meta, dict)


def test_metadata_has_standard_keys(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    meta = pdf.metadata
    for key in ["title", "author", "subject", "keywords", "creator", "producer",
                "creation_date", "mod_date"]:
        assert key in meta, f"metadata missing key: {key}"


# ---------------------------------------------------------------------------
# bookmarks
# ---------------------------------------------------------------------------


def test_bookmarks_is_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    bm = pdf.bookmarks()
    assert isinstance(bm, list)


# ---------------------------------------------------------------------------
# Page properties
# ---------------------------------------------------------------------------


def test_page_number(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    page = pdf.pages[0]
    assert page.page_number == 0


def test_page_dimensions(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    page = pdf.pages[0]
    assert abs(page.width - 612.0) < 0.5
    assert abs(page.height - 792.0) < 0.5


# ---------------------------------------------------------------------------
# Page extraction methods (empty page — verify types and no crashes)
# ---------------------------------------------------------------------------


def test_chars_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].chars()
    assert isinstance(result, list)


def test_extract_text_returns_str(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].extract_text()
    assert isinstance(result, str)


def test_extract_text_layout_returns_str(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].extract_text(layout=True)
    assert isinstance(result, str)


def test_extract_words_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].extract_words()
    assert isinstance(result, list)


def test_extract_words_tolerances(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].extract_words(x_tolerance=5.0, y_tolerance=5.0)
    assert isinstance(result, list)


def test_lines_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].lines()
    assert isinstance(result, list)


def test_rects_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].rects()
    assert isinstance(result, list)


def test_curves_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].curves()
    assert isinstance(result, list)


def test_images_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].images()
    assert isinstance(result, list)


def test_find_tables_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].find_tables()
    assert isinstance(result, list)


def test_extract_tables_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].extract_tables()
    assert isinstance(result, list)


def test_search_returns_list(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].search("hello")
    assert isinstance(result, list)


def test_search_literal(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].search("hello", regex=False, case=False)
    assert isinstance(result, list)


# ---------------------------------------------------------------------------
# crop / within_bbox / outside_bbox
# ---------------------------------------------------------------------------


def test_crop_returns_cropped_page(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    page = pdf.pages[0]
    cropped = page.crop((0.0, 0.0, 306.0, 396.0))
    assert isinstance(cropped, pdfplumber.CroppedPage)


def test_crop_dimensions(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 306.0, 396.0))
    assert abs(cropped.width - 306.0) < 0.5
    assert abs(cropped.height - 396.0) < 0.5


def test_within_bbox_returns_cropped_page(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].within_bbox((0.0, 0.0, 306.0, 396.0))
    assert isinstance(result, pdfplumber.CroppedPage)


def test_outside_bbox_returns_cropped_page(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    result = pdf.pages[0].outside_bbox((100.0, 100.0, 200.0, 200.0))
    assert isinstance(result, pdfplumber.CroppedPage)


# ---------------------------------------------------------------------------
# CroppedPage methods
# ---------------------------------------------------------------------------


def test_cropped_page_chars(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.chars(), list)


def test_cropped_page_extract_text(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.extract_text(), str)


def test_cropped_page_extract_words(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.extract_words(), list)


def test_cropped_page_lines(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.lines(), list)


def test_cropped_page_rects(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.rects(), list)


def test_cropped_page_curves(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.curves(), list)


def test_cropped_page_images(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.images(), list)


def test_cropped_page_find_tables(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.find_tables(), list)


def test_cropped_page_extract_tables(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 400.0))
    assert isinstance(cropped.extract_tables(), list)


def test_cropped_page_further_crop(minimal_pdf_bytes):
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    cropped = pdf.pages[0].crop((0.0, 0.0, 400.0, 500.0))
    further = cropped.crop((0.0, 0.0, 200.0, 250.0))
    assert isinstance(further, pdfplumber.CroppedPage)
    assert abs(further.width - 200.0) < 0.5
    assert abs(further.height - 250.0) < 0.5


# ---------------------------------------------------------------------------
# Char dict keys (when chars are present in real fixture PDFs)
# ---------------------------------------------------------------------------


def test_char_dict_has_required_keys_if_present(minimal_pdf_bytes):
    """If any chars exist on a page, verify they carry the full expected schema."""
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    chars = pdf.pages[0].chars()
    required_keys = {
        "text", "x0", "top", "x1", "bottom",
        "fontname", "size", "doctop", "upright", "direction",
        "stroking_color", "non_stroking_color",
    }
    for ch in chars:
        assert required_keys <= set(ch.keys()), \
            f"Char dict missing keys: {required_keys - set(ch.keys())}"


def test_word_dict_has_required_keys_if_present(minimal_pdf_bytes):
    """If any words exist, verify the word dict schema."""
    pdf = pdfplumber.PDF.open_bytes(minimal_pdf_bytes)
    words = pdf.pages[0].extract_words()
    required_keys = {"text", "x0", "top", "x1", "bottom", "doctop", "direction"}
    for w in words:
        assert required_keys <= set(w.keys()), \
            f"Word dict missing keys: {required_keys - set(w.keys())}"
