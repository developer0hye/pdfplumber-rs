"""Contracts for Rust release pull-request SemVer enforcement."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPO_ROOT / "scripts" / "check_rust_release.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "rust-semver.yml"
ACTION_REVISION = "6b69fcf40e9b5fb17adeb57e4b6ecd020649a239"
PUBLISHABLE_PACKAGES = (
    "pdfplumber-core",
    "pdfplumber-parse",
    "pdfplumber",
    "pdfplumber-cli",
)
SEMVER_LIBRARY_PACKAGES = "pdfplumber-core,pdfplumber-parse,pdfplumber"


class RustReleaseFixture:
    """A minimal Git repository with this project's release topology."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.output_path = root / "github-output.txt"
        self._run("git", "init", "-q")
        self._run("git", "config", "user.name", "Release Test")
        self._run("git", "config", "user.email", "release@example.com")
        self.write_versions("0.3.0")
        self.write_changelog("0.3.0", migration_note=None)
        self._run("git", "add", ".")
        self._run("git", "commit", "-q", "-m", "baseline")
        self.base_revision = self._run("git", "rev-parse", "HEAD").stdout.strip()

    def _run(self, *command: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            command,
            cwd=self.root,
            check=True,
            capture_output=True,
            text=True,
        )

    def write_versions(
        self, version: str, packages: tuple[str, ...] = PUBLISHABLE_PACKAGES
    ) -> None:
        for package in packages:
            manifest = self.root / "crates" / package / "Cargo.toml"
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text(
                f'[package]\nname = "{package}"\nversion = "{version}"\n',
                encoding="utf-8",
            )

    def write_changelog(self, version: str, migration_note: str | None) -> None:
        migration = ""
        if migration_note is not None:
            migration = (
                f"\n### Changed\n\n- **Migration:** Breaking: {migration_note}\n"
            )
        (self.root / "CHANGELOG.md").write_text(
            "# Changelog\n\n"
            "## [Unreleased]\n\n"
            f"## [{version}] - 2026-08-27\n"
            f"{migration}",
            encoding="utf-8",
        )

    def check(self, *extra_arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            (
                sys.executable,
                str(CHECKER_PATH),
                "--repo-root",
                str(self.root),
                "--base-rev",
                self.base_revision,
                "--github-output",
                str(self.output_path),
                *extra_arguments,
            ),
            cwd=self.root,
            check=False,
            capture_output=True,
            text=True,
        )


class RustReleaseCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.fixture = RustReleaseFixture(Path(self.temporary_directory.name))

    def test_non_release_change_is_reported_without_running_semver(self) -> None:
        result = self.fixture.check()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.fixture.output_path.read_text(encoding="utf-8"),
            "is-release=false\n",
        )
        self.assertIn("no publishable Rust package version changed", result.stdout)

    def test_release_must_bump_every_publishable_package_together(self) -> None:
        self.fixture.write_versions("0.4.0", packages=("pdfplumber",))

        result = self.fixture.check()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "must update all publishable Rust package versions together", result.stderr
        )
        self.assertIn("pdfplumber-cli", result.stderr)

    def test_release_exports_semver_library_packages(self) -> None:
        self.fixture.write_versions("0.4.0")
        self.fixture.write_changelog("0.4.0", migration_note=None)

        result = self.fixture.check()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.fixture.output_path.read_text(encoding="utf-8"),
            "is-release=true\n"
            "release-version=0.4.0\n"
            f"semver-packages={SEMVER_LIBRARY_PACKAGES}\n",
        )

    def test_approved_break_requires_actionable_migration_note(self) -> None:
        self.fixture.write_versions("0.4.0")
        self.fixture.write_changelog("0.4.0", migration_note=None)

        missing = self.fixture.check("--require-migration-notes")

        self.assertNotEqual(missing.returncode, 0)
        self.assertIn("actionable migration note", missing.stderr)

        self.fixture.write_changelog(
            "0.4.0",
            migration_note=(
                "`OldApi` was removed. Consumers must review the changed public "
                "surface before upgrading."
            ),
        )
        vague = self.fixture.check("--require-migration-notes")

        self.assertNotEqual(vague.returncode, 0)
        self.assertIn("concrete replacement", vague.stderr)

        self.fixture.write_changelog(
            "0.4.0",
            migration_note="`OldApi` was removed. Replace it with `NewApi` before upgrading.",
        )
        present = self.fixture.check("--require-migration-notes")

        self.assertEqual(present.returncode, 0, present.stderr)
        self.assertIn("migration note covers Rust release 0.4.0", present.stdout)


class RustReleaseWorkflowTests(unittest.TestCase):
    def workflow(self) -> str:
        self.assertTrue(WORKFLOW_PATH.is_file(), "missing Rust SemVer workflow")
        if not WORKFLOW_PATH.is_file():
            return ""
        return WORKFLOW_PATH.read_text(encoding="utf-8")

    def test_every_main_pull_request_gets_release_detection(self) -> None:
        workflow = self.workflow()

        self.assertIn("pull_request:\n    branches: [main]", workflow)
        self.assertIn("fetch-depth: 0", workflow)
        self.assertIn("github.event.pull_request.base.sha", workflow)
        self.assertIn("scripts/check_rust_release.py", workflow)
        self.assertIn('--github-output "$GITHUB_OUTPUT"', workflow)

    def test_release_semver_checks_cover_every_publishable_library(self) -> None:
        workflow = self.workflow()

        self.assertEqual(
            workflow.count("obi1kenobi/cargo-semver-checks-action@" + ACTION_REVISION),
            2,
        )
        self.assertEqual(
            workflow.count("package: ${{ steps.release.outputs.semver-packages }}"), 2
        )
        self.assertIn("if: steps.release.outputs.is-release == 'true'", workflow)

    def test_strict_detection_precedes_version_aware_approval(self) -> None:
        workflow = self.workflow()

        strict = workflow.index("id: strict_semver")
        approval = workflow.index("name: Validate approved breaking release")
        migration = workflow.index("name: Require breaking-change migration notes")
        self.assertLess(strict, approval)
        self.assertLess(approval, migration)
        self.assertIn("continue-on-error: true", workflow[strict:approval])
        self.assertIn("release-type: patch", workflow[strict:approval])
        self.assertIn(
            "steps.strict_semver.outcome == 'failure'", workflow[approval:migration]
        )

    def test_approved_breaking_release_requires_checker_migration_mode(self) -> None:
        workflow = self.workflow()
        migration_step = workflow[
            workflow.index("name: Require breaking-change migration notes") :
        ]

        self.assertIn("steps.strict_semver.outcome == 'failure'", migration_step)
        self.assertIn("--require-migration-notes", migration_step)
        self.assertIn("--base-rev", migration_step)

    def test_public_policy_reference_and_roadmap_are_traceable(self) -> None:
        rust_api = (REPO_ROOT / "docs" / "rust-api.md").read_text(encoding="utf-8")
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        references = (REPO_ROOT / "references" / "INDEX.md").read_text(encoding="utf-8")
        reference_path = REPO_ROOT / "references" / "rust-semver-checks.md"

        self.assertIn("Release SemVer gate", rust_api)
        self.assertIn("latest normal, non-yanked crates.io", rust_api)
        self.assertIn("**Migration:** Breaking:", rust_api)
        self.assertIn("release SemVer policy", readme)
        self.assertRegex(changelog, r"(?is)cargo-semver-checks.*migration notes")
        self.assertTrue(reference_path.is_file())
        self.assertIn("rust-semver-checks.md", references)
        self.assertIn("release-type: patch", reference_path.read_text(encoding="utf-8"))
        self.assertIn("DX-017", roadmap)
        self.assertNotIn("DX-012", roadmap)


if __name__ == "__main__":
    unittest.main()
