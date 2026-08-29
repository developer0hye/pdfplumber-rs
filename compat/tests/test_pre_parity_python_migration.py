"""Contracts for migration from the pre-parity Python binding (DOC-004)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "pre-parity-python-migration.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class PreParityPythonMigrationContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_guide_is_publicly_linked_and_exactly_release_scoped(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": (
                "[pre-parity Python migration guide]"
                "(docs/pre-parity-python-migration.md)"
            ),
            "crates/pdfplumber-py/README.md": (
                "[pre-parity binding guide](../../docs/pre-parity-python-migration.md)"
            ),
            "docs/python-migration.md": (
                "[pre-parity binding migration guide](pre-parity-python-migration.md)"
            ),
            "docs/faq.md": (
                "[pre-parity binding guide](pre-parity-python-migration.md)"
            ),
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertRegex(
            changelog,
            r"(?im)^- \*\*Migration:\*\* .*pre-parity.*migration guide",
        )

        for statement in (
            "`pdfplumber-rs==0.2.0`",
            "`caf412d9307d7d22769b6cd5fb330ad0594ef0bf`",
            "`da0663ce27f35bfc641055c0cebf8fae97932ac4`",
            "`pdfplumber-rs==0.3.0`",
            "current `0.3.x` alpha",
            "not a complete drop-in replacement",
            "one release is not evidence for another release",
            "v0.2.0 was published under `MIT OR Apache-2.0`",
            "v0.3.0 uses `Apache-2.0`",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        self.assertNotRegex(self.guide, r"\b\d+(?:\.\d+)?%\b")

    def test_release_environments_are_isolated_identified_and_recoverable(
        self,
    ) -> None:
        for command in (
            "python3.13 -m venv .venv-pdfplumber-rs-020",
            "python3.13 -m venv .venv-pdfplumber-rs-030",
            "python -m pip install 'pdfplumber-rs==0.2.0'",
            "python -m pip install 'pdfplumber-rs==0.3.0'",
            "python -m pip show pdfplumber-rs",
            "python -m pip show pdfplumber",
            'm.version("pdfplumber-rs")',
            "pdfplumber.__file__",
        ):
            with self.subTest(command=command):
                self.assertIn(command, self.guide)

        for statement in (
            "Both releases install the same distribution and import names.",
            "Do not upgrade the 0.2.0 environment in place.",
            "the separate `pdfplumber` distribution must be absent",
            "keep the complete 0.2.0 environment",
            "discard the 0.3.0 environment",
            "current-source policy supports exactly CPython 3.13 for the next release",
            "Published metadata is not execution evidence for your deployment",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_legacy_calls_have_explicit_public_or_extension_migrations(self) -> None:
        for heading in (
            "## 1. Freeze the 0.2.0 application contract",
            "## 2. Build isolated release environments",
            "## 3. Rewrite calls deliberately",
            "## 4. Re-baseline observable behavior",
            "## 5. Cut over or roll back",
        ):
            with self.subTest(heading=heading):
                self.assertIn(heading, self.guide)

        for legacy, candidate in (
            ("pdfplumber.PDF.open(path)", "pdfplumber.open(path)"),
            ("pdfplumber.PDF.open_bytes(data)", "pdfplumber.open(BytesIO(data))"),
            ("page.chars()", "page.chars"),
            ("page.lines()", "page.lines"),
            ("page.rects()", "page.rects"),
            ("page.curves()", "page.curves"),
            ("page.images()", "page.images"),
            ("cropped.chars()", "cropped.chars"),
            ("pdf.bookmarks()", "pdf.rust.bookmarks()"),
        ):
            with self.subTest(legacy=legacy):
                self.assertIn(legacy, self.guide)
                self.assertIn(candidate, self.guide)

        for boundary in (
            "`page.page_number` changed from 0-based to 1-based",
            "zero-based destinations",
            "from io import BytesIO",
            "from pdfplumber import _native",
            "_native.PDF.open_bytes(data)",
            "private native extension path",
            "does not count as compatibility evidence",
        ):
            with self.subTest(boundary=boundary):
                self.assertIn(boundary, self.compact_guide)

    def test_outputs_failures_and_state_are_rebaselined_without_inference(
        self,
    ) -> None:
        for observation in (
            "the same PDF bytes",
            "dictionary keys, key order, nested values, numeric types, and `None` placement",
            "exception classes, messages, and arguments",
            "cache identity, mutation, `flush_cache()`, `close()`, and context-manager behavior",
            "crop, `within_bbox`, and `outside_bbox` coordinates and inclusion rules",
            "table geometry, rows, extracted values, and `accuracy`",
            "no automatic translation",
            "legacy performance claims are not migration evidence",
        ):
            with self.subTest(observation=observation):
                self.assertIn(observation, self.compact_guide)

        for link in (
            "[Python migration guide](python-migration.md)",
            "[workflow scorecard](compatibility/workflows-v0.3.0.md)",
            "[compatibility terminology](compatibility/terms.md)",
            "[Python support policy](python-support.md)",
        ):
            with self.subTest(link=link):
                self.assertIn(link, self.guide)

        for outcome in (
            "Exact",
            "Application-accepted change",
            "Unsupported",
            "Legacy failure",
            "Candidate failure",
            "Not tested",
        ):
            with self.subTest(outcome=outcome):
                self.assertIn(outcome, self.guide)

        for boundary in (
            "application decision, not an approved compatibility delta",
            "approved-delta registry does not approve differences from 0.2.0",
            "scorecard does not compare 0.2.0 with 0.3.0",
        ):
            with self.subTest(boundary=boundary):
                self.assertIn(boundary, self.compact_guide)

    def test_cutover_requires_complete_evidence_and_environment_rollback(
        self,
    ) -> None:
        for requirement in (
            "exact 0.3.0 artifact tested",
            "every required legacy workflow",
            "Unsupported, Candidate failure, and Not tested block cutover",
            "stop routing work to the 0.3.0 environment",
            "restore the separately locked 0.2.0 environment",
            "Do not reinstall 0.2.0 over the candidate environment.",
            "rerun the migration evaluation",
        ):
            with self.subTest(requirement=requirement):
                self.assertIn(requirement, self.compact_guide)

        self.assertNotRegex(
            self.compact_guide.lower(),
            re.compile(
                r"(?:is|works as) (?:a )?(?:complete |full )?drop-in replacement"
            ),
        )


if __name__ == "__main__":
    unittest.main()
