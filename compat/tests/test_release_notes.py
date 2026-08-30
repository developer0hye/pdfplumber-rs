"""Contracts for the versioned, user-facing release notes."""

from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
SUPPORT_MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
SUPPORT_MATRIX = tomllib.loads(SUPPORT_MATRIX_PATH.read_text(encoding="utf-8"))
RELEASE_NOTES_PATH = REPO_ROOT / SUPPORT_MATRIX["release_notes"]
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


def without_claim_evidence(value: str) -> str:
    return re.sub(
        r" \(\[evidence\]\(https://github\.com/developer0hye/"
        r"pdfplumber-rs/blob/main/[^)]+\)\)",
        "",
        value,
    )


class ReleaseNoteContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.notes = RELEASE_NOTES_PATH.read_text(encoding="utf-8")
        cls.matrix = SUPPORT_MATRIX

    def test_required_sections_are_present_once_and_in_order(self) -> None:
        version = self.matrix["release_version"]
        self.assertTrue(self.notes.startswith(f"# v{version} release notes\n"))
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
            normalize_markdown(without_claim_evidence(behavior_changes)),
        )

    def test_notes_name_every_surface_absent_from_its_public_registry(self) -> None:
        """A partial publication must be visible in the release notes.

        A release can publish some registries and fail others, so the notes may
        not single out one surface as the only unpublished one.
        """
        unpublished = [
            surface
            for surface in SUPPORT_MATRIX["surfaces"]
            if surface["source_version"] != surface["registry_version"]
        ]

        summary = next(
            line for line in self.notes.splitlines() if "is a prerelease" in line
        )

        for surface in unpublished:
            with self.subTest(surface=surface["id"]):
                # The summary paragraph is what a reader sees before the table,
                # so it must not leave any unpublished registry unmentioned.
                self.assertIn(surface["name"], summary)
                self.assertIn(f"`{surface['registry_version']}`", summary)
                self.assertIn(
                    f"| `{surface['source_version']}` | "
                    f"`{surface['registry_version']}` | "
                    "Not published for this release |",
                    self.notes,
                )

        for surface in SUPPORT_MATRIX["surfaces"]:
            if surface in unpublished:
                continue
            with self.subTest(published=surface["id"]):
                self.assertIn(
                    f"| `{surface['source_version']}` | "
                    f"`{surface['registry_version']}` | "
                    "Published for this release |",
                    self.notes,
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

                expected_state = (
                    "Published for this release"
                    if surface["source_version"] == surface["registry_version"]
                    else "Not published for this release"
                )
                self.assertIn(expected_state, artifacts)

    def test_evidence_links_are_versioned_and_release_workflow_keeps_them(self) -> None:
        evidence = markdown_section(self.notes, "Evidence")
        repository_files = f"{self.matrix['repository']}/blob/main"
        version = self.matrix["release_version"]
        release_match = re.search(
            rf"^## \[{re.escape(version)}\] - (?P<date>\d{{4}}-\d{{2}}-\d{{2}})$",
            CHANGELOG_PATH.read_text(encoding="utf-8"),
            re.MULTILINE,
        )
        self.assertIsNotNone(release_match)
        assert release_match is not None
        changelog_anchor = f"{version.replace('.', '')}---{release_match['date']}"
        for link in (
            f"[Changelog]({repository_files}/CHANGELOG.md#{changelog_anchor})",
            f"[Support matrix]({repository_files}/docs/support.md)",
            f"[Readiness snapshot]({repository_files}/docs/readiness/v{version}.md)",
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
