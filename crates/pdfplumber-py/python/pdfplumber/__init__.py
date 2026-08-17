"""Python package boundary for the pdfplumber-rs native extension."""

from . import _native as _native

PDF = _native.PDF
open = PDF.open

__all__ = ["_native"]
