"""Contracts for the generated public support matrix."""

from __future__ import annotations

import subprocess
import sys
import tomllib
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "support-matrix.toml"
GENERATOR_PATH = REPO_ROOT / "scripts" / "generate_support_matrix.py"
OUTPUT_PATH = REPO_ROOT / "docs" / "support.md"


class SupportMatrixContractTests(unittest.TestCase):
    def test_readme_links_to_the_generated_support_matrix(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("(docs/support.md)", readme)

    def test_generated_matrix_is_current_and_complete(self) -> None:
        for path in (SOURCE_PATH, GENERATOR_PATH, OUTPUT_PATH):
            with self.subTest(path=path.name):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        completed = subprocess.run(
            [sys.executable, str(GENERATOR_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            completed.returncode,
            0,
            f"{completed.stdout}{completed.stderr}",
        )

        source = tomllib.loads(SOURCE_PATH.read_text(encoding="utf-8"))
        self.assertEqual(source["schema_version"], 1)
        self.assertEqual(
            [surface["id"] for surface in source["surfaces"]],
            ["rust", "python", "cli", "wasm"],
        )

        output = OUTPUT_PATH.read_text(encoding="utf-8")
        for phrase in (
            "CI-verified platforms",
            "Release-configured targets",
            "Source version",
            "Observed registry version",
            "Known limitations",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, output)


if __name__ == "__main__":
    unittest.main()
