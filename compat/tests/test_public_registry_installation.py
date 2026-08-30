"""Contracts for exact public-registry installation after publication."""

from __future__ import annotations

import importlib.util
import io
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = REPO_ROOT / "scripts" / "check_public_registry_release.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "post-publish-verification.md"
RECOVERY_PATH = REPO_ROOT / "docs" / "release-recovery.md"
README_PATH = REPO_ROOT / "README.md"
PRD_PATH = REPO_ROOT / "PRD.md"
RELEASE_VERSION = tomllib.loads(
    (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
)["workspace"]["package"]["version"]


def load_checker():
    if not CHECKER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location(
        "check_public_registry_release", CHECKER_PATH
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def job(workflow: str, name: str) -> str:
    match = re.search(
        rf"^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-z0-9-]+:\n|\Z)",
        workflow,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        return ""
    return match.group(0)


class PublicRegistryInstallationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.checker = load_checker()

    def require_checker(self):
        self.assertIsNotNone(
            self.checker, "missing public-registry installation checker"
        )
        return self.checker

    def test_release_tag_must_match_the_workspace_version_exactly(self) -> None:
        checker = self.require_checker()
        self.assertEqual(checker.release_version_from_tag("v1.2.3", "1.2.3"), "1.2.3")
        for tag, workspace_version in (
            ("1.2.3", "1.2.3"),
            ("v1.2.4", "1.2.3"),
            ("vlatest", "1.2.3"),
        ):
            with (
                self.subTest(tag=tag, workspace_version=workspace_version),
                self.assertRaises(checker.PublicRegistryError),
            ):
                checker.release_version_from_tag(tag, workspace_version)

    def test_registry_resolution_is_exact_bounded_and_retried(self) -> None:
        checker = self.require_checker()
        current = [0.0]
        observations = iter((False, False, True))
        calls: list[tuple[str, str, float]] = []

        def probe(family: str, version: str, timeout_seconds: float) -> bool:
            calls.append((family, version, timeout_seconds))
            return next(observations)

        result = checker.wait_for_registry(
            "pypi",
            "1.2.3",
            timeout_seconds=10.0,
            initial_delay_seconds=1.0,
            maximum_delay_seconds=2.0,
            probe_timeout_seconds=3.0,
            probe=probe,
            monotonic=lambda: current[0],
            sleeper=lambda delay: current.__setitem__(0, current[0] + delay),
            emit=lambda _message: None,
        )

        self.assertEqual(result.family, "pypi")
        self.assertEqual(result.version, "1.2.3")
        self.assertEqual(result.attempts, 3)
        self.assertEqual([call[:2] for call in calls], [("pypi", "1.2.3")] * 3)
        self.assertTrue(all(0 < call[2] <= 3.0 for call in calls))

        current[0] = 0.0
        with self.assertRaises(checker.PublicRegistryError):
            checker.wait_for_registry(
                "npm",
                "1.2.3",
                timeout_seconds=2.0,
                initial_delay_seconds=1.0,
                maximum_delay_seconds=1.0,
                probe_timeout_seconds=1.0,
                probe=lambda _family, _version, _timeout: False,
                monotonic=lambda: current[0],
                sleeper=lambda delay: current.__setitem__(0, current[0] + delay),
                emit=lambda _message: None,
            )

    def test_install_commands_cannot_fall_back_to_the_checkout(self) -> None:
        checker = self.require_checker()
        root = Path("/tmp/public-registry")
        python = root / "venv" / "bin" / "python"

        self.assertEqual(
            checker.cargo_install_command("1.2.3", root),
            (
                "cargo",
                "install",
                "pdfplumber-cli",
                "--version",
                "=1.2.3",
                "--registry",
                "crates-io",
                "--locked",
                "--root",
                str(root),
                "--color",
                "never",
            ),
        )
        self.assertEqual(
            checker.pypi_install_command(python, "1.2.3"),
            (
                str(python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-cache-dir",
                "--only-binary=:all:",
                "--index-url",
                "https://pypi.org/simple",
                "pdfplumber-rs==1.2.3",
            ),
        )
        self.assertEqual(
            checker.npm_install_command("1.2.3"),
            (
                "npm",
                "install",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--package-lock=false",
                "--save-exact",
                "--registry",
                "https://registry.npmjs.org",
                "pdfplumber-wasm@1.2.3",
            ),
        )

        flattened = " ".join(
            checker.cargo_install_command("1.2.3", root)
            + checker.pypi_install_command(python, "1.2.3")
            + checker.npm_install_command("1.2.3")
        )
        self.assertNotIn("--path", flattened)
        self.assertNotIn("file:", flattened)

    def test_each_installed_family_runs_the_exact_fixture_smoke(self) -> None:
        checker = self.require_checker()
        source = CHECKER_PATH.read_text(encoding="utf-8")
        for symbol in (
            "run_crates_smoke",
            "run_pypi_smoke",
            "run_npm_browser_smoke",
            "cli-release-smoke.toml",
            "run_browser_consumer",
            "validate_runtime_result",
            "https://crates.io/api/v1/crates/pdfplumber-cli",
            "https://registry.npmjs.org/pdfplumber-wasm",
            "CARGO_HOME",
            "npm_config_cache",
            "--porcelain=v1",
        ):
            with self.subTest(symbol=symbol):
                self.assertIn(symbol, source)
        self.assertEqual(set(checker.PUBLIC_FAMILIES), {"crates", "pypi", "npm"})

    def test_release_workflow_fails_closed_before_github_release(self) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        dependencies = {
            "postpublish-crates": "publish",
            "postpublish-pypi": "publish-pypi",
            "postpublish-npm": "publish-npm",
        }
        for check_job, publish_job in dependencies.items():
            with self.subTest(check_job=check_job):
                body = job(workflow, check_job)
                self.assertTrue(body, f"missing {check_job} job")
                self.assertIn(publish_job, body)
                self.assertIn("check_public_registry_release.py", body)
                self.assertIn('--release-tag "$GITHUB_REF_NAME"', body)
                self.assertIn("--timeout-seconds 600", body)

        release = job(workflow, "release")
        for check_job in dependencies:
            with self.subTest(release_dependency=check_job):
                self.assertIn(check_job, release)

    def test_identity_failures_are_retained_as_incomplete_evidence(self) -> None:
        checker = self.require_checker()
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "failed.json"
            with (
                mock.patch.object(
                    checker,
                    "source_commit",
                    side_effect=checker.PublicRegistryError("source checkout is dirty"),
                ),
                mock.patch("sys.stderr", new=io.StringIO()),
            ):
                result = checker.main(
                    [
                        "crates",
                        "--release-tag",
                        f"v{RELEASE_VERSION}",
                        "--output",
                        str(output),
                    ]
                )
            self.assertEqual(result, 1)
            evidence = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(evidence["family"], "crates")
            self.assertEqual(evidence["version"], RELEASE_VERSION)
            self.assertEqual(evidence["outcome"], "failed")
            self.assertIn("source checkout is dirty", evidence["error"])

    def test_public_guide_and_recovery_runbook_describe_partial_publication(
        self,
    ) -> None:
        self.assertTrue(GUIDE_PATH.is_file(), "missing post-publish verification guide")
        guide = GUIDE_PATH.read_text(encoding="utf-8") if GUIDE_PATH.is_file() else ""
        recovery = RECOVERY_PATH.read_text(encoding="utf-8")
        for phrase in (
            "public registries",
            "exact version",
            "GitHub Release",
            "incomplete",
            "crates.io",
            "PyPI",
            "npm",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, guide)
        self.assertIn("post-publish installation", recovery)
        self.assertIn(
            "[post-publish verification](docs/post-publish-verification.md)",
            README_PATH.read_text(encoding="utf-8"),
        )

    def test_prd_records_the_red_first_partial_claim(self) -> None:
        prd = PRD_PATH.read_text(encoding="utf-8")
        self.assertIn("- [ ] **DIST-007**", prd)
        active_row = next(
            (
                line
                for line in prd.splitlines()
                if line.startswith("| `DIST-007` | Codex |")
            ),
            "",
        )
        self.assertTrue(active_row, "DIST-007 is not claimed in active work")
        self.assertIn("red-first", active_row)
        self.assertIn("real tagged publication", active_row)


if __name__ == "__main__":
    unittest.main()
