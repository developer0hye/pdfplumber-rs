"""Contracts for public package and release metadata agreement."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_metadata.py"
RELEASE_NOTES_PATH = REPO_ROOT / "docs" / "releases" / "v0.3.0.md"


class PackageMetadataContractTests(unittest.TestCase):
    def test_support_matrix_defines_the_canonical_release_identity(self) -> None:
        matrix = tomllib.loads(MATRIX_PATH.read_text(encoding="utf-8"))

        self.assertEqual(matrix["release_version"], "0.3.0")
        self.assertEqual(matrix["license"], "Apache-2.0")
        self.assertEqual(
            matrix["repository"],
            "https://github.com/developer0hye/pdfplumber-rs",
        )
        self.assertEqual(matrix["release_notes"], "docs/releases/v0.3.0.md")
        self.assertIs(matrix["github_prerelease"], True)

        surfaces = {surface["id"]: surface for surface in matrix["surfaces"]}
        expected = {
            "rust": {
                "package": "pdfplumber",
                "import_name": "pdfplumber",
                "readme": "README.md",
                "maturity": "alpha",
            },
            "python": {
                "package": "pdfplumber-rs",
                "import_name": "pdfplumber",
                "native_module": "pdfplumber._native",
                "readme": "crates/pdfplumber-py/README.md",
                "maturity": "alpha",
            },
            "cli": {
                "package": "pdfplumber-cli",
                "executable": "pdfplumber",
                "readme": "crates/pdfplumber-cli/README.md",
                "maturity": "alpha",
            },
            "wasm": {
                "package": "pdfplumber-wasm",
                "import_name": "pdfplumber-wasm",
                "readme": "crates/pdfplumber-wasm/README.md",
                "maturity": "experimental",
            },
        }
        for surface_id, fields in expected.items():
            for field, value in fields.items():
                with self.subTest(surface=surface_id, field=field):
                    self.assertEqual(surfaces[surface_id][field], value)

    def test_source_and_release_metadata_are_machine_checked(self) -> None:
        self.assertTrue(CHECKER_PATH.is_file(), "missing package metadata checker")
        if not CHECKER_PATH.is_file():
            return

        source = subprocess.run(
            [sys.executable, str(CHECKER_PATH), "--source"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(source.returncode, 0, f"{source.stdout}{source.stderr}")

        with tempfile.NamedTemporaryFile() as github_output:
            release = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER_PATH),
                    "--release-tag",
                    "v0.3.0",
                    "--github-output",
                    github_output.name,
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(
                release.returncode,
                0,
                f"{release.stdout}{release.stderr}",
            )
            github_output.seek(0)
            outputs = github_output.read().decode("utf-8")
        self.assertIn("release-notes=docs/releases/v0.3.0.md\n", outputs)
        self.assertIn("prerelease=true\n", outputs)

        with tempfile.NamedTemporaryFile() as github_output:
            mismatched = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER_PATH),
                    "--release-tag",
                    "v0.3.1",
                    "--github-output",
                    github_output.name,
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
        self.assertNotEqual(mismatched.returncode, 0)
        self.assertIn("release tag v0.3.1 != source v0.3.0", mismatched.stderr)

    def test_public_readmes_and_release_notes_display_exact_identity(self) -> None:
        required_readme_phrases = {
            "README.md": (
                "Release `0.3.0`",
                "Rust crate `pdfplumber` (import `pdfplumber`) is alpha",
                "Python distribution `pdfplumber-rs` (import `pdfplumber`) is alpha",
                "CLI crate `pdfplumber-cli` installs `pdfplumber` and is alpha",
                "npm package `pdfplumber-wasm` is experimental",
            ),
            "crates/pdfplumber-py/README.md": (
                "Distribution `pdfplumber-rs` installs import package `pdfplumber`",
                "Release `0.3.0` is alpha",
            ),
            "crates/pdfplumber-cli/README.md": (
                "Crate `pdfplumber-cli` installs executable `pdfplumber`",
                "Release `0.3.0` is alpha",
            ),
            "crates/pdfplumber-wasm/README.md": (
                "npm package and import name are `pdfplumber-wasm`",
                "Release `0.3.0` is experimental",
            ),
        }
        for relative, phrases in required_readme_phrases.items():
            content = (REPO_ROOT / relative).read_text(encoding="utf-8")
            for phrase in phrases:
                with self.subTest(readme=relative, phrase=phrase):
                    self.assertIn(phrase, content)

        self.assertTrue(RELEASE_NOTES_PATH.is_file(), "missing v0.3.0 release notes")
        if not RELEASE_NOTES_PATH.is_file():
            return
        notes = RELEASE_NOTES_PATH.read_text(encoding="utf-8")
        for phrase in (
            "# v0.3.0 release metadata",
            "Apache-2.0",
            "https://github.com/developer0hye/pdfplumber-rs",
            "`pdfplumber` | `pdfplumber` | Alpha",
            "`pdfplumber-rs` | `pdfplumber` | Alpha",
            "`pdfplumber-cli` | `pdfplumber` | Alpha",
            "`pdfplumber-wasm` | `pdfplumber-wasm` | Experimental",
            "This GitHub release is a prerelease",
        ):
            with self.subTest(release_note_phrase=phrase):
                self.assertIn(phrase, notes)

    def test_ci_and_release_workflows_enforce_metadata(self) -> None:
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        for command in (
            "python scripts/check_package_metadata.py --source",
            "python scripts/check_package_metadata.py --rust target/package/*.crate",
            "python scripts/check_package_metadata.py --python dist/*.whl dist/*.tar.gz",
            "python scripts/check_package_metadata.py --npm crates/pdfplumber-wasm/pkg",
        ):
            with self.subTest(ci_command=command):
                self.assertIn(command, ci)

        release = (REPO_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        for phrase in (
            "python scripts/check_package_metadata.py --release-tag",
            '"$GITHUB_REF_NAME" --github-output "$GITHUB_OUTPUT"',
            "body_path: ${{ needs.metadata.outputs.release-notes }}",
            "prerelease: ${{ needs.metadata.outputs.prerelease }}",
            "generate_release_notes: true",
        ):
            with self.subTest(release_workflow=phrase):
                self.assertIn(phrase, release)
        self.assertEqual(release.count("needs: [ci, metadata]"), 4)
        self.assertIn("needs: [publish, metadata]", release)


if __name__ == "__main__":
    unittest.main()
