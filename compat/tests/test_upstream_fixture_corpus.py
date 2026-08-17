"""Exact upstream PDF fixture-corpus contracts (PARITY-021)."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from compat.harness import upstream_fixture_corpus


REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = REPO_ROOT / "compat" / "upstream-fixtures.toml"


class UpstreamFixtureCorpusContractTests(unittest.TestCase):
    def test_manifest_pins_and_verifies_the_exact_committed_corpus(self) -> None:
        config = upstream_fixture_corpus.load_manifest(MANIFEST_PATH)

        self.assertEqual(config.project, "pdfplumber")
        self.assertEqual(config.version, "0.11.10")
        self.assertEqual(config.tag, "v0.11.10")
        self.assertEqual(
            config.commit,
            "7d4f2f582f2d99f9e60ba522fdf7afd2f6d54c62",
        )
        self.assertEqual(config.source_root, Path("tests/pdfs"))
        self.assertEqual(
            config.destination_root,
            Path("compat/fixtures/upstream/pdfplumber-v0.11.10"),
        )

        result = upstream_fixture_corpus.verify_import(REPO_ROOT, config)

        self.assertEqual(result.file_count, 81)
        corpus_root = REPO_ROOT / config.destination_root
        self.assertTrue((corpus_root / "tests/pdfs/empty.pdf").is_file())
        self.assertEqual((corpus_root / "tests/pdfs/empty.pdf").stat().st_size, 0)
        self.assertTrue(
            (
                corpus_root
                / "tests/pdfs/from-oss-fuzz/load/4591020179783680.pdf"
            ).is_file()
        )
        self.assertFalse((corpus_root / "tests/pdfs/oss-fuzz").exists())

    def test_verifier_rejects_missing_changed_extra_and_flattened_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self._write_fixture_tree(root)
            destination = (
                root / "compat/fixtures/upstream/pdfplumber-v0.11.10"
            )
            fingerprint = upstream_fixture_corpus.fingerprint_pdf_tree(
                destination, Path("tests/pdfs")
            )
            config = self._config(fingerprint)
            upstream_fixture_corpus.verify_import(root, config)

            nested = (
                root
                / config.destination_root
                / "tests/pdfs/from-oss-fuzz/load/nested.pdf"
            )
            nested.unlink()
            with self.assertRaisesRegex(
                upstream_fixture_corpus.CorpusMismatch,
                r"fingerprint mismatch",
            ):
                upstream_fixture_corpus.verify_import(root, config)

            nested.write_bytes(b"changed")
            with self.assertRaisesRegex(
                upstream_fixture_corpus.CorpusMismatch,
                r"fingerprint mismatch",
            ):
                upstream_fixture_corpus.verify_import(root, config)

            nested.write_bytes(b"nested")
            extra = root / config.destination_root / "tests/pdfs/extra.pdf"
            extra.write_bytes(b"extra")
            with self.assertRaisesRegex(
                upstream_fixture_corpus.CorpusMismatch,
                r"fingerprint mismatch",
            ):
                upstream_fixture_corpus.verify_import(root, config)

            extra.unlink()
            flattened = root / config.destination_root / "tests/pdfs/nested.pdf"
            nested.rename(flattened)
            with self.assertRaisesRegex(
                upstream_fixture_corpus.CorpusMismatch,
                r"fingerprint mismatch",
            ):
                upstream_fixture_corpus.verify_import(root, config)

    def test_materializer_preserves_paths_and_rejects_the_wrong_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            checkout = root / "checkout"
            checkout.mkdir()
            self._write_source_tree(checkout)
            self._git(checkout, "init", "--quiet")
            self._git(checkout, "config", "user.name", "Fixture Test")
            self._git(checkout, "config", "user.email", "fixture@example.test")
            self._git(checkout, "add", "tests/pdfs")
            self._git(checkout, "commit", "--quiet", "-m", "fixtures")
            commit = self._git(checkout, "rev-parse", "HEAD").stdout.strip()
            fingerprint = upstream_fixture_corpus.fingerprint_pdf_tree(
                checkout, Path("tests/pdfs")
            )
            config = replace(self._config(fingerprint), commit=commit)

            upstream_fixture_corpus.materialize_corpus(checkout, root, config)

            destination = root / config.destination_root
            self.assertTrue((destination / "tests/pdfs/root.pdf").is_file())
            self.assertTrue(
                (
                    destination
                    / "tests/pdfs/from-oss-fuzz/load/nested.pdf"
                ).is_file()
            )
            self.assertFalse((destination / "tests/pdfs/nested.pdf").exists())
            upstream_fixture_corpus.verify_import(root, config)

            with self.assertRaisesRegex(
                upstream_fixture_corpus.CorpusMismatch,
                r"commit .* does not match",
            ):
                upstream_fixture_corpus.verify_source_checkout(
                    checkout, replace(config, commit="0" * 40)
                )

    @staticmethod
    def _write_source_tree(root: Path) -> None:
        source = root / "tests/pdfs"
        nested = source / "from-oss-fuzz/load"
        nested.mkdir(parents=True)
        (source / "root.pdf").write_bytes(b"root")
        (source / "empty.pdf").write_bytes(b"")
        (nested / "nested.pdf").write_bytes(b"nested")

    @classmethod
    def _write_fixture_tree(cls, root: Path) -> None:
        destination = (
            root / "compat/fixtures/upstream/pdfplumber-v0.11.10"
        )
        cls._write_source_tree(destination)

    @staticmethod
    def _config(
        fingerprint: upstream_fixture_corpus.Fingerprint,
    ) -> upstream_fixture_corpus.CorpusConfig:
        return upstream_fixture_corpus.CorpusConfig(
            project="pdfplumber",
            version="0.11.10",
            tag="v0.11.10",
            commit="a" * 40,
            repository="https://example.test/pdfplumber",
            source_root=Path("tests/pdfs"),
            destination_root=Path(
                "compat/fixtures/upstream/pdfplumber-v0.11.10"
            ),
            sha256=fingerprint.sha256,
            file_count=fingerprint.file_count,
        )

    @staticmethod
    def _git(checkout: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", "-C", str(checkout), *arguments],
            check=True,
            capture_output=True,
            text=True,
        )


if __name__ == "__main__":
    unittest.main()
