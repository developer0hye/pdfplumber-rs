"""Tests for the pinned reference environment (PARITY-001, PARITY-003).

The golden data this repository compares itself against is only meaningful if
the environment that produced it can be rebuilt exactly. These tests assert the
structural properties that make that true, without needing network access.
"""

import unittest

from compat.harness import lockfile, upstream


class LockfilePinningTest(unittest.TestCase):
    """Every locked requirement must be reproducible on its own."""

    def setUp(self) -> None:
        self.requirements: list[lockfile.LockedRequirement] = lockfile.load()

    def test_lockfile_is_not_empty(self) -> None:
        self.assertGreater(len(self.requirements), 0)

    def test_every_requirement_is_exactly_pinned(self) -> None:
        """A range such as `Pillow>=12.2.0` would silently drift over time."""
        unpinned: list[str] = [r.name for r in self.requirements if not r.is_exactly_pinned]
        self.assertEqual([], unpinned)

    def test_every_requirement_carries_at_least_one_hash(self) -> None:
        """`pip install --require-hashes` refuses any requirement without one."""
        unhashed: list[str] = [r.name for r in self.requirements if not r.hashes]
        self.assertEqual([], unhashed)

    def test_every_hash_is_sha256(self) -> None:
        for requirement in self.requirements:
            for digest in requirement.hashes:
                self.assertRegex(digest, r"^sha256:[0-9a-f]{64}$", requirement.name)

    def test_no_requirement_is_locked_twice(self) -> None:
        names: list[str] = [r.canonical_name for r in self.requirements]
        self.assertEqual(sorted(set(names)), sorted(names))

    def test_requirements_are_sorted_by_canonical_name(self) -> None:
        """A stable order keeps regeneration diffs reviewable."""
        names: list[str] = [r.canonical_name for r in self.requirements]
        self.assertEqual(sorted(names), names)


class LockfileTargetTest(unittest.TestCase):
    """The lock must pin the compatibility target the PRD declares."""

    def setUp(self) -> None:
        self.requirements: list[lockfile.LockedRequirement] = lockfile.load()
        self.target: upstream.Target = upstream.load_target()

    def test_lockfile_pins_the_declared_compatibility_target(self) -> None:
        pinned: lockfile.LockedRequirement = lockfile.find(self.requirements, self.target.project)
        self.assertEqual(self.target.version, pinned.version)

    def test_direct_dependencies_of_the_target_are_locked(self) -> None:
        """pdfplumber itself only floats these; the lock is what fixes them."""
        locked: set[str] = {r.canonical_name for r in self.requirements}
        for dependency in ("pdfminer-six", "pillow", "pypdfium2"):
            self.assertIn(dependency, locked)

    def test_transitive_dependencies_are_locked(self) -> None:
        """A hash-pinned install fails unless the whole closure is present."""
        locked: set[str] = {r.canonical_name for r in self.requirements}
        for dependency in ("charset-normalizer", "cryptography", "cffi", "pycparser"):
            self.assertIn(dependency, locked)

    def test_python_versions_below_the_target_floor_are_supported(self) -> None:
        """cryptography needs typing-extensions on the oldest supported Python."""
        locked: set[str] = {r.canonical_name for r in self.requirements}
        self.assertIn("typing-extensions", locked)


class LockfileMarkerTest(unittest.TestCase):
    """Markers must survive the lock, or PyPy and old Python installs break."""

    def setUp(self) -> None:
        self.requirements: list[lockfile.LockedRequirement] = lockfile.load()

    def test_cffi_keeps_its_pypy_marker(self) -> None:
        cffi: lockfile.LockedRequirement = lockfile.find(self.requirements, "cffi")
        self.assertIn("PyPy", cffi.marker)

    def test_typing_extensions_keeps_its_python_version_marker(self) -> None:
        pinned: lockfile.LockedRequirement = lockfile.find(self.requirements, "typing-extensions")
        self.assertIn("python_full_version", pinned.marker)


class LockfileDigestTest(unittest.TestCase):
    """The lock's own digest is what golden provenance refers back to."""

    def test_digest_is_a_sha256_hex_string(self) -> None:
        self.assertRegex(lockfile.digest(), r"^[0-9a-f]{64}$")

    def test_digest_is_stable_across_calls(self) -> None:
        self.assertEqual(lockfile.digest(), lockfile.digest())


if __name__ == "__main__":
    unittest.main()
