"""Unified compatibility-corpus index contracts (PARITY-022)."""

from __future__ import annotations

import unittest
from copy import deepcopy
from pathlib import Path

from compat.harness import corpus_index


REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"


def synthetic_registry() -> dict[str, object]:
    return {
        "schema": {"version": 2},
        "sources": [
            {"id": "source-a"},
            {"id": "source-b"},
        ],
        "collections": [
            {"id": "external-parser", "description": "External parser PDFs"},
            {"id": "rust-regression", "description": "Rust regressions"},
        ],
        "fixtures": [
            {
                "path": "fixtures/z-last.pdf",
                "sha256": "a" * 64,
                "source": "source-a",
                "source_path": "tests/z-last.pdf",
                "collection": "rust-regression",
            },
            {
                "path": "fixtures/nested/a-first.pdf",
                "sha256": "b" * 64,
                "source": "source-b",
                "source_path": "test/pdfs/a-first.pdf",
                "collection": "external-parser",
            },
        ],
    }


class CorpusIndexContractTests(unittest.TestCase):
    def test_repository_registry_is_the_single_complete_corpus_index(self) -> None:
        index = corpus_index.audit_repository(REPO_ROOT, REGISTRY_PATH)

        self.assertEqual(len(index.fixtures), 223)
        self.assertEqual(
            index.collection_counts(),
            {
                "external-parser": 28,
                "project-generated": 26,
                "rust-regression": 88,
                "upstream-v0.11.10": 81,
            },
        )
        self.assertEqual(
            tuple(fixture.path for fixture in index.fixtures),
            tuple(sorted(fixture.path for fixture in index.fixtures)),
        )
        self.assertEqual(
            index.fixture(
                "compat/fixtures/upstream/pdfplumber-v0.11.10/"
                "tests/pdfs/from-oss-fuzz/load/4591020179783680.pdf"
            ).collection,
            "upstream-v0.11.10",
        )
        self.assertEqual(
            index.fixture(
                "crates/pdfplumber/tests/fixtures/pdfs/issue-1279-example.pdf"
            ).collection,
            "rust-regression",
        )
        self.assertEqual(
            index.fixture(
                "crates/pdfplumber/tests/fixtures/pdfs/pdfjs/vertical.pdf"
            ).collection,
            "external-parser",
        )
        self.assertEqual(
            index.fixture(
                "tests/fixtures/real-world/tables/simple-bordered-table.pdf"
            ).collection,
            "project-generated",
        )

    def test_index_normalizes_order_and_supports_exact_collection_lookups(self) -> None:
        index = corpus_index.validate_index(synthetic_registry())

        self.assertEqual(
            tuple(fixture.path for fixture in index.fixtures),
            (
                "fixtures/nested/a-first.pdf",
                "fixtures/z-last.pdf",
            ),
        )
        self.assertEqual(
            tuple(
                fixture.path
                for fixture in index.fixtures_for("external-parser")
            ),
            ("fixtures/nested/a-first.pdf",),
        )
        with self.assertRaisesRegex(
            corpus_index.CorpusIndexError, r"unknown fixture path"
        ):
            index.fixture("fixtures/missing.pdf")

    def test_rejects_duplicate_unknown_or_unclassified_entries(self) -> None:
        cases: tuple[tuple[str, object, str], ...] = (
            (
                "duplicate path",
                deepcopy(synthetic_registry()["fixtures"][0]),  # type: ignore[index]
                r"duplicate fixture path",
            ),
            ("unknown collection", "missing", r"unknown collection"),
            ("unknown source", "missing", r"unknown source"),
            ("no collection", None, r"non-empty collection"),
        )

        for case, value, message in cases:
            with self.subTest(case=case):
                registry = synthetic_registry()
                if case == "duplicate path":
                    registry["fixtures"].append(value)  # type: ignore[union-attr]
                elif case == "unknown collection":
                    registry["fixtures"][0]["collection"] = value  # type: ignore[index]
                elif case == "unknown source":
                    registry["fixtures"][0]["source"] = value  # type: ignore[index]
                else:
                    del registry["fixtures"][0]["collection"]  # type: ignore[index]

                with self.assertRaisesRegex(
                    corpus_index.CorpusIndexError, message
                ):
                    corpus_index.validate_index(registry)

    def test_rejects_duplicate_collection_ids_and_unsafe_paths(self) -> None:
        registry = synthetic_registry()
        duplicate = deepcopy(registry["collections"][0])  # type: ignore[index]
        registry["collections"].append(duplicate)  # type: ignore[union-attr]
        with self.assertRaisesRegex(
            corpus_index.CorpusIndexError, r"duplicate collection id"
        ):
            corpus_index.validate_index(registry)

        registry = synthetic_registry()
        registry["fixtures"][0]["path"] = "../private.pdf"  # type: ignore[index]
        with self.assertRaisesRegex(
            corpus_index.CorpusIndexError, r"unsafe fixture path"
        ):
            corpus_index.validate_index(registry)


if __name__ == "__main__":
    unittest.main()
