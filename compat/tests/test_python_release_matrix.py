"""Contracts for the versioned Python pdfplumber release matrix."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE = REPO_ROOT / "compat" / "python-release-matrix-v0.3.0.toml"
GENERATOR = REPO_ROOT / "scripts" / "generate_python_release_matrix.py"
OUTPUT = REPO_ROOT / "docs" / "compatibility" / "python-release-matrix-v0.3.0.md"
SCORECARD = REPO_ROOT / "docs" / "compatibility" / "scorecard-v0.3.0.json"
EXPECTED_RELEASES = tuple(f"0.11.{patch}" for patch in range(10, -1, -1))


class PythonReleaseMatrixContractTests(unittest.TestCase):
    def test_registry_enumerates_exact_011_releases_without_inference(self) -> None:
        self.require_file(SOURCE)
        with SOURCE.open("rb") as source_file:
            source = tomllib.load(source_file)

        self.assertEqual(source["schema_version"], 1)
        self.assertEqual(source["subject_version"], "0.3.0")
        self.assertEqual(source["release_series"], "0.11")
        self.assertEqual(
            source["release_index"],
            "https://github.com/jsvine/pdfplumber/tags",
        )
        releases = source["releases"]
        self.assertEqual(
            tuple(release["version"] for release in releases),
            EXPECTED_RELEASES,
        )
        self.assertEqual(
            tuple(release["tag"] for release in releases),
            tuple(f"v{version}" for version in EXPECTED_RELEASES),
        )

        observed = [release for release in releases if release["status"] == "observed"]
        self.assertEqual(len(observed), 1)
        self.assertEqual(observed[0]["version"], "0.11.10")
        self.assertEqual(
            observed[0]["scorecard"],
            "docs/compatibility/scorecard-v0.3.0.json",
        )
        for release in releases[1:]:
            with self.subTest(release=release["version"]):
                self.assertEqual(release["status"], "not_tested")
                self.assertNotIn("scorecard", release)
                self.assertIn("release-specific", release["reason"])

    def test_published_matrix_is_current_precise_and_percentage_free(self) -> None:
        for required in (SOURCE, GENERATOR, OUTPUT):
            with self.subTest(path=required.name):
                self.require_file(required)

        completed = subprocess.run(
            [sys.executable, str(GENERATOR), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

        report = OUTPUT.read_text(encoding="utf-8")
        self.assertIn(
            "# Python pdfplumber release matrix for pdfplumber-rs v0.3.0", report
        )
        self.assertIn("[compatibility terminology](terms.md)", report)
        self.assertIn("[machine-readable scorecard](scorecard-v0.3.0.json)", report)
        self.assertIn("does not make a blanket compatibility claim", report)
        self.assertIn("Unlisted releases are also not tested", report)
        self.assertNotIn("%", report)
        for version in EXPECTED_RELEASES:
            with self.subTest(release=version):
                self.assertEqual(report.count(f"[`{version}`]"), 1)

    def test_observed_row_preserves_scorecard_target_and_all_outcome_counts(
        self,
    ) -> None:
        self.require_file(OUTPUT)
        scorecard = json.loads(SCORECARD.read_text(encoding="utf-8"))
        report = OUTPUT.read_text(encoding="utf-8")

        self.assertEqual(scorecard["target"]["version"], "0.11.10")
        self.assertEqual(scorecard["target"]["tag"], "v0.11.10")
        self.assertIn(scorecard["target"]["commit"], report)
        self.assertIn(scorecard["subject"]["revision"], report)
        for status, count in scorecard["summary"]["status_counts"].items():
            with self.subTest(status=status):
                self.assertIn(f"{status}={count}", report)
        self.assertIn("Not tested — no release-specific scorecard", report)

    def test_generator_rejects_observed_release_target_mismatch(self) -> None:
        self.require_file(SOURCE)
        self.require_file(GENERATOR)
        mutated = SOURCE.read_text(encoding="utf-8").replace(
            'version = "0.11.10"\ntag = "v0.11.10"\nstatus = "observed"',
            'version = "0.11.9"\ntag = "v0.11.9"\nstatus = "observed"',
            1,
        )
        with tempfile.TemporaryDirectory() as temporary_directory:
            mutated_source = Path(temporary_directory) / SOURCE.name
            mutated_source.write_text(mutated, encoding="utf-8")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(GENERATOR),
                    "--check",
                    "--source",
                    str(mutated_source),
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertNotEqual(completed.returncode, 0)
        self.assertIn(
            "observed release 0.11.9 differs from scorecard target 0.11.10",
            completed.stderr,
        )

    def test_matrix_is_publicly_linked_and_ci_contracted(self) -> None:
        expected_path = "docs/compatibility/python-release-matrix-v0.3.0.md"
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        terms = (REPO_ROOT / "docs" / "compatibility" / "terms.md").read_text(
            encoding="utf-8"
        )
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn(expected_path, readme)
        self.assertIn("python-release-matrix-v0.3.0.md", terms)
        self.assertIn("Python-release compatibility matrix", changelog)
        self.assertIn(
            "python scripts/generate_python_release_matrix.py --check",
            workflow,
        )

    def require_file(self, path: Path) -> None:
        self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    unittest.main()
