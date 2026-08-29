"""Contracts for the Python pdfplumber migration guide (DOC-003)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE_PATH = REPO_ROOT / "docs" / "python-migration.md"


def compact(text: str) -> str:
    return " ".join(text.split())


class PythonMigrationGuideContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.guide = (
            GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        )
        cls.compact_guide = compact(cls.guide)

    def test_guide_is_publicly_linked_and_release_scoped(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), f"missing guide: {GUIDE_PATH}")

        links = {
            "README.md": "[Python migration guide](docs/python-migration.md)",
            "crates/pdfplumber-py/README.md": (
                "[migration guide](../../docs/python-migration.md)"
            ),
            "docs/faq.md": "[migration guide](python-migration.md)",
        }
        for relative, link in links.items():
            with self.subTest(document=relative):
                rendered = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertIn(link, rendered)

        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertRegex(changelog, r"(?im)^- \*\*Migration:\*\* .*migration guide")

    def test_scope_names_exact_reference_candidate_and_evidence_boundaries(
        self,
    ) -> None:
        for statement in (
            "Python `pdfplumber` v0.11.10",
            "`pdfplumber-rs` `0.3.x` alpha",
            "not a complete drop-in replacement",
            "one release is not evidence for another release",
            "installed artifact",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

        for link in (
            "[Python-release compatibility matrix](compatibility/python-release-matrix-v0.3.0.md)",
            "[workflow scorecard](compatibility/workflows-v0.3.0.md)",
            "[compatibility terminology](compatibility/terms.md)",
            "[Python support policy](python-support.md)",
            "[support matrix](support.md#python)",
        ):
            with self.subTest(link=link):
                self.assertIn(link, self.guide)

        self.assertNotRegex(self.guide, r"\b\d+(?:\.\d+)?%\b")

    def test_environment_cutover_is_isolated_detectable_and_recoverable(self) -> None:
        for command in (
            "python3.13 -m venv .venv-pdfplumber-reference",
            "python3.13 -m venv .venv-pdfplumber-rs",
            "python -m pip show pdfplumber",
            "python -m pip show pdfplumber-rs",
            "python -m pip install 'pdfplumber==0.11.10'",
            "python -m pip install 'pdfplumber-rs==0.3.0'",
        ):
            with self.subTest(command=command):
                self.assertIn(command, self.guide)

        for statement in (
            "Never install both distributions in one environment.",
            "`pip check` can still succeed for a mixed package",
            "discard the candidate environment",
            "Uninstalling one distribution is not a safe rollback",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.compact_guide)

    def test_workflow_requires_inventory_and_like_for_like_observations(self) -> None:
        for heading in (
            "## 1. Inventory the application",
            "## 2. Build isolated environments",
            "## 3. Run the same workload",
            "## 4. Interpret every result",
            "## 5. Cut over or roll back",
        ):
            with self.subTest(heading=heading):
                self.assertIn(heading, self.guide)

        for inventory in (
            "imports and public exports",
            "call signatures and defaults",
            "return values, ordering, and runtime types",
            "exceptions, warnings, and messages",
            "caching, mutation, close, and context-manager behavior",
            "optional executables and side effects",
        ):
            with self.subTest(inventory=inventory):
                self.assertIn(inventory, self.compact_guide)

        for comparison in (
            "same PDF bytes",
            "same page selection",
            "same positional and keyword arguments",
            "same operating-system and architecture scope",
            "reference output",
            "candidate output",
        ):
            with self.subTest(comparison=comparison):
                self.assertIn(comparison, self.compact_guide)

    def test_result_labels_and_extension_boundary_control_the_decision(self) -> None:
        for outcome in (
            "Exact",
            "Approved delta",
            "Unsupported",
            "Reference failure",
            "Candidate failure",
            "Not tested",
        ):
            with self.subTest(outcome=outcome):
                self.assertIn(outcome, self.guide)

        for boundary in (
            "Only Exact observations permit an unqualified compatibility claim",
            "Unsupported, Reference failure, Candidate failure, and Not tested are not compatible results",
            "`pdfplumber._native` is a packaging boundary",
            "`document.rust` is an extension namespace",
            "extensions do not count as parity evidence",
            "keep the reference environment",
        ):
            with self.subTest(boundary=boundary):
                self.assertIn(boundary, self.compact_guide)

        self.assertNotRegex(
            self.compact_guide.lower(),
            re.compile(
                r"(?:is|works as) (?:a )?(?:complete |full )?drop-in replacement"
            ),
        )


if __name__ == "__main__":
    unittest.main()
