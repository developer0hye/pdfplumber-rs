"""Contracts for the public machine-readable compatibility scorecard."""

from __future__ import annotations

import json
import unittest

from compat.harness import compatibility_scorecard, corpus_index


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
