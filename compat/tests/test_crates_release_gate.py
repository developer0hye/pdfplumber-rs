"""Contracts for verified crates.io release-candidate packages."""

from __future__ import annotations

import io
import json
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_crates_release

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPO_ROOT / "scripts" / "check_crates_release.py"
CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "crates-release.md"
PUBLISHABLE_PACKAGES = [
    "pdfplumber-core",
    "pdfplumber-parse",
    "pdfplumber",
    "pdfplumber-cli",
]


class CratesReleaseGateTests(unittest.TestCase):
    def checker(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        self.assertTrue(CHECKER_PATH.is_file(), "missing crates.io release checker")
        if not CHECKER_PATH.is_file():
            return subprocess.CompletedProcess([], 1, "", "checker is missing")
        return subprocess.run(
            (
                sys.executable,
                str(CHECKER_PATH),
                "--repo-root",
                str(REPO_ROOT),
                *arguments,
            ),
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_checker_discovers_every_publishable_workspace_package(self) -> None:
        result = self.checker("--list-packages")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), PUBLISHABLE_PACKAGES)

    def test_checker_rejects_a_non_exact_commit_before_packaging(self) -> None:
        mismatched_commit = "0" * 40
        result = self.checker("--expected-commit", mismatched_commit)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact source commit", result.stderr)
        self.assertIn(mismatched_commit, result.stderr)

    def test_archive_accepts_cargo_clean_omission_and_rejects_dirty_true(
        self,
    ) -> None:
        source_commit = "1" * 40
        package = check_crates_release.WorkspacePackage(
            name="example-package",
            version="1.2.3",
            manifest_path=Path("example-package/Cargo.toml"),
            dependencies=frozenset(),
        )

        with tempfile.TemporaryDirectory() as temporary_directory:
            archive = Path(temporary_directory) / "example-package-1.2.3.crate"

            def write_archive(git: dict[str, object]) -> None:
                vcs_info = json.dumps(
                    {"git": git, "path_in_vcs": "example-package"}
                ).encode("utf-8")
                member = tarfile.TarInfo(
                    "example-package-1.2.3/.cargo_vcs_info.json"
                )
                member.size = len(vcs_info)
                with tarfile.open(archive, mode="w:gz") as package_archive:
                    package_archive.addfile(member, io.BytesIO(vcs_info))

            write_archive({"sha1": source_commit})
            check_crates_release.verify_archive(archive, package, source_commit)

            write_archive({"sha1": source_commit, "dirty": True})
            with self.assertRaisesRegex(
                check_crates_release.CratesReleaseError,
                "not bound to clean commit",
            ):
                check_crates_release.verify_archive(
                    archive,
                    package,
                    source_commit,
                )

    def test_full_gate_does_not_require_dry_run_to_retain_an_archive(self) -> None:
        source_commit = "2" * 40
        package = check_crates_release.WorkspacePackage(
            name="example-package",
            version="1.2.3",
            manifest_path=Path("example-package/Cargo.toml"),
            dependencies=frozenset(),
        )

        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            workspace = check_crates_release.Workspace(
                root=root,
                target_directory=root / "target",
                packages=(package,),
            )
            archive = check_crates_release.archive_path(workspace, package)

            def simulate_cargo(command: list[str], cwd: Path) -> None:
                self.assertEqual(cwd, root)
                archive.parent.mkdir(parents=True, exist_ok=True)
                archive.unlink(missing_ok=True)
                if command[1] != "package":
                    return
                vcs_info = json.dumps(
                    {
                        "git": {"sha1": source_commit},
                        "path_in_vcs": "example-package",
                    }
                ).encode("utf-8")
                member = tarfile.TarInfo(
                    "example-package-1.2.3/.cargo_vcs_info.json"
                )
                member.size = len(vcs_info)
                with tarfile.open(archive, mode="w:gz") as package_archive:
                    package_archive.addfile(member, io.BytesIO(vcs_info))

            with mock.patch.object(
                check_crates_release,
                "run_live",
                side_effect=simulate_cargo,
            ):
                check_crates_release.run_gate(
                    workspace,
                    source_commit,
                    package_only=False,
                )

    def test_continuous_integration_builds_verified_candidate_archives(self) -> None:
        workflow = CI_PATH.read_text(encoding="utf-8")
        artifact_step = workflow[
            workflow.index("name: Build and verify Rust crate artifacts") :
            workflow.index("name: Test rendered Rust and CLI quick starts")
        ]

        self.assertIn("scripts/check_crates_release.py", artifact_step)
        self.assertIn('--expected-commit "$GITHUB_SHA"', artifact_step)
        self.assertIn("--package-only", artifact_step)
        self.assertNotIn("--no-verify", artifact_step)
        self.assertIn("check_package_licenses.py", artifact_step)
        self.assertIn("check_package_metadata.py", artifact_step)

    def test_tag_preflight_blocks_publication_until_every_dry_run_passes(self) -> None:
        workflow = RELEASE_PATH.read_text(encoding="utf-8")
        preflight = workflow.index("crates-package-preflight:")
        publish = workflow.index("\n  publish:")

        self.assertLess(preflight, publish)
        preflight_job = workflow[preflight:publish]
        self.assertIn("needs: [ci, metadata]", preflight_job)
        self.assertIn("scripts/check_crates_release.py", preflight_job)
        self.assertIn('--release-tag "$GITHUB_REF_NAME"', preflight_job)
        self.assertNotIn("--package-only", preflight_job)
        self.assertNotIn("--no-verify", preflight_job)

        publish_job = workflow[publish:]
        self.assertIn("needs: [crates-package-preflight, metadata]", publish_job)
        for package in PUBLISHABLE_PACKAGES:
            with self.subTest(package=package):
                self.assertEqual(
                    publish_job.count(f"cargo publish -p {package}\n"),
                    1,
                )
        self.assertNotIn("cargo publish --no-verify", publish_job)

    def test_public_guide_states_candidate_and_registry_boundaries(self) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), "missing crates.io release guide")
        if not GUIDE_PATH.is_file():
            return
        guide = GUIDE_PATH.read_text(encoding="utf-8")

        for phrase in (
            "cargo package",
            "cargo publish --dry-run",
            "exact release commit",
            "command-line `[patch.crates-io]`",
            "does not upload",
            "registry-backed verification",
            "DIST-007",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, guide)

    def test_prd_and_roadmap_advance_after_verified_package_evidence(self) -> None:
        prd = (REPO_ROOT / "PRD.md").read_text(encoding="utf-8")
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")

        self.assertIn(
            "- [x] **DIST-001** Build and smoke-test every crates.io package",
            prd,
        )
        self.assertIn("| `DIST-001` | 2026-08-28 | Codex |", prd)
        self.assertIn("Turn compatibility results into a public scorecard", roadmap)
        self.assertIn("`SCORE-010`", roadmap)
        self.assertNotIn(
            "Detailed tasks: [`DIST-001`](PRD.md#824-p1--distribution-and-installation)",
            roadmap,
        )


if __name__ == "__main__":
    unittest.main()
