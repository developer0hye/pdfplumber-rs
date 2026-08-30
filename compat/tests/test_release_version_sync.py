"""Single-source release-version contracts (DIST-014)."""

from __future__ import annotations

import importlib.util
import re
import subprocess
import sys
import unittest
from pathlib import Path
from types import ModuleType

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKSPACE_PATH = REPO_ROOT / "Cargo.toml"
SUPPORT_PATH = REPO_ROOT / "support-matrix.toml"
READINESS_PATH = REPO_ROOT / "readiness.toml"
PYPROJECT_PATH = REPO_ROOT / "crates" / "pdfplumber-py" / "pyproject.toml"
NATIVE_SOURCE_PATH = REPO_ROOT / "crates" / "pdfplumber-py" / "src" / "lib.rs"
NATIVE_TEST_PATH = (
    REPO_ROOT / "crates" / "pdfplumber-py" / "tests" / "test_native_layout.py"
)
WASM_SOURCE_PATH = REPO_ROOT / "crates" / "pdfplumber-wasm" / "src" / "lib.rs"
METADATA_CHECKER_PATH = REPO_ROOT / "scripts" / "check_package_metadata.py"
RELEASE_HELPER_PATH = REPO_ROOT / "scripts" / "release_version.py"
VERSION_GUIDE_PATH = REPO_ROOT / "docs" / "release-versioning.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
CI_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
README_PATH = REPO_ROOT / "README.md"


def load_toml(path: Path) -> dict[str, object]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def load_release_helper() -> ModuleType | None:
    if not RELEASE_HELPER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("release_version", RELEASE_HELPER_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ReleaseVersionSyncTests(unittest.TestCase):
    def test_workspace_package_version_is_inherited_by_every_member(self) -> None:
        workspace = load_toml(WORKSPACE_PATH)["workspace"]
        release_version = workspace["package"]["version"]
        self.assertRegex(release_version, r"^(?:0|[1-9][0-9]*)\.[0-9]+\.[0-9]+$")
        members = workspace["members"]
        self.assertEqual(
            members,
            [
                "crates/pdfplumber-core",
                "crates/pdfplumber-parse",
                "crates/pdfplumber",
                "crates/pdfplumber-cli",
                "crates/pdfplumber-py",
                "crates/pdfplumber-wasm",
            ],
        )
        for member in members:
            package = load_toml(REPO_ROOT / member / "Cargo.toml")["package"]
            with self.subTest(member=member):
                self.assertEqual(package["version"], {"workspace": True})

    def test_internal_dependency_requirements_match_the_workspace_release(self) -> None:
        workspace = load_toml(WORKSPACE_PATH)["workspace"]
        release_version = workspace["package"]["version"]
        internal_names = {
            load_toml(REPO_ROOT / member / "Cargo.toml")["package"]["name"]
            for member in workspace["members"]
        }
        requirements: list[tuple[str, str, str]] = []
        for member in workspace["members"]:
            manifest = load_toml(REPO_ROOT / member / "Cargo.toml")
            dependency_tables = [manifest.get("dependencies", {})]
            dependency_tables.extend(
                target.get("dependencies", {})
                for target in manifest.get("target", {}).values()
            )
            for dependencies in dependency_tables:
                for name, value in dependencies.items():
                    if name in internal_names and isinstance(value, dict):
                        version = value.get("version")
                        if version is not None:
                            requirements.append((member, name, version))

        self.assertTrue(requirements, "no publish-time internal versions discovered")
        for member, dependency, version in requirements:
            with self.subTest(member=member, dependency=dependency):
                self.assertEqual(version, release_version)

    def test_python_metadata_and_native_version_derive_from_cargo(self) -> None:
        pyproject = load_toml(PYPROJECT_PATH)
        self.assertEqual(pyproject["project"]["dynamic"], ["version"])

        native_source = NATIVE_SOURCE_PATH.read_text(encoding="utf-8")
        self.assertIn(
            'pub const VERSION: &str = env!("CARGO_PKG_VERSION");', native_source
        )
        self.assertIn('m.add("__version__", VERSION)?;', native_source)

        installed_test = NATIVE_TEST_PATH.read_text(encoding="utf-8")
        self.assertIn("import importlib.metadata", installed_test)
        self.assertIn('importlib.metadata.version("pdfplumber-rs")', installed_test)
        self.assertIn("_native.__version__", installed_test)

    def test_npm_candidate_uses_the_workspace_release(self) -> None:
        wasm_source = WASM_SOURCE_PATH.read_text(encoding="utf-8")
        self.assertIn('env!("CARGO_PKG_VERSION")', wasm_source)

        checker = METADATA_CHECKER_PATH.read_text(encoding="utf-8")
        self.assertIn('package.get("version") == release.version', checker)
        ci = CI_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "python scripts/check_package_metadata.py --npm "
            "crates/pdfplumber-wasm/pkg-browser",
            ci,
        )

    def test_documentation_selectors_follow_the_workspace_release(self) -> None:
        workspace_version = load_toml(WORKSPACE_PATH)["workspace"]["package"][
            "version"
        ]
        support = load_toml(SUPPORT_PATH)
        readiness = load_toml(READINESS_PATH)
        self.assertEqual(support["release_version"], workspace_version)
        self.assertEqual(readiness["release_version"], workspace_version)
        self.assertEqual(
            support["release_notes"], f"docs/releases/v{workspace_version}.md"
        )
        self.assertTrue(
            (REPO_ROOT / "docs" / "releases" / f"v{workspace_version}.md").is_file()
        )
        self.assertTrue(
            (REPO_ROOT / "docs" / "readiness" / f"v{workspace_version}.md").is_file()
        )

        self.assertTrue(VERSION_GUIDE_PATH.is_file())
        if VERSION_GUIDE_PATH.is_file():
            guide = VERSION_GUIDE_PATH.read_text(encoding="utf-8")
            for phrase in (
                "[workspace.package]",
                "version.workspace = true",
                "Python",
                "npm",
                "__version__",
                "release tag",
            ):
                with self.subTest(guide_phrase=phrase):
                    self.assertIn(phrase, guide)
        self.assertIn(
            "release-versioning.md",
            REFERENCE_INDEX_PATH.read_text(encoding="utf-8"),
        )

    def test_readme_crate_requirements_match_the_workspace_release(self) -> None:
        release_version = load_toml(WORKSPACE_PATH)["workspace"]["package"][
            "version"
        ]
        readme = README_PATH.read_text(encoding="utf-8")
        cargo_examples = re.findall(r"```toml\n(.*?)```", readme, flags=re.DOTALL)
        requirements: list[str] = []
        for cargo_example in cargo_examples:
            dependency = tomllib.loads(cargo_example).get("dependencies", {}).get(
                "pdfplumber"
            )
            if isinstance(dependency, str):
                requirements.append(dependency)
            elif isinstance(dependency, dict) and isinstance(
                dependency.get("version"), str
            ):
                requirements.append(dependency["version"])

        self.assertEqual(
            requirements,
            [release_version, release_version],
            "README must show the exact workspace release in both Cargo examples",
        )

    def test_release_helper_and_tag_gate_use_the_workspace_identity(self) -> None:
        self.assertTrue(RELEASE_HELPER_PATH.is_file(), "missing release-version helper")
        helper = load_release_helper()
        self.assertIsNotNone(helper)
        if helper is None:
            return

        release = helper.load_release_identity(REPO_ROOT)
        workspace_version = load_toml(WORKSPACE_PATH)["workspace"]["package"][
            "version"
        ]
        self.assertEqual(release.version, workspace_version)
        self.assertEqual(release.tag, f"v{workspace_version}")
        self.assertEqual(
            release.release_notes, Path(f"docs/releases/v{workspace_version}.md")
        )
        self.assertEqual(
            release.readiness, Path(f"docs/readiness/v{workspace_version}.md")
        )

        release_workflow = RELEASE_PATH.read_text(encoding="utf-8")
        self.assertIn(
            'check_package_metadata.py --release-tag "$GITHUB_REF_NAME"',
            release_workflow,
        )
        checker = METADATA_CHECKER_PATH.read_text(encoding="utf-8")
        self.assertIn("load_release_identity", checker)

    def test_release_commands_accept_the_current_identity_and_reject_drift(
        self,
    ) -> None:
        source = subprocess.run(
            [sys.executable, str(METADATA_CHECKER_PATH), "--source"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(source.returncode, 0, f"{source.stdout}{source.stderr}")

        workspace_version = load_toml(WORKSPACE_PATH)["workspace"]["package"][
            "version"
        ]
        major, minor, patch = (int(part) for part in workspace_version.split("."))
        mismatched_version = f"{major}.{minor}.{patch + 1}"
        mismatch = subprocess.run(
            [
                sys.executable,
                str(METADATA_CHECKER_PATH),
                "--release-tag",
                f"v{mismatched_version}",
                "--github-output",
                "/dev/null",
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(mismatch.returncode, 0)
        self.assertIn(
            f"release tag v{mismatched_version} != source v{workspace_version}",
            mismatch.stderr,
        )


if __name__ == "__main__":
    unittest.main()
