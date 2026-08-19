"""Installed-artifact contracts for the private native extension boundary."""

from __future__ import annotations

import importlib.machinery
from pathlib import Path
import unittest

import pdfplumber
from pdfplumber import _native


class NativeLayoutTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
