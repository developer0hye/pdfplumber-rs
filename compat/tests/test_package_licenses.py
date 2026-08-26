"""Contracts for the repository-wide package license policy."""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = REPO_ROOT / "license-policy.toml"
POLICY_DOC_PATH = REPO_ROOT / "docs" / "license.md"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_licenses.py"


class PackageLicenseContractTests(unittest.TestCase):
    def test_source_policy_is_machine_checked(self) -> None:
        for path in (POLICY_PATH, POLICY_DOC_PATH, CHECKER_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(
                    path.is_file(), f"missing {path.relative_to(REPO_ROOT)}"
                )

        if not CHECKER_PATH.is_file():
            return

        completed = subprocess.run(
            [sys.executable, str(CHECKER_PATH), "--source"],
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

    def test_public_package_readmes_use_the_current_policy(self) -> None:
        expected = "Licensed under the [Apache License, Version 2.0](../../LICENSE)."
        readmes = (
            "crates/pdfplumber-cli/README.md",
            "crates/pdfplumber-py/README.md",
            "crates/pdfplumber-wasm/README.md",
        )

        for relative_path in readmes:
            with self.subTest(readme=relative_path):
                content = (REPO_ROOT / relative_path).read_text(encoding="utf-8")
                self.assertIn(expected, content)
                self.assertNotIn("MIT OR Apache-2.0", content)
                self.assertNotIn("Dual-licensed", content)

    def test_ci_checks_source_and_every_built_artifact_family(self) -> None:
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        required_commands = (
            "python scripts/check_package_licenses.py --source",
            "python scripts/check_package_licenses.py --rust target/package/*.crate",
            "python scripts/check_package_licenses.py --python dist/*.whl dist/*.tar.gz",
            "python scripts/check_package_licenses.py --npm crates/pdfplumber-wasm/pkg",
            "wasm-pack build --target bundler --out-dir pkg crates/pdfplumber-wasm",
        )

        for command in required_commands:
            with self.subTest(command=command):
                self.assertIn(command, workflow)


if __name__ == "__main__":
    unittest.main()
