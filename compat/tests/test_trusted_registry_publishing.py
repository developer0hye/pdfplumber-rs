"""Contracts for short-lived release publishing identities."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "trusted-publishing.md"
REFERENCE_PATH = REPO_ROOT / "references" / "trusted-publishing.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
README_PATH = REPO_ROOT / "README.md"
PRD_PATH = REPO_ROOT / "PRD.md"


def job(document: str, name: str) -> str:
    match = re.search(
        rf"^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
        document,
        re.MULTILINE | re.DOTALL,
    )
    return "" if match is None else match.group(0)


def text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.is_file() else ""


class TrustedRegistryPublishingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = text(WORKFLOW_PATH)
        cls.guide = text(GUIDE_PATH)

    def test_oidc_and_write_permissions_are_scoped_to_publishers(self) -> None:
        workflow_header = self.workflow.split("jobs:", maxsplit=1)[0]
        self.assertIn("permissions:\n  contents: read", workflow_header)
        self.assertNotIn("contents: write", workflow_header)
        self.assertNotIn("id-token: write", workflow_header)
        self.assertNotIn("attestations: write", workflow_header)

        for name in ("publish", "publish-pypi", "publish-npm"):
            with self.subTest(job=name):
                publish_job = job(self.workflow, name)
                self.assertIn("permissions:", publish_job)
                self.assertIn("id-token: write", publish_job)

        github_release = job(self.workflow, "release")
        self.assertIn("contents: write", github_release)
        self.assertNotIn("id-token: write", github_release)
        self.assertEqual(self.workflow.count("contents: write"), 1)

    def test_crates_io_uses_one_oidc_exchange_for_all_four_crates(self) -> None:
        crates = job(self.workflow, "publish")
        self.assertIn("environment:\n      name: crates-io", crates)
        self.assertIn("uses: rust-lang/crates-io-auth-action@v1", crates)
        self.assertIn("id: crates-io-auth", crates)
        self.assertEqual(
            crates.count(
                "CARGO_REGISTRY_TOKEN: "
                "${{ steps.crates-io-auth.outputs.token }}"
            ),
            4,
        )
        self.assertNotIn("secrets.CARGO_REGISTRY_TOKEN", self.workflow)

    def test_pypi_uses_the_registered_environment_without_a_password(self) -> None:
        pypi = job(self.workflow, "publish-pypi")
        self.assertIn("environment:\n      name: pypi", pypi)
        self.assertIn("url: https://pypi.org/project/pdfplumber-rs/", pypi)
        self.assertIn("uses: pypa/gh-action-pypi-publish@release/v1", pypi)
        self.assertIn("packages-dir: release-python/subjects", pypi)
        self.assertNotRegex(pypi, r"(?m)^\s+(password|user|username):")
        self.assertNotIn("secrets.PYPI_API_TOKEN", self.workflow)

    def test_npm_uses_an_oidc_capable_pinned_node_and_no_auth_token(self) -> None:
        npm = job(self.workflow, "publish-npm")
        self.assertIn("environment:\n      name: npm", npm)
        self.assertIn("url: https://www.npmjs.com/package/pdfplumber-wasm", npm)
        self.assertIn('node-version: "24.5.0"', npm)
        self.assertIn(
            "npm publish crates/pdfplumber-wasm/pkg --access public",
            npm,
        )
        self.assertNotIn("npm config set", npm)
        self.assertNotIn("NPM_TOKEN", self.workflow)
        self.assertNotIn("NODE_AUTH_TOKEN", self.workflow)

    def test_github_release_uses_only_the_job_scoped_github_token(self) -> None:
        github_release = job(self.workflow, "release")
        self.assertIn("uses: softprops/action-gh-release@v2", github_release)
        self.assertNotRegex(
            github_release,
            r"(?i)(secrets\.(?:GH|GITHUB|RELEASE|PAT)|github[_-]?token:)",
        )
        self.assertIn("short-lived", self.guide)
        self.assertIn("GITHUB_TOKEN", self.guide)

    def test_guide_records_exact_registry_bindings_and_safe_migration(self) -> None:
        for value in (
            "developer0hye",
            "pdfplumber-rs",
            "release.yml",
            "pdfplumber-core",
            "pdfplumber-parse",
            "pdfplumber-cli",
            "crates-io",
            "pypi",
            "pdfplumber-wasm",
            "npm publish",
        ):
            with self.subTest(value=value):
                self.assertIn(value, self.guide)

        normalized = " ".join(self.guide.split())
        self.assertRegex(normalized, r"(?i)configure.*verify.*revoke")
        self.assertRegex(normalized, r"(?i)do not.*publish.*test|without publishing")
        self.assertRegex(normalized, r"(?i)registry.*private|not publicly visible")

    def test_public_guidance_and_task_state_preserve_the_evidence_boundary(self) -> None:
        self.assertIn(
            "[trusted publishing](docs/trusted-publishing.md)",
            text(README_PATH),
        )
        self.assertTrue(REFERENCE_PATH.is_file())
        self.assertLessEqual(len(text(REFERENCE_PATH).splitlines()), 50)
        for official_url in (
            "https://crates.io/docs/trusted-publishing",
            "https://docs.pypi.org/trusted-publishers/using-a-publisher/",
            "https://docs.npmjs.com/trusted-publishers/",
            "https://docs.github.com/en/actions/concepts/security/github_token",
        ):
            with self.subTest(url=official_url):
                self.assertIn(official_url, text(REFERENCE_PATH))
        self.assertIn(
            "[trusted-publishing.md](trusted-publishing.md)",
            text(REFERENCE_INDEX_PATH),
        )
        self.assertIn("- [ ] **DIST-006**", text(PRD_PATH))


if __name__ == "__main__":
    unittest.main()
