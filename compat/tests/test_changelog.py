"""Contracts for the user-facing project changelog."""

from __future__ import annotations

import re
import unittest
from datetime import date
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"
KEEP_A_CHANGELOG_URL = "https://keepachangelog.com/en/2.0.0/"
SEMVER_URL = "https://semver.org/spec/v2.0.0.html"
RELEASE_HEADING = re.compile(
    r"^## \[(?P<version>\d+\.\d+\.\d+)\] - (?P<released>\d{4}-\d{2}-\d{2})$",
    re.MULTILINE,
)
CHANGE_TYPE_HEADING = re.compile(r"^### (?P<change_type>[^\n]+)$", re.MULTILINE)
ALLOWED_CHANGE_TYPES = {
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
}
EXPECTED_RELEASES = [
    ("0.4.0", "2026-08-30"),
    ("0.3.0", "2026-08-22"),
]


class ChangelogContractTests(unittest.TestCase):
    def changelog(self) -> str:
        self.assertTrue(CHANGELOG_PATH.is_file(), "missing CHANGELOG.md")
        if not CHANGELOG_PATH.is_file():
            return ""
        return CHANGELOG_PATH.read_text(encoding="utf-8")

    def test_changelog_declares_its_format_and_release_order(self) -> None:
        changelog = self.changelog()
        if not changelog:
            return

        self.assertTrue(changelog.startswith("# Changelog\n"))
        self.assertIn(f"[Keep a Changelog]({KEEP_A_CHANGELOG_URL})", changelog)
        self.assertIn(f"[Semantic Versioning]({SEMVER_URL})", changelog)
        self.assertEqual(changelog.count("## [Unreleased]"), 1)

        releases = RELEASE_HEADING.findall(changelog)
        self.assertEqual(releases, EXPECTED_RELEASES)
        self.assertLess(
            changelog.index("## [Unreleased]"),
            changelog.index(f"## [{releases[0][0]}]"),
        )
        versions = [tuple(map(int, version.split("."))) for version, _ in releases]
        self.assertEqual(versions, sorted(versions, reverse=True))
        for version, released in releases:
            with self.subTest(version=version):
                date.fromisoformat(released)

    def test_changelog_uses_only_nonempty_standard_change_types(self) -> None:
        changelog = self.changelog()
        if not changelog:
            return

        headings = list(CHANGE_TYPE_HEADING.finditer(changelog))
        self.assertTrue(headings)
        for index, heading in enumerate(headings):
            change_type = heading.group("change_type")
            end = (
                headings[index + 1].start()
                if index + 1 < len(headings)
                else len(changelog)
            )
            body = changelog[heading.end() : end]
            with self.subTest(change_type=change_type):
                self.assertIn(change_type, ALLOWED_CHANGE_TYPES)
                self.assertRegex(body, r"(?m)^- \S")

    def test_changelog_covers_user_visible_change_areas(self) -> None:
        changelog = self.changelog()
        if not changelog:
            return

        for area in ("Compatibility", "API", "Performance", "Platform", "Migration"):
            with self.subTest(area=area):
                self.assertRegex(changelog, rf"(?m)^- \*\*{area}:\*\* \S")

    def test_changelog_has_release_comparison_links_and_readme_navigation(self) -> None:
        changelog = self.changelog()
        if not changelog:
            return

        repository = "https://github.com/developer0hye/pdfplumber-rs"
        latest_version = EXPECTED_RELEASES[0][0]
        self.assertIn(
            f"[Unreleased]: {repository}/compare/v{latest_version}...HEAD",
            changelog,
        )
        for index, (version, _) in enumerate(EXPECTED_RELEASES):
            previous = (
                EXPECTED_RELEASES[index + 1][0]
                if index + 1 < len(EXPECTED_RELEASES)
                else "0.2.0"
            )
            with self.subTest(version=version):
                self.assertIn(
                    f"[{version}]: {repository}/compare/v{previous}...v{version}",
                    changelog,
                )

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("[changelog](CHANGELOG.md)", readme)


if __name__ == "__main__":
    unittest.main()
