"""Contracts for public package and release metadata agreement."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import ModuleType

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "support-matrix.toml"
WORKSPACE_PATH = REPO_ROOT / "Cargo.toml"
CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_metadata.py"
RELEASE_VERSION = tomllib.loads(WORKSPACE_PATH.read_text(encoding="utf-8"))[
    "workspace"
]["package"]["version"]
RELEASE_TAG = f"v{RELEASE_VERSION}"
RELEASE_NOTES_PATH = REPO_ROOT / "docs" / "releases" / f"{RELEASE_TAG}.md"


def load_checker() -> ModuleType | None:
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location(
        "check_package_metadata", CHECKER_PATH
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    scripts_directory = str(CHECKER_PATH.parent)
    sys.path.insert(0, scripts_directory)
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(scripts_directory)
    return module


def tracked_lockfiles() -> list[Path]:
    listing = subprocess.run(
        ["git", "ls-files", "*Cargo.lock"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [REPO_ROOT / line for line in listing.stdout.split() if line]


def workspace_member_names() -> set[str]:
    workspace = tomllib.loads(WORKSPACE_PATH.read_text(encoding="utf-8"))
    names: set[str] = set()
    for member in workspace["workspace"]["members"]:
        manifest = tomllib.loads(
            (REPO_ROOT / member / "Cargo.toml").read_text(encoding="utf-8")
        )
        names.add(manifest["package"]["name"])
    return names


class PackageMetadataContractTests(unittest.TestCase):
    def test_support_matrix_tracks_the_workspace_release_identity(self) -> None:
        matrix = tomllib.loads(MATRIX_PATH.read_text(encoding="utf-8"))
        workspace = tomllib.loads(WORKSPACE_PATH.read_text(encoding="utf-8"))

        self.assertEqual(
            matrix["release_version"], workspace["workspace"]["package"]["version"]
        )
        self.assertEqual(matrix["license"], "Apache-2.0")
        self.assertEqual(
            matrix["repository"],
            "https://github.com/developer0hye/pdfplumber-rs",
        )
        self.assertEqual(matrix["release_notes"], f"docs/releases/{RELEASE_TAG}.md")
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
                    RELEASE_TAG,
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
        self.assertIn(f"release-notes=docs/releases/{RELEASE_TAG}.md\n", outputs)
        self.assertIn("prerelease=true\n", outputs)

        with tempfile.NamedTemporaryFile() as github_output:
            mismatched = subprocess.run(
                [
                    sys.executable,
                    str(CHECKER_PATH),
                    "--release-tag",
                    f"v{RELEASE_VERSION}.invalid",
                    "--github-output",
                    github_output.name,
                ],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
        self.assertNotEqual(mismatched.returncode, 0)
        self.assertIn(
            f"release tag v{RELEASE_VERSION}.invalid != source {RELEASE_TAG}",
            mismatched.stderr,
        )

    def test_public_readmes_and_release_notes_display_exact_identity(self) -> None:
        required_readme_phrases = {
            "README.md": (
                f"Release `{RELEASE_VERSION}`",
                "Rust crate `pdfplumber` (import `pdfplumber`) is alpha",
                "Python distribution `pdfplumber-rs` (import `pdfplumber`) is alpha",
                "CLI crate `pdfplumber-cli` installs `pdfplumber` and is alpha",
                "npm package `pdfplumber-wasm` is experimental",
            ),
            "crates/pdfplumber-py/README.md": (
                "Distribution `pdfplumber-rs` installs import package `pdfplumber`",
                f"Release `{RELEASE_VERSION}` is alpha",
            ),
            "crates/pdfplumber-cli/README.md": (
                "Crate `pdfplumber-cli` installs executable `pdfplumber`",
                f"Release `{RELEASE_VERSION}` is alpha",
            ),
            "crates/pdfplumber-wasm/README.md": (
                "npm package and import name are `pdfplumber-wasm`",
                f"Release `{RELEASE_VERSION}` is experimental",
            ),
        }
        for relative, phrases in required_readme_phrases.items():
            content = (REPO_ROOT / relative).read_text(encoding="utf-8")
            for phrase in phrases:
                with self.subTest(readme=relative, phrase=phrase):
                    self.assertIn(phrase, content)

        self.assertTrue(
            RELEASE_NOTES_PATH.is_file(), f"missing {RELEASE_TAG} release notes"
        )
        if not RELEASE_NOTES_PATH.is_file():
            return
        notes = RELEASE_NOTES_PATH.read_text(encoding="utf-8")
        for phrase in (
            f"# {RELEASE_TAG} release notes",
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
            "python scripts/check_package_metadata.py --npm crates/pdfplumber-wasm/pkg-browser",
        ):
            with self.subTest(ci_command=command):
                self.assertIn(command, ci)

        release = (REPO_ROOT / ".github" / "workflows" / "release.yml").read_text(
            encoding="utf-8"
        )
        for phrase in (
            "python scripts/check_package_metadata.py --release-tag",
            '"$GITHUB_REF_NAME" --github-output "$GITHUB_OUTPUT"',
            "uses: ./.github/workflows/release-candidate-scorecards.yml",
            "body_path: ${{ needs.metadata.outputs.release-notes }}",
            "prerelease: ${{ needs.metadata.outputs.prerelease }}",
            "generate_release_notes: true",
            "pattern: release-candidate-scorecards-*",
            "files: |",
            "release-scorecards/*",
        ):
            with self.subTest(release_workflow=phrase):
                self.assertIn(phrase, release)
        for dependency in (
            "needs: [ci, metadata, scorecards]",
            "needs: [release-artifacts, metadata, scorecards, integrity]",
            "needs: [release-artifacts, scorecards, integrity]",
        ):
            with self.subTest(release_dependency=dependency):
                self.assertIn(dependency, release)
        for release_dependency in (
            "publish,",
            "publish-pypi,",
            "metadata,",
            "scorecards,",
            "cli-binaries,",
            "release-artifacts,",
            "integrity,",
        ):
            with self.subTest(github_release_dependency=release_dependency):
                self.assertIn(release_dependency, release)

    def test_every_tracked_lockfile_pins_the_release_version(self) -> None:
        """A stale lock fails the `--locked` builds the release pipeline runs.

        The competitor benchmark adapter resolves the workspace crates by path,
        so a version bump that leaves its lock behind only surfaces on the
        release runner, after the tag is already published.
        """
        internal_names = workspace_member_names()
        lockfiles = tracked_lockfiles()
        self.assertTrue(lockfiles, "no tracked Cargo.lock files were found")

        for lockfile in lockfiles:
            locked = tomllib.loads(lockfile.read_text(encoding="utf-8"))
            relative = lockfile.relative_to(REPO_ROOT).as_posix()
            for entry in locked.get("package", []):
                if entry.get("name") not in internal_names:
                    continue
                with self.subTest(lockfile=relative, package=entry["name"]):
                    self.assertEqual(entry.get("version"), RELEASE_VERSION)

    def test_metadata_checker_rejects_lockfile_version_drift(self) -> None:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing package metadata checker")
        if checker is None:
            return

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "Cargo.lock").write_text(
                '[[package]]\nname = "pdfplumber"\nversion = "1.2.3"\n\n'
                '[[package]]\nname = "serde"\nversion = "1.0.0"\n',
                encoding="utf-8",
            )

            checker.check_lockfile_versions(root, {"pdfplumber"}, "1.2.3")

            with self.assertRaises(checker.MetadataError) as drifted:
                checker.check_lockfile_versions(root, {"pdfplumber"}, "4.5.6")
            self.assertIn("Cargo.lock", str(drifted.exception))
            self.assertIn("1.2.3", str(drifted.exception))

            # An external package at an unrelated version is not a workspace
            # crate and must never be rewritten to the release version.
            checker.check_lockfile_versions(root, {"absent-crate"}, "9.9.9")

    def test_lockfile_discovery_covers_nested_builds_and_skips_vendored_trees(
        self,
    ) -> None:
        checker = load_checker()
        self.assertIsNotNone(checker, "missing package metadata checker")
        if checker is None:
            return

        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            expected = {
                root / "Cargo.lock",
                root / "benchmarks" / "adapters" / "rust" / "Cargo.lock",
            }
            ignored = {
                root / "target" / "Cargo.lock",
                root / "benchmarks" / "adapters" / ".sources" / "dep" / "Cargo.lock",
            }
            for lockfile in expected | ignored:
                lockfile.parent.mkdir(parents=True, exist_ok=True)
                lockfile.write_text("version = 4\n", encoding="utf-8")

            self.assertEqual(set(checker.workspace_lockfiles(root)), expected)


if __name__ == "__main__":
    unittest.main()
