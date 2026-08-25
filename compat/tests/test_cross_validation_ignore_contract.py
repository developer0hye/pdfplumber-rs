"""Scheduling contract for the recovered cases tracked by issue #217."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

RECOVERED_CASES = {
    "cv_pdfbox_3127_vfont",
    "cv_pdfjs_vertical",
    "cv_python_annotations_rot180",
    "cv_python_annotations_rot270",
    "cv_python_issue_1147",
    "cv_python_issue_1181",
}

RETAINED_IGNORED_CASES = {
    "cv_python_issue_1279",
    "cv_python_issue_848",
}


def ignored_cross_validation_tests() -> set[str]:
    """Return the tests the compiled Rust harness still marks ignored."""

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
        raise AssertionError("could not enumerate ignored tests:\n" + output)

    return {
        line.removesuffix(": test")
        for line in completed.stdout.splitlines()
        if line.endswith(": test")
    }


class CrossValidationIgnoreContractTests(unittest.TestCase):
    def test_recovered_cases_are_normal_tests(self) -> None:
        ignored = ignored_cross_validation_tests()

        stale_ignored = RECOVERED_CASES & ignored
        self.assertFalse(
            stale_ignored,
            f"recovered cases are still ignored: {sorted(stale_ignored)}",
        )
        self.assertTrue(
            RETAINED_IGNORED_CASES <= ignored,
            "unresolved cases must remain visible as ignored tests: "
            f"{sorted(RETAINED_IGNORED_CASES - ignored)}",
        )


if __name__ == "__main__":
    unittest.main()
