"""Pytest configuration and shared fixtures for pdfplumber-py integration tests."""
import io
import struct
import pytest


def _minimal_pdf_bytes() -> bytes:
    """
    Build a minimal valid 1-page PDF in pure Python (no external deps).
    Page is 612x792 pt (US Letter), empty content stream.
    """
    # We'll build a minimal PDF by hand — the structure we need:
    # 1: catalog, 2: pages, 3: page, 4: content stream
    objects = {}

    content_stream = b""
    content_obj = b"4 0 obj\n<< /Length 0 >>\nstream\n\nendstream\nendobj\n"

    page_obj = (
        b"3 0 obj\n"
        b"<< /Type /Page /Parent 2 0 R "
        b"/MediaBox [0 0 612 792] "
        b"/Resources << >> "
        b"/Contents 4 0 R >>\n"
        b"endobj\n"
    )

    pages_obj = (
        b"2 0 obj\n"
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>\n"
        b"endobj\n"
    )

    catalog_obj = (
        b"1 0 obj\n"
        b"<< /Type /Catalog /Pages 2 0 R >>\n"
        b"endobj\n"
    )

    header = b"%PDF-1.7\n"
    body = catalog_obj + pages_obj + page_obj + content_obj

    # xref table
    offsets = []
    pos = len(header)
    for obj_bytes in [catalog_obj, pages_obj, page_obj, content_obj]:
        offsets.append(pos)
        pos += len(obj_bytes)

    xref_offset = len(header) + len(body)
    xref = b"xref\n0 5\n0000000000 65535 f \n"
    for off in offsets:
        xref += f"{off:010d} 00000 n \n".encode()

    trailer = (
        b"trailer\n<< /Size 5 /Root 1 0 R >>\n"
        b"startxref\n" + str(xref_offset).encode() + b"\n%%EOF\n"
    )

    return header + body + xref + trailer


@pytest.fixture(scope="session")
def minimal_pdf_bytes():
    return _minimal_pdf_bytes()


@pytest.fixture(scope="session")
def minimal_pdf_path(tmp_path_factory, minimal_pdf_bytes):
    p = tmp_path_factory.mktemp("pdfs") / "minimal.pdf"
    p.write_bytes(minimal_pdf_bytes)
    return str(p)
