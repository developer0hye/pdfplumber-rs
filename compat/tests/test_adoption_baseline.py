"""Contracts for the dated adoption baseline (ADOPT-017)."""

from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "adoption-baseline.toml"
GENERATOR_PATH = REPO_ROOT / "scripts" / "generate_adoption_baseline.py"
OUTPUT_PATH = REPO_ROOT / "docs" / "adoption" / "baseline-2026-08-26.md"

EXPECTED_CRATES = {
    "pdfplumber": (23305, 21719, 29),
    "pdfplumber-core": (23472, 21798, 40),
    "pdfplumber-parse": (23385, 21752, 33),
    "pdfplumber-cli": (52, 31, 19),
}

EXPECTED_DEPENDENTS = {
    "moyangzhan/mango-finder": ("rust", 'pdfplumber = "0.2.0"'),
    "RyderFreeman4Logos/verbatim": (
        "rust",
        'pdfplumber = { version = "0.2", optional = true }',
    ),
    "zcaceres/markitdown-ts": ("wasm", '"pdfplumber-wasm": "^0.2.0"'),
}


class AdoptionBaselineContractTests(unittest.TestCase):
    def source(self) -> dict[str, object]:
        self.assertTrue(SOURCE_PATH.is_file(), "adoption-baseline.toml is missing")
        if not SOURCE_PATH.is_file():
            return {}
        return tomllib.loads(SOURCE_PATH.read_text(encoding="utf-8"))

    def page(self) -> str:
        self.assertTrue(OUTPUT_PATH.is_file(), f"{OUTPUT_PATH.relative_to(REPO_ROOT)} is missing")
        if not OUTPUT_PATH.is_file():
            return ""
        return OUTPUT_PATH.read_text(encoding="utf-8")

    def test_snapshot_source_and_generated_page_are_ci_contracted(self) -> None:
        self.assertTrue(GENERATOR_PATH.is_file(), "adoption baseline generator is missing")
        self.assertTrue(OUTPUT_PATH.is_file(), "generated adoption baseline is missing")
        if not GENERATOR_PATH.is_file() or not OUTPUT_PATH.is_file():
            return

        completed = subprocess.run(
            [sys.executable, str(GENERATOR_PATH), "--check"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr or completed.stdout)

        workflow = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertIn("python scripts/generate_adoption_baseline.py --check", workflow)

    def test_registry_download_baseline_is_frozen_with_source_limits(self) -> None:
        source = self.source()
        self.assertEqual(source.get("schema_version"), 1)
        self.assertEqual(source.get("snapshot_date"), "2026-08-26")
        self.assertEqual(source.get("observed_at_utc"), "2026-08-26T12:43:44Z")
        self.assertEqual(source.get("release_version"), "0.3.0")
        self.assertEqual(
            source.get("output_path"),
            "docs/adoption/baseline-2026-08-26.md",
        )

        packages = {
            row["package"]: row for row in source.get("registry_packages", [])
        }
        self.assertEqual(
            set(packages),
            {*EXPECTED_CRATES, "pdfplumber-rs", "pdfplumber-wasm"},
        )
        for package, counts in EXPECTED_CRATES.items():
            with self.subTest(package=package):
                row = packages[package]
                self.assertEqual(row["ecosystem"], "crates.io")
                self.assertEqual(
                    (
                        row["downloads_all_time"],
                        row["downloads_recent"],
                        row["downloads_release"],
                    ),
                    counts,
                )
                self.assertEqual(row["source_kind"], "registry-api")

        pypi = packages["pdfplumber-rs"]
        self.assertEqual(pypi["downloads_window"], 584)
        self.assertEqual(pypi["window_start"], "2026-08-22")
        self.assertEqual(pypi["window_end"], "2026-08-25")
        self.assertEqual(pypi["official_json_downloads"], -1)
        self.assertFalse(pypi["mirrors_included"])
        self.assertEqual(pypi["source_kind"], "secondary-bigquery-proxy")
        self.assertIn("2026-08-22 through 2026-08-25", pypi["query_transform"])

        npm = packages["pdfplumber-wasm"]
        self.assertEqual(npm["downloads_window"], 14)
        self.assertEqual(npm["window_start"], "2026-08-19")
        self.assertEqual(npm["window_end"], "2026-08-25")
        self.assertEqual(npm["source_kind"], "registry-api")

        for package, row in packages.items():
            with self.subTest(source=package):
                self.assertRegex(row["source_url"], r"^https://")
                self.assertTrue(row["limitations"])

    def test_non_registry_baselines_keep_observation_and_adoption_separate(self) -> None:
        source = self.source()
        dependents = source["dependent_repositories"]
        entries = {row["repository"]: row for row in dependents["entries"]}
        self.assertEqual(dependents["observed_count"], 3)
        self.assertEqual(dependents["crates_io_external_count"], 0)
        self.assertEqual(dependents["python_manifest_count"], 0)
        self.assertEqual(set(entries), set(EXPECTED_DEPENDENTS))
        self.assertEqual(
            set(dependents["search_queries"]),
            {
                "pdfplumber filename:Cargo.toml",
                "pdfplumber-rs filename:pyproject.toml OR filename:requirements.txt",
                "pdfplumber-wasm filename:package.json",
            },
        )
        self.assertEqual(len(dependents["crates_io_source_urls"]), 4)
        for repository, (surface, requirement) in EXPECTED_DEPENDENTS.items():
            with self.subTest(dependent=repository):
                entry = entries[repository]
                self.assertEqual(entry["surface"], surface)
                self.assertEqual(entry["requirement"], requirement)
                self.assertRegex(
                    entry["manifest_url"],
                    rf"^https://github\.com/{re.escape(repository)}/blob/[0-9a-f]{{40}}/",
                )

        visits = source["documentation_visits"]
        self.assertEqual((visits["views"], visits["unique_visitors"]), (71, 20))
        self.assertEqual((visits["overview_views"], visits["overview_uniques"]), (29, 16))
        self.assertEqual((visits["readme_views"], visits["readme_uniques"]), (1, 1))
        self.assertEqual(visits["window_start"], "2026-08-12")
        self.assertEqual(visits["window_end"], "2026-08-25")
        self.assertEqual(visits["docs_paths_in_top_ten"], 0)
        self.assertTrue(visits["views_source_url"].endswith("/traffic/views"))
        self.assertTrue(visits["paths_source_url"].endswith("/traffic/popular/paths"))

        activation = source["activation_failures"]
        self.assertEqual(activation["measurement_status"], "not-instrumented")
        self.assertEqual(activation["public_issue_proxy_count"], 0)
        self.assertEqual(activation["candidate_issue_numbers"], [74, 155])
        self.assertIn(r"crates\.io", activation["query_expression"])
        self.assertIn("All 54 GitHub issues", activation["query_scope"])

        issues = source["issues"]
        self.assertEqual((issues["total"], issues["open"], issues["closed"]), (54, 0, 54))

        adopters = source["external_adopters"]
        self.assertEqual(adopters["confirmed_count"], 0)
        self.assertEqual(adopters["external_evaluator_count"], 1)
        self.assertEqual(adopters["evaluator_issue_number"], 286)

        targets = source["targets"]
        self.assertFalse(targets["quarterly_growth_targets_defined"])

    def test_public_page_explains_every_non_additive_limit_and_is_linked(self) -> None:
        page = self.page()
        for heading in (
            "## Registry downloads",
            "## Public dependent repositories",
            "## Documentation visits",
            "## Activation failures and issues",
            "## External adopters",
            "## Growth-target status",
        ):
            with self.subTest(heading=heading):
                self.assertIn(heading, page)

        for limitation in (
            "downloads are not people or adopters",
            "Do not sum the Rust package counters",
            "secondary BigQuery proxy",
            "top 10 paths",
            "does not mean zero documentation visits",
            "public-code search is a lower bound",
            "does not mean zero activation failures",
            "not counted as a confirmed adopter",
            "No quarterly growth target is defined",
        ):
            with self.subTest(limitation=limitation):
                self.assertIn(limitation, page)

        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn(
            "[dated adoption baseline](docs/adoption/baseline-2026-08-26.md)",
            readme,
        )
        changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertIn("dated adoption baseline", changelog)
        roadmap = (REPO_ROOT / "ROADMAP.md").read_text(encoding="utf-8")
        self.assertNotIn("ADOPT-017", roadmap)


if __name__ == "__main__":
    unittest.main()
