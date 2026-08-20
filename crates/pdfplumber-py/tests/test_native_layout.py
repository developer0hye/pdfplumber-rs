"""Installed-artifact contracts for the private native extension boundary."""

from __future__ import annotations

import io
import importlib.machinery
import os
from pathlib import Path
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
        return (
            Path(__file__).resolve().parents[3]
            / "compat"
            / "fixtures"
            / "upstream"
            / "pdfplumber-v0.11.10"
            / "tests"
            / "pdfs"
            / "annotations-unicode-issues.pdf"
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

    def test_open_applies_and_exposes_unicode_normalization(self) -> None:
        with pdfplumber.open(self.fixture()) as unchanged:
            self.assertIsNone(unchanged.unicode_norm)
            self.assertEqual(unchanged.pages[0].chars()[142]["text"], "é")

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
                        document.pages[0].chars()[142]["text"], canonical[form]
                    )
                with pdfplumber.open(
                    self.compatibility_ligature_fixture(), unicode_norm=form
                ) as document:
                    self.assertEqual(
                        document.pages[0].chars()[0]["text"], compatibility[form]
                    )

        for value in ("nfc", ""):
            with self.subTest(invalid_form=value):
                document = pdfplumber.open(self.fixture(), unicode_norm=value)
                try:
                    self.assertEqual(document.unicode_norm, value)
                    with self.assertRaisesRegex(
                        ValueError, "^invalid normalization form$"
                    ):
                        _ = document.pages
                finally:
                    document.close()
        for value, type_name in ((1, "int"), (True, "bool"), (b"NFC", "bytes")):
            with self.subTest(invalid_type=type_name):
                document = pdfplumber.open(self.fixture(), unicode_norm=value)
                try:
                    self.assertEqual(document.unicode_norm, value)
                    with self.assertRaisesRegex(
                        TypeError,
                        rf"^normalize\(\) argument 1 must be str, not {type_name}$",
                    ):
                        _ = document.pages
                finally:
                    document.close()

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


if __name__ == "__main__":
    unittest.main()
