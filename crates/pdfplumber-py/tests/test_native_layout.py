"""Installed-artifact contracts for the private native extension boundary."""

from __future__ import annotations

import io
import importlib.machinery
import inspect
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from types import MappingProxyType
import unittest
from unittest import mock

import pdfplumber
from pdfplumber import _native


class NativeLayoutTests(unittest.TestCase):
    @staticmethod
    def fixture() -> Path:
        return (
            Path(__file__).resolve().parents[3]
            / "tests"
            / "fixtures"
            / "generated"
            / "basic_text.pdf"
        )

    @staticmethod
    def multipage_fixture() -> Path:
        return (
            Path(__file__).resolve().parents[3]
            / "tests"
            / "fixtures"
            / "generated"
            / "long_document.pdf"
        )

    @staticmethod
    def password_fixture() -> Path:
        return (
            Path(__file__).resolve().parents[3]
            / "crates"
            / "pdfplumber"
            / "tests"
            / "fixtures"
            / "pdfs"
            / "password-example.pdf"
        )

    @staticmethod
    def compatibility_ligature_fixture() -> Path:
        return (
            Path(__file__).resolve().parents[3]
            / "compat"
            / "fixtures"
            / "upstream"
            / "pdfplumber-v0.11.10"
            / "tests"
            / "pdfs"
            / "line-char-render-example.pdf"
        )

    @staticmethod
    def annotation_unicode_fixture() -> Path:
        return NativeLayoutTests.annotation_fixture("annotations-unicode-issues.pdf")

    @staticmethod
    def annotation_fixture(name: str) -> Path:
        return (
            Path(__file__).resolve().parents[3]
            / "compat"
            / "fixtures"
            / "upstream"
            / "pdfplumber-v0.11.10"
            / "tests"
            / "pdfs"
            / name
        )

    @staticmethod
    def cyclic_metadata_pdf() -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>",
            b"<< /Loop 4 0 R >>",
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    "trailer\n<< /Root 1 0 R /Info 4 0 R /Size 5 >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def rich_metadata_pdf() -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>",
            (
                b"<< /Title (Hello) /Custom (custom) "
                b"/Encoded (bullet \\200) /Utf16 <FEFF004100E9> "
                b"/Named /Blue /Integer 7 /Real 2.5 /Boolean true "
                b"/Null null /List [(one) /Two 3 4.5 false null 5 0 R] "
                b"/Nested << /Inner (value) /Name /NameValue "
                b"/List [(inner) 9] >> /Raw#20Key (spaced) >>"
            ),
            b"(indirect)",
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    "trailer\n<< /Root 1 0 R /Info 4 0 R /Size 6 >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def normalized_rotation_pdf() -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            (
                b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 "
                b"/Rotate -90 >>"
            ),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] >>",
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] "
                b"/Rotate 450 >>"
            ),
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] "
                b"/Rotate 360 >>"
            ),
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def inherited_cropbox_pdf() -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            (
                b"<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 3 "
                b"/MediaBox [0 0 100 200] /Rotate 360 >>"
            ),
            (
                b"<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 5 0 R] /Count 2 "
                b"/MediaBox [0 0 100 200] /CropBox [10 20 90 180] "
                b"/Rotate -90 >>"
            ),
            b"<< /Type /Page /Parent 3 0 R >>",
            (
                b"<< /Type /Page /Parent 3 0 R /CropBox [90 180 10 20] "
                b"/Rotate 450 >>"
            ),
            b"<< /Type /Page /Parent 2 0 R >>",
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def build_inline_pdf(objects: tuple[bytes, ...]) -> bytes:
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @classmethod
    def single_page_rotation_pdf(cls, rotation: int) -> bytes:
        content = b"10 40 20 30 re S\nBT /F1 12 Tf 10 20 Td (ROTATE PAGE) Tj ET"
        return cls.build_inline_pdf(
            (
                b"<< /Type /Catalog /Pages 2 0 R >>",
                b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
                (
                    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200] "
                    + f"/Rotate {rotation} ".encode()
                    + b"/Resources << /Font << /F1 5 0 R >> >> "
                    b"/Contents 4 0 R >>"
                ),
                f"<< /Length {len(content)} >>\nstream\n".encode()
                + content
                + b"\nendstream",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            )
        )

    @classmethod
    def mixed_page_and_text_rotations_pdf(cls) -> bytes:
        content = (
            b"BT /F1 10 Tf "
            b"1 0 0 1 20 210 Tm (UP) Tj "
            b"0 1 -1 0 170 20 Tm (RIGHT) Tj "
            b"-1 0 0 -1 180 40 Tm (DOWN) Tj "
            b"0 -1 1 0 40 220 Tm (LEFT) Tj ET"
        )
        page_ids = (3, 5, 7, 9)
        objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R 9 0 R] /Count 4 >>",
        ]
        for page_id, rotation in zip(page_ids, (0, 90, 180, 270), strict=True):
            content_id = page_id + 1
            objects.extend(
                [
                    (
                        b"<< /Type /Page /Parent 2 0 R "
                        b"/MediaBox [0 0 200 240] "
                        + f"/Rotate {rotation} ".encode()
                        + b"/Resources << /Font << /F1 11 0 R >> >> "
                        + f"/Contents {content_id} 0 R >>".encode()
                    ),
                    f"<< /Length {len(content)} >>\nstream\n".encode()
                    + content
                    + b"\nendstream",
                ]
            )
        objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
        return cls.build_inline_pdf(tuple(objects))

    @classmethod
    def exact_geometry_pdf(cls) -> bytes:
        content = (
            b"0.75 w "
            b"10.25 20.5 30.75 40.125 re S "
            b"5.5 8.25 m 80.125 70.875 l S "
            b"20.5 30.25 m 40.75 90.125 100.5 90.125 120.25 30.25 c S "
            b"BT /F1 12.5 Tf 1 0 0 1 31.25 241.75 Tm (AB) Tj ET"
        )
        return cls.build_inline_pdf(
            (
                b"<< /Type /Catalog /Pages 2 0 R >>",
                b"<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
                (
                    b"<< /Type /Page /Parent 2 0 R "
                    b"/MediaBox [0 0 200.5 300.25] "
                    b"/Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R >>"
                ),
                f"<< /Length {len(content)} >>\nstream\n".encode()
                + content
                + b"\nendstream",
                (
                    b"<< /Type /Page /Parent 2 0 R "
                    b"/MediaBox [0 0 200.5 300.25] /Rotate 90 "
                    b"/Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>"
                ),
                f"<< /Length {len(content)} >>\nstream\n".encode()
                + content
                + b"\nendstream",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            )
        )

    @classmethod
    def split_level_page_tree_pdf(cls) -> bytes:
        contents = (
            b"BT /FRoot 12 Tf 30 40 Td (DEEP-A) Tj ET",
            b"BT /FRoot 12 Tf 30 80 Td (DEEP-B) Tj ET",
        )
        return cls.build_inline_pdf(
            (
                b"<< /Type /Catalog /Pages 2 0 R >>",
                (
                    b"<< /Type /Pages /Kids [3 0 R] /Count 2 /Rotate -90 "
                    b"/Resources 10 0 R >>"
                ),
                (
                    b"<< /Type /Pages /Parent 2 0 R /Kids [4 0 R] /Count 2 "
                    b"/MediaBox [10 20 110 220] >>"
                ),
                (
                    b"<< /Type /Pages /Parent 3 0 R /Kids [5 0 R 7 0 R] "
                    b"/Count 2 /CropBox [20 30 100 200] >>"
                ),
                b"<< /Type /Page /Parent 4 0 R /Contents 6 0 R >>",
                f"<< /Length {len(contents[0])} >>\nstream\n".encode()
                + contents[0]
                + b"\nendstream",
                b"<< /Type /Page /Parent 4 0 R /Contents 8 0 R >>",
                f"<< /Length {len(contents[1])} >>\nstream\n".encode()
                + contents[1]
                + b"\nendstream",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
                b"<< /Font << /FRoot 9 0 R >> >>",
            )
        )

    @classmethod
    def nearest_override_page_tree_pdf(cls) -> bytes:
        contents = (
            b"BT /FBranch 12 Tf 30 80 Td (BRANCH) Tj ET",
            b"BT /FPage 12 Tf 30 80 Td (PAGE) Tj ET",
            b"BT /FRoot 12 Tf 30 80 Td (ROOT) Tj ET",
        )
        return cls.build_inline_pdf(
            (
                b"<< /Type /Catalog /Pages 2 0 R >>",
                (
                    b"<< /Type /Pages /Kids [3 0 R 9 0 R] /Count 3 "
                    b"/MediaBox [0 0 100 200] /CropBox [5 10 95 190] "
                    b"/Rotate 90 /Resources 14 0 R >>"
                ),
                (
                    b"<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 7 0 R] "
                    b"/Count 2 /MediaBox [10 20 130 260] /Rotate 180 "
                    b"/Resources 15 0 R >>"
                ),
                (
                    b"<< /Type /Pages /Parent 3 0 R /Kids [5 0 R] /Count 1 "
                    b"/CropBox [20 40 120 240] >>"
                ),
                b"<< /Type /Page /Parent 4 0 R /Contents 6 0 R >>",
                f"<< /Length {len(contents[0])} >>\nstream\n".encode()
                + contents[0]
                + b"\nendstream",
                (
                    b"<< /Type /Page /Parent 3 0 R /MediaBox [0 0 140 280] "
                    b"/CropBox [30 50 130 270] /Rotate 270 /Resources 16 0 R "
                    b"/Contents 8 0 R >>"
                ),
                f"<< /Length {len(contents[1])} >>\nstream\n".encode()
                + contents[1]
                + b"\nendstream",
                b"<< /Type /Page /Parent 2 0 R /Contents 10 0 R >>",
                f"<< /Length {len(contents[2])} >>\nstream\n".encode()
                + contents[2]
                + b"\nendstream",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>",
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>",
                b"<< /Font << /FRoot 11 0 R >> >>",
                b"<< /Font << /FBranch 12 0 R >> >>",
                b"<< /Font << /FPage 13 0 R >> >>",
            )
        )

    @staticmethod
    def optional_page_box_pdf(box_name: bytes) -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            (
                b"<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 3 "
                b"/MediaBox [0 0 100 200] /Rotate 360 >>"
            ),
            (
                b"<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 5 0 R] /Count 2 "
                b"/MediaBox [0 0 100 200] /"
                + box_name
                + b" [10 20 90 180] "
                b"/Rotate -90 >>"
            ),
            b"<< /Type /Page /Parent 3 0 R >>",
            (
                b"<< /Type /Page /Parent 3 0 R /"
                + box_name
                + b" [90 180 10 20] /Rotate 450 >>"
            ),
            b"<< /Type /Page /Parent 2 0 R >>",
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @classmethod
    def trimbox_pdf(cls) -> bytes:
        return cls.optional_page_box_pdf(b"TrimBox")

    @classmethod
    def bleedbox_pdf(cls) -> bytes:
        return cls.optional_page_box_pdf(b"BleedBox")

    @classmethod
    def artbox_pdf(cls) -> bytes:
        return cls.optional_page_box_pdf(b"ArtBox")

    @staticmethod
    def nonzero_origin_rotated_pdf() -> bytes:
        objects = (
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 110 220] >>",
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 110 220] "
                b"/Rotate 90 >>"
            ),
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 110 220] "
                b"/Rotate 180 >>"
            ),
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [10 20 110 220] "
                b"/Rotate 270 >>"
            ),
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def negative_origin_rotated_rect_pdf() -> bytes:
        content = b"-90 -190 20 30 re S\n"
        objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R 9 0 R] /Count 4 >>",
        ]
        for index, rotation in enumerate((0, 90, 180, 270)):
            page_id = 3 + index * 2
            content_id = page_id + 1
            rotate = b"" if rotation == 0 else f" /Rotate {rotation}".encode()
            objects.extend(
                [
                    (
                        b"<< /Type /Page /Parent 2 0 R "
                        b"/MediaBox [-100 -200 100 200]"
                        + rotate
                        + f" /Contents {content_id} 0 R >>".encode()
                    ),
                    (
                        f"<< /Length {len(content)} >>\nstream\n".encode()
                        + content
                        + b"endstream"
                    ),
                ]
            )

        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def userunit_variants_pdf() -> bytes:
        content = b"10 20 20 30 re S\n"
        variants = (
            (b"", 0),
            (b"/UserUnit 2", 0),
            (b"/UserUnit 0.5", 0),
            (b"/UserUnit 4", 90),
            (b"/UserUnit 0", 0),
            (b"/UserUnit -2", 0),
            (b"/UserUnit /Bogus", 0),
            (b"/UserUnit (two)", 0),
            (b"/UserUnit [2]", 0),
            (b"/UserUnit << /V 2 >>", 0),
            (b"/UserUnit null", 0),
            (b"/UserUnit 27 0 R", 0),
        )
        page_ids = tuple(3 + index * 2 for index in range(len(variants)))
        kids = b" ".join(f"{page_id} 0 R".encode() for page_id in page_ids)
        objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            (
                b"<< /Type /Pages /Kids ["
                + kids
                + f"] /Count {len(variants)} /UserUnit 3 >>".encode()
            ),
        ]
        for page_id, (userunit, rotation) in zip(page_ids, variants, strict=True):
            content_id = page_id + 1
            userunit_entry = b" " + userunit if userunit else b""
            rotate = b"" if rotation == 0 else f" /Rotate {rotation}".encode()
            objects.extend(
                [
                    (
                        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 200]"
                        + userunit_entry
                        + rotate
                        + f" /Contents {content_id} 0 R >>".encode()
                    ),
                    (
                        f"<< /Length {len(content)} >>\nstream\n".encode()
                        + content
                        + b"endstream"
                    ),
                ]
            )
        objects.append(b"2")

        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def rust_extension_pdf() -> bytes:
        content = b"q 10 0 0 10 0 0 cm /Im0 Do Q\n"
        image = b"\x7f"
        objects = (
            (
                b"<< /Type /WrongCatalog /Pages 2 0 R /Outlines 5 0 R "
                b"/AcroForm 7 0 R >>"
            ),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            (
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] "
                b"/Resources << /XObject << /Im0 10 0 R >> >> "
                b"/Contents 4 0 R /Annots [8 0 R 9 0 R] >>"
            ),
            b"<< /Length %d >>\nstream\n" % len(content)
            + content
            + b"endstream",
            b"<< /Type /Outlines /First 6 0 R /Last 6 0 R /Count 1 >>",
            (
                b"<< /Title (Chapter 1) /Parent 5 0 R "
                b"/Dest [3 0 R /XYZ 0 100 null] >>"
            ),
            b"<< /Fields [8 0 R 9 0 R] >>",
            (
                b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) "
                b"/V (Alice) /DV (Default) /Rect [10 20 80 40] "
                b"/P 3 0 R /Ff 1 >>"
            ),
            (
                b"<< /Type /Annot /Subtype /Widget /FT /Sig /T (approval) "
                b"/Rect [10 50 80 70] /P 3 0 R /V 11 0 R >>"
            ),
            (
                b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 "
                b"/ColorSpace /DeviceGray /BitsPerComponent 8 /Length %d >>\n"
                b"stream\n" % len(image)
                + image
                + b"\nendstream"
            ),
            (
                b"<< /Type /Sig /Name (Signer) /M (D:20260820) "
                b"/Reason (Approval) /Location (Seoul) "
                b"/ContactInfo (signer@example.com) >>"
            ),
        )
        parts = [b"%PDF-1.4\n"]
        offsets = []
        for number, body in enumerate(objects, start=1):
            offsets.append(sum(map(len, parts)))
            parts.append(f"{number} 0 obj\n".encode() + body + b"\nendobj\n")
        xref_offset = sum(map(len, parts))
        parts.extend(
            [
                f"xref\n0 {len(objects) + 1}\n".encode(),
                b"0000000000 65535 f \n",
                *(f"{offset:010d} 00000 n \n".encode() for offset in offsets),
                (
                    f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\n"
                    f"startxref\n{xref_offset}\n%%EOF\n"
                ).encode(),
            ]
        )
        return b"".join(parts)

    @staticmethod
    def fake_ghostscript(directory: str) -> Path:
        executable = Path(directory) / "gs"
        executable.write_text(
            f"#!{sys.executable}\n"
            "import os\n"
            "import pathlib\n"
            "import sys\n"
            "arguments = os.environ.get('PDFPLUMBER_FAKE_GS_ARGS')\n"
            "if arguments:\n"
            "    pathlib.Path(arguments).write_text('\\n'.join(sys.argv[1:]))\n"
            "source = sys.argv[-1]\n"
            "payload = (sys.stdin.buffer.read() if source == '-' "
            "else pathlib.Path(source).read_bytes())\n"
            "if not payload.startswith(b'%PDF-'):\n"
            "    payload = b'%PDF-1.3\\n' + payload\n"
            "output = os.environ.get('PDFPLUMBER_FAKE_GS_OUTPUT')\n"
            "if output:\n"
            "    payload = pathlib.Path(output).read_bytes()\n"
            "sys.stdout.buffer.write(payload)\n"
        )
        executable.chmod(0o755)
        return executable

    def test_pdfplumber_is_a_python_package(self) -> None:
        self.assertEqual(pdfplumber.__name__, "pdfplumber")
        self.assertEqual(Path(pdfplumber.__file__).name, "__init__.py")
        self.assertIsNotNone(pdfplumber.__path__)

    def test_native_extension_has_private_qualified_identity(self) -> None:
        self.assertEqual(_native.__name__, "pdfplumber._native")
        self.assertEqual(_native.__package__, "pdfplumber")
        self.assertEqual(_native.__spec__.name, "pdfplumber._native")
        self.assertTrue(
            any(
                _native.__file__.endswith(suffix)
                for suffix in importlib.machinery.EXTENSION_SUFFIXES
            ),
            _native.__file__,
        )

    def test_native_exceptions_use_private_qualified_identity(self) -> None:
        self.assertEqual(_native.PdfParseError.__module__, "pdfplumber._native")
        self.assertEqual(_native.PdfInvalidPassword.__module__, "pdfplumber._native")

    def test_top_level_open_is_the_pdf_open_alias(self) -> None:
        self.assertEqual(pdfplumber.open, pdfplumber.PDF.open)
        document = pdfplumber.open(str(self.fixture()))
        try:
            self.assertIsInstance(document, pdfplumber.PDF)
            self.assertGreater(len(document.pages), 0)
        finally:
            document.close()

    def test_open_accepts_pathlib_path(self) -> None:
        document = pdfplumber.open(self.fixture())
        try:
            self.assertIsInstance(document, pdfplumber.PDF)
            self.assertGreater(len(document.pages), 0)
        finally:
            document.close()

    def test_open_accepts_seekable_binary_streams(self) -> None:
        payload = self.fixture().read_bytes()
        streams = (
            ("BytesIO", io.BytesIO(payload)),
            ("binary file", self.fixture().open("rb")),
        )
        for label, stream in streams:
            with self.subTest(stream=label):
                try:
                    stream.seek(7)
                    document = pdfplumber.open(stream)
                    self.assertIsInstance(document, pdfplumber.PDF)
                    self.assertGreater(len(document.pages), 0)
                    self.assertFalse(stream.closed)
                    self.assertEqual(stream.tell(), len(payload))
                finally:
                    stream.close()

    def test_open_exposes_upstream_resource_state(self) -> None:
        document = pdfplumber.open(self.fixture())
        self.assertEqual(document.path, self.fixture())
        self.assertIsNone(document.password)
        self.assertFalse(document.stream.closed)
        document.close()
        self.assertTrue(document.stream.closed)

        external = io.BytesIO(self.fixture().read_bytes())
        try:
            document = pdfplumber.open(external)
            self.assertIs(document.stream, external)
            self.assertIsNone(document.path)
            self.assertIsNone(document.password)
        finally:
            external.close()

    def test_document_repr_and_diagnostic_attributes_match_upstream(self) -> None:
        selected_pages = (1,)
        document = pdfplumber.open(self.fixture(), pages=selected_pages)
        try:
            cached_properties = getattr(pdfplumber.PDF, "cached_properties", None)
            actual = {
                "module": type(document).__module__,
                "qualname": type(document).__qualname__,
                "repr": re.sub(r"0x[0-9a-fA-F]+", "0xADDR", repr(document)),
                "str": re.sub(r"0x[0-9a-fA-F]+", "0xADDR", str(document)),
                "cached_properties": cached_properties,
                "cached_properties_class_identity": cached_properties
                is getattr(pdfplumber.PDF, "cached_properties", None),
                "cached_properties_instance_identity": cached_properties
                is getattr(document, "cached_properties", None),
                "pages_to_parse_identity": getattr(document, "pages_to_parse", None)
                is selected_pages,
                "stream_is_external": getattr(
                    document, "stream_is_external", None
                ),
            }
        finally:
            document.close()

        external = io.BytesIO(self.fixture().read_bytes())
        try:
            caller_owned = pdfplumber.open(external)
            try:
                actual["external_pages_to_parse"] = getattr(
                    caller_owned, "pages_to_parse", object()
                )
                actual["external_stream_is_external"] = getattr(
                    caller_owned, "stream_is_external", None
                )
            finally:
                caller_owned.close()
            actual["external_remains_open"] = not external.closed
        finally:
            external.close()

        self.maxDiff = None
        self.assertEqual(
            actual,
            {
                "module": "pdfplumber.pdf",
                "qualname": "PDF",
                "repr": "<pdfplumber.pdf.PDF object at 0xADDR>",
                "str": "<pdfplumber.pdf.PDF object at 0xADDR>",
                "cached_properties": [
                    "_rect_edges",
                    "_curve_edges",
                    "_edges",
                    "_objects",
                    "_pages",
                ],
                "cached_properties_class_identity": True,
                "cached_properties_instance_identity": True,
                "pages_to_parse_identity": True,
                "stream_is_external": False,
                "external_pages_to_parse": None,
                "external_stream_is_external": True,
                "external_remains_open": True,
            },
        )

    def test_rust_document_extensions_are_explicitly_namespaced(self) -> None:
        with pdfplumber.open(io.BytesIO(self.rust_extension_pdf())) as document:
            self.assertFalse(hasattr(pdfplumber.PDF, "bookmarks"))
            self.assertFalse(hasattr(document, "bookmarks"))
            self.assertIsInstance(document.rust, _native.RustPDF)
            self.assertEqual(type(document.rust).__module__, "pdfplumber._native")
            self.assertEqual(type(document.rust).__qualname__, "RustPDF")

    def test_rust_document_extensions_return_native_data(self) -> None:
        with pdfplumber.open(io.BytesIO(self.rust_extension_pdf())) as document:
            extensions = document.rust
            self.assertEqual(
                extensions.bookmarks(),
                [
                    {
                        "title": "Chapter 1",
                        "level": 0,
                        "page_number": 0,
                        "dest_top": 100.0,
                    }
                ],
            )
            self.assertEqual(
                extensions.form_fields(),
                [
                    {
                        "name": "name",
                        "field_type": "Text",
                        "value": "Alice",
                        "default_value": "Default",
                        "bbox": (10.0, 20.0, 80.0, 40.0),
                        "options": [],
                        "flags": 1,
                        "page_index": 0,
                    },
                    {
                        "name": "approval",
                        "field_type": "Signature",
                        "value": None,
                        "default_value": None,
                        "bbox": (10.0, 50.0, 80.0, 70.0),
                        "options": [],
                        "flags": 0,
                        "page_index": 0,
                    },
                ],
            )
            self.assertEqual(
                extensions.signatures(),
                [
                    {
                        "signer_name": "Signer",
                        "sign_date": "D:20260820",
                        "reason": "Approval",
                        "location": "Seoul",
                        "contact_info": "signer@example.com",
                        "is_signed": True,
                    }
                ],
            )
            self.assertEqual(
                extensions.validate(),
                [
                    {
                        "severity": "warning",
                        "code": "WRONG_CATALOG_TYPE",
                        "message": (
                            "catalog /Type is 'WrongCatalog' instead of 'Catalog'"
                        ),
                        "location": "object 1 0",
                    }
                ],
            )
            self.assertEqual(
                extensions.extract_images(0),
                [
                    {
                        "image": {
                            "object_type": "image",
                            "x0": 0.0,
                            "top": 90.0,
                            "x1": 10.0,
                            "bottom": 100.0,
                            "width": 10.0,
                            "height": 10.0,
                            "name": "Im0",
                            "src_width": 1,
                            "src_height": 1,
                            "bits_per_component": 8,
                            "color_space": "DeviceGray",
                        },
                        "data": b"\x7f",
                        "format": "raw",
                        "width": 1,
                        "height": 1,
                    }
                ],
            )

    def test_document_properties_match_observable_identity_policy(self) -> None:
        fixture = self.fixture()
        password = "".join(("identity-", "password-", "x" * 40))
        document = pdfplumber.open(fixture, password=password)
        metadata = document.metadata
        marker = object()
        metadata["__identity_marker__"] = marker
        serialized = document.to_dict(object_types=[])
        identity = {
            "stream": document.stream is document.stream,
            "path": document.path is document.path,
            "password": document.password is password,
            "metadata": document.metadata is metadata,
            "to_dict_metadata": serialized["metadata"] is metadata,
            "to_dict_mutation": serialized["metadata"].get("__identity_marker__")
            is marker,
            "pages": document.pages is document.pages,
            "objects": document.objects is document.objects,
            "annots_fresh": document.annots is not document.annots,
            "hyperlinks_fresh": document.hyperlinks is not document.hyperlinks,
            "structure_tree_fresh": document.structure_tree
            is not document.structure_tree,
        }
        document.flush_cache()
        identity["metadata_after_flush"] = document.metadata is metadata
        identity["mutation_after_flush"] = (
            document.metadata.get("__identity_marker__") is marker
        )
        document.close()
        identity["metadata_after_close"] = document.metadata is metadata
        identity["mutation_after_close"] = (
            document.metadata.get("__identity_marker__") is marker
        )

        self.maxDiff = None
        self.assertEqual(identity, dict.fromkeys(identity, True))

    def test_close_respects_stream_ownership_and_is_idempotent(self) -> None:
        document = pdfplumber.open(self.fixture())
        owned_stream = document.stream
        first_pages = document.pages
        first_page = first_pages[0]
        marker = object()
        first_pages.append(marker)

        self.assertIsNone(document.close())
        self.assertTrue(owned_stream.closed)
        second_pages = document.pages
        self.assertIsNot(second_pages, first_pages)
        self.assertIsNot(second_pages[0], first_page)
        self.assertNotIn(marker, second_pages)
        self.assertEqual(len(second_pages), 1)
        self.assertGreater(len(first_page.chars), 0)

        self.assertIsNone(document.close())
        third_pages = document.pages
        self.assertIsNot(third_pages, second_pages)
        self.assertIsNot(third_pages[0], second_pages[0])
        self.assertEqual(len(third_pages), 1)

        external = io.BytesIO(self.fixture().read_bytes())
        try:
            document = pdfplumber.open(external)
            self.assertIsNone(document.close())
            self.assertFalse(external.closed)
            self.assertIsNone(document.close())
        finally:
            external.close()

    def test_close_materializes_pages_before_closing_owned_stream(self) -> None:
        document = pdfplumber.open(self.fixture(), pages=1)
        owned_stream = document.stream
        try:
            with self.assertRaisesRegex(
                TypeError, "^argument of type 'int' is not iterable$"
            ):
                document.close()
            self.assertFalse(owned_stream.closed)
        finally:
            owned_stream.close()

    def test_flush_cache_matches_container_property_selection(self) -> None:
        document = pdfplumber.open(self.fixture())
        owned_stream = document.stream
        try:
            first_pages = document.pages
            first_page = first_pages[0]
            first_chars = first_page.chars
            marker = object()
            first_pages.append(marker)

            for properties in ([], ["_missing"], "_pages"):
                with self.subTest(properties=properties):
                    self.assertIsNone(document.flush_cache(properties))
                    self.assertIs(document.pages, first_pages)
                    self.assertIn(marker, document.pages)

            self.assertIsNone(document.flush_cache(["_pages"]))
            second_pages = document.pages
            self.assertIsNot(second_pages, first_pages)
            self.assertIsNot(second_pages[0], first_page)
            self.assertNotIn(marker, second_pages)
            self.assertEqual(first_page.chars, first_chars)
            self.assertFalse(owned_stream.closed)

            self.assertIsNone(document.flush_cache())
            third_pages = document.pages
            self.assertIsNot(third_pages, second_pages)
            self.assertIsNot(third_pages[0], second_pages[0])

            self.assertIsNone(document.flush_cache(None))
            fourth_pages = document.pages
            self.assertIsNot(fourth_pages, third_pages)

            with self.assertRaisesRegex(TypeError, "^'int' object is not iterable$"):
                document.flush_cache(1)
            self.assertIs(document.pages, fourth_pages)

            fourth_pages.append(marker)
            with self.assertRaisesRegex(
                TypeError, "^attribute name must be string, not 'int'$"
            ):
                document.flush_cache(["_pages", 1])
            self.assertIsNot(document.pages, fourth_pages)
            self.assertNotIn(marker, document.pages)
            self.assertFalse(owned_stream.closed)
        finally:
            document.close()

    def test_objects_aggregates_present_types_from_real_fixtures(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        cases = (
            (
                repository / "tests/fixtures/generated/basic_text.pdf",
                {"char": 258},
            ),
            (
                repository
                / "crates/pdfplumber/tests/fixtures/pdfs/table-curves-example.pdf",
                {"char": 1992, "rect": 208, "curve": 33},
            ),
            (
                repository / "tests/fixtures/real-world/images/inline-image.pdf",
                {"char": 22, "image": 1},
            ),
            (
                repository / "tests/fixtures/real-world/edge-cases/empty-page.pdf",
                {},
            ),
        )
        for fixture, expected_counts in cases:
            with self.subTest(fixture=fixture.name):
                with pdfplumber.open(fixture) as document:
                    objects = document.objects
                    self.assertEqual(list(objects), list(expected_counts))
                    self.assertEqual(
                        {kind: len(values) for kind, values in objects.items()},
                        expected_counts,
                    )
                    self.assertIs(document.objects, objects)

    def test_objects_cache_and_page_cache_invalidate_independently(self) -> None:
        document = pdfplumber.open(self.multipage_fixture(), pages=(3, 5))
        try:
            first_pages = document.pages
            expected_chars = [char for page in first_pages for char in page.chars]
            expected_lines = [line for page in first_pages for line in page.lines]
            first_objects = document.objects

            self.assertEqual(list(first_objects), ["char", "line"])
            self.assertEqual(first_objects["char"], expected_chars)
            self.assertEqual(first_objects["line"], expected_lines)
            self.assertEqual(
                {kind: len(values) for kind, values in first_objects.items()},
                {"char": 1386, "line": 2},
            )
            self.assertIs(document.objects["char"], first_objects["char"])

            marker = object()
            first_objects["char"].append(marker)
            document.flush_cache(["_pages"])
            self.assertIsNot(document.pages, first_pages)
            self.assertIs(document.objects, first_objects)
            self.assertIn(marker, document.objects["char"])

            second_pages = document.pages
            document.flush_cache(["_objects"])
            second_objects = document.objects
            self.assertIs(document.pages, second_pages)
            self.assertIsNot(second_objects, first_objects)
            self.assertNotIn(marker, second_objects["char"])
            self.assertEqual(second_objects["char"], expected_chars)
            self.assertEqual(second_objects["line"], expected_lines)

            document.flush_cache()
            self.assertIsNot(document.pages, second_pages)
            self.assertIsNot(document.objects, second_objects)
        finally:
            document.close()

    def test_context_manager_closes_only_owned_streams(self) -> None:
        document = pdfplumber.open(self.fixture())
        with document as entered:
            self.assertIs(entered, document)
            self.assertFalse(document.stream.closed)
        self.assertTrue(document.stream.closed)

        external = io.BytesIO(self.fixture().read_bytes())
        try:
            document = pdfplumber.open(external)
            with document as entered:
                self.assertIs(entered, document)
                self.assertIs(document.stream, external)
            self.assertFalse(external.closed)
        finally:
            external.close()

        document = pdfplumber.open(self.fixture())
        with self.assertRaisesRegex(RuntimeError, "context sentinel"):
            with document:
                raise RuntimeError("context sentinel")
        self.assertTrue(document.stream.closed)

    def test_open_selects_one_based_pages_in_document_order(self) -> None:
        fixture = self.multipage_fixture()
        with pdfplumber.open(fixture) as complete:
            all_text = [page.extract_text() for page in complete.pages]

        with pdfplumber.open(fixture, pages=(5, 3, 5, 0, -1, 99)) as selected:
            self.assertEqual(
                [page.extract_text() for page in selected.pages],
                [all_text[2], all_text[4]],
            )

        with pdfplumber.open(fixture, pages=[]) as selected:
            self.assertEqual(selected.pages, [])

        with pdfplumber.open(fixture, pages=["1"]) as selected:
            self.assertEqual(selected.pages, [])

        with pdfplumber.open(fixture, pages=[True]) as selected:
            self.assertEqual(
                [page.extract_text() for page in selected.pages], [all_text[0]]
            )

        selected = pdfplumber.open(fixture, pages=1)
        try:
            with self.assertRaisesRegex(TypeError, "not iterable"):
                _ = selected.pages
        finally:
            selected.stream.close()

    def test_empty_zero_page_and_invalid_selection_edges(self) -> None:
        from pdfplumber.utils.exceptions import PdfminerException

        empty = io.BytesIO(b"")
        try:
            with self.assertRaisesRegex(
                PdfminerException,
                r"^No /Root object! - Is this really a PDF\?$",
            ):
                pdfplumber.open(empty)
            self.assertEqual(empty.tell(), 0)
            self.assertFalse(empty.closed)
        finally:
            empty.close()

        repository = Path(__file__).resolve().parents[3]
        zero_page = (
            repository
            / "crates/pdfplumber/tests/fixtures/pdfs/issue-297-example.pdf"
        )
        with pdfplumber.open(zero_page) as document:
            pages = document.pages
            self.assertEqual(pages, [])
            self.assertIs(document.pages, pages)
            self.assertEqual(document.objects, {})
            self.assertEqual(document.annots, [])
            self.assertEqual(document.hyperlinks, [])
            self.assertEqual(document.structure_tree, [])
            self.assertEqual(
                document.to_dict(),
                {
                    "metadata": {
                        "Producer": "PyPDF2",
                        "Title": "IntMetadata",
                        "Copies": 0,
                    },
                    "pages": [],
                },
            )
            self.assertEqual(
                document.to_json(),
                '{"metadata": {"Producer": "PyPDF2", '
                '"Title": "IntMetadata", "Copies": 0}, "pages": []}',
            )
            self.assertEqual(
                document.to_csv(),
                "object_type,page_number,x0,x1,y0,y1,doctop,top,bottom,"
                "width,height\r\n",
            )

        with pdfplumber.open(
            self.multipage_fixture(), pages=(0, -1, 99, "1")
        ) as selected:
            self.assertEqual(selected.pages, [])

        scalar = pdfplumber.open(self.multipage_fixture(), pages=1)
        try:
            with self.assertRaisesRegex(
                TypeError, r"^argument of type 'int' is not iterable$"
            ):
                _ = scalar.pages
        finally:
            scalar.stream.close()

    def test_truncated_document_recovery_matches_upstream_boundary(self) -> None:
        from pdfplumber.utils.exceptions import PdfminerException

        complete = self.fixture().read_bytes()
        with pdfplumber.open(io.BytesIO(complete)) as baseline:
            expected_metadata = dict(baseline.metadata)
            expected_text = [page.extract_text() for page in baseline.pages]

        for removed_bytes in (1, 5, 10, 11, 20, 21):
            with self.subTest(recoverable_removed_bytes=removed_bytes):
                payload = complete[:-removed_bytes]
                stream = io.BytesIO(payload)
                try:
                    with pdfplumber.open(stream) as document:
                        self.assertEqual(dict(document.metadata), expected_metadata)
                        self.assertEqual(
                            [page.extract_text() for page in document.pages],
                            expected_text,
                        )
                    self.assertEqual(stream.tell(), len(payload))
                    self.assertFalse(stream.closed)
                finally:
                    stream.close()

        for removed_bytes in (12, 15, 19, 50, 500, len(complete) // 2):
            with self.subTest(rejected_removed_bytes=removed_bytes):
                payload = complete[:-removed_bytes]
                stream = io.BytesIO(payload)
                try:
                    with self.assertRaises(PdfminerException):
                        pdfplumber.open(stream)
                    self.assertEqual(stream.tell(), len(payload))
                    self.assertFalse(stream.closed)
                finally:
                    stream.close()

    def test_selected_pages_keep_original_numbers_and_selected_doctop(self) -> None:
        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            pages = document.pages

        self.assertEqual([page.page_number for page in pages], [3, 5])

        char_offsets = [
            page.chars[0]["doctop"] - page.chars[0]["top"] for page in pages
        ]
        word_offsets = [
            page.extract_words()[0]["doctop"] - page.extract_words()[0]["top"]
            for page in pages
        ]
        for offsets in (char_offsets, word_offsets):
            self.assertAlmostEqual(offsets[0], 0.0)
            self.assertAlmostEqual(offsets[1], pages[0].height)

    def test_page_initial_doctop_matches_full_and_selected_views(self) -> None:
        def snapshot(page: object) -> tuple[object, ...]:
            value = getattr(page, "initial_doctop", "MISSING")
            serialized = page.to_dict([])["initial_doctop"]
            return (
                page.page_number,
                value,
                type(value).__name__,
                serialized,
                type(serialized).__name__,
            )

        with pdfplumber.open(self.multipage_fixture()) as document:
            full = [snapshot(page) for page in document.pages]
        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            selected = [snapshot(page) for page in document.pages]

        self.assertEqual(
            full,
            [
                (1, 0, "int", 0, "int"),
                (2, 841.89, "float", 841.89, "float"),
                (3, 1683.78, "float", 1683.78, "float"),
                (4, 2525.67, "float", 2525.67, "float"),
                (5, 3367.56, "float", 3367.56, "float"),
            ],
        )
        self.assertEqual(
            selected,
            [
                (3, 0, "int", 0, "int"),
                (5, 841.89, "float", 841.89, "float"),
            ],
        )

    def test_page_rotation_is_inherited_normalized_and_serialized(self) -> None:
        with pdfplumber.open(io.BytesIO(self.normalized_rotation_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    getattr(page, "rotation", "MISSING"),
                    page.to_dict([])["rotation"],
                    page.width,
                    page.height,
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, 270, 270, 200, 100),
                (2, 90, 90, 200, 100),
                (3, 0, 0, 100, 200),
            ],
        )

    def test_page_mediabox_matches_rotation_aware_serialized_geometry(self) -> None:
        with pdfplumber.open(io.BytesIO(self.normalized_rotation_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    getattr(page, "mediabox", "MISSING"),
                    page.to_dict([])["mediabox"],
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, (0, 0, 200, 100), (0, 0, 200, 100)),
                (2, (0, 0, 200, 100), (0, 0, 200, 100)),
                (3, (0, 0, 100, 200), (0, 0, 100, 200)),
            ],
        )

    def test_page_cropbox_matches_inheritance_rotation_and_fallback(self) -> None:
        with pdfplumber.open(io.BytesIO(self.inherited_cropbox_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    getattr(page, "cropbox", "MISSING"),
                    page.to_dict([])["cropbox"],
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, (20, 10, 180, 90), (20, 10, 180, 90)),
                (2, (20, 10, 180, 90), (20, 10, 180, 90)),
                (3, (0, 0, 100, 200), (0, 0, 100, 200)),
            ],
        )

    def test_nested_page_tree_inherits_split_resources_boxes_and_rotation(self) -> None:
        with pdfplumber.open(io.BytesIO(self.split_level_page_tree_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    page.rotation,
                    page.mediabox,
                    page.cropbox,
                    page.bbox,
                    page.width,
                    page.height,
                    "".join(char["text"] for char in page.chars),
                    tuple(sorted({char["fontname"] for char in page.chars})),
                    page.to_dict([])["rotation"],
                    page.to_dict([])["mediabox"],
                    page.to_dict([])["cropbox"],
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (
                    1,
                    270,
                    (20, -10, 220, 90),
                    (30, 0, 200, 80),
                    (20, -10, 220, 90),
                    200,
                    100,
                    "DEEP-A",
                    ("Helvetica",),
                    270,
                    (20, -10, 220, 90),
                    (30, 0, 200, 80),
                ),
                (
                    2,
                    270,
                    (20, -10, 220, 90),
                    (30, 0, 200, 80),
                    (20, -10, 220, 90),
                    200,
                    100,
                    "DEEP-B",
                    ("Helvetica",),
                    270,
                    (20, -10, 220, 90),
                    (30, 0, 200, 80),
                ),
            ],
        )

    def test_nested_page_tree_prefers_nearest_inheritable_values(self) -> None:
        with pdfplumber.open(
            io.BytesIO(self.nearest_override_page_tree_pdf())
        ) as document:
            snapshots = [
                (
                    page.page_number,
                    page.rotation,
                    page.mediabox,
                    page.cropbox,
                    page.bbox,
                    page.width,
                    page.height,
                    "".join(char["text"] for char in page.chars),
                    tuple(sorted({char["fontname"] for char in page.chars})),
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (
                    1,
                    180,
                    (10, -20, 130, 220),
                    (20, 0, 120, 200),
                    (10, -20, 130, 220),
                    120,
                    240,
                    "BRANCH",
                    ("Courier",),
                ),
                (
                    2,
                    270,
                    (0, 0, 280, 140),
                    (50, 10, 270, 110),
                    (0, 0, 280, 140),
                    280,
                    140,
                    "PAGE",
                    ("Times-Roman",),
                ),
                (
                    3,
                    90,
                    (0, 0, 200, 100),
                    (10, 5, 190, 95),
                    (0, 0, 200, 100),
                    200,
                    100,
                    "ROOT",
                    ("Helvetica",),
                ),
            ],
        )

    def test_all_four_page_rotations_match_text_and_geometry(self) -> None:
        snapshots = []
        for rotation in (0, 90, 180, 270):
            with pdfplumber.open(
                io.BytesIO(self.single_page_rotation_pdf(rotation))
            ) as document:
                page = document.pages[0]
                rectangle = page.rects[0]
                serialized = page.to_dict(["rect"])
                snapshots.append(
                    (
                        rotation,
                        page.rotation,
                        page.bbox,
                        page.width,
                        page.height,
                        page.extract_text(),
                        "".join(char["text"] for char in page.chars),
                        tuple(sorted({char["upright"] for char in page.chars})),
                        tuple(word["text"] for word in page.extract_words()),
                        (
                            rectangle["x0"],
                            rectangle["top"],
                            rectangle["x1"],
                            rectangle["bottom"],
                        ),
                        serialized["rotation"],
                        serialized["bbox"],
                    )
                )

        self.assertEqual(
            snapshots,
            [
                (
                    0,
                    0,
                    (0, 0, 100, 200),
                    100,
                    200,
                    "ROTATE PAGE",
                    "ROTATE PAGE",
                    (True,),
                    ("ROTATE", "PAGE"),
                    (10, 130, 30, 160),
                    0,
                    (0, 0, 100, 200),
                ),
                (
                    90,
                    90,
                    (0, 0, 200, 100),
                    200,
                    100,
                    "ROTATE\nPAGE",
                    "ROTATE PAGE",
                    (False,),
                    ("ROTATE", "PAGE"),
                    (40, 10, 70, 30),
                    90,
                    (0, 0, 200, 100),
                ),
                (
                    180,
                    180,
                    (0, 0, 100, 200),
                    100,
                    200,
                    "EGAP ETATOR",
                    "ROTATE PAGE",
                    (True,),
                    ("EGAP", "ETATOR"),
                    (70, 40, 90, 70),
                    180,
                    (0, 0, 100, 200),
                ),
                (
                    270,
                    270,
                    (0, 0, 200, 100),
                    200,
                    100,
                    "EGAP\nETATOR",
                    "ROTATE PAGE",
                    (False,),
                    ("EGAP", "ETATOR"),
                    (130, 70, 160, 90),
                    270,
                    (0, 0, 200, 100),
                ),
            ],
        )

    def test_mixed_page_and_content_rotations_match_reading_semantics(self) -> None:
        with pdfplumber.open(
            io.BytesIO(self.mixed_page_and_text_rotations_pdf())
        ) as document:
            snapshots = []
            for page in document.pages:
                snapshots.append(
                    (
                        page.page_number,
                        page.rotation,
                        page.bbox,
                        page.width,
                        page.height,
                        page.initial_doctop,
                        page.extract_text(),
                        "".join(char["text"] for char in page.chars),
                        tuple(char["upright"] for char in page.chars),
                        tuple(
                            (word["text"], word["direction"])
                            for word in page.extract_words()
                        ),
                        page.chars[0]["doctop"],
                        page.to_dict([])["initial_doctop"],
                    )
                )

        self.assertEqual(
            snapshots,
            [
                (
                    1,
                    0,
                    (0, 0, 200, 240),
                    200,
                    240,
                    0,
                    "UP\nTHGIR\nNWOD\nLEFT",
                    "UPRIGHTDOWNLEFT",
                    (
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                    ),
                    (("UP", "ltr"), ("THGIR", "ttb"), ("NWOD", "ltr"), ("LEFT", "ttb")),
                    22.069999999999993,
                    0,
                ),
                (
                    2,
                    90,
                    (0, 0, 240, 200),
                    240,
                    200,
                    240,
                    "UP\nRIGHT\nNWOD\nTFEL",
                    "UPRIGHTDOWNLEFT",
                    (
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                    ),
                    (("UP", "ttb"), ("RIGHT", "ltr"), ("NWOD", "ttb"), ("TFEL", "ltr")),
                    260.0,
                    240,
                ),
                (
                    3,
                    180,
                    (0, 0, 200, 240),
                    200,
                    240,
                    440,
                    "PU\nRIGHT\nDOWN\nTFEL",
                    "UPRIGHTDOWNLEFT",
                    (
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                    ),
                    (("PU", "ltr"), ("RIGHT", "ttb"), ("DOWN", "ltr"), ("TFEL", "ttb")),
                    647.9300000000001,
                    440,
                ),
                (
                    4,
                    270,
                    (0, 0, 240, 200),
                    240,
                    200,
                    680,
                    "PU\nTHGIR\nDOWN\nLEFT",
                    "UPRIGHTDOWNLEFT",
                    (
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                        True,
                        False,
                        False,
                        False,
                        False,
                        True,
                        True,
                        True,
                        True,
                    ),
                    (("PU", "ttb"), ("THGIR", "ltr"), ("DOWN", "ttb"), ("LEFT", "ltr")),
                    852.78,
                    680,
                ),
            ],
        )

    def test_page_and_object_geometry_matches_exact_values(self) -> None:
        geometry_fields = (
            "x0",
            "y0",
            "x1",
            "y1",
            "top",
            "bottom",
            "width",
            "height",
            "doctop",
        )
        word_fields = ("x0", "x1", "top", "bottom", "width", "height", "doctop")

        with pdfplumber.open(io.BytesIO(self.exact_geometry_pdf())) as document:
            snapshots = []
            for page in document.pages:
                character = page.chars[0]
                word = page.extract_words()[0]
                line = page.lines[0]
                rectangle = page.rects[0]
                curve = page.curves[0]
                object_values = (
                    tuple(character.get(name, "MISSING") for name in geometry_fields),
                    tuple(word.get(name, "MISSING") for name in word_fields),
                    tuple(line.get(name, "MISSING") for name in geometry_fields),
                    tuple(rectangle.get(name, "MISSING") for name in geometry_fields),
                    tuple(curve.get(name, "MISSING") for name in geometry_fields),
                )
                snapshots.append(
                    (
                        page.page_number,
                        page.rotation,
                        page.bbox,
                        page.width,
                        page.height,
                        page.initial_doctop,
                        page.point2coord((12.125, 42.875)),
                        *object_values,
                        tuple(tuple(point) for point in curve["pts"]),
                    )
                )
                for values in object_values:
                    self.assertTrue(all(type(value) is float for value in values))
                self.assertTrue(
                    all(type(value) is float for point in curve["pts"] for value in point)
                )

        self.assertEqual(
            snapshots,
            [
                (
                    1,
                    0,
                    (0, 0.0, 200.5, 300.25),
                    200.5,
                    300.25,
                    0,
                    (12.125, 257.375),
                    (
                        31.25,
                        239.1625,
                        39.5875,
                        251.6625,
                        48.587500000000006,
                        61.087500000000006,
                        8.337499999999999,
                        12.5,
                        48.587500000000006,
                    ),
                    (
                        31.25,
                        47.925,
                        48.587500000000006,
                        61.087500000000006,
                        16.674999999999997,
                        12.5,
                        48.587500000000006,
                    ),
                    (5.5, 8.25, 80.125, 70.875, 229.375, 292.0, 74.625, 62.625, 229.375),
                    (10.25, 20.5, 41.0, 60.625, 239.625, 279.75, 30.75, 40.125, 239.625),
                    (20.5, 30.25, 120.25, 30.25, 270.0, 270.0, 99.75, 0.0, 270.0),
                    ((20.5, 270.0), (120.25, 270.0)),
                ),
                (
                    2,
                    90,
                    (0, 0.0, 300.25, 200.5),
                    300.25,
                    200.5,
                    300.25,
                    (12.125, 157.625),
                    (
                        239.1625,
                        160.9125,
                        251.6625,
                        169.25,
                        31.25,
                        39.587500000000006,
                        12.5,
                        8.337500000000006,
                        331.5,
                    ),
                    (
                        239.1625,
                        251.6625,
                        31.25,
                        47.92500000000001,
                        12.5,
                        16.67500000000001,
                        331.5,
                    ),
                    (8.25, 120.375, 70.875, 195.0, 5.5, 80.125, 62.625, 74.625, 305.75),
                    (20.5, 159.5, 60.625, 190.25, 10.25, 41.0, 40.125, 30.75, 310.5),
                    (30.25, 80.25, 30.25, 180.0, 20.5, 120.25, 0.0, 99.75, 320.75),
                    ((30.25, 20.5), (30.25, 120.25)),
                ),
            ],
        )

    def test_page_trimbox_matches_direct_presence_rotation_and_absence(self) -> None:
        with pdfplumber.open(io.BytesIO(self.trimbox_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    hasattr(page, "trimbox"),
                    getattr(page, "trimbox", "MISSING"),
                    "trimbox" in vars(page),
                    "trimbox" in page.to_dict([]),
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, False, "MISSING", False, False),
                (2, True, (20, 10, 180, 90), True, False),
                (3, False, "MISSING", False, False),
            ],
        )

    def test_page_bleedbox_matches_direct_presence_rotation_and_absence(self) -> None:
        with pdfplumber.open(io.BytesIO(self.bleedbox_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    hasattr(page, "bleedbox"),
                    getattr(page, "bleedbox", "MISSING"),
                    "bleedbox" in vars(page),
                    "bleedbox" in page.to_dict([]),
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, False, "MISSING", False, False),
                (2, True, (20, 10, 180, 90), True, False),
                (3, False, "MISSING", False, False),
            ],
        )

    def test_page_artbox_matches_direct_presence_rotation_and_absence(self) -> None:
        with pdfplumber.open(io.BytesIO(self.artbox_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    hasattr(page, "artbox"),
                    getattr(page, "artbox", "MISSING"),
                    "artbox" in vars(page),
                    "artbox" in page.to_dict([]),
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (1, False, "MISSING", False, False),
                (2, True, (20, 10, 180, 90), True, False),
                (3, False, "MISSING", False, False),
            ],
        )

    def test_page_bbox_dimensions_match_nonzero_origin_across_rotations(self) -> None:
        with pdfplumber.open(io.BytesIO(self.nonzero_origin_rotated_pdf())) as document:
            snapshots = [
                (
                    page.page_number,
                    page.rotation,
                    hasattr(page, "bbox"),
                    getattr(page, "bbox", "MISSING"),
                    "bbox" in vars(page),
                    page.width,
                    page.height,
                    page.to_dict([])["bbox"],
                    page.to_dict([])["width"],
                    page.to_dict([])["height"],
                )
                for page in document.pages
            ]

        self.assertEqual(
            snapshots,
            [
                (
                    1,
                    0,
                    True,
                    (10, -20, 110, 180),
                    True,
                    100,
                    200,
                    (10, -20, 110, 180),
                    100,
                    200,
                ),
                (
                    2,
                    90,
                    True,
                    (20, -10, 220, 90),
                    True,
                    200,
                    100,
                    (20, -10, 220, 90),
                    200,
                    100,
                ),
                (
                    3,
                    180,
                    True,
                    (10, -20, 110, 180),
                    True,
                    100,
                    200,
                    (10, -20, 110, 180),
                    100,
                    200,
                ),
                (
                    4,
                    270,
                    True,
                    (20, -10, 220, 90),
                    True,
                    200,
                    100,
                    (20, -10, 220, 90),
                    200,
                    100,
                ),
            ],
        )

    def test_page_geometry_numeric_types_preserve_source_numbers(self) -> None:
        def scalar_type(value: object) -> str:
            return type(value).__name__

        def bbox_types(value: object) -> tuple[str, ...]:
            return tuple(scalar_type(number) for number in value)

        def snapshot(page: object) -> dict[str, object]:
            serialized = page.to_dict([])
            json_value = json.loads(page.to_json(object_types=[]))
            result: dict[str, object] = {}
            for surface_name, surface in (
                (
                    "direct",
                    {
                        "initial_doctop": page.initial_doctop,
                        "bbox": page.bbox,
                        "mediabox": page.mediabox,
                        "cropbox": page.cropbox,
                        "width": page.width,
                        "height": page.height,
                    },
                ),
                ("dict", serialized),
                ("json", json_value),
            ):
                result[surface_name] = {
                    "initial_doctop": scalar_type(surface["initial_doctop"]),
                    "bbox": bbox_types(surface["bbox"]),
                    "mediabox": bbox_types(surface["mediabox"]),
                    "cropbox": bbox_types(surface["cropbox"]),
                    "width": scalar_type(surface["width"]),
                    "height": scalar_type(surface["height"]),
                }
            result["point2coord_int"] = bbox_types(page.point2coord((1, 2)))
            result["point2coord_mixed"] = bbox_types(page.point2coord((1, 2.5)))
            return result

        integer_box = ("int", "int", "int", "int")
        integer_surface = {
            "initial_doctop": "int",
            "bbox": integer_box,
            "mediabox": integer_box,
            "cropbox": integer_box,
            "width": "int",
            "height": "int",
        }
        integer_page = {
            "direct": integer_surface,
            "dict": integer_surface,
            "json": integer_surface,
            "point2coord_int": ("int", "int"),
            "point2coord_mixed": ("int", "float"),
        }
        with pdfplumber.open(
            io.BytesIO(self.normalized_rotation_pdf())
        ) as document:
            self.assertEqual(
                [snapshot(page) for page in document.pages],
                [integer_page, integer_page, integer_page],
            )

        mixed_box = ("int", "float", "float", "float")
        mixed_surface_int_doctop = {
            "initial_doctop": "int",
            "bbox": mixed_box,
            "mediabox": mixed_box,
            "cropbox": mixed_box,
            "width": "float",
            "height": "float",
        }
        mixed_surface_float_doctop = {
            **mixed_surface_int_doctop,
            "initial_doctop": "float",
        }
        with pdfplumber.open(io.BytesIO(self.exact_geometry_pdf())) as document:
            snapshots = [snapshot(page) for page in document.pages]
        self.assertEqual(
            snapshots,
            [
                {
                    "direct": mixed_surface_int_doctop,
                    "dict": mixed_surface_int_doctop,
                    "json": mixed_surface_int_doctop,
                    "point2coord_int": ("int", "float"),
                    "point2coord_mixed": ("int", "float"),
                },
                {
                    "direct": mixed_surface_float_doctop,
                    "dict": mixed_surface_float_doctop,
                    "json": mixed_surface_float_doctop,
                    "point2coord_int": ("int", "float"),
                    "point2coord_mixed": ("int", "float"),
                },
            ],
        )

        for factory, attribute in (
            (self.trimbox_pdf, "trimbox"),
            (self.bleedbox_pdf, "bleedbox"),
            (self.artbox_pdf, "artbox"),
        ):
            with self.subTest(attribute=attribute):
                with pdfplumber.open(io.BytesIO(factory())) as document:
                    self.assertEqual(bbox_types(getattr(document.pages[1], attribute)), integer_box)

        repository = Path(__file__).resolve().parents[3]
        real_fixture = (
            repository
            / "compat/fixtures/upstream/pdfplumber-v0.11.10/tests/pdfs/page-boxes-example.pdf"
        )
        with pdfplumber.open(real_fixture) as document:
            page = document.pages[0]
            self.assertEqual(bbox_types(page.bbox), mixed_box)
            self.assertEqual(bbox_types(page.mediabox), mixed_box)

        with pdfplumber.open(
            io.BytesIO(self.normalized_rotation_pdf()), pages=(2, 3)
        ) as document:
            self.assertEqual(
                [scalar_type(page.initial_doctop) for page in document.pages],
                ["int", "int"],
            )

    def test_negative_mediabox_origin_is_retained_for_rotated_content(self) -> None:
        with pdfplumber.open(io.BytesIO(self.negative_origin_rotated_rect_pdf())) as document:
            snapshots = []
            for page in document.pages:
                rectangle = page.to_dict(["rect"])["rects"][0]
                snapshots.append(
                    (
                        page.page_number,
                        page.rotation,
                        page.bbox,
                        page.width,
                        page.height,
                        (
                            rectangle["x0"],
                            rectangle["top"],
                            rectangle["x1"],
                            rectangle["bottom"],
                        ),
                    )
                )

        self.assertEqual(
            snapshots,
            [
                (1, 0, (-100, 200, 100, 600), 200, 400, (-90, 560, -70, 590)),
                (2, 90, (-200, 100, 200, 300), 400, 200, (-190, 110, -160, 130)),
                (3, 180, (-100, 200, 100, 600), 200, 400, (70, 210, 90, 240)),
                (4, 270, (-200, 100, 200, 300), 400, 200, (160, 270, 190, 290)),
            ],
        )

    def test_userunit_is_ignored_without_public_or_serialized_state(self) -> None:
        with pdfplumber.open(io.BytesIO(self.userunit_variants_pdf())) as document:
            snapshots = []
            for page in document.pages:
                rectangle = page.to_dict(["rect"])["rects"][0]
                snapshots.append(
                    (
                        page.page_number,
                        page.rotation,
                        page.bbox,
                        page.width,
                        page.height,
                        (
                            rectangle["x0"],
                            rectangle["top"],
                            rectangle["x1"],
                            rectangle["bottom"],
                        ),
                        hasattr(page, "userunit"),
                        "userunit" in vars(page),
                        "userunit" in page.to_dict([]),
                    )
                )

        unrotated = (
            0,
            (0, 0, 100, 200),
            100,
            200,
            (10, 150, 30, 180),
            False,
            False,
            False,
        )
        rotated = (
            90,
            (0, 0, 200, 100),
            200,
            100,
            (20, 10, 50, 30),
            False,
            False,
            False,
        )
        self.assertEqual(
            snapshots,
            [
                (page_number, *(rotated if page_number == 4 else unrotated))
                for page_number in range(1, 13)
            ],
        )

    def test_derived_pages_retain_root_and_immediate_parent_identity(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            original = document.pages[0]
            cropped = original.crop((0, 0, 300, 400))
            nested = cropped.crop((10, 10, 200, 300))
            within = nested.within_bbox((20, 20, 100, 100))
            outside = within.outside_bbox((30, 30, 50, 50))
            pages = [original, cropped, nested, within, outside]

            self.assertEqual(
                [page.is_original for page in pages],
                [True, False, False, False, False],
            )
            self.assertTrue(all(page.root_page is original for page in pages))
            self.assertFalse(hasattr(original, "parent_page"))
            self.assertIs(cropped.parent_page, original)
            self.assertIs(nested.parent_page, cropped)
            self.assertIs(within.parent_page, nested)
            self.assertIs(outside.parent_page, within)
            self.assertEqual(["root_page" in vars(page) for page in pages], [True] * 5)
            self.assertEqual(
                ["parent_page" in vars(page) for page in pages],
                [False, True, True, True, True],
            )
            self.assertEqual(
                ["is_original" in vars(page) for page in pages],
                [False] * 5,
            )
            self.assertIs(type(original).is_original, True)
            self.assertIs(type(cropped).is_original, False)

            marker = object()
            cropped.root_page = marker
            mutated_nested = cropped.crop((20, 20, 150, 250))
            self.assertIs(mutated_nested.root_page, marker)
            self.assertIs(mutated_nested.parent_page, cropped)

    def test_point2coord_uses_current_page_view_geometry(self) -> None:
        with pdfplumber.open(io.BytesIO(self.nonzero_origin_rotated_pdf())) as document:
            self.assertEqual(
                [page.point2coord((1, 2)) for page in document.pages],
                [(11, 178), (21, 88), (11, 178), (21, 88)],
            )

            original = document.pages[0]
            cropped = original.crop((20, 0, 80, 50))
            nested = cropped.crop((30, 10, 70, 30))
            self.assertEqual(
                list(inspect.signature(type(original).point2coord).parameters),
                ["self", "pt"],
            )
            self.assertEqual(
                list(inspect.signature(type(cropped).point2coord).parameters),
                ["self", "pt"],
            )
            self.assertEqual(cropped.point2coord((1, 2)), (11, 28))
            self.assertEqual(nested.point2coord((1, 2)), (11, -2))
            self.assertEqual(
                [
                    original.point2coord((1, 2)),
                    original.point2coord([1, 2]),
                    original.point2coord({0: 1, 1: 2}),
                    original.point2coord((True, False)),
                    original.point2coord(pt=(1, 2)),
                ],
                [(11, 178), (11, 178), (11, 178), (11, 180), (11, 178)],
            )
            self.assertEqual(
                ["mediabox" in vars(page) for page in (original, cropped, nested)],
                [True, True, True],
            )

            marker = (100, 200, 300, 400)
            original.mediabox = marker
            self.assertEqual(original.point2coord((1, 2)), (101, 398))
            self.assertEqual(cropped.point2coord((1, 2)), (11, 28))
            mutated_child = original.crop((20, 0, 80, 50))
            self.assertEqual(mutated_child.mediabox, marker)
            self.assertEqual(mutated_child.point2coord((1, 2)), (101, 248))
            cropped.mediabox = marker
            self.assertEqual(cropped.point2coord((1, 2)), (101, 248))
            self.assertEqual(nested.point2coord((1, 2)), (11, -2))
            mutated_nested = cropped.crop((30, 10, 70, 30))
            self.assertEqual(mutated_nested.mediabox, marker)
            self.assertEqual(mutated_nested.point2coord((1, 2)), (101, 218))

            for point in ((), (1,)):
                with self.subTest(point=point):
                    with self.assertRaisesRegex(IndexError, "^tuple index out of range$"):
                        original.point2coord(point)
            with self.assertRaisesRegex(TypeError, "^'int' object is not subscriptable$"):
                original.point2coord(1)
            with self.assertRaises(TypeError):
                original.point2coord((None, 2))
            with self.assertRaises(TypeError):
                original.point2coord("12")
            with self.assertRaisesRegex(
                TypeError,
                re.escape(
                    "Page.point2coord() missing 1 required positional argument: 'pt'"
                ),
            ):
                original.point2coord()
            with self.assertRaisesRegex(
                TypeError,
                re.escape(
                    "Page.point2coord() takes 2 positional arguments but 3 were given"
                ),
            ):
                original.point2coord((1, 2), (3, 4))
            with self.assertRaisesRegex(
                TypeError,
                re.escape(
                    "Page.point2coord() got multiple values for argument 'pt'"
                ),
            ):
                original.point2coord((1, 2), pt=(3, 4))
            with self.assertRaisesRegex(
                TypeError,
                re.escape(
                    "Page.point2coord() got an unexpected keyword argument 'point'"
                ),
            ):
                original.point2coord(point=(1, 2))

    def test_page_repr_matches_original_and_derived_page_numbers(self) -> None:
        for fixture in (self.fixture(), self.multipage_fixture()):
            with self.subTest(fixture=fixture.name):
                with pdfplumber.open(fixture) as document:
                    original = document.pages[0]
                    cropped = original.crop(original.bbox)
                    nested = cropped.crop((0, 0, cropped.width, cropped.height))
                    self.assertEqual(
                        [repr(original), repr(cropped), repr(nested)],
                        ["<Page:1>", "<Page:1>", "<Page:1>"],
                    )
                    self.assertEqual(str(original), "<Page:1>")
                    cropped.page_number = "custom"
                    mutated_nested = cropped.crop(
                        (0, 0, cropped.width, cropped.height)
                    )
                    self.assertEqual(
                        [repr(cropped), repr(mutated_nested)],
                        ["<Page:custom>", "<Page:custom>"],
                    )

        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            self.assertEqual(
                [repr(page) for page in document.pages],
                ["<Page:3>", "<Page:5>"],
            )
            selected_crop = document.pages[0].crop(document.pages[0].bbox)
            self.assertEqual(repr(selected_crop), "<Page:3>")

    def test_page_objects_are_lazily_cached_by_present_type(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        fixtures = (
            (self.fixture(), {}, ["char"], {"char": 258}),
            (
                self.multipage_fixture(),
                {"pages": (3,)},
                ["char", "line"],
                {"char": 693, "line": 1},
            ),
            (
                repository
                / "crates/pdfplumber/tests/fixtures/pdfs/table-curves-example.pdf",
                {},
                ["char", "rect", "curve"],
                {"char": 1992, "rect": 208, "curve": 33},
            ),
            (
                repository / "tests/fixtures/real-world/images/inline-image.pdf",
                {},
                ["char", "image"],
                {"char": 22, "image": 1},
            ),
            (
                repository / "tests/fixtures/real-world/edge-cases/empty-page.pdf",
                {},
                [],
                {},
            ),
        )

        for fixture, open_kwargs, expected_keys, expected_counts in fixtures:
            with self.subTest(fixture=fixture.name):
                with pdfplumber.open(fixture, **open_kwargs) as document:
                    page = document.pages[0]
                    self.assertNotIn("_objects", vars(page))
                    objects = page.objects
                    self.assertFalse(callable(objects))
                    self.assertEqual(list(objects), expected_keys)
                    self.assertEqual(
                        {key: len(value) for key, value in objects.items()},
                        expected_counts,
                    )
                    self.assertIs(page.objects, objects)
                    self.assertIs(vars(page)["_objects"], objects)
                    for key, values in objects.items():
                        self.assertIs(page.objects[key], values)
                    cropped_objects = page.crop(page.bbox).objects
                    serialized = page.to_dict(expected_keys)
                    json_value = json.loads(page.to_json(object_types=expected_keys))
                    for key in expected_keys:
                        for dictionaries in (
                            objects[key],
                            document.objects[key],
                            cropped_objects[key],
                            serialized[f"{key}s"],
                            json_value[f"{key}s"],
                        ):
                            self.assertEqual(
                                {value.get("object_type") for value in dictionaries},
                                {key},
                            )
                            self.assertEqual(
                                {value.get("page_number") for value in dictionaries},
                                {page.page_number},
                            )
                            self.assertTrue(
                                all(
                                    type(value.get("page_number")) is int
                                    for value in dictionaries
                                )
                            )
                    objects["marker"] = []
                    self.assertIn("marker", page.objects)

    def test_page_image_geometry_matches_exact_upstream_values(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        geometry_fields = (
            "x0",
            "x1",
            "y0",
            "y1",
            "top",
            "bottom",
            "width",
            "height",
            "doctop",
        )
        cases = (
            (
                repository / "tests/fixtures/real-world/images/inline-image.pdf",
                0,
                1,
                (200.0, 300.0, 400.0, 500.0, 292.0, 392.0, 100.0, 100.0, 292.0),
            ),
            (
                repository / "tests/fixtures/real-world/images/xobject-image.pdf",
                0,
                1,
                (100.0, 300.0, 400.0, 550.0, 242.0, 392.0, 200.0, 150.0, 242.0),
            ),
            (
                repository
                / "crates/pdfplumber/tests/fixtures/pdfs/issue-71-duplicate-chars.pdf",
                1,
                16,
                (
                    331.79100300000005,
                    338.24400306750005,
                    602.7019984325,
                    609.1549985,
                    232.84500149999997,
                    239.29800156750002,
                    6.4530000675,
                    6.4530000675000565,
                    1074.8450014999999,
                ),
            ),
        )

        for fixture, page_index, expected_count, expected_geometry in cases:
            with self.subTest(fixture=fixture.name, page=page_index + 1):
                with pdfplumber.open(fixture) as document:
                    page = document.pages[page_index]
                    document_images = [
                        image
                        for image in document.objects["image"]
                        if image["page_number"] == page.page_number
                    ]
                    image_lists = (
                        page.images,
                        page.crop(page.bbox).images,
                        document_images,
                        page.to_dict(["image"])["images"],
                        json.loads(page.to_json(object_types=["image"]))["images"],
                    )
                    self.assertTrue(
                        all(len(images) == expected_count for images in image_lists)
                    )
                    self.assertEqual(
                        tuple(
                            tuple(images[0].get(name, "MISSING") for name in geometry_fields)
                            for images in image_lists
                        ),
                        (expected_geometry,) * len(image_lists),
                    )
                    self.assertTrue(
                        all(
                            type(image[name]) is float
                            for images in image_lists
                            for image in images
                            for name in geometry_fields
                        )
                    )

    def test_cropped_page_objects_preserve_parent_keys_in_distinct_cache(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        fixture = (
            repository
            / "crates/pdfplumber/tests/fixtures/pdfs/table-curves-example.pdf"
        )
        with pdfplumber.open(fixture) as document:
            parent = document.pages[0]
            parent_objects = parent.objects
            cropped = parent.crop((0, 0, 20, 20))
            self.assertNotIn("_objects", vars(cropped))
            objects = cropped.objects
            self.assertEqual(list(objects), ["char", "rect", "curve"])
            self.assertEqual(
                {key: len(value) for key, value in objects.items()},
                {"char": 0, "rect": 0, "curve": 0},
            )
            self.assertIs(cropped.objects, objects)
            self.assertIs(vars(cropped)["_objects"], objects)
            for key in parent_objects:
                self.assertIsNot(objects[key], parent_objects[key])

            nested = cropped.crop((0, 0, 10, 10))
            self.assertEqual(list(nested.objects), ["char", "rect", "curve"])
            self.assertIsNot(nested.objects, objects)
            objects["marker"] = []
            self.assertIn("marker", cropped.objects)
            self.assertNotIn("marker", parent.objects)
            self.assertNotIn("marker", nested.objects)

    def test_derived_cache_materialization_snapshots_parent_containers(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            parent = document.pages[0]
            cropped = parent.crop(parent.bbox)
            within = parent.within_bbox(parent.bbox)
            outside = parent.outside_bbox((0, 0, 1, 1))
            parent_objects = parent.objects
            parent_char = parent_objects["char"][0]
            parent_char["text"] = "before-derived-cache"
            parent_char["mutable_marker"] = ["parent"]
            parent_objects["custom"] = [parent_char]

            cropped_objects = cropped.objects
            within_objects = within.objects
            outside_objects = outside.objects
            for objects in (cropped_objects, within_objects, outside_objects):
                self.assertEqual(list(objects), ["char", "custom"])
                self.assertEqual(objects["char"][0]["text"], "before-derived-cache")
                self.assertIsNot(objects, parent_objects)
                self.assertIsNot(objects["char"], parent_objects["char"])
                self.assertIsNot(objects["custom"], parent_objects["custom"])

            self.assertIsNot(cropped_objects["char"][0], parent_char)
            self.assertIsNot(cropped_objects["custom"][0], parent_char)
            self.assertIsNot(
                cropped_objects["char"][0],
                cropped_objects["custom"][0],
            )
            self.assertIs(within_objects["char"][0], parent_char)
            self.assertIs(within_objects["custom"][0], parent_char)
            self.assertIs(outside_objects["char"][0], parent_char)
            self.assertIs(outside_objects["custom"][0], parent_char)

            parent_char["text"] = "after-derived-cache"
            parent_char["mutable_marker"].append("shared")
            parent_objects["char"].append(dict(parent_char))
            self.assertEqual(cropped_objects["char"][0]["text"], "before-derived-cache")
            self.assertEqual(within_objects["char"][0]["text"], "after-derived-cache")
            self.assertEqual(outside_objects["char"][0]["text"], "after-derived-cache")
            self.assertEqual(
                cropped_objects["char"][0]["mutable_marker"],
                ["parent", "shared"],
            )
            self.assertEqual(
                [
                    len(parent_objects["char"]),
                    len(cropped_objects["char"]),
                    len(within_objects["char"]),
                    len(outside_objects["char"]),
                ],
                [259, 258, 258, 258],
            )

    def test_sibling_and_nested_derived_cache_identity_matches_upstream(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            parent = document.pages[0]
            first = parent.crop(parent.bbox)
            second = parent.crop(parent.bbox)
            first_objects = first.objects
            second_objects = second.objects
            full_bbox = (0, 0, first.width, first.height)
            nested_crop = first.crop(full_bbox)
            nested_within = first.within_bbox(full_bbox)
            nested_crop_objects = nested_crop.objects
            nested_within_objects = nested_within.objects

            first_char = first_objects["char"][0]
            self.assertIsNot(first_char, second_objects["char"][0])
            self.assertIsNot(first_char, nested_crop_objects["char"][0])
            self.assertIs(first_char, nested_within_objects["char"][0])
            first_char["nested_marker"] = True
            self.assertNotIn("nested_marker", second_objects["char"][0])
            self.assertNotIn("nested_marker", nested_crop_objects["char"][0])
            self.assertTrue(nested_within_objects["char"][0]["nested_marker"])

        repository = Path(__file__).resolve().parents[3]
        curve_fixture = (
            repository
            / "crates/pdfplumber/tests/fixtures/pdfs/table-curves-example.pdf"
        )
        with pdfplumber.open(curve_fixture) as document:
            parent = document.pages[0]
            cropped = parent.crop(parent.bbox)
            parent_curve = parent.curves[0]
            cropped_curve = cropped.curves[0]
            self.assertIsNot(parent_curve, cropped_curve)
            self.assertIs(parent_curve["pts"], cropped_curve["pts"])
            marker = object()
            parent_curve["pts"].append(marker)
            self.assertIs(cropped_curve["pts"][-1], marker)

    def test_page_object_lists_are_cache_backed_properties(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            page = document.pages[0]
            objects = page.objects
            names = ("chars", "lines", "rects", "curves", "images")
            first = {name: getattr(page, name) for name in names}
            second = {name: getattr(page, name) for name in names}
            self.assertEqual(
                {name: len(values) for name, values in first.items()},
                {"chars": 258, "lines": 0, "rects": 0, "curves": 0, "images": 0},
            )
            self.assertIs(first["chars"], objects["char"])
            self.assertIs(second["chars"], first["chars"])
            for name in names[1:]:
                self.assertIsNot(second[name], first[name])
            marker = {"object_type": "char", "text": "marker"}
            objects["char"].append(marker)
            self.assertIs(page.chars[-1], marker)
            self.assertIs(page.to_dict(["char"])["chars"], page.chars)
            self.assertIs(page.to_dict()["chars"], page.chars)
            with self.assertRaisesRegex(TypeError, "^'list' object is not callable$"):
                page.chars()

    def test_cropped_object_lists_reuse_only_present_parent_keys(self) -> None:
        repository = Path(__file__).resolve().parents[3]
        fixture = (
            repository
            / "crates/pdfplumber/tests/fixtures/pdfs/table-curves-example.pdf"
        )
        with pdfplumber.open(fixture) as document:
            cropped = document.pages[0].crop((0, 0, 20, 20))
            names = ("chars", "lines", "rects", "curves", "images")
            kinds = {
                "chars": "char",
                "lines": "line",
                "rects": "rect",
                "curves": "curve",
                "images": "image",
            }
            first = {name: getattr(cropped, name) for name in names}
            second = {name: getattr(cropped, name) for name in names}
            self.assertEqual([len(first[name]) for name in names], [0, 0, 0, 0, 0])
            for name in ("chars", "rects", "curves"):
                self.assertIs(first[name], cropped.objects[kinds[name]])
                self.assertIs(second[name], first[name])
            for name in ("lines", "images"):
                self.assertIsNot(second[name], first[name])

    def test_annotation_properties_remain_fresh_lists_and_dictionaries(self) -> None:
        fixture = self.annotation_fixture("issue-463-example.pdf")
        with pdfplumber.open(fixture) as document:
            page = document.pages[0]
            first_annots = page.annots
            second_annots = page.annots
            first_links = page.hyperlinks
            second_links = page.hyperlinks
            self.assertEqual([len(first_annots), len(first_links)], [2, 0])
            self.assertIsNot(first_annots, second_annots)
            self.assertIsNot(first_annots[0], second_annots[0])
            self.assertIsNot(first_links, second_links)
            self.assertFalse(callable(first_annots))
            self.assertFalse(callable(first_links))

    def test_open_accepts_and_validates_laparams(self) -> None:
        fixture = self.fixture()
        with pdfplumber.open(fixture, laparams={}) as defaults:
            default_text = defaults.pages[0].extract_text()
        all_options = {
            "line_overlap": 0.4,
            "char_margin": 1.5,
            "line_margin": 0.75,
            "word_margin": 0.2,
            "boxes_flow": None,
            "detect_vertical": True,
            "all_texts": True,
        }
        with pdfplumber.open(fixture, laparams=all_options) as tuned:
            self.assertEqual(tuned.pages[0].extract_text(), default_text)
        with pdfplumber.open(
            fixture, laparams=MappingProxyType({"line_margin": 0.75})
        ) as mapped:
            self.assertEqual(mapped.pages[0].extract_text(), default_text)

        with self.assertRaisesRegex(TypeError, "must be a mapping"):
            pdfplumber.open(fixture, laparams=1)
        with self.assertRaisesRegex(TypeError, "unexpected keyword argument 'unknown'"):
            pdfplumber.open(fixture, laparams={"unknown": 1})
        with self.assertRaisesRegex(ValueError, r"between -1 and \+1"):
            pdfplumber.open(fixture, laparams={"boxes_flow": 2.0})

    def test_laparams_exposes_cached_horizontal_layout_objects(self) -> None:
        property_names = (
            "textboxhorizontals",
            "textboxverticals",
            "textlinehorizontals",
            "textlineverticals",
        )
        with pdfplumber.open(self.fixture()) as document:
            page = document.pages[0]
            self.assertEqual(list(page.objects), ["char"])
            for name in property_names:
                self.assertEqual(getattr(page, name), [])
                self.assertIsNot(getattr(page, name), getattr(page, name))

        expected_texts = [
            "The quick brown fox jumps over the lazy dog.\n",
            'Special chars: "quotes", copyright ©, registered ®, section §, degree °, plus-minus ±\n',
            "Accented: café, naïve, résumé, über, piñata, à la carte\n",
            "Numbers: 0 1 2 3 4 5 6 7 8 9. Price: $1,234.56. Ratio: 3:1. Percent: 99.9%\n",
        ]
        expected_schema = [
            "x0",
            "y0",
            "x1",
            "y1",
            "width",
            "height",
            "object_type",
            "page_number",
            "text",
            "top",
            "bottom",
            "doctop",
        ]
        with pdfplumber.open(self.fixture(), laparams={}) as document:
            page = document.pages[0]
            objects = page.objects
            self.assertEqual(
                {kind: len(values) for kind, values in objects.items()},
                {"textboxhorizontal": 4, "textlinehorizontal": 4, "char": 258},
            )
            self.assertEqual(list(objects), list(document.objects))
            self.assertEqual(
                [item["text"] for item in objects["textboxhorizontal"]],
                expected_texts,
            )
            self.assertEqual(
                [item["text"] for item in objects["textlinehorizontal"]],
                expected_texts,
            )
            self.assertEqual(list(objects["textboxhorizontal"][0]), expected_schema)
            first_box = objects["textboxhorizontal"][0]
            self.assertEqual(
                {
                    key: first_box[key]
                    for key in ("x0", "y0", "y1", "height")
                },
                {"x0": 31.18, "y0": 798.956, "y1": 810.956, "height": 12.0},
            )
            self.assertEqual(first_box["width"], first_box["x1"] - first_box["x0"])
            self.assertIs(page.textboxhorizontals, objects["textboxhorizontal"])
            self.assertIs(page.textlinehorizontals, objects["textlinehorizontal"])
            self.assertIs(
                document.textboxhorizontals,
                document.objects["textboxhorizontal"],
            )
            for name in ("textboxverticals", "textlineverticals"):
                self.assertIsNot(getattr(page, name), getattr(page, name))
            self.assertEqual(
                list(page.to_dict()),
                [
                    "page_number",
                    "initial_doctop",
                    "rotation",
                    "cropbox",
                    "mediabox",
                    "bbox",
                    "width",
                    "height",
                    "textboxhorizontals",
                    "textlinehorizontals",
                    "chars",
                    "annots",
                ],
            )
            cropped = page.crop(page.bbox)
            self.assertEqual(
                {kind: len(values) for kind, values in cropped.objects.items()},
                {"textboxhorizontal": 4, "textlinehorizontal": 4, "char": 258},
            )
            self.assertIs(
                cropped.textboxhorizontals,
                cropped.objects["textboxhorizontal"],
            )

    def test_detect_vertical_layout_objects_keep_exact_type_tags(self) -> None:
        object_types = (
            "textboxhorizontal",
            "textboxvertical",
            "textlinehorizontal",
            "textlinevertical",
        )
        expected_counts = {object_type: 2 for object_type in object_types}
        with pdfplumber.open(
            io.BytesIO(self.mixed_page_and_text_rotations_pdf()),
            laparams={"detect_vertical": True},
        ) as document:
            for page in document.pages:
                serialized = page.to_dict(object_types)
                json_value = json.loads(page.to_json(object_types=object_types))
                sources = (
                    page.objects,
                    page.crop(page.bbox).objects,
                    {
                        object_type: serialized[f"{object_type}s"]
                        for object_type in object_types
                    },
                    {
                        object_type: json_value[f"{object_type}s"]
                        for object_type in object_types
                    },
                )
                for objects in sources:
                    self.assertEqual(
                        {
                            object_type: len(objects.get(object_type, []))
                            for object_type in object_types
                        },
                        expected_counts,
                    )
                    for object_type in object_types:
                        self.assertEqual(
                            {
                                value.get("object_type")
                                for value in objects[object_type]
                            },
                            {object_type},
                        )
                        self.assertEqual(
                            {
                                value.get("page_number")
                                for value in objects[object_type]
                            },
                            {page.page_number},
                        )
                        self.assertTrue(
                            all(
                                type(value.get("page_number")) is int
                                for value in objects[object_type]
                            )
                        )

            self.assertEqual(
                {
                    object_type: len(document.objects.get(object_type, []))
                    for object_type in object_types
                },
                {object_type: 8 for object_type in object_types},
            )
            for object_type in object_types:
                self.assertEqual(
                    {
                        value.get("object_type")
                        for value in document.objects[object_type]
                    },
                    {object_type},
                )
                self.assertEqual(
                    {
                        value.get("page_number")
                        for value in document.objects[object_type]
                    },
                    {1, 2, 3, 4},
                )
                self.assertTrue(
                    all(
                        type(value.get("page_number")) is int
                        for value in document.objects[object_type]
                    )
                )

    def test_laparams_line_margin_controls_textbox_grouping(self) -> None:
        fixture = (
            Path(__file__).resolve().parents[3]
            / "tests/fixtures/generated/multi_font.pdf"
        )
        expected_lines = [
            "Document Title\n",
            "A subtitle in italic style\n",
            "This  is  the  body  text  in  regular  12pt  Helvetica.  It  contains  multiple  sentences  to  provide  enough\n",
            "characters for font analysis. The quick brown fox jumps over the lazy dog.\n",
            "def hello():\n",
            "    print('Hello, World!')\n",
            "    return 42\n",
        ]
        expected_default_boxes = [
            expected_lines[0],
            expected_lines[1],
            expected_lines[2] + expected_lines[3],
            "".join(expected_lines[4:]),
        ]
        for line_margin, expected_boxes in (
            (0, expected_lines),
            (0.5, expected_default_boxes),
            (2, ["".join(expected_lines)]),
        ):
            with self.subTest(line_margin=line_margin):
                with pdfplumber.open(
                    fixture, laparams={"line_margin": line_margin}
                ) as document:
                    page = document.pages[0]
                    self.assertEqual(
                        [item["text"] for item in page.textlinehorizontals],
                        expected_lines,
                    )
                    self.assertEqual(
                        [item["text"] for item in page.textboxhorizontals],
                        expected_boxes,
                    )

    def test_laparams_preserves_page_and_document_layout_order(self) -> None:
        fixture = self.annotation_fixture("issue-1181.pdf")
        with pdfplumber.open(fixture) as document:
            self.assertEqual(list(document.pages[0].objects), ["rect", "char", "line"])
            self.assertEqual(list(document.objects), ["rect", "char", "line"])

        mixed_shape_fixture = self.annotation_fixture(
            "issue-71-duplicate-chars-2.pdf"
        )
        with pdfplumber.open(mixed_shape_fixture) as document:
            self.assertEqual(
                [list(page.objects) for page in document.pages[:3]],
                [
                    ["line", "char", "image", "curve", "rect"],
                    ["char", "rect", "line", "curve"],
                    ["char", "rect", "curve", "line"],
                ],
            )

        expected_lines = [
            "FooCol1\n",
            "FooCol2\n",
            "FooCol3\n",
            "BarCol1\n",
            "BarCol2\n",
            "BarCol3\n",
            "Foo4\n",
            "Foo7\n",
            "Foo5\n",
            "Foo8\n",
            "Foo10\n",
            "Foo11\n",
            "Foo6\n",
            "Foo9\n",
            "Foo12\n",
            "Bar4\n",
            "Bar7\n",
            "Bar5\n",
            "Bar8\n",
            "Bar10\n",
            "Bar11\n",
            "Bar6\n",
            "Bar9\n",
            "Bar12\n",
        ]
        expected_chars = "".join(line.rstrip("\n") for line in expected_lines)

        with pdfplumber.open(
            fixture,
            laparams={"detect_vertical": True},
        ) as document:
            for page in document.pages:
                self.assertEqual(
                    list(page.objects),
                    [
                        "textboxhorizontal",
                        "textlinehorizontal",
                        "char",
                        "rect",
                        "line",
                    ],
                )
                for object_type in ("textboxhorizontal", "textlinehorizontal"):
                    self.assertEqual(
                        [item["text"] for item in page.objects[object_type]],
                        expected_lines,
                    )
                self.assertEqual(
                    "".join(item["text"] for item in page.objects["char"]),
                    expected_chars,
                )
                self.assertEqual(
                    [item["text"] for item in page.to_dict()["chars"]],
                    list(expected_chars),
                )

            self.assertEqual(list(document.objects), list(document.pages[0].objects))
            self.assertEqual(
                [item["text"] for item in document.objects["textboxhorizontal"]],
                expected_lines * 2,
            )
            self.assertEqual(
                "".join(item["text"] for item in document.objects["char"]),
                expected_chars * 2,
            )
            self.assertEqual(
                [item["page_number"] for item in document.objects["char"]],
                [1] * len(expected_chars) + [2] * len(expected_chars),
            )
            serialized = json.loads(document.to_json())
            self.assertEqual(
                [item["text"] for item in serialized["pages"][0]["chars"]],
                list(expected_chars),
            )

        expected_geometric_lines = [
            *expected_lines[:6],
            "Foo4\n",
            "Foo5\n",
            "Foo6\n",
            "Bar4\n",
            "Bar5\n",
            "Bar6\n",
            "Foo7\n",
            "Foo8\n",
            "Foo9\n",
            "Bar7\n",
            "Bar8\n",
            "Bar9\n",
            "Foo10\n",
            "Foo11\n",
            "Foo12\n",
            "Bar10\n",
            "Bar11\n",
            "Bar12\n",
        ]
        with pdfplumber.open(
            fixture,
            laparams={"detect_vertical": True, "boxes_flow": None},
        ) as document:
            for page in document.pages:
                self.assertEqual(
                    [item["text"] for item in page.textboxhorizontals],
                    expected_geometric_lines,
                )
                self.assertEqual(
                    [item["text"] for item in page.textlinehorizontals],
                    expected_geometric_lines,
                )
                self.assertEqual(
                    "".join(item["text"] for item in page.chars),
                    "".join(line.rstrip("\n") for line in expected_geometric_lines),
                )

    def test_page_close_flushes_only_the_target_page_cache(self) -> None:
        with pdfplumber.open(self.fixture(), laparams={}) as document:
            pages = document.pages
            page = pages[0]
            first_objects = page.objects
            first_chars = page.chars
            document_objects = document.objects
            document_chars = document_objects["char"]
            cropped = page.crop(page.bbox)
            cropped_objects = cropped.objects
            first_objects["marker"] = []
            cropped_objects["marker"] = []

            self.assertIsNone(page.close())
            self.assertNotIn("_objects", vars(page))
            self.assertIsNot(page.objects, first_objects)
            self.assertIsNot(page.chars, first_chars)
            self.assertNotIn("marker", page.objects)
            self.assertIs(document.pages, pages)
            self.assertIs(document.pages[0], page)
            self.assertIs(document.objects, document_objects)
            self.assertIs(document.objects["char"], document_chars)
            self.assertIs(cropped.objects, cropped_objects)
            self.assertIn("marker", cropped.objects)
            self.assertFalse(document.stream.closed)

            second_objects = page.objects
            self.assertIsNone(page.close())
            self.assertIsNot(page.objects, second_objects)

            self.assertIsNone(cropped.close())
            self.assertIsNot(cropped.objects, cropped_objects)
            self.assertNotIn("marker", cropped.objects)
            self.assertFalse(document.stream.closed)

    def test_page_flush_cache_matches_selective_container_behavior(self) -> None:
        with pdfplumber.open(self.fixture(), laparams={}) as document:
            page = document.pages[0]
            cropped = page.crop(page.bbox)
            cached_properties = type(page).cached_properties
            self.assertEqual(
                cached_properties,
                ["_rect_edges", "_curve_edges", "_edges", "_objects", "_layout"],
            )
            self.assertIs(cached_properties, page.cached_properties)
            self.assertIs(cached_properties, type(cropped).cached_properties)
            self.assertIs(cached_properties, cropped.cached_properties)

            page_objects = page.objects
            cropped_objects = cropped.objects
            document_objects = document.objects
            page.marker = object()
            cropped.marker = object()

            for properties in ([], ["_missing"], "_objects"):
                with self.subTest(properties=properties):
                    self.assertIsNone(page.flush_cache(properties))
                    self.assertIs(page.objects, page_objects)

            self.assertIsNone(page.flush_cache(["marker"]))
            self.assertFalse(hasattr(page, "marker"))
            self.assertTrue(hasattr(cropped, "marker"))

            self.assertIsNone(page.flush_cache(["_objects"]))
            second_objects = page.objects
            self.assertIsNot(second_objects, page_objects)
            self.assertIs(cropped.objects, cropped_objects)
            self.assertIs(document.objects, document_objects)

            self.assertIsNone(page.flush_cache(["_layout"]))
            self.assertIs(page.objects, second_objects)
            self.assertIsNone(page.flush_cache())
            self.assertIsNot(page.objects, second_objects)
            self.assertIs(cropped.objects, cropped_objects)
            self.assertIs(document.objects, document_objects)
            self.assertFalse(document.stream.closed)

            self.assertIsNone(cropped.flush_cache(None))
            self.assertIsNot(cropped.objects, cropped_objects)
            self.assertTrue(hasattr(cropped, "marker"))
            self.assertFalse(document.stream.closed)

    def test_page_flush_cache_honors_dynamic_properties_and_error_order(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            page = document.pages[0]
            first_objects = page.objects

            with self.assertRaisesRegex(TypeError, "^'int' object is not iterable$"):
                page.flush_cache(1)
            self.assertIs(page.objects, first_objects)

            page.marker = object()
            with self.assertRaisesRegex(
                TypeError, "^attribute name must be string, not 'int'$"
            ):
                page.flush_cache(["_objects", 1, "marker"])
            self.assertIsNot(page.objects, first_objects)
            self.assertTrue(hasattr(page, "marker"))

            second_objects = page.objects
            self.assertIsNone(page.flush_cache(value for value in ["_objects"]))
            self.assertIsNot(page.objects, second_objects)

            third_objects = page.objects
            page.cached_properties = ["marker"]
            self.assertIsNone(page.flush_cache())
            self.assertFalse(hasattr(page, "marker"))
            self.assertIs(page.objects, third_objects)

            page.cached_properties = ["_objects"]
            self.assertIsNone(page.flush_cache(None))
            fourth_objects = page.objects
            self.assertIsNot(fourth_objects, third_objects)
            self.assertIsNone(page.close())
            self.assertIsNot(page.objects, fourth_objects)

    def test_open_uses_password_for_paths_and_external_streams(self) -> None:
        from pdfplumber.utils.exceptions import PdfminerException

        fixture = self.password_fixture()
        with pdfplumber.open(fixture, password="test") as document:
            self.assertEqual(document.password, "test")
            self.assertEqual(len(document.pages), 4)
            self.assertTrue(document.pages[0].extract_text())

        external = io.BytesIO(fixture.read_bytes())
        external.seek(10)
        with pdfplumber.open(external, password="test") as document:
            self.assertIs(document.stream, external)
            self.assertEqual(document.password, "test")
            self.assertEqual(len(document.pages), 4)
            self.assertEqual(external.tell(), len(external.getvalue()))
        self.assertFalse(external.closed)

        missing = io.BytesIO(fixture.read_bytes())
        with self.assertRaises(PdfminerException):
            pdfplumber.open(missing)
        self.assertFalse(missing.closed)

        wrong = io.BytesIO(fixture.read_bytes())
        with self.assertRaises(PdfminerException):
            pdfplumber.open(wrong, password="wrong")
        self.assertFalse(wrong.closed)

    def test_open_uses_upstream_exception_taxonomy(self) -> None:
        from pdfplumber.utils.exceptions import PdfminerException

        self.assertEqual(
            PdfminerException.__module__, "pdfplumber.utils.exceptions"
        )
        self.assertTrue(issubclass(PdfminerException, Exception))
        self.assertFalse(issubclass(PdfminerException, RuntimeError))

        missing = self.fixture().with_name("does-not-exist.pdf")
        with self.assertRaises(FileNotFoundError):
            pdfplumber.open(missing)
        with self.assertRaises(IsADirectoryError):
            pdfplumber.open(self.fixture().parent)

        malformed_inputs = (
            ("empty stream", io.BytesIO(b"")),
            ("garbage stream", io.BytesIO(b"not a pdf")),
        )
        for label, stream in malformed_inputs:
            with self.subTest(input=label):
                with self.assertRaisesRegex(
                    PdfminerException,
                    r"^No /Root object! - Is this really a PDF\?$",
                ):
                    pdfplumber.open(stream)
                self.assertFalse(stream.closed)

        closed = io.BytesIO(self.fixture().read_bytes())
        closed.close()
        with self.assertRaisesRegex(
            PdfminerException, r"^I/O operation on closed file\.$"
        ):
            pdfplumber.open(closed)

        encrypted = self.password_fixture().read_bytes()
        for password in (None, "", "wrong"):
            with self.subTest(password=password):
                stream = io.BytesIO(encrypted)
                with self.assertRaises(PdfminerException) as caught:
                    pdfplumber.open(stream, password=password)
                self.assertEqual(str(caught.exception), "")
                self.assertFalse(stream.closed)

        unsupported = encrypted.replace(b"/V 2", b"/V 9", 1)
        self.assertNotEqual(unsupported, encrypted)
        for password in (None, "test"):
            with self.subTest(unsupported_password=password):
                stream = io.BytesIO(unsupported)
                with self.assertRaises(PdfminerException) as caught:
                    pdfplumber.open(stream, password=password)
                self.assertTrue(str(caught.exception))
                self.assertFalse(stream.closed)

    def test_open_accepts_strict_metadata_and_rejects_cycles(self) -> None:
        with pdfplumber.open(self.fixture(), strict_metadata=False) as permissive:
            permissive_metadata = permissive.metadata
        with pdfplumber.open(self.fixture(), strict_metadata=True) as strict:
            self.assertEqual(strict.metadata, permissive_metadata)

        permissive_stream = io.BytesIO(self.cyclic_metadata_pdf())
        with pdfplumber.open(
            permissive_stream, strict_metadata=False
        ) as permissive_cycle:
            self.assertIsInstance(permissive_cycle.metadata, dict)
        self.assertFalse(permissive_stream.closed)

        strict_stream = io.BytesIO(self.cyclic_metadata_pdf())
        with self.assertRaisesRegex(
            RecursionError, r"^maximum recursion depth exceeded$"
        ):
            pdfplumber.open(strict_stream, strict_metadata=True)
        self.assertFalse(strict_stream.closed)

    def test_metadata_preserves_keys_decoded_values_and_nested_lists(self) -> None:
        with pdfplumber.open(io.BytesIO(self.rich_metadata_pdf())) as document:
            self.assertEqual(
                document.metadata,
                {
                    "Title": "Hello",
                    "Custom": "custom",
                    "Encoded": "bullet \N{BULLET}",
                    "Utf16": "A\N{LATIN SMALL LETTER E WITH ACUTE}",
                    "Named": "Blue",
                    "Integer": 7,
                    "Real": 2.5,
                    "Boolean": True,
                    "List": ["one", "Two", 3, 4.5, False, None, "indirect"],
                    "Nested": {
                        "Inner": "value",
                        "Name": "NameValue",
                        "List": ["inner", 9],
                    },
                    "Raw Key": "spaced",
                },
            )

        fixture = (
            Path(__file__).resolve().parents[3]
            / "compat"
            / "fixtures"
            / "upstream"
            / "pdfplumber-v0.11.10"
            / "tests"
            / "pdfs"
            / "issue-316-example.pdf"
        )
        with pdfplumber.open(fixture) as document:
            self.assertEqual(document.metadata["SPDF"], 1082)
            self.assertEqual(
                document.metadata["Changes"][1],
                {
                    "ModDate": "D:20061211145545-00'00'",
                    "Product": "APStripFiles",
                    "Version": "1.8",
                    "Vendor": "Appligent",
                    "Comments": (
                        "SPDF Build Number 1082 for Linux 7, "
                        "Application Build Date: Mar 26 2003"
                    ),
                },
            )

    def test_permissive_metadata_cycle_logs_and_retains_reference(self) -> None:
        stream = io.BytesIO(self.cyclic_metadata_pdf())
        with self.assertLogs("pdfplumber.pdf", level="WARNING") as logs:
            with pdfplumber.open(stream, strict_metadata=False) as document:
                self.assertEqual(list(document.metadata), ["Loop"])
                loop_type = type(document.metadata["Loop"])
                self.assertEqual(loop_type.__module__, "pdfminer.pdftypes")
                self.assertEqual(loop_type.__qualname__, "PDFObjRef")
        self.assertEqual(
            logs.output,
            [
                "WARNING:pdfplumber.pdf:[WARNING] Metadata key \"Loop\" "
                "could not be parsed due to exception: maximum recursion depth exceeded"
            ],
        )
        self.assertFalse(stream.closed)

    def test_open_applies_and_exposes_unicode_normalization(self) -> None:
        with pdfplumber.open(self.fixture()) as unchanged:
            self.assertIsNone(unchanged.unicode_norm)
            self.assertEqual(unchanged.pages[0].chars[142]["text"], "é")

        canonical = {
            "NFC": "é",
            "NFD": "e\N{COMBINING ACUTE ACCENT}",
            "NFKC": "é",
            "NFKD": "e\N{COMBINING ACUTE ACCENT}",
        }
        compatibility = {
            "NFC": "ﬁ",
            "NFD": "ﬁ",
            "NFKC": "fi",
            "NFKD": "fi",
        }
        for form in ("NFC", "NFD", "NFKC", "NFKD"):
            with self.subTest(form=form):
                with pdfplumber.open(self.fixture(), unicode_norm=form) as document:
                    self.assertEqual(document.unicode_norm, form)
                    self.assertEqual(
                        document.pages[0].chars[142]["text"], canonical[form]
                    )
                with pdfplumber.open(
                    self.compatibility_ligature_fixture(), unicode_norm=form
                ) as document:
                    self.assertEqual(
                        document.pages[0].chars[0]["text"], compatibility[form]
                    )

        for value in ("nfc", ""):
            with self.subTest(invalid_form=value):
                document = pdfplumber.open(self.fixture(), unicode_norm=value)
                try:
                    self.assertEqual(document.unicode_norm, value)
                    pages = document.pages
                    self.assertEqual(pages[0].page_number, 1)
                    self.assertGreater(pages[0].width, 0)
                    with self.assertRaisesRegex(
                        ValueError, "^invalid normalization form$"
                    ):
                        pages[0].chars
                finally:
                    document.close()
        for value, type_name in ((1, "int"), (True, "bool"), (b"NFC", "bytes")):
            with self.subTest(invalid_type=type_name):
                document = pdfplumber.open(self.fixture(), unicode_norm=value)
                try:
                    self.assertEqual(document.unicode_norm, value)
                    pages = document.pages
                    self.assertEqual(pages[0].page_number, 1)
                    self.assertGreater(pages[0].width, 0)
                    with self.assertRaisesRegex(
                        TypeError,
                        rf"^normalize\(\) argument 1 must be str, not {type_name}$",
                    ):
                        pages[0].chars
                finally:
                    document.close()

    def test_pages_reuses_the_mutable_page_list_and_page_instances(self) -> None:
        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            first_pages = document.pages
            second_pages = document.pages

            self.assertIs(first_pages, second_pages)
            self.assertIs(first_pages[0], second_pages[0])
            self.assertEqual([page.page_number for page in first_pages], [3, 5])

            marker = object()
            first_pages.append(marker)
            self.assertIs(document.pages[-1], marker)
            first_pages.pop()

    def test_pages_retains_the_partial_cache_after_selection_failure(self) -> None:
        class FailingSelection:
            def __init__(self) -> None:
                self.calls = 0

            def __contains__(self, _page_number: object) -> bool:
                self.calls += 1
                if self.calls == 2:
                    raise RuntimeError("selection failed")
                return True

        selection = FailingSelection()
        with pdfplumber.open(self.multipage_fixture(), pages=selection) as document:
            with self.assertRaisesRegex(RuntimeError, "^selection failed$"):
                _ = document.pages

            first_pages = document.pages
            self.assertIs(first_pages, document.pages)
            self.assertEqual([page.page_number for page in first_pages], [1])
            self.assertEqual(selection.calls, 2)

    def test_open_repairs_through_ghostscript_with_upstream_ownership(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = self.fake_ghostscript(directory)
            path = os.pathsep.join((directory, os.environ.get("PATH", "")))

            with mock.patch.dict(
                os.environ,
                {
                    "PATH": path,
                    "PDFPLUMBER_FAKE_GS_OUTPUT": str(self.fixture().resolve()),
                },
            ):
                document = pdfplumber.open(self.fixture(), repair=True)
                repaired_stream = document.stream
                self.assertIsInstance(repaired_stream, io.BytesIO)
                self.assertIsNone(document.path)
                self.assertGreater(len(document.pages), 0)
                document.close()
                self.assertTrue(repaired_stream.closed)

                payload = self.fixture().read_bytes()
                external = io.BytesIO(payload)
                external.seek(10)
                document = pdfplumber.open(external, repair=True)
                try:
                    self.assertIsInstance(document.stream, io.BytesIO)
                    self.assertIsNot(document.stream, external)
                    self.assertIsNone(document.path)
                    self.assertGreater(len(document.pages), 0)
                    self.assertEqual(external.tell(), len(payload))
                    self.assertFalse(external.closed)
                finally:
                    repaired_stream = document.stream
                    document.close()
                    external.close()
                self.assertTrue(repaired_stream.closed)

                encrypted = io.BytesIO(self.password_fixture().read_bytes())
                with mock.patch.dict(
                    os.environ,
                    {
                        "PDFPLUMBER_FAKE_GS_OUTPUT": str(
                            self.password_fixture().resolve()
                        )
                    },
                ):
                    with pdfplumber.open(
                        encrypted, password="test", repair=True
                    ) as document:
                        self.assertEqual(len(document.pages), 4)
                        self.assertTrue(document.pages[0].extract_text())
                self.assertFalse(encrypted.closed)
                encrypted.close()

        with mock.patch.dict(os.environ, {"PATH": ""}):
            with pdfplumber.open(self.fixture(), repair=False) as document:
                self.assertGreater(len(document.pages), 0)
            with self.assertRaisesRegex(
                Exception,
                "^Cannot find Ghostscript, which is required for repairs\\.\\n",
            ):
                pdfplumber.open(self.fixture(), repair=True)

    def test_open_accepts_explicit_ghostscript_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = self.fake_ghostscript(directory)
            environment = {
                "PATH": "",
                "PDFPLUMBER_FAKE_GS_OUTPUT": str(self.fixture().resolve()),
            }
            with mock.patch.dict(os.environ, environment):
                for gs_path in (str(executable), executable):
                    with self.subTest(gs_path_type=type(gs_path).__name__):
                        with pdfplumber.open(
                            self.fixture(), repair=True, gs_path=gs_path
                        ) as document:
                            self.assertEqual(len(document.pages), 1)
                            self.assertTrue(document.pages[0].extract_text())

                with pdfplumber.open(
                    self.fixture(), repair=False, gs_path=object()
                ) as document:
                    self.assertEqual(len(document.pages), 1)

                missing = Path(directory) / "missing-gs"
                with self.assertRaises(FileNotFoundError) as native_missing:
                    subprocess.run([missing], check=True)
                with self.assertRaises(FileNotFoundError) as caught:
                    pdfplumber.open(self.fixture(), repair=True, gs_path=missing)
                self.assertEqual(caught.exception.errno, native_missing.exception.errno)
                self.assertEqual(
                    caught.exception.filename, native_missing.exception.filename
                )
                self.assertEqual(str(caught.exception), str(native_missing.exception))

    def test_open_forwards_all_ghostscript_repair_presets(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = self.fake_ghostscript(directory)
            arguments = Path(directory) / "arguments.txt"
            environment = {
                "PATH": "",
                "PDFPLUMBER_FAKE_GS_ARGS": str(arguments),
                "PDFPLUMBER_FAKE_GS_OUTPUT": str(self.fixture().resolve()),
            }
            with mock.patch.dict(os.environ, environment):
                for setting in (
                    "default",
                    "prepress",
                    "printer",
                    "ebook",
                    "screen",
                ):
                    with self.subTest(repair_setting=setting):
                        with pdfplumber.open(
                            self.fixture(),
                            repair=True,
                            gs_path=executable,
                            repair_setting=setting,
                        ) as document:
                            self.assertEqual(len(document.pages), 1)
                        invocation = arguments.read_text().splitlines()
                        self.assertEqual(
                            invocation.count(f"-dPDFSETTINGS=/{setting}"), 1
                        )

                with pdfplumber.open(
                    self.fixture(), repair=False, repair_setting=object()
                ) as document:
                    self.assertEqual(len(document.pages), 1)

    def test_open_retains_raise_unicode_errors_exactly(self) -> None:
        marker = object()
        for value in (True, False, None, 0, 1, "", "false", marker):
            with self.subTest(value=repr(value)):
                with pdfplumber.open(
                    self.annotation_unicode_fixture(), raise_unicode_errors=value
                ) as document:
                    self.assertIs(document.raise_unicode_errors, value)
                    self.assertEqual(len(document.pages), 1)

        with pdfplumber.open(self.annotation_unicode_fixture()) as document:
            self.assertIs(document.raise_unicode_errors, True)

    def test_annots_aggregate_real_multipage_fixture_in_page_order(self) -> None:
        fixture = self.annotation_fixture("issue-463-example.pdf")
        expected_keys = [
            "page_number",
            "object_type",
            "x0",
            "y0",
            "x1",
            "y1",
            "doctop",
            "top",
            "bottom",
            "width",
            "height",
            "uri",
            "title",
            "contents",
            "data",
        ]
        with pdfplumber.open(fixture) as document:
            page_annots = [page.annots for page in document.pages]
            self.assertEqual([len(values) for values in page_annots], [2, 4, 0])

            first = document.annots
            expected = [annot for values in page_annots for annot in values]
            self.assertEqual(first, expected)
            self.assertEqual(
                [annot["page_number"] for annot in first], [1, 1, 2, 2, 2, 2]
            )
            self.assertTrue(all(annot["object_type"] == "annot" for annot in first))
            self.assertTrue(all(list(annot) == expected_keys for annot in first))

            marker = {"marker": True}
            first.append(marker)
            second = document.annots
            self.assertIsNot(second, first)
            self.assertIsNot(second[0], first[0])
            self.assertNotIn(marker, second)
            self.assertEqual(second, expected)

        with pdfplumber.open(self.annotation_fixture("issue-598-example.pdf")) as document:
            self.assertEqual(
                [annot["uri"] for annot in document.annots],
                ["http://www.ck12.org"],
            )

    def test_annots_respect_selected_pages_and_selected_doctop(self) -> None:
        fixture = self.annotation_fixture("issue-463-example.pdf")
        with pdfplumber.open(fixture, pages=(2,)) as document:
            self.assertEqual([page.page_number for page in document.pages], [2])
            self.assertEqual([len(page.annots) for page in document.pages], [4])
            annots = document.annots
            self.assertEqual(len(annots), 4)
            self.assertTrue(all(annot["page_number"] == 2 for annot in annots))
            self.assertTrue(all(annot["doctop"] == annot["top"] for annot in annots))

    def test_hyperlinks_filter_and_aggregate_fresh_annotation_dicts(self) -> None:
        fixture = self.annotation_fixture("issue-982-example.pdf")
        with pdfplumber.open(fixture) as document:
            page_links = [page.hyperlinks for page in document.pages]
            self.assertEqual(
                [len(values) for values in page_links], [0, 0, 0, 0, 34, 0, 0, 0]
            )
            expected = [link for values in page_links for link in values]
            first = document.hyperlinks
            self.assertEqual(first, expected)
            self.assertEqual(len(first), 34)
            self.assertTrue(all(link["page_number"] == 5 for link in first))
            self.assertTrue(all(link["uri"] is not None for link in first))

            second = document.hyperlinks
            self.assertIsNot(second, first)
            self.assertIsNot(second[0], first[0])
            self.assertEqual(second, first)

        with pdfplumber.open(fixture, pages=(2, 5)) as document:
            self.assertEqual([page.page_number for page in document.pages], [2, 5])
            self.assertEqual([len(page.hyperlinks) for page in document.pages], [0, 34])
            self.assertEqual(len(document.hyperlinks), 34)
            self.assertTrue(
                all(link["page_number"] == 5 for link in document.hyperlinks)
            )

    def test_hyperlinks_preserve_multipage_counts_and_empty_results(self) -> None:
        with pdfplumber.open(
            self.annotation_fixture("pdffill-demo.pdf")
        ) as document:
            self.assertEqual(
                [len(page.hyperlinks) for page in document.pages],
                [1, 1, 1, 4, 2, 7, 1],
            )
            self.assertEqual(
                [link["page_number"] for link in document.hyperlinks],
                [1, 2, 3, 4, 4, 4, 4, 5, 5, 6, 6, 6, 6, 6, 6, 6, 7],
            )

        with pdfplumber.open(
            self.annotation_fixture("issue-463-example.pdf")
        ) as document:
            first = document.hyperlinks
            self.assertEqual(first, [])
            self.assertIsNot(document.hyperlinks, first)

    def test_structure_tree_matches_compact_document_and_page_hierarchy(self) -> None:
        fixture = self.annotation_fixture("image_structure.pdf")
        page_tree = [
            {
                "type": "Document",
                "children": [
                    {"type": "P", "mcids": [0]},
                    {"type": "P", "mcids": [1]},
                    {
                        "type": "Figure",
                        "alt_text": (
                            "pdfplumber on github\n\n"
                            "a screen capture of the github page for pdfplumber"
                        ),
                        "mcids": [2],
                    },
                ],
            }
        ]
        document_tree = [
            {
                "type": "Document",
                "children": [
                    {"type": "P", "page_number": 1, "mcids": [0]},
                    {"type": "P", "page_number": 1, "mcids": [1]},
                    {
                        "type": "Figure",
                        "alt_text": (
                            "pdfplumber on github\n\n"
                            "a screen capture of the github page for pdfplumber"
                        ),
                        "page_number": 1,
                        "mcids": [2],
                    },
                ],
            }
        ]

        with pdfplumber.open(fixture) as document:
            first = document.structure_tree
            self.assertEqual(first, document_tree)
            self.assertEqual(document.pages[0].structure_tree, page_tree)
            second = document.structure_tree
            self.assertIsNot(second, first)
            self.assertIsNot(second[0], first[0])
            self.assertEqual(second, first)

    def test_structure_tree_returns_fresh_empty_lists_for_untagged_pdf(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            first = document.structure_tree
            page_first = document.pages[0].structure_tree
            self.assertEqual(first, [])
            self.assertEqual(page_first, [])
            self.assertIsNot(document.structure_tree, first)
            self.assertIsNot(document.pages[0].structure_tree, page_first)

    def test_to_dict_matches_default_and_filtered_document_shape(self) -> None:
        geometry = {
            "page_number": 1,
            "initial_doctop": 0,
            "rotation": 0,
            "cropbox": (0, 0.0, 595.28, 841.89),
            "mediabox": (0, 0.0, 595.28, 841.89),
            "bbox": (0, 0.0, 595.28, 841.89),
            "width": 595.28,
            "height": 841.89,
        }
        with pdfplumber.open(self.fixture()) as document:
            self.assertEqual(document.pages[0].to_dict([]), geometry)
            page_annots = document.pages[0].to_dict(("annot",))
            self.assertEqual(list(page_annots), [*geometry, "annots"])
            self.assertEqual(page_annots["annots"], [])

            empty = document.to_dict([])
            self.assertEqual(list(empty), ["metadata", "pages"])
            self.assertEqual(
                empty,
                {
                    "metadata": {"CreationDate": "D:20260228140604Z"},
                    "pages": [geometry],
                },
            )

            filtered = document.to_dict(["char"])
            self.assertEqual(list(filtered["pages"][0]), [*geometry, "chars"])
            self.assertEqual(filtered["pages"][0]["chars"], document.pages[0].chars)
            self.assertEqual(len(filtered["pages"][0]["chars"]), 258)

            default = document.to_dict()
            self.assertEqual(
                list(default["pages"][0]), [*geometry, "chars", "annots"]
            )
            self.assertEqual(len(default["pages"][0]["chars"]), 258)
            self.assertEqual(default["pages"][0]["annots"], [])

    def test_to_dict_preserves_selected_geometry_and_invalid_input_failures(self) -> None:
        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            pages = document.to_dict([])["pages"]
            self.assertEqual([page["page_number"] for page in pages], [3, 5])
            self.assertEqual([page["initial_doctop"] for page in pages], [0, 841.89])
            self.assertEqual(
                [list(page) for page in pages],
                [
                    [
                        "page_number",
                        "initial_doctop",
                        "rotation",
                        "cropbox",
                        "mediabox",
                        "bbox",
                        "width",
                        "height",
                    ],
                    [
                        "page_number",
                        "initial_doctop",
                        "rotation",
                        "cropbox",
                        "mediabox",
                        "bbox",
                        "width",
                        "height",
                    ],
                ],
            )

            generator_pages = document.to_dict(iter(["char"]))["pages"]
            self.assertEqual(len(generator_pages[0]["chars"]), 693)
            self.assertNotIn("chars", generator_pages[1])

            cases = (
                ("char", AttributeError, "'Page' object has no attribute 'cs'"),
                (["bogus"], AttributeError, "'Page' object has no attribute 'boguss'"),
                ((1,), TypeError, "unsupported operand type(s) for +: 'int' and 'str'"),
                (1, TypeError, "'int' object is not iterable"),
            )
            for value, error_type, message in cases:
                with self.subTest(value=value):
                    with self.assertRaises(error_type) as raised:
                        document.to_dict(value)
                    self.assertEqual(str(raised.exception), message)

    def test_to_json_serializes_to_dict_in_compact_pretty_and_stream_forms(
        self,
    ) -> None:
        compact = (
            '{"metadata": {"CreationDate": "D:20260228140604Z"}, "pages": '
            '[{"page_number": 1, "initial_doctop": 0, "rotation": 0, '
            '"cropbox": [0, 0.0, 595.28, 841.89], "mediabox": '
            '[0, 0.0, 595.28, 841.89], "bbox": '
            '[0, 0.0, 595.28, 841.89], "width": 595.28, '
            '"height": 841.89}]}'
        )
        pretty_page = """{
  "page_number": 1,
  "initial_doctop": 0,
  "rotation": 0,
  "cropbox": [
    0,
    0.0,
    595.3,
    841.9
  ],
  "mediabox": [
    0,
    0.0,
    595.3,
    841.9
  ],
  "bbox": [
    0,
    0.0,
    595.3,
    841.9
  ],
  "width": 595.3,
  "height": 841.9
}"""
        with pdfplumber.open(self.fixture()) as document:
            self.assertEqual(document.to_json(object_types=[]), compact)
            self.assertEqual(
                document.pages[0].to_json(
                    object_types=[], precision=1, indent=2
                ),
                pretty_page,
            )

            stream = io.StringIO()
            self.assertIsNone(document.to_json(stream, object_types=[]))
            self.assertEqual(stream.getvalue(), compact)

    def test_to_json_filters_object_attrs_and_matches_validation_failures(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            included = json.loads(
                document.to_json(
                    object_types=["char"], include_attrs=["text", "x0"]
                )
            )
            self.assertEqual(
                included["pages"][0]["chars"][0],
                {"text": "T", "x0": 31.18, "object_type": "char"},
            )

            excluded = json.loads(
                document.to_json(
                    object_types=["char"],
                    exclude_attrs=["fontname", "size"],
                    precision=2,
                )
            )["pages"][0]["chars"][0]
            self.assertEqual(excluded["object_type"], "char")
            self.assertEqual(excluded["upright"], 1)
            self.assertEqual(excluded["top"], 30.93)
            self.assertNotIn("fontname", excluded)
            self.assertNotIn("size", excluded)

            cases = (
                (
                    {"object_types": [], "include_attrs": [], "exclude_attrs": []},
                    ValueError,
                    "Cannot specify `include_attrs` and `exclude_attrs` at the same time.",
                ),
                (
                    {"object_types": [], "exclude_attrs": ["object_type"]},
                    ValueError,
                    "Cannot exclude these required properties: ['object_type']",
                ),
                (
                    {"object_types": [], "include_attrs": ("text",)},
                    TypeError,
                    'can only concatenate list (not "tuple") to list',
                ),
                (
                    {"object_types": [], "precision": "1"},
                    TypeError,
                    "'str' object cannot be interpreted as an integer",
                ),
            )
            for kwargs, error_type, message in cases:
                with self.subTest(kwargs=kwargs):
                    with self.assertRaises(error_type) as raised:
                        document.to_json(**kwargs)
                    self.assertEqual(str(raised.exception), message)

            for target in (document, document.pages[0]):
                with self.subTest(target=type(target).__name__, call="too-many"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_json(None, None, None, None, None, None, None)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_json() takes from 1 to 7 positional "
                        "arguments but 8 were given",
                    )
                with self.subTest(target=type(target).__name__, call="unexpected"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_json(nope=1)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_json() got an unexpected keyword argument "
                        "'nope'",
                    )
                with self.subTest(target=type(target).__name__, call="duplicate"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_json(None, stream=None)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_json() got multiple values for argument 'stream'",
                    )

    def test_to_csv_serializes_objects_empty_selection_and_stream_forms(
        self,
    ) -> None:
        empty = (
            "object_type,page_number,x0,x1,y0,y1,doctop,top,bottom,"
            "width,height\r\n"
        )
        with pdfplumber.open(self.fixture()) as document:
            self.assertEqual(document.to_csv(object_types=[]), empty)
            self.assertEqual(document.pages[0].to_csv(object_types=[]), empty)

            included = document.to_csv(
                object_types=["char"],
                include_attrs=["page_number", "text", "x0"],
            )
            assert included is not None
            lines = included.splitlines()
            self.assertEqual(lines[0], "object_type,page_number,x0,text")
            self.assertEqual(lines[1], "char,1,31.18,T")
            self.assertEqual(len(lines), 259)

            positional = document.pages[0].to_csv(
                None, ["char"], 1, ["text", "x0"]
            )
            assert positional is not None
            self.assertEqual(
                positional.splitlines()[:2],
                ["object_type,x0,text", "char,31.2,T"],
            )

            stream = io.StringIO()
            self.assertIsNone(document.to_csv(stream, object_types=[]))
            self.assertEqual(stream.getvalue(), empty)
            self.assertEqual(stream.tell(), len(empty))

    def test_to_csv_matches_filter_and_call_shape_failures(self) -> None:
        with pdfplumber.open(self.fixture()) as document:
            cases = (
                (
                    {"object_types": [], "include_attrs": [], "exclude_attrs": []},
                    ValueError,
                    "Cannot specify `include_attrs` and `exclude_attrs` at the same time.",
                ),
                (
                    {"object_types": [], "exclude_attrs": ["object_type"]},
                    ValueError,
                    "Cannot exclude these required properties: ['object_type']",
                ),
                (
                    {"object_types": [], "include_attrs": ("text",)},
                    TypeError,
                    'can only concatenate list (not "tuple") to list',
                ),
                (
                    {"object_types": ["char"], "precision": "1"},
                    TypeError,
                    "'str' object cannot be interpreted as an integer",
                ),
            )
            for kwargs, error_type, message in cases:
                with self.subTest(kwargs=kwargs):
                    with self.assertRaises(error_type) as raised:
                        document.to_csv(**kwargs)
                    self.assertEqual(str(raised.exception), message)

            for target in (document, document.pages[0]):
                with self.subTest(target=type(target).__name__, call="too-many"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_csv(None, None, None, None, None, None)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_csv() takes from 1 to 6 positional "
                        "arguments but 7 were given",
                    )
                with self.subTest(target=type(target).__name__, call="unexpected"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_csv(nope=1)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_csv() got an unexpected keyword argument "
                        "'nope'",
                    )
                with self.subTest(target=type(target).__name__, call="duplicate"):
                    with self.assertRaises(TypeError) as raised:
                        target.to_csv(None, stream=None)
                    self.assertEqual(
                        str(raised.exception),
                        "Container.to_csv() got multiple values for argument 'stream'",
                    )

    def test_json_serializer_matches_recursive_compatibility_values(self) -> None:
        from pdfplumber.convert import Serializer

        class PDFStream:
            def __init__(self, rawdata: bytes | None) -> None:
                self.rawdata = rawdata

        serializer = Serializer(
            precision=2,
            include_attrs=["value", "nested", "stream", "bytes"],
        )
        serialized = serializer.serialize(
            {
                "metadata": {"unicode": "한", "bytes": b"plain"},
                "objects": [
                    {
                        "object_type": "char",
                        "value": 1.235,
                        "nested": (True, [2.345, None]),
                        "stream": PDFStream(b"\x00\xff"),
                        "bytes": b"\xff",
                        "ignored": "excluded",
                    },
                    {
                        "object_type": "image",
                        "stream": PDFStream(None),
                    },
                ],
            }
        )

        self.assertEqual(
            serialized,
            {
                "metadata": {"unicode": "한", "bytes": "plain"},
                "objects": [
                    {
                        "object_type": "char",
                        "value": 1.24,
                        "nested": (1, [2.35, None]),
                        "stream": {"rawdata": "AP8="},
                        "bytes": None,
                    },
                    {"object_type": "image", "stream": {"rawdata": None}},
                ],
            },
        )
        self.assertIs(type(serialized["objects"][0]["nested"][0]), int)
        self.assertEqual(
            json.dumps(serialized["metadata"]),
            r'{"unicode": "\ud55c", "bytes": "plain"}',
        )


if __name__ == "__main__":
    unittest.main()
