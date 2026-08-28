"""Contracts for the reproducible Rust contributor environment (DIST-015)."""

from __future__ import annotations

import json
import re
import stat
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEVCONTAINER = ROOT / ".devcontainer" / "devcontainer.json"
DOCKERFILE = ROOT / ".devcontainer" / "Dockerfile"
CONTAINER_RUNNER = ROOT / "scripts" / "check_rust_dev_container.sh"
ENVIRONMENT_CHECKER = ROOT / "scripts" / "check_rust_dev_environment.sh"
GUIDE = ROOT / "docs" / "rust-development.md"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
PINNED_RUST_IMAGE = (
    "rust:1.98.0-bookworm@"
    "sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922"
)


class RustDevelopmentEnvironmentContractTests(unittest.TestCase):
    def test_devcontainer_uses_the_repository_dockerfile_as_a_non_root_user(
        self,
    ) -> None:
        self.assertTrue(DEVCONTAINER.is_file(), "missing devcontainer.json")
        if not DEVCONTAINER.is_file():
            return

        configuration = json.loads(DEVCONTAINER.read_text(encoding="utf-8"))
        self.assertEqual(configuration["build"]["dockerfile"], "Dockerfile")
        self.assertEqual(configuration["build"]["context"], "..")
        self.assertEqual(configuration["remoteUser"], "rustdev")
        self.assertIs(configuration["updateRemoteUserUID"], True)

    def test_container_base_is_an_immutable_multi_platform_rust_snapshot(self) -> None:
        self.assertTrue(DOCKERFILE.is_file(), "missing development Dockerfile")
        if not DOCKERFILE.is_file():
            return

        dockerfile = DOCKERFILE.read_text(encoding="utf-8")
        self.assertIn(f"FROM {PINNED_RUST_IMAGE}", dockerfile)
        self.assertRegex(dockerfile, r"(?m)^USER rustdev$")
        self.assertRegex(dockerfile, r"(?m)^WORKDIR /workspaces/pdfplumber-rs$")
        self.assertNotRegex(dockerfile, r"(?i)apt-get|curl|wget|latest")

    def test_environment_checker_runs_first_use_and_focused_ci_paths(self) -> None:
        self.assertTrue(
            ENVIRONMENT_CHECKER.is_file(), "missing in-container environment checker"
        )
        if not ENVIRONMENT_CHECKER.is_file():
            return

        checker = ENVIRONMENT_CHECKER.read_text(encoding="utf-8")
        for command in (
            "python3 scripts/check_doc_quickstarts.py --rust",
            "cargo test -p pdfplumber --test feature_semantics",
            "cargo test -p pdfplumber --features parallel --test concurrency",
            "cargo check -p pdfplumber --examples --all-features",
        ):
            with self.subTest(command=command):
                self.assertIn(command, checker)
        self.assertIn("1.98.0", checker)
        self.assertIn("python3 --version", checker)

    def test_one_host_command_builds_and_runs_the_clean_environment(self) -> None:
        self.assertTrue(CONTAINER_RUNNER.is_file(), "missing container runner")
        if not CONTAINER_RUNNER.is_file():
            return

        mode = CONTAINER_RUNNER.stat().st_mode
        self.assertTrue(mode & stat.S_IXUSR, "container runner is not executable")
        runner = CONTAINER_RUNNER.read_text(encoding="utf-8")
        self.assertIn("docker build", runner)
        self.assertIn("--pull", runner)
        self.assertIn("docker run", runner)
        self.assertIn(":/workspaces/pdfplumber-rs:ro", runner)
        self.assertIn("CARGO_HOME=/tmp/pdfplumber-cargo", runner)
        self.assertIn("CARGO_TARGET_DIR=/tmp/pdfplumber-target", runner)
        self.assertIn("scripts/check_rust_dev_environment.sh", runner)
        self.assertNotIn("--privileged", runner)
        self.assertNotIn("docker.sock", runner)

    def test_shell_entrypoints_are_syntactically_valid(self) -> None:
        for script in (CONTAINER_RUNNER, ENVIRONMENT_CHECKER):
            with self.subTest(script=script.name):
                self.assertTrue(script.is_file(), f"missing {script}")
                if not script.is_file():
                    continue
                completed = subprocess.run(
                    ["bash", "-n", str(script)],
                    cwd=ROOT,
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_ci_and_public_docs_use_the_same_container_command(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        support_source = (ROOT / "support-matrix.toml").read_text(encoding="utf-8")
        changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        reference_index = (ROOT / "references" / "INDEX.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("name: Rust development container", workflow)
        self.assertIn("scripts/check_rust_dev_container.sh", workflow)
        self.assertTrue(GUIDE.is_file(), "missing public Rust development guide")
        self.assertIn("docs/rust-development.md", readme)
        self.assertIn("docs/rust-development.md", support_source)
        self.assertIn("compat/tests/test_rust_dev_environment.py", support_source)
        self.assertRegex(changelog, r"(?is)reproducible.*Rust.*development container")
        self.assertIn("rust-dev-containers.md", reference_index)

    def test_policy_and_evidence_distinguish_snapshot_from_support_floor(self) -> None:
        self.assertTrue(GUIDE.is_file(), "missing public Rust development guide")
        if not GUIDE.is_file():
            return
        guide = GUIDE.read_text(encoding="utf-8")
        normalized = " ".join(guide.split())
        prd = (ROOT / "PRD.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "ROADMAP.md").read_text(encoding="utf-8")

        self.assertIn(PINNED_RUST_IMAGE, guide)
        self.assertRegex(normalized, r"(?i)reproducible snapshot.*not.*minimum supported")
        self.assertRegex(normalized, r"(?i)rolling stable.*update.*tag.*digest")
        self.assertRegex(normalized, r"(?i)amd64.*arm64")
        self.assertIn("scripts/check_rust_dev_container.sh", guide)
        self.assertIn("- [x] **DIST-015**", prd)
        self.assertRegex(
            prd,
            r"(?m)^\| `DIST-015` \| 2026-08-28 \| Codex \| PR #\d+ \|",
        )
        self.assertNotIn("### Prove reproducible Rust development", roadmap)
        self.assertIn("SCORE-010", roadmap)

    def test_digest_has_the_complete_sha256_shape(self) -> None:
        digest = PINNED_RUST_IMAGE.rsplit("@sha256:", 1)[1]
        self.assertTrue(re.fullmatch(r"[0-9a-f]{64}", digest))


if __name__ == "__main__":
    unittest.main()
