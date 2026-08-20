"""Installed-artifact contracts for the private native extension boundary."""

from __future__ import annotations

import io
import importlib.machinery
from pathlib import Path
from types import MappingProxyType
import unittest

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

    def test_close_respects_stream_ownership_and_is_idempotent(self) -> None:
        document = pdfplumber.open(self.fixture())
        owned_stream = document.stream
        first_page = document.pages[0]

        self.assertIsNone(document.close())
        self.assertTrue(owned_stream.closed)
        self.assertIsNone(document.close())
        self.assertEqual(len(document.pages), 1)
        self.assertGreater(len(first_page.chars()), 0)

        external = io.BytesIO(self.fixture().read_bytes())
        try:
            document = pdfplumber.open(external)
            self.assertIsNone(document.close())
            self.assertFalse(external.closed)
            self.assertIsNone(document.close())
        finally:
            external.close()

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

        with pdfplumber.open(fixture, pages=1) as selected:
            with self.assertRaisesRegex(TypeError, "not iterable"):
                _ = selected.pages

    def test_selected_pages_keep_original_numbers_and_selected_doctop(self) -> None:
        with pdfplumber.open(self.multipage_fixture(), pages=(3, 5)) as document:
            pages = document.pages

        self.assertEqual([page.page_number for page in pages], [3, 5])

        char_offsets = [
            page.chars()[0]["doctop"] - page.chars()[0]["top"] for page in pages
        ]
        word_offsets = [
            page.extract_words()[0]["doctop"] - page.extract_words()[0]["top"]
            for page in pages
        ]
        for offsets in (char_offsets, word_offsets):
            self.assertAlmostEqual(offsets[0], 0.0)
            self.assertAlmostEqual(offsets[1], pages[0].height)

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


if __name__ == "__main__":
    unittest.main()
