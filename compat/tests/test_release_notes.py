"""Contracts for the versioned, user-facing release notes."""

from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_NOTES_PATH = REPO_ROOT / "docs" / "releases" / "v0.3.0.md"
SUPPORT_MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"
GENERATOR_PATH = REPO_ROOT / "scripts" / "generate_release_notes.py"
REQUIRED_SECTIONS = (
    "Who should upgrade?",
    "Behavior changes",
    "Known limitations",
    "Artifact matrix",
    "Evidence",
)


def markdown_section(document: str, heading: str) -> str:
    match = re.search(
        rf"^## {re.escape(heading)}\n(?P<body>.*?)(?=^## |\Z)",
        document,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        return ""
    return match.group("body").strip()


def changelog_release_body(changelog: str, version: str) -> str:
    match = re.search(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}\n"
        r"(?P<body>.*?)(?=^## |^\[Unreleased\]:|\Z)",
        changelog,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        return ""
    return match.group("body").strip()


def normalize_markdown(value: str) -> str:
    return " ".join(value.split())


class ReleaseNoteContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.notes = RELEASE_NOTES_PATH.read_text(encoding="utf-8")
        cls.matrix = tomllib.loads(SUPPORT_MATRIX_PATH.read_text(encoding="utf-8"))

    def test_required_sections_are_present_once_and_in_order(self) -> None:
        self.assertTrue(self.notes.startswith("# v0.3.0 release notes\n"))
        self.assertEqual(
            re.findall(r"^## (.+)$", self.notes, re.MULTILINE),
            list(REQUIRED_SECTIONS),
        )

        generated = subprocess.run(
            [sys.executable, str(GENERATOR_PATH), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(generated.returncode, 0, generated.stdout + generated.stderr)
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("python scripts/generate_release_notes.py --check", ci)

    def test_upgrade_guidance_and_behavior_are_source_backed(self) -> None:
        upgrade_guidance = self.matrix["release_upgrade_guidance"]
        self.assertGreaterEqual(len(upgrade_guidance), 3)
        who_should_upgrade = markdown_section(self.notes, "Who should upgrade?")
        for guidance in upgrade_guidance:
            with self.subTest(guidance=guidance):
                self.assertIn(f"- {guidance}", who_should_upgrade)

        changelog = CHANGELOG_PATH.read_text(encoding="utf-8")
        release_changes = changelog_release_body(
            changelog,
            self.matrix["release_version"],
        )
        self.assertTrue(release_changes)
        behavior_changes = markdown_section(self.notes, "Behavior changes")
        self.assertIn(
            normalize_markdown(release_changes),
            normalize_markdown(behavior_changes),
        )

    def test_limitations_and_artifacts_match_the_support_matrix(self) -> None:
        limitations = markdown_section(self.notes, "Known limitations")
        artifacts = markdown_section(self.notes, "Artifact matrix")

        for surface in self.matrix["surfaces"]:
            with self.subTest(surface=surface["id"]):
                for limitation in surface["known_limitations"]:
                    self.assertIn(
                        f"- **{surface['name']}:** {limitation}",
                        limitations,
                    )
                self.assertIn(
                    f"[`{surface['package']}`]({surface['registry_url']})",
                    artifacts,
                )
                self.assertIn(f"`{surface['source_version']}`", artifacts)
                self.assertIn(f"`{surface['registry_version']}`", artifacts)
                self.assertIn(surface["ci_verified_platforms"][0], artifacts)

        self.assertIn("Not published for this release", artifacts)
        self.assertIn("Published for this release", artifacts)

    def test_evidence_links_are_versioned_and_release_workflow_keeps_them(self) -> None:
        evidence = markdown_section(self.notes, "Evidence")
        repository_files = f"{self.matrix['repository']}/blob/main"
        for link in (
            f"[Changelog]({repository_files}/CHANGELOG.md#030---2026-08-22)",
            f"[Support matrix]({repository_files}/docs/support.md)",
            f"[Readiness snapshot]({repository_files}/docs/readiness/v0.3.0.md)",
            f"[Evidence ledger]({repository_files}/PRD.md#13-evidence-ledger)",
            f"[Continuous Integration gates]({repository_files}/.github/workflows/ci.yml)",
            f"[Release workflow]({repository_files}/.github/workflows/release.yml)",
        ):
            with self.subTest(link=link):
                self.assertIn(link, evidence)

        workflow = (REPO_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "body_path: ${{ needs.metadata.outputs.release-notes }}", workflow
        )
        self.assertIn("generate_release_notes: true", workflow)


if __name__ == "__main__":
    unittest.main()
