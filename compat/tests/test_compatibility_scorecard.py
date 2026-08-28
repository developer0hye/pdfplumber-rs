"""Contracts for the public machine-readable compatibility scorecard."""

from __future__ import annotations

import json
import subprocess
import sys
import unittest
from dataclasses import replace
from pathlib import Path

from compat.harness import compatibility_scorecard, corpus_index


REPO_ROOT = Path(__file__).resolve().parents[2]
PUBLIC_SOURCE = REPO_ROOT / "compat" / "scorecard-v0.3.0.toml"
PUBLIC_GENERATOR = REPO_ROOT / "scripts" / "generate_compatibility_scorecard.py"
PUBLIC_OUTPUT = REPO_ROOT / "docs" / "compatibility" / "scorecard-v0.3.0.json"


class CompatibilityScorecardContractTests(unittest.TestCase):
    def test_scorecard_keeps_runs_dimensions_and_domain_outcomes(self) -> None:
        scorecard = compatibility_scorecard.build(
            subject_version="0.3.0",
            subject_revision="a" * 40,
            corpus=self.corpus(),
            corpus_sha256="b" * 64,
            runs=(self.observed_run(), self.not_tested_run()),
        )

        self.assertEqual(scorecard["schema_version"], 1)
        self.assertEqual(
            set(scorecard["dimensions"]),
            {
                "apis",
                "options",
                "fixture_classes",
                "pages",
                "platforms",
                "artifact_types",
            },
        )
        self.assertEqual(
            set(scorecard["status_vocabulary"]),
            {
                "exact",
                "approved_delta",
                "unsupported",
                "reference_failure",
                "candidate_failure",
                "not_tested",
            },
        )
        statuses = {record["status"] for record in scorecard["observations"]}
        self.assertEqual(statuses, set(scorecard["status_vocabulary"]))

        observed = scorecard["runs"][0]
        self.assertEqual(observed["platform_id"], "macos-15-arm64-cpython-3.13")
        self.assertEqual(observed["artifact_type"], "wheel")
        self.assertEqual(observed["artifact_sha256"], "c" * 64)
        self.assertEqual(
            observed["command"],
            ".venv-reference/bin/python scripts/parity_report.py --json parity.json",
        )
        self.assertEqual(scorecard["runs"][1]["status"], "not_tested")

        summary = scorecard["summary"]
        for dimension in (
            "api",
            "option",
            "fixture_class",
            "page",
            "platform",
            "artifact_type",
        ):
            self.assertIn(f"by_{dimension}", summary)
        self.assertNotIn("percentage", json.dumps(scorecard).lower())

        fixture = scorecard["dimensions"]["pages"][0]
        self.assertEqual(fixture["fixture_class"], "generated")
        self.assertEqual(fixture["fixture_id"], "tests/fixtures/generated/basic.pdf")
        self.assertEqual(fixture["page_number"], 1)
        self.assertEqual(
            json.loads(compatibility_scorecard.render(scorecard)),
            scorecard,
        )
        self.assertEqual(
            compatibility_scorecard.render(scorecard),
            compatibility_scorecard.render(scorecard),
        )

    def test_api_and_option_scopes_keep_their_actual_artifact_types(self) -> None:
        combined = self.observed_run()
        source_run = replace(
            combined,
            id="macos-arm64-source",
            artifact_type="source",
            artifact_name="pdfplumber-rs-a" + "a" * 39,
            artifact_sha256="d" * 64,
            scopes=("api",),
        )
        wheel_run = replace(
            combined,
            id="macos-arm64-wheel-options",
            scopes=("option",),
        )

        scorecard = compatibility_scorecard.build(
            subject_version="0.3.0",
            subject_revision="a" * 40,
            corpus=self.corpus(),
            corpus_sha256="b" * 64,
            runs=(source_run, wheel_run),
        )

        api_observations = [
            record
            for record in scorecard["observations"]
            if record["kind"] in {"api", "fixture"}
        ]
        option_observations = [
            record
            for record in scorecard["observations"]
            if record["kind"] == "option"
        ]
        self.assertTrue(api_observations)
        self.assertTrue(option_observations)
        self.assertEqual(
            {record["artifact_type"] for record in api_observations},
            {"source"},
        )
        self.assertEqual(
            {record["artifact_type"] for record in option_observations},
            {"wheel"},
        )
        self.assertEqual(scorecard["runs"][0]["scopes"], ["api"])
        self.assertEqual(scorecard["runs"][1]["scopes"], ["option"])

    def test_published_scorecard_is_current_and_linked(self) -> None:
        for path in (PUBLIC_SOURCE, PUBLIC_GENERATOR, PUBLIC_OUTPUT):
            with self.subTest(path=path.name):
                self.assertTrue(path.is_file(), f"missing {path.relative_to(REPO_ROOT)}")

        completed = subprocess.run(
            [sys.executable, str(PUBLIC_GENERATOR), "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

        scorecard = json.loads(PUBLIC_OUTPUT.read_text(encoding="utf-8"))
        self.assertRegex(scorecard["subject"]["revision"], r"^[0-9a-f]{40}$")
        self.assertEqual(
            {run["artifact_type"] for run in scorecard["runs"]},
            {"source", "wheel", "sdist"},
        )
        self.assertEqual(
            {
                (run["artifact_type"], run["status"])
                for run in scorecard["runs"]
            },
            {
                ("source", "observed"),
                ("wheel", "observed"),
                ("source", "not_tested"),
                ("wheel", "not_tested"),
                ("sdist", "not_tested"),
            },
        )
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("docs/compatibility/scorecard-v0.3.0.json", readme)

    def test_unknown_fixture_is_rejected_instead_of_dropped(self) -> None:
        run = self.observed_run()
        run.report["fixtures"][0]["fixture_id"] = "missing.pdf"

        with self.assertRaisesRegex(
            compatibility_scorecard.ScorecardError,
            "unknown fixture",
        ):
            compatibility_scorecard.build(
                subject_version="0.3.0",
                subject_revision="a" * 40,
                corpus=self.corpus(),
                corpus_sha256="b" * 64,
                runs=(run,),
            )

    def test_indexed_fixture_absent_from_api_report_is_explicitly_not_tested(
        self,
    ) -> None:
        run = self.observed_run()
        run.report["fixtures"] = run.report["fixtures"][:1]
        run = replace(run, scopes=("api",))

        scorecard = compatibility_scorecard.build(
            subject_version="0.3.0",
            subject_revision="a" * 40,
            corpus=self.corpus(),
            corpus_sha256="b" * 64,
            runs=(run,),
        )

        missing = [
            record
            for record in scorecard["observations"]
            if record.get("fixture_id")
            == "tests/fixtures/downloaded/broken.pdf"
        ]
        self.assertEqual(
            missing,
            [
                {
                    "id": (
                        "macos-arm64-wheel::fixture::"
                        "tests/fixtures/downloaded/broken.pdf::not-tested"
                    ),
                    "run_id": "macos-arm64-wheel",
                    "kind": "fixture",
                    "platform_id": "macos-15-arm64-cpython-3.13",
                    "artifact_type": "wheel",
                    "fixture_class": "regression",
                    "fixture_id": "tests/fixtures/downloaded/broken.pdf",
                    "status": "not_tested",
                    "reason": "absent_from_parity_report",
                }
            ],
        )

    @staticmethod
    def corpus() -> corpus_index.CorpusIndex:
        return corpus_index.CorpusIndex(
            collections=(
                corpus_index.Collection("generated", "Generated compatibility PDFs."),
                corpus_index.Collection("regression", "Pinned regressions."),
            ),
            fixtures=(
                corpus_index.Fixture(
                    path="tests/fixtures/generated/basic.pdf",
                    sha256="1" * 64,
                    source="generated",
                    source_path="basic.pdf",
                    collection="generated",
                ),
                corpus_index.Fixture(
                    path="tests/fixtures/downloaded/broken.pdf",
                    sha256="2" * 64,
                    source="upstream",
                    source_path="broken.pdf",
                    collection="regression",
                ),
            ),
        )

    @staticmethod
    def observed_run() -> compatibility_scorecard.RunInput:
        page_apis = {
            api: {
                "status": "exact",
                "comparison": {"equal": True},
            }
            for api in compatibility_scorecard.PAGE_APIS
        }
        page_apis["page_text"] = {
            "status": "different",
            "comparison": {"equal": False},
            "delta_gate": {
                "status": "approved",
                "id": "DELTA-TEXT-001",
                "upstream_sha256": "3" * 64,
                "rust_sha256": "4" * 64,
            },
        }
        page_apis["simple_text"] = {
            "status": "unsupported",
            "comparison": {
                "status": "unsupported_in_rust",
                "task_id": "TEXT-002",
                "equal": False,
            },
        }
        page_apis["search"] = {
            "status": "different",
            "comparison": {"equal": False},
            "delta_gate": {
                "status": "unregistered",
                "upstream_sha256": "5" * 64,
                "rust_sha256": "6" * 64,
            },
        }
        report = {
            "schema_version": 1,
            "target": {
                "project": "pdfplumber",
                "version": "0.11.10",
                "tag": "v0.11.10",
                "commit": "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
                "repository": "https://github.com/jsvine/pdfplumber",
            },
            "fixtures": [
                {
                    "fixture_id": "tests/fixtures/generated/basic.pdf",
                    "status": "compared",
                    "pages": [
                        {
                            "page_number": 1,
                            "status": "compared",
                            "apis": page_apis,
                        }
                    ],
                },
                {
                    "fixture_id": "tests/fixtures/downloaded/broken.pdf",
                    "status": "python_failed",
                    "error": "xref is malformed",
                },
            ],
            "options": [
                {
                    "id": "text.extract_words.x_tolerance",
                    "api": "extract_words",
                    "fixture_path": "tests/fixtures/generated/basic.pdf",
                    "page_number": 1,
                    "covers": ["extract_words.x_tolerance"],
                    "options": {"x_tolerance": 1},
                    "comparison": {"status": "equal", "equal": True},
                },
                {
                    "id": "text.extract_words.y_tolerance",
                    "api": "extract_words",
                    "fixture_path": "tests/fixtures/generated/basic.pdf",
                    "page_number": 1,
                    "covers": ["extract_words.y_tolerance"],
                    "options": {"y_tolerance": 1},
                    "comparison": {"status": "not_compared"},
                },
            ],
        }
        return compatibility_scorecard.RunInput(
            id="macos-arm64-wheel",
            platform=compatibility_scorecard.Platform(
                id="macos-15-arm64-cpython-3.13",
                system="macOS",
                release="15.6.1",
                machine="arm64",
                python_version="3.13",
            ),
            artifact_type="wheel",
            artifact_name="pdfplumber_rs-0.3.0-cp313-cp313-macosx.whl",
            artifact_sha256="c" * 64,
            command=(
                ".venv-reference/bin/python scripts/parity_report.py "
                "--json parity.json"
            ),
            report=report,
        )

    @staticmethod
    def not_tested_run() -> compatibility_scorecard.RunInput:
        return compatibility_scorecard.RunInput(
            id="linux-x86_64-sdist",
            platform=compatibility_scorecard.Platform(
                id="ubuntu-x86_64-cpython-3.13",
                system="Ubuntu Linux",
                release="runner",
                machine="x86_64",
                python_version="3.13",
            ),
            artifact_type="sdist",
            not_tested_reason=(
                "The source distribution is smoke-tested, but parity is not run "
                "from the installed artifact."
            ),
            evidence=(".github/workflows/ci.yml",),
        )


if __name__ == "__main__":
    unittest.main()
