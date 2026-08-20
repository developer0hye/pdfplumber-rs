"""Installed-artifact contracts for the private native extension boundary."""

from __future__ import annotations

import io
import importlib.machinery
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
        self.assertGreater(len(first_page.chars()), 0)

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
            first_chars = first_page.chars()
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
            self.assertEqual(first_page.chars(), first_chars)
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
            expected_chars = [char for page in first_pages for char in page.chars()]
            expected_lines = [line for page in first_pages for line in page.lines()]
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
                    pages = document.pages
                    self.assertEqual(pages[0].page_number, 1)
                    self.assertGreater(pages[0].width, 0)
                    with self.assertRaisesRegex(
                        ValueError, "^invalid normalization form$"
                    ):
                        pages[0].chars()
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
                        pages[0].chars()
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
            self.assertEqual(filtered["pages"][0]["chars"], document.pages[0].chars())
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
            '[{"page_number": 1, "initial_doctop": 0.0, "rotation": 0, '
            '"cropbox": [0.0, 0.0, 595.28, 841.89], "mediabox": '
            '[0.0, 0.0, 595.28, 841.89], "bbox": '
            '[0.0, 0.0, 595.28, 841.89], "width": 595.28, '
            '"height": 841.89}]}'
        )
        pretty_page = """{
  "page_number": 1,
  "initial_doctop": 0.0,
  "rotation": 0,
  "cropbox": [
    0.0,
    0.0,
    595.3,
    841.9
  ],
  "mediabox": [
    0.0,
    0.0,
    595.3,
    841.9
  ],
  "bbox": [
    0.0,
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
