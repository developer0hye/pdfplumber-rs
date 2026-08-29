"""Contracts for Python support claims and installed-artifact coverage."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
PYPROJECT_PATH = REPO_ROOT / "crates" / "pdfplumber-py" / "pyproject.toml"
CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_ARTIFACTS_PATH = (
    REPO_ROOT / ".github" / "workflows" / "release-artifacts.yml"
)
CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_metadata.py"
GUIDE_PATH = REPO_ROOT / "docs" / "python-support.md"
REFERENCE_PATH = REPO_ROOT / "references" / "python-support-metadata.md"


class PythonSupportPolicyContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = tomllib.loads(MATRIX_PATH.read_text(encoding="utf-8"))
        self.pyproject = tomllib.loads(PYPROJECT_PATH.read_text(encoding="utf-8"))
        self.ci = CI_PATH.read_text(encoding="utf-8")
        self.release_artifacts = RELEASE_ARTIFACTS_PATH.read_text(encoding="utf-8")
        self.checker = CHECKER_PATH.read_text(encoding="utf-8")

    def test_policy_names_only_the_installed_artifact_matrix(self) -> None:
        policy = self.matrix.get("python_support") or {}

        self.assertEqual(policy.get("implementation"), "CPython")
        self.assertEqual(policy.get("tested_versions"), ["3.13"])
        self.assertEqual(policy.get("installed_artifacts"), ["wheel", "sdist"])
        self.assertEqual(policy.get("explicitly_excluded_versions"), ["3.14"])

    def test_static_metadata_is_derived_from_the_policy(self) -> None:
        project = self.pyproject["project"]
        python_classifiers = {
            classifier
            for classifier in project["classifiers"]
            if classifier.startswith("Programming Language :: Python")
        }

        self.assertEqual(project["requires-python"].replace(" ", ""), ">=3.13,<3.14")
        self.assertEqual(
            python_classifiers,
            {
                "Programming Language :: Python :: 3",
                "Programming Language :: Python :: 3.13",
                "Programming Language :: Python :: Implementation :: CPython",
            },
        )

    def test_required_ci_consumes_the_policy_as_its_matrix(self) -> None:
        for phrase in (
            "--python-support-matrix",
            "needs: python-support-policy",
            "matrix: ${{ fromJSON(needs.python-support-policy.outputs.matrix) }}",
            "python-version: ${{ matrix.python-version }}",
        ):
            with self.subTest(ci_phrase=phrase):
                self.assertIn(phrase, self.ci)

        for phrase in (
            'modes.add_argument("--python-support-matrix", action="store_true")',
            'output.write(f"matrix={json.dumps(matrix_output, separators=(\',\', \':\'))}\\n")',
            'metadata.get("Requires-Python")',
        ):
            with self.subTest(checker_phrase=phrase):
                self.assertIn(phrase, self.checker)

    def test_release_wheels_do_not_target_unclaimed_versions(self) -> None:
        wheels = self.release_artifacts.split("\n  wheels:\n", 1)[1].split(
            "\n  sdist:\n", 1
        )[0]
        interpreters = re.findall(r'^\s+interpreter: "([^"]+)"$', wheels, re.MULTILINE)

        self.assertEqual(len(interpreters), 5)
        self.assertEqual(set(interpreters), {"3.13"})
        for unsupported in ("3.9", "3.10", "3.11", "3.12", "3.14"):
            with self.subTest(unsupported=unsupported):
                self.assertNotRegex(
                    wheels,
                    rf"(?m)^[ \t]+{re.escape(unsupported)}[ \t]*$",
                )

    def test_public_guidance_states_the_exclusions(self) -> None:
        for path in (GUIDE_PATH, REFERENCE_PATH):
            with self.subTest(path=path.relative_to(REPO_ROOT)):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        if GUIDE_PATH.is_file():
            guide = GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "CPython 3.13",
                "Python 3.14 is excluded",
                "PyPy is not supported",
                "wheel and source distribution",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)

        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        for task in ("DIST-008", "DIST-009", "PYAPI-016"):
            with self.subTest(task=task):
                self.assertIn(f"- [ ] **{task}**", prd)


if __name__ == "__main__":
    unittest.main()
