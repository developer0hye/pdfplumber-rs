"""Public adoption-document contracts."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
README_PATH = REPO_ROOT / "README.md"
COMPARISON_PATH = REPO_ROOT / "docs" / "comparison.md"


class AdoptionDocsContractTests(unittest.TestCase):
    def test_readme_links_to_the_comparison_page(self) -> None:
        readme = README_PATH.read_text(encoding="utf-8")

        self.assertIn("(docs/comparison.md)", readme)

    def test_comparison_separates_claim_types_and_pins_primary_sources(self) -> None:
        self.assertTrue(COMPARISON_PATH.is_file(), "docs/comparison.md is missing")
        comparison = COMPARISON_PATH.read_text(encoding="utf-8")

        for heading in (
            "## Observed facts",
            "## Reproducible measurements",
            "## Product interpretation",
        ):
            with self.subTest(heading=heading):
                self.assertIn(heading, comparison)

        for project in (
            "`pdfplumber-rs`",
            "`pdf_oxide`",
            "`pdfsink-rs`",
            "Python `pdfplumber`",
            "`pdf-extract`",
        ):
            with self.subTest(project=project):
                self.assertIn(project, comparison)

        self.assertIn(
            "No cross-project performance result is currently claimed by "
            "`pdfplumber-rs`.",
            comparison,
        )

        for repository in (
            "yfedoseev/pdf_oxide",
            "clark-labs-inc/pdfsink-rs",
            "jsvine/pdfplumber",
            "jrmuizel/pdf-extract",
        ):
            with self.subTest(repository=repository):
                self.assertRegex(
                    comparison,
                    re.compile(
                        rf"https://github\.com/{re.escape(repository)}/blob/"
                        r"[0-9a-f]{40}/README\.md"
                    ),
                )


if __name__ == "__main__":
    unittest.main()
