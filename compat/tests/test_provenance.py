"""Tests for golden-artifact provenance (PARITY-002).

A golden file is evidence. Evidence that does not say what produced it cannot
be checked later, so every artifact carries the upstream release, the locked
dependency set, the source fixture, and the machine that generated it.
"""

import hashlib
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from compat.harness import provenance

REQUIRED_FIELDS: tuple[str, ...] = (
    "upstream_project",
    "upstream_version",
    "upstream_tag",
    "upstream_commit",
    "lockfile_sha256",
    "fixture_path",
    "fixture_sha256",
    "generated_by",
    "python_version",
    "platform_system",
    "platform_machine",
)


class ProvenanceFieldTest(unittest.TestCase):
    def setUp(self) -> None:
        self._directory = TemporaryDirectory()
        self.fixture: Path = Path(self._directory.name) / "sample.pdf"
        self.fixture.write_bytes(b"%PDF-1.7\n% not a real document\n")

    def tearDown(self) -> None:
        self._directory.cleanup()

    def test_every_required_field_is_present(self) -> None:
        record: dict[str, object] = provenance.build(self.fixture)
        for field in REQUIRED_FIELDS:
            self.assertIn(field, record)

    def test_no_required_field_is_empty(self) -> None:
        record: dict[str, object] = provenance.build(self.fixture)
        for field in REQUIRED_FIELDS:
            self.assertTrue(record[field], field)

    def test_fixture_hash_matches_the_source_bytes(self) -> None:
        """A changed fixture must invalidate the golden data derived from it."""
        expected: str = hashlib.sha256(self.fixture.read_bytes()).hexdigest()
        record: dict[str, object] = provenance.build(self.fixture)
        self.assertEqual(expected, record["fixture_sha256"])

    def test_fixture_hash_changes_when_the_fixture_changes(self) -> None:
        before: dict[str, object] = provenance.build(self.fixture)
        self.fixture.write_bytes(b"%PDF-1.7\n% a different document\n")
        after: dict[str, object] = provenance.build(self.fixture)
        self.assertNotEqual(before["fixture_sha256"], after["fixture_sha256"])

    def test_fixture_path_is_repository_relative(self) -> None:
        """Absolute paths would leak the generating machine's directory layout."""
        record: dict[str, object] = provenance.build(self.fixture)
        self.assertFalse(str(record["fixture_path"]).startswith("/"))

    def test_generation_command_is_recorded(self) -> None:
        record: dict[str, object] = provenance.build(self.fixture)
        self.assertIn("generate_golden.py", str(record["generated_by"]))


class ProvenanceDeterminismTest(unittest.TestCase):
    """Regenerating unchanged input on one machine must not churn the diff."""

    def setUp(self) -> None:
        self._directory = TemporaryDirectory()
        self.fixture: Path = Path(self._directory.name) / "sample.pdf"
        self.fixture.write_bytes(b"%PDF-1.7\n% not a real document\n")

    def tearDown(self) -> None:
        self._directory.cleanup()

    def test_repeated_builds_are_identical(self) -> None:
        self.assertEqual(provenance.build(self.fixture), provenance.build(self.fixture))

    def test_no_timestamp_is_recorded(self) -> None:
        """A timestamp would make every regeneration a diff, hiding real change."""
        record: dict[str, object] = provenance.build(self.fixture)
        for key in record:
            self.assertNotIn("time", key)
            self.assertNotIn("date", key)


class ProvenanceUpstreamBindingTest(unittest.TestCase):
    """Provenance must name the exact upstream release, not merely `pdfplumber`."""

    def setUp(self) -> None:
        self._directory = TemporaryDirectory()
        self.fixture: Path = Path(self._directory.name) / "sample.pdf"
        self.fixture.write_bytes(b"%PDF-1.7\n")
        self.record: dict[str, object] = provenance.build(self.fixture)

    def tearDown(self) -> None:
        self._directory.cleanup()

    def test_upstream_commit_is_a_full_sha(self) -> None:
        self.assertRegex(str(self.record["upstream_commit"]), r"^[0-9a-f]{40}$")

    def test_lockfile_hash_is_a_sha256_hex_string(self) -> None:
        self.assertRegex(str(self.record["lockfile_sha256"]), r"^[0-9a-f]{64}$")

    def test_upstream_tag_carries_the_version(self) -> None:
        version: str = str(self.record["upstream_version"])
        self.assertIn(version, str(self.record["upstream_tag"]))


if __name__ == "__main__":
    unittest.main()
