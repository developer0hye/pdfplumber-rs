"""Public Python distribution/import-name and conflict-policy contracts."""

from __future__ import annotations

import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PYTHON_README = REPO_ROOT / "crates" / "pdfplumber-py" / "README.md"


class PythonDistributionConflictContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        rendered = PYTHON_README.read_text(encoding="utf-8")
        cls.readme = " ".join(rendered.split())

    def test_distribution_import_and_native_names_are_explicit(self) -> None:
        for statement in (
            "## Distribution and import names",
            "The installable distribution is `pdfplumber-rs`.",
            "The Python import package is `pdfplumber`.",
            "The private native module is `pdfplumber._native`.",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.readme)

    def test_coinstallation_is_explicitly_unsupported(self) -> None:
        for statement in (
            "Do not install `pdfplumber-rs` and Python `pdfplumber` in the same environment.",
            "Both distributions write files under `pdfplumber/`.",
            "`pip` treats their distribution names as different",
            "`pip check` can still succeed",
            "installation order can silently select a mixed package",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.readme)

    def test_detection_and_recovery_policy_is_exact(self) -> None:
        for statement in (
            "`python -m pip show pdfplumber`",
            "`python -m pip show pdfplumber-rs`",
            "Use a new, dedicated virtual environment that contains exactly one of these distributions.",
            "Uninstalling only one distribution is not a repair",
            "discard that environment and create a new one",
        ):
            with self.subTest(statement=statement):
                self.assertIn(statement, self.readme)


if __name__ == "__main__":
    unittest.main()
