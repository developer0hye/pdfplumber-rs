"""Redistributable benchmark-corpus contracts (SCORE-001)."""

from __future__ import annotations

import unittest
from copy import deepcopy
from pathlib import Path

import tomllib

from compat.harness import benchmark_corpus, corpus_index

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = REPO_ROOT / "benchmarks" / "corpus-v0.3.0.toml"
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"
REPORT_PATH = REPO_ROOT / "docs" / "benchmarks" / "corpus-v0.3.0.md"


class BenchmarkCorpusContractTests(unittest.TestCase):
    def setUp(self) -> None:
        with MANIFEST_PATH.open("rb") as manifest_file:
            self.manifest = tomllib.load(manifest_file)
        self.index = corpus_index.audit_repository(REPO_ROOT, REGISTRY_PATH)

    def test_repository_corpus_covers_every_required_workload(self) -> None:
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            MANIFEST_PATH,
            REGISTRY_PATH,
        )

        self.assertEqual(corpus.id, "pdfplumber-rs-v0.3.0")
        self.assertEqual(corpus.release, "0.3.0")
        self.assertEqual(len(corpus.fixtures), 10)
        self.assertEqual(
            corpus.semantic_classes(),
            benchmark_corpus.REQUIRED_SEMANTIC_CLASSES,
        )
        self.assertEqual(
            corpus.size_classes(),
            frozenset({"small", "medium", "large"}),
        )
        self.assertEqual(
            tuple(fixture.id for fixture in corpus.fixtures),
            tuple(sorted(fixture.id for fixture in corpus.fixtures)),
        )

    def test_selection_is_digest_bound_to_the_licensed_corpus_index(self) -> None:
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            MANIFEST_PATH,
            REGISTRY_PATH,
        )

        for fixture in corpus.fixtures:
            indexed = self.index.fixture(fixture.path)
            self.assertEqual(fixture.sha256, indexed.sha256)
            self.assertEqual(fixture.source, indexed.source)
            self.assertEqual(
                fixture.byte_size,
                (REPO_ROOT / fixture.path).stat().st_size,
            )

    def test_sizes_and_access_metadata_are_explicit_and_measurable(self) -> None:
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            MANIFEST_PATH,
            REGISTRY_PATH,
        )
        by_id = {fixture.id: fixture for fixture in corpus.fixtures}

        self.assertLessEqual(
            by_id["small-text"].byte_size,
            corpus.small_max_bytes,
        )
        self.assertGreaterEqual(
            by_id["large-multipage"].byte_size,
            corpus.large_min_bytes,
        )
        encrypted = by_id["encrypted-document"]
        self.assertEqual(encrypted.password, "test")
        self.assertEqual(encrypted.semantic_classes, ("encrypted",))
        self.assertEqual(
            by_id["recoverable-malformed"].semantic_classes,
            ("malformed",),
        )
        self.assertTrue(
            all(
                fixture.password is None
                for fixture in corpus.fixtures
                if "encrypted" not in fixture.semantic_classes
            )
        )

    def test_rejects_missing_unknown_or_duplicate_classification(self) -> None:
        cases: tuple[tuple[str, dict[str, object], str], ...] = ()

        missing = deepcopy(self.manifest)
        missing["fixtures"] = [
            fixture
            for fixture in missing["fixtures"]
            if "right-to-left" not in fixture["semantic_classes"]
        ]
        cases += (("missing", missing, r"missing semantic classes: right-to-left"),)

        unknown = deepcopy(self.manifest)
        unknown["fixtures"][0]["semantic_classes"].append("marketing")
        cases += (("unknown", unknown, r"unknown semantic class: marketing"),)

        duplicate = deepcopy(self.manifest)
        duplicate["fixtures"].append(deepcopy(duplicate["fixtures"][0]))
        cases += (("duplicate", duplicate, r"duplicate fixture id"),)

        for case, manifest, message in cases:
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(
                    benchmark_corpus.BenchmarkCorpusError,
                    message,
                ),
            ):
                benchmark_corpus.validate_manifest(
                    manifest,
                    self.index,
                    REPO_ROOT,
                )

    def test_rejects_unindexed_stale_or_duplicated_fixture_paths(self) -> None:
        cases: tuple[tuple[str, dict[str, object], str], ...] = ()

        unindexed = deepcopy(self.manifest)
        unindexed["fixtures"][0]["path"] = "fixtures/private.pdf"
        cases += (("unindexed", unindexed, r"unknown fixture path"),)

        stale = deepcopy(self.manifest)
        stale["fixtures"][0]["sha256"] = "0" * 64
        cases += (("stale", stale, r"digest disagrees with corpus index"),)

        duplicate = deepcopy(self.manifest)
        duplicate["fixtures"][1]["path"] = duplicate["fixtures"][0]["path"]
        duplicate["fixtures"][1]["sha256"] = duplicate["fixtures"][0]["sha256"]
        cases += (("duplicate", duplicate, r"duplicate fixture path"),)

        for case, manifest, message in cases:
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(
                    benchmark_corpus.BenchmarkCorpusError,
                    message,
                ),
            ):
                benchmark_corpus.validate_manifest(
                    manifest,
                    self.index,
                    REPO_ROOT,
                )

    def test_rejects_false_size_or_access_metadata(self) -> None:
        cases: tuple[tuple[str, dict[str, object], str], ...] = ()

        false_large = deepcopy(self.manifest)
        false_large["fixtures"][0]["size_class"] = "large"
        cases += (("large", false_large, r"does not meet large threshold"),)

        missing_password = deepcopy(self.manifest)
        encrypted = next(
            fixture
            for fixture in missing_password["fixtures"]
            if "encrypted" in fixture["semantic_classes"]
        )
        del encrypted["password"]
        cases += (("password", missing_password, r"needs a non-empty password"),)

        leaked_password = deepcopy(self.manifest)
        leaked_password["fixtures"][0]["password"] = "secret"
        cases += (("access", leaked_password, r"password without encrypted class"),)

        invalid_pages = deepcopy(self.manifest)
        invalid_pages["fixtures"][0]["page_count"] = 0
        cases += (("pages", invalid_pages, r"positive page_count"),)

        for case, manifest, message in cases:
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(
                    benchmark_corpus.BenchmarkCorpusError,
                    message,
                ),
            ):
                benchmark_corpus.validate_manifest(
                    manifest,
                    self.index,
                    REPO_ROOT,
                )

    def test_generated_report_and_public_integration_are_current(self) -> None:
        corpus = benchmark_corpus.audit_repository(
            REPO_ROOT,
            MANIFEST_PATH,
            REGISTRY_PATH,
        )

        self.assertEqual(
            REPORT_PATH.read_text(encoding="utf-8"),
            benchmark_corpus.render_markdown(corpus),
        )
        self.assertIn(
            "docs/benchmarks/corpus-v0.3.0.md",
            (REPO_ROOT / "README.md").read_text(encoding="utf-8"),
        )
        workflow = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "python scripts/generate_benchmark_corpus.py --check",
            workflow,
        )
        self.assertNotIn("% faster", benchmark_corpus.render_markdown(corpus))


if __name__ == "__main__":
    unittest.main()
