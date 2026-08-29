"""Contracts for the cross-surface error and resource-limit guide (DOC-012)."""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "errors-and-resource-limits.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class ErrorResourceLimitDocumentationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_canonical_guide_is_linked_from_every_error_entry_point(self) -> None:
        with self.subTest(artifact="canonical guide"):
            self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": (
                "[error and resource-limit guide]"
                "(docs/errors-and-resource-limits.md)"
            ),
            "crates/pdfplumber-cli/README.md": (
                "[error and resource-limit guide]"
                "(../../docs/errors-and-resource-limits.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[error and resource-limit guide]"
                "(../../docs/errors-and-resource-limits.md)"
            ),
            "crates/pdfplumber-wasm/README.md": (
                "[error and resource-limit guide]"
                "(../../docs/errors-and-resource-limits.md)"
            ),
            "docs/faq.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
            "docs/pre-parity-python-migration.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
            "docs/python-migration.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
            "docs/rust-api.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
            "docs/rust-concurrency.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
            "docs/rust-errors.md": (
                "[error and resource-limit guide](errors-and-resource-limits.md)"
            ),
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        with self.subTest(document="CHANGELOG.md"):
            self.assertRegex(
                changelog,
                r"(?im)^- \*\*Errors and resource limits:\*\* .*resource-limit guide",
            )

    def test_pinned_python_errors_and_unbounded_behavior_are_exact(self) -> None:
        for statement in (
            "Pinned Python `pdfplumber==0.11.10` defines two library exception classes",
            "`PdfminerException(Exception)` and `MalformedPDFException(Exception)`",
            "neither is a `RuntimeError`, `ValueError`, or subclass of the other",
            "wraps the original exception as its sole argument",
            "the wrapper's string is the wrapped exception's string",
            "missing and incorrect passwords both raise `PdfminerException` with an empty message",
            "an empty byte stream raises `PdfminerException` whose sole argument is a `PDFSyntaxError`",
            "No /Root object! - Is this really a PDF?",
            "`strict_metadata=False` logs a warning and retains the unresolved value",
            "`strict_metadata=True` re-raises the underlying metadata exception",
            "`raise_unicode_errors=True` propagates `UnicodeDecodeError`",
            "`raise_unicode_errors=False` emits `UserWarning` and continues",
            "argument and geometry validation can raise ordinary `TypeError` and `ValueError`",
            "repair failures raise plain `Exception` with Ghostscript's standard error text",
            "`pages=` is a one-based selection filter, not a page-count budget",
            "still enumerates the complete `PDFPage.create_pages` generator",
            "no input-byte, page-count, object-count, image-byte, recursion, memory, or time budget",
            "the Ghostscript repair subprocess has no timeout",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/pdf.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/utils/exceptions.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/page.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/display.py",
            "https://github.com/jsvine/pdfplumber/blob/v0.11.10/pdfplumber/repair.py",
            "f3d2dc69906c1c5b946916f80dce661b5f00f32f",
            "f96cc03949830dbd9bea34b2d2a9e0854dca9008",
            "286e7e158c12da8305520ecc1f550f3bd8f1a906",
            "4b915da24aa7cc7066bdec0e8aebc0457fd1783c",
            "2e4df9aaf0c034077e4ef68b5c776c975fa1eed4",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        contract = json.loads(
            (
                REPO_ROOT
                / "compat/contracts/pdfplumber-v0.11.10-error-behavior.json"
            ).read_text(encoding="utf-8")
        )
        by_id = {record["id"]: record for record in contract["cases"]}
        for identifier in ("password.missing", "password.wrong"):
            with self.subTest(case=identifier):
                outcome = by_id[identifier]["outcome"]
                self.assertEqual(
                    outcome["type"],
                    "pdfplumber.utils.exceptions.PdfminerException",
                )
                self.assertEqual(outcome["message"], "")

    def test_rust_error_warning_and_safe_reporting_contract_is_complete(self) -> None:
        for statement in (
            "`PdfErrorKind::{Parse, Io, Font, Interpreter, ResourceLimit, PasswordRequired, InvalidPassword, Other}`",
            "`PARSE`, `IO`, `FONT`, `INTERPRETER`, `RESOURCE_LIMIT`, `PASSWORD_REQUIRED`, `INVALID_PASSWORD`, and `OTHER`",
            "match the non-exhaustive enum with a wildcard",
            "`PdfErrorContext` carries an operation, zero-based page index, and PDF object number and generation",
            "`PdfResourceLimit` carries `name`, `limit`, and `observed`",
            "ordinary `Display` and `Debug` omit paths, document content, passwords, and the source message",
            "`std::error::Error::source()` is opt-in and can expose sensitive parser or operating-system details",
            "`ExtractWarningCode`, `ExtractWarning`, and `ExtractResult<T>`",
            "`Page::warnings()` returns the warnings retained during that extraction",
            "`collect_warnings=true` retains warnings and `false` discards them",
            "`strict_mode=false` is the default but the field is not consulted by extraction",
            "`ExtractWarning::to_error()` is a manual helper, not automatic strict-mode wiring",
            "it produces `PdfErrorKind::Other` with operation `strict warning escalation`",
            "`RESOURCE_LIMIT_REACHED` is declared but no extraction path emits it",
            "a fatal limit breach returns `PdfErrorKind::ResourceLimit` rather than a warning",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        error_source = (
            REPO_ROOT / "crates/pdfplumber-core/src/error.rs"
        ).read_text(encoding="utf-8")
        runtime_source = "\n".join(
            (
                (REPO_ROOT / "crates/pdfplumber/src/pdf.rs").read_text(
                    encoding="utf-8"
                ),
                (REPO_ROOT / "crates/pdfplumber-parse/src/interpreter.rs").read_text(
                    encoding="utf-8"
                ),
            )
        )
        for field in ("max_objects_per_page", "max_stream_bytes", "strict_mode"):
            with self.subTest(declarative_field=field):
                self.assertIn(f"pub {field}:", error_source)
                self.assertNotIn(field, runtime_source)

    def test_every_rust_resource_control_has_status_scope_and_timing(self) -> None:
        for statement in (
            "`max_input_bytes` | `None` | Enforced",
            "byte slices report their complete length; readers stop after at most `limit + 1` bytes",
            "repair checks both the original input and the repaired byte result",
            "`max_pages` | `None` | Enforced",
            "after parsing discovers the document page count and before page geometry is cached",
            "`max_total_objects` | `None` | Enforced",
            "characters, lines, rectangles, curves, and images",
            "annotations, hyperlinks, form fields, words, tables, and derived edges are not counted",
            "`max_total_image_bytes` | `None` | Enforced with `extract_image_data=true`",
            "only bytes stored in returned `Image.data` values are counted",
            "`extract_image_content` and `extract_images_with_content` do not debit this byte counter",
            "`max_recursion_depth` | `10` | Enforced as interpreter failure",
            "page content starts at depth zero; nested Form XObjects above depth 10 fail",
            "returns `PdfErrorKind::Interpreter`, not `PdfErrorKind::ResourceLimit`",
            "`max_objects_per_page` | `100_000` | Declared, not enforced",
            "`max_stream_bytes` | `100 MiB` | Declared, not enforced",
            "document-wide totals are charged after a complete page extraction",
            "the extraction that crosses a total limit has already performed its page-local work",
            "repeated pages and failed over-limit attempts remain charged",
            "an object-limit failure occurs before the image-byte counter is updated",
            "parallel callers share atomic counters and the first failing page is scheduling-dependent",
            "all four optional enforced limits are unbounded by default",
            "no enforced CPU-time, wall-time, process-memory, decoded-stream, page-dimension, output-size, or table-complexity limit",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertGreaterEqual(self.guide.count("```rust"), 2)

    def test_adapter_boundaries_operations_and_claim_limits_remain_explicit(self) -> None:
        for statement in (
            "Rust is the only current surface that accepts `ExtractOptions`",
            "the Python adapter passes `None` to every native open method",
            "the Python adapter exposes no warning collection or resource-budget argument",
            "parse and password failures are mapped to the compatibility `PdfminerException`",
            "`PdfIoError`, `PdfFontError`, `PdfInterpreterError`, and `PdfResourceLimitError` live in private `pdfplumber._native`",
            "`PdfResourceLimitError` is not reachable through the current public constructors because callers cannot supply a limit",
            "the Command-Line Interface has no resource-budget or timeout flag",
            "uses default `ExtractOptions` and prints Rust's safe outer `Display` to standard error",
            "runtime failures exit with status 1; argument parsing is owned by Clap",
            "`--pages` selects work but is not a document page-count budget",
            "WebAssembly calls `Pdf::open_bytes(data, None)`",
            "converts the safe outer `Display` to `JsError`",
            "exposes no resource, warning, password, repair, or timeout control",
            "run untrusted inputs in a process or worker with host-enforced memory and time limits",
            "do not log Python exception arguments, Rust source chains, passwords, paths, or document bytes to an unprotected sink",
            "does not establish error compatibility",
            "error and resource-limit documentation is not compatibility evidence",
            "does not approve a compatibility deviation",
            "DOC-012 changes no runtime behavior",
            "../crates/pdfplumber-core/src/error.rs",
            "../crates/pdfplumber/src/pdf.rs",
            "../crates/pdfplumber-py/src/lib.rs",
            "../crates/pdfplumber-py/python/pdfplumber/_native.pyi",
            "../crates/pdfplumber-cli/src/shared.rs",
            "../crates/pdfplumber-cli/src/cli.rs",
            "../crates/pdfplumber-wasm/src/lib.rs",
            "../compat/contracts/pdfplumber-v0.11.10-error-behavior.json",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertIsNotNone(
            re.search(
                r"(?s)## Pinned Python error behavior.*"
                r"## Rust errors and warnings.*"
                r"## Rust resource controls.*"
                r"## Current adapter boundaries.*"
                r"## Operational policy.*"
                r"## Validation and provenance.*"
                r"## Claim boundary",
                self.guide,
            )
        )
        self.assertGreaterEqual(self.guide.count("```python"), 2)
        self.assertGreaterEqual(self.guide.count("```console"), 1)
        self.assertNotRegex(self.guide, re.compile(r"\b\d+(?:\.\d+)?%\b"))


if __name__ == "__main__":
    unittest.main()
