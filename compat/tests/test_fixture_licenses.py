"""Fixture provenance, licensing, and redistribution contracts (PARITY-020)."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path

from compat.harness import fixture_licenses


REPO_ROOT = Path(__file__).resolve().parents[2]
REGISTRY_PATH = REPO_ROOT / "compat" / "fixture-provenance.toml"


def synthetic_registry(digest: str) -> dict[str, object]:
    return {
        "schema": {"version": 1},
        "sources": [
            {
                "id": "public-upstream",
                "kind": "external",
                "repository": "https://example.test/upstream",
                "revision": "a" * 40,
                "license": "MIT",
                "license_evidence": (
                    "https://example.test/upstream/blob/"
                    + "a" * 40
                    + "/LICENSE"
                ),
                "public": True,
                "redistribution": "allowed",
            }
        ],
        "fixtures": [
            {
                "path": "fixtures/sample.pdf",
                "sha256": digest,
                "source": "public-upstream",
                "source_path": "tests/sample.pdf",
            }
        ],
    }


class FixtureLicenseContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)
        fixture = self.repo_root / "fixtures" / "sample.pdf"
        fixture.parent.mkdir(parents=True)
        fixture.write_bytes(b"%PDF-1.4\nsynthetic fixture\n%%EOF\n")
        self.digest = hashlib.sha256(fixture.read_bytes()).hexdigest()

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_accepts_complete_public_redistributable_metadata(self) -> None:
        result = fixture_licenses.validate_registry(
            synthetic_registry(self.digest),
            self.repo_root,
            {"fixtures/sample.pdf"},
        )

        self.assertEqual(result.fixture_count, 1)
        self.assertEqual(result.source_count, 1)

    def test_rejects_missing_and_stale_fixture_entries(self) -> None:
        registry = synthetic_registry(self.digest)

        with self.assertRaisesRegex(
            fixture_licenses.FixtureMetadataError,
            r"unregistered PDF: fixtures/unregistered\.pdf",
        ):
            fixture_licenses.validate_registry(
                registry,
                self.repo_root,
                {"fixtures/sample.pdf", "fixtures/unregistered.pdf"},
            )

        with self.assertRaisesRegex(
            fixture_licenses.FixtureMetadataError,
            r"stale fixture entry: fixtures/sample\.pdf",
        ):
            fixture_licenses.validate_registry(registry, self.repo_root, set())

    def test_rejects_digest_mismatch(self) -> None:
        registry = synthetic_registry("0" * 64)

        with self.assertRaisesRegex(
            fixture_licenses.FixtureMetadataError,
            r"SHA-256 mismatch for fixtures/sample\.pdf",
        ):
            fixture_licenses.validate_registry(
                registry, self.repo_root, {"fixtures/sample.pdf"}
            )

    def test_rejects_unpinned_private_or_restricted_sources(self) -> None:
        cases = (
            ("revision", "main", r"immutable 40-character revision"),
            ("license", "not licensed", r"valid SPDX license"),
            ("license", "not-a-real-spdx-id", r"approved SPDX license"),
            ("license", "LicenseRef-Proprietary", r"approved SPDX license"),
            ("license_evidence", "", r"license evidence"),
            ("public", False, r"is not public"),
            ("redistribution", "restricted", r"does not allow redistribution"),
        )

        for field, value, message in cases:
            with self.subTest(field=field):
                registry = synthetic_registry(self.digest)
                registry["sources"][0][field] = value  # type: ignore[index]
                with self.assertRaisesRegex(
                    fixture_licenses.FixtureMetadataError, message
                ):
                    fixture_licenses.validate_registry(
                        registry, self.repo_root, {"fixtures/sample.pdf"}
                    )

    def test_rejects_duplicate_or_unsafe_paths_and_unknown_sources(self) -> None:
        fixture_list = synthetic_registry(self.digest)["fixtures"]
        duplicate = deepcopy(fixture_list[0])  # type: ignore[index]
        cases: tuple[tuple[str, str, object, str], ...] = (
            (
                "duplicate",
                "",
                duplicate,
                r"duplicate fixture path",
            ),
            ("unsafe", "path", "../private.pdf", r"unsafe fixture path"),
            ("source", "source", "missing-source", r"unknown source"),
        )

        for case, field, value, message in cases:
            with self.subTest(case=case):
                registry = synthetic_registry(self.digest)
                if case == "duplicate":
                    registry["fixtures"].append(value)  # type: ignore[union-attr]
                else:
                    registry["fixtures"][0][field] = value  # type: ignore[index]
                with self.assertRaisesRegex(
                    fixture_licenses.FixtureMetadataError, message
                ):
                    fixture_licenses.validate_registry(
                        registry, self.repo_root, {"fixtures/sample.pdf"}
                    )

    def test_rejects_unsafe_external_source_paths(self) -> None:
        registry = synthetic_registry(self.digest)
        registry["fixtures"][0]["source_path"] = "../private.pdf"  # type: ignore[index]

        with self.assertRaisesRegex(
            fixture_licenses.FixtureMetadataError, r"unsafe source_path"
        ):
            fixture_licenses.validate_registry(
                registry, self.repo_root, {"fixtures/sample.pdf"}
            )

    def test_repository_registry_covers_every_committed_pdf(self) -> None:
        result = fixture_licenses.audit_repository(REPO_ROOT, REGISTRY_PATH)

        self.assertEqual(result.fixture_count, 223)
        self.assertEqual(result.source_count, 5)

    def test_downloaders_use_the_registered_immutable_revisions(self) -> None:
        registry = fixture_licenses.load_registry(REGISTRY_PATH)
        sources = {source["id"]: source for source in registry["sources"]}
        corpus_downloader = (
            REPO_ROOT / "scripts" / "download_test_fixtures.sh"
        ).read_text(encoding="utf-8")
        local_downloader = (
            REPO_ROOT / "tests" / "fixtures" / "download_fixtures.sh"
        ).read_text(encoding="utf-8")

        for source_id in (
            "pdfplumber-upstream",
            "pdfjs-upstream",
            "pdfbox-upstream",
            "poppler-test-upstream",
        ):
            self.assertIn(sources[source_id]["revision"], corpus_downloader)
        self.assertIn(
            sources["pdfplumber-upstream"]["revision"], local_downloader
        )
        for floating_ref in ("/stable/", "/master/", "/trunk/"):
            self.assertNotIn(floating_ref, corpus_downloader)
            self.assertNotIn(floating_ref, local_downloader)


if __name__ == "__main__":
    unittest.main()
