"""Behavioral test scheduling contract for issue #217 / PARITY-024."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

STALE_PASSING_CASES = {
    "cv_pdfbox_3127_vfont",
    "cv_pdfjs_vertical",
    "cv_python_annotations_rot180",
    "cv_python_annotations_rot270",
    "cv_python_issue_1147",
    "cv_python_issue_1181",
    "cv_python_issue_1279",
}

RETAINED_IGNORED_CASES = {
    "cv_python_hello_structure",
    "cv_python_issue_848",
}


def ignored_cross_validation_tests() -> set[str]:
    """Ask the compiled Rust test harness which tests are still ignored."""

    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "pdfplumber",
            "--test",
            "cross_validation",
            "--",
            "--ignored",
            "--list",
        ],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    output = f"{completed.stdout}\n{completed.stderr}"
    if completed.returncode != 0:
        raise AssertionError(
            "could not enumerate ignored cross-validation tests:\n" + output
        )

    return {
        line.removesuffix(": test")
        for line in completed.stdout.splitlines()
        if line.endswith(": test")
    }


class CrossValidationIgnoreContractTests(unittest.TestCase):
    def test_issue_217_cases_have_the_required_schedule(self) -> None:
        ignored = ignored_cross_validation_tests()

        stale_ignored = STALE_PASSING_CASES & ignored
        self.assertFalse(
            stale_ignored,
            f"stale passing cases are still ignored: {sorted(stale_ignored)}",
        )
        self.assertTrue(
            RETAINED_IGNORED_CASES <= ignored,
            "the two cases explicitly retained by issue #217 must stay ignored: "
            f"{sorted(RETAINED_IGNORED_CASES - ignored)}",
        )


if __name__ == "__main__":
    unittest.main()
