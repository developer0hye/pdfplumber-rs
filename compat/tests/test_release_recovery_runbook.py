"""Contracts for release rollback and partial-publication recovery guidance."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUNBOOK_PATH = REPO_ROOT / "docs" / "release-recovery.md"
CRATES_GUIDE_PATH = REPO_ROOT / "docs" / "crates-release.md"
README_PATH = REPO_ROOT / "README.md"
REFERENCE_INDEX_PATH = REPO_ROOT / "references" / "INDEX.md"
REFERENCE_PATH = REPO_ROOT / "references" / "release-recovery.md"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"


def section(document: str, heading: str) -> str:
    match = re.search(
        rf"^## {re.escape(heading)}\n(?P<body>.*?)(?=^## |\Z)",
        document,
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        return ""
    return match.group("body").strip()


class ReleaseRecoveryRunbookTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if not RUNBOOK_PATH.is_file():
            raise AssertionError("missing release recovery runbook")
        cls.runbook = RUNBOOK_PATH.read_text(encoding="utf-8")

    def test_runbook_captures_the_actual_independent_publication_graph(self) -> None:
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        topology = section(self.runbook, "Release topology and first response")
        normalized_topology = " ".join(topology.split())

        for package in (
            "pdfplumber-core",
            "pdfplumber-parse",
            "pdfplumber",
            "pdfplumber-cli",
            "pdfplumber-rs",
            "pdfplumber-wasm",
            "GitHub Release",
        ):
            with self.subTest(package=package):
                self.assertIn(package, topology)

        self.assertIn("needs: [publish, metadata, scorecards, cli-binaries]", workflow)
        self.assertIn("crates.io", topology)
        self.assertRegex(
            normalized_topology,
            r"(?i)PyPI.*npm.*independent|independent.*PyPI.*npm",
        )
        self.assertIn("gh run cancel <run-id>", topology)
        self.assertRegex(
            normalized_topology,
            r"(?i)do not (re-?run|restart) the whole workflow",
        )

    def test_incident_record_preserves_exact_identity_and_per_target_state(self) -> None:
        incident = section(self.runbook, "Incident record")

        for field in (
            "release tag",
            "commit SHA",
            "workflow run URL",
            "first observed UTC",
            "incident owner",
            "artifact digest",
        ):
            with self.subTest(field=field):
                self.assertIn(field, incident)

        for state in ("not published", "published", "withdrawn", "unknown"):
            with self.subTest(state=state):
                self.assertIn(state, incident)
        self.assertRegex(incident, r"(?i)never.*token|secret.*never")

    def test_registry_actions_distinguish_reversible_and_irreversible_controls(self) -> None:
        actions = section(self.runbook, "Registry containment and verification")

        for command in (
            "cargo info <crate>@<version>",
            "cargo yank <crate>@<version>",
            "https://pypi.org/pypi/pdfplumber-rs/<version>/json",
            "https://pypi.org/manage/project/pdfplumber-rs/releases/",
            'npm view "pdfplumber-wasm@<version>" version',
            'npm deprecate "pdfplumber-wasm@<version>" "<reason and replacement>"',
            "gh release view <tag>",
        ):
            with self.subTest(command=command):
                self.assertIn(command, actions)

        self.assertRegex(actions, r"(?i)Cargo.*yank.*does not delete")
        self.assertRegex(actions, r"(?i)PyPI.*yank.*non-destructive")
        self.assertRegex(actions, r"(?i)npm.*unpublish.*irreversible")
        self.assertRegex(actions, r"(?i)new version.*same version|same version.*new version")

    def test_required_failure_scenarios_have_actionable_stop_and_exit_gates(self) -> None:
        scenarios = {
            heading: section(self.runbook, heading)
            for heading in (
                "Registry lag",
                "One-package failure",
                "Compromised credentials",
                "Incorrect compatibility claim",
            )
        }
        for heading, body in scenarios.items():
            normalized_body = " ".join(body.split())
            with self.subTest(heading=heading):
                self.assertTrue(body, f"missing {heading} section")
                self.assertRegex(
                    normalized_body, r"(?i)(stop|cancel|freeze|do not publish)"
                )
                self.assertRegex(
                    normalized_body,
                    r"(?i)(resume|close|exit|complete|publish again)",
                )

        scenarios = {
            heading: " ".join(body.split()) for heading, body in scenarios.items()
        }

        self.assertRegex(scenarios["Registry lag"], r"(?i)bounded.*poll")
        self.assertRegex(scenarios["Registry lag"], r"(?i)expected version.*resolv")
        self.assertRegex(scenarios["One-package failure"], r"(?i)already published")
        self.assertRegex(scenarios["One-package failure"], r"(?i)dependency order")
        self.assertRegex(scenarios["Compromised credentials"], r"(?i)revoke.*provider")
        self.assertRegex(scenarios["Compromised credentials"], r"(?i)delet.*secret.*not.*revoke")
        self.assertRegex(scenarios["Compromised credentials"], r"(?i)audit.*scope")
        self.assertRegex(
            scenarios["Incorrect compatibility claim"],
            r"(?i)preserve.*evidence|evidence.*preserve",
        )
        self.assertRegex(
            scenarios["Incorrect compatibility claim"],
            r"(?i)do not.*(weaken|replace silently|overwrite)",
        )

    def test_runbook_is_discoverable_and_official_recovery_sources_are_indexed(self) -> None:
        self.assertIn(
            "[release recovery runbook](release-recovery.md)",
            CRATES_GUIDE_PATH.read_text(encoding="utf-8"),
        )
        self.assertIn(
            "[release recovery runbook](docs/release-recovery.md)",
            README_PATH.read_text(encoding="utf-8"),
        )
        self.assertTrue(REFERENCE_PATH.is_file(), "missing recovery reference note")
        references = REFERENCE_PATH.read_text(encoding="utf-8")
        self.assertLessEqual(len(references.splitlines()), 50)
        for official_url in (
            "https://doc.rust-lang.org/cargo/commands/cargo-yank.html",
            "https://docs.pypi.org/project-management/yanking/",
            "https://docs.npmjs.com/policies/unpublish/",
            "https://docs.github.com/en/code-security/tutorials/"
            "remediate-leaked-secrets/remediating-a-leaked-secret",
        ):
            with self.subTest(official_url=official_url):
                self.assertIn(official_url, references)
        self.assertIn(
            "[release-recovery.md](release-recovery.md)",
            REFERENCE_INDEX_PATH.read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
