"""Contracts for the versioned human compatibility workflow scorecard."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import unittest
from pathlib import Path

from compat.harness import workflow_scorecard

REPO_ROOT = Path(__file__).resolve().parents[2]
MACHINE_SCORECARD = REPO_ROOT / "docs" / "compatibility" / "scorecard-v0.3.0.json"
WORKFLOW_SOURCE = REPO_ROOT / "compat" / "workflow-scorecard-v0.3.0.toml"
WORKFLOW_GENERATOR = REPO_ROOT / "scripts" / "generate_workflow_scorecard.py"
WORKFLOW_OUTPUT = REPO_ROOT / "docs" / "compatibility" / "workflows-v0.3.0.md"

WORKFLOW_IDS = (
    "open",
    "text",
    "words",
    "crop",
    "search",
    "tables",
    "serialization",
    "annotations",
    "structure",
    "rendering",
    "cli",
)


class WorkflowScorecardContractTests(unittest.TestCase):
    def test_workflows_partition_api_dimensions_and_preserve_outcomes(self) -> None:
        report = workflow_scorecard.build(
            self.machine_scorecard(),
            self.workflow_definitions(),
            machine_path="scorecard-v0.3.0.json",
            machine_sha256="a" * 64,
        )

        self.assertEqual(
            tuple(workflow["id"] for workflow in report["workflows"]),
            WORKFLOW_IDS,
        )
        by_id = {workflow["id"]: workflow for workflow in report["workflows"]}
        self.assertEqual(
            by_id["text"]["status_counts"],
            self.counts(exact=1, candidate_failure=1),
        )
        self.assertEqual(
            by_id["words"]["status_counts"],
            self.counts(candidate_failure=1),
        )
        self.assertEqual(
            by_id["annotations"]["status_counts"],
            self.counts(approved_delta=1),
        )
        self.assertEqual(
            by_id["structure"]["status_counts"],
            self.counts(unsupported=1),
        )
        self.assertEqual(by_id["text"]["observation_count"], 2)

        covered_apis = [
            api_id
            for workflow in report["workflows"]
            for api_id in workflow["api_ids"]
        ]
        self.assertEqual(
            sorted(covered_apis),
            sorted(self.machine_scorecard()["dimensions"]["apis"]),
        )
        self.assertEqual(len(covered_apis), len(set(covered_apis)))

    def test_open_projection_counts_each_fixture_once(self) -> None:
        report = workflow_scorecard.build(
            self.machine_scorecard(),
            self.workflow_definitions(),
            machine_path="scorecard-v0.3.0.json",
            machine_sha256="a" * 64,
        )
        opened = report["workflows"][0]

        self.assertEqual(opened["projection"], "document_open")
        self.assertEqual(opened["observation_count"], 3)
        self.assertEqual(
            opened["status_counts"],
            self.counts(exact=1, reference_failure=1, not_tested=1),
        )
        self.assertIn("fixture", opened["evidence_kinds"])
        self.assertIn("api", opened["evidence_kinds"])

    def test_open_projection_keeps_an_unreported_indexed_fixture_not_tested(
        self,
    ) -> None:
        machine = self.machine_scorecard()
        machine["observations"] = [
            observation
            for observation in machine["observations"]
            if observation["id"] != "fixture-not-tested"
        ]

        report = workflow_scorecard.build(
            machine,
            self.workflow_definitions(),
            machine_path="scorecard-v0.3.0.json",
            machine_sha256="a" * 64,
            indexed_fixture_ids=(
                "fixtures/observed.pdf",
                "fixtures/reference-failure.pdf",
                "fixtures/not-tested.pdf",
            ),
        )
        opened = report["workflows"][0]

        self.assertEqual(opened["unreported_fixture_count"], 1)
        self.assertEqual(
            opened["status_counts"],
            self.counts(exact=1, reference_failure=1, not_tested=1),
        )

    def test_absent_workflow_dimension_is_not_tested_instead_of_inferred(self) -> None:
        report = workflow_scorecard.build(
            self.machine_scorecard(),
            self.workflow_definitions(),
            machine_path="scorecard-v0.3.0.json",
            machine_sha256="a" * 64,
        )
        by_id = {workflow["id"]: workflow for workflow in report["workflows"]}

        for identifier in ("crop", "serialization", "rendering", "cli"):
            with self.subTest(workflow=identifier):
                workflow = by_id[identifier]
                self.assertEqual(workflow["coverage"], "not_tested")
                self.assertEqual(workflow["observation_count"], 0)
                self.assertEqual(workflow["status_counts"], self.counts())
                self.assertIn("machine scorecard", workflow["not_tested_reason"])

    def test_duplicate_or_missing_api_mapping_is_rejected(self) -> None:
        definitions = list(self.workflow_definitions())
        definitions[2] = workflow_scorecard.WorkflowDefinition(
            identifier="words",
            title="Words",
            api_ids=("extract_text", "extract_words"),
        )

        with self.assertRaisesRegex(
            workflow_scorecard.WorkflowScorecardError,
            "exactly one workflow",
        ):
            workflow_scorecard.build(
                self.machine_scorecard(),
                definitions,
                machine_path="scorecard-v0.3.0.json",
                machine_sha256="a" * 64,
            )

    def test_render_is_workflow_ordered_and_never_reports_a_percentage(self) -> None:
        report = workflow_scorecard.build(
            self.machine_scorecard(),
            self.workflow_definitions(),
            machine_path="scorecard-v0.3.0.json",
            machine_sha256="a" * 64,
        )
        rendered = workflow_scorecard.render(report)

        self.assertIn(
            "[machine-readable scorecard](scorecard-v0.3.0.json)", rendered
        )
        self.assertIn("No success percentage is computed", rendered)
        self.assertNotIn("%", rendered)
        headings = [
            line.removeprefix("### ")
            for line in rendered.splitlines()
            if line.startswith("### ")
        ]
        self.assertEqual(
            headings,
            [definition.title for definition in self.workflow_definitions()],
        )
        for label in (
            "Exact",
            "Approved delta",
            "Unsupported",
            "Reference failure",
            "Candidate failure",
            "Not tested",
        ):
            with self.subTest(label=label):
                self.assertIn(label, rendered)
        self.assertIn("macOS source", rendered)
        self.assertIn("Ubuntu wheel", rendered)

    def test_published_workflow_report_is_current_linked_and_ci_contracted(
        self,
    ) -> None:
        for path in (WORKFLOW_SOURCE, WORKFLOW_GENERATOR, WORKFLOW_OUTPUT):
            with self.subTest(path=path.name):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        completed = subprocess.run(
            [sys.executable, str(WORKFLOW_GENERATOR), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

        report = WORKFLOW_OUTPUT.read_text(encoding="utf-8")
        machine = json.loads(MACHINE_SCORECARD.read_text(encoding="utf-8"))
        machine_sha256 = hashlib.sha256(MACHINE_SCORECARD.read_bytes()).hexdigest()
        self.assertIn(machine["subject"]["revision"], report)
        self.assertIn(machine_sha256, report)
        self.assertNotIn("%", report)
        for definition in self.workflow_definitions():
            with self.subTest(workflow=definition.identifier):
                self.assertEqual(report.count(f"### {definition.title}\n"), 1)

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("docs/compatibility/workflows-v0.3.0.md", readme)
        self.assertIn(
            "python scripts/generate_workflow_scorecard.py --check", workflow
        )

    @staticmethod
    def counts(**overrides: int) -> dict[str, int]:
        counts = {status: 0 for status in workflow_scorecard.STATUS_ORDER}
        counts.update(overrides)
        return counts

    @staticmethod
    def workflow_definitions() -> tuple[workflow_scorecard.WorkflowDefinition, ...]:
        gap = "No dedicated machine scorecard observation exists for this workflow."
        return (
            workflow_scorecard.WorkflowDefinition(
                identifier="open",
                title="Open",
                projection="document_open",
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="text",
                title="Text",
                api_ids=("extract_text",),
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="words",
                title="Words",
                api_ids=("extract_words",),
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="crop",
                title="Crop",
                not_tested_reason=gap,
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="search",
                title="Search",
                api_ids=("search",),
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="tables",
                title="Tables",
                not_tested_reason=gap,
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="serialization",
                title="Serialization",
                not_tested_reason=gap,
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="annotations",
                title="Annotations",
                api_ids=("annotations",),
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="structure",
                title="Structure",
                api_ids=("structure_tree",),
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="rendering",
                title="Rendering",
                not_tested_reason=gap,
            ),
            workflow_scorecard.WorkflowDefinition(
                identifier="cli",
                title="Command-Line Interface",
                not_tested_reason=gap,
            ),
        )

    @staticmethod
    def machine_scorecard() -> dict[str, object]:
        return {
            "schema_version": 1,
            "subject": {
                "project": "pdfplumber-rs",
                "version": "0.3.0",
                "revision": "1" * 40,
            },
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "tag": "v0.11.10",
                "commit": "2" * 40,
                "repository": "https://github.com/jsvine/pdfplumber",
            },
            "corpus": {
                "index": "compat/fixture-provenance.toml",
                "fixture_count": 3,
                "sha256": "3" * 64,
            },
            "status_vocabulary": {
                status: f"Definition for {status}."
                for status in workflow_scorecard.STATUS_ORDER
            },
            "dimensions": {
                "apis": [
                    "annotations",
                    "extract_text",
                    "extract_words",
                    "search",
                    "structure_tree",
                ],
                "options": [{"id": "text.case", "api": "extract_text"}],
                "fixture_classes": [],
                "artifact_types": ["source", "wheel"],
            },
            "runs": [
                {
                    "id": "mac-source",
                    "platform_id": "macos-arm64",
                    "platform": {"label": "macOS"},
                    "artifact_type": "source",
                    "status": "observed",
                    "scopes": ["api"],
                },
                {
                    "id": "ubuntu-wheel",
                    "platform_id": "ubuntu-x86_64",
                    "platform": {"label": "Ubuntu"},
                    "artifact_type": "wheel",
                    "status": "not_tested",
                    "scopes": ["option"],
                },
            ],
            "observations": [
                {
                    "id": "text-exact",
                    "kind": "api",
                    "api_id": "extract_text",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "exact",
                },
                {
                    "id": "text-option-delta",
                    "kind": "option",
                    "api_id": "extract_text",
                    "option_id": "text.case",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "candidate_failure",
                },
                {
                    "id": "words-failure",
                    "kind": "api",
                    "api_id": "extract_words",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "candidate_failure",
                },
                {
                    "id": "search-exact",
                    "kind": "api",
                    "api_id": "search",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "exact",
                },
                {
                    "id": "annotations-approved",
                    "kind": "api",
                    "api_id": "annotations",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "approved_delta",
                },
                {
                    "id": "structure-unsupported",
                    "kind": "api",
                    "api_id": "structure_tree",
                    "fixture_id": "fixtures/observed.pdf",
                    "status": "unsupported",
                },
                {
                    "id": "reference-failure",
                    "kind": "fixture",
                    "fixture_id": "fixtures/reference-failure.pdf",
                    "status": "reference_failure",
                },
                {
                    "id": "fixture-not-tested",
                    "kind": "fixture",
                    "fixture_id": "fixtures/not-tested.pdf",
                    "status": "not_tested",
                },
                {
                    "id": "ubuntu-wheel-not-tested",
                    "kind": "run",
                    "run_id": "ubuntu-wheel",
                    "status": "not_tested",
                },
            ],
        }


if __name__ == "__main__":
    unittest.main()
