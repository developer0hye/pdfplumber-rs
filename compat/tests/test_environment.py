"""Tests for reference/candidate environment isolation (PARITY-004).

Both the upstream Python package and this project's binding are imported as
`pdfplumber`. If a runner ever imports the wrong one, a parity report compares
an implementation against itself and reports perfect agreement. These checks
make that failure loud instead of silent.
"""

import unittest
from pathlib import Path
from types import ModuleType

from compat.harness import environment, upstream

TARGET: upstream.Target = upstream.load_target()


def fake_module(version: str, path: str) -> ModuleType:
    """A stand-in for an imported `pdfplumber`, real or native."""
    module = ModuleType("pdfplumber")
    module.__version__ = version
    module.__file__ = path
    return module


REFERENCE_PATH: str = "/repo/.venv-reference/lib/python3.13/site-packages/pdfplumber/__init__.py"
CANDIDATE_PATH: str = "/repo/.venv-candidate/lib/python3.13/site-packages/pdfplumber.abi3.so"


class ReferenceGuardTest(unittest.TestCase):
    def test_pinned_pure_python_upstream_is_accepted(self) -> None:
        module: ModuleType = fake_module(TARGET.version, REFERENCE_PATH)
        environment.verify_reference(module)

    def test_wrong_upstream_version_is_rejected(self) -> None:
        module: ModuleType = fake_module("0.11.9", REFERENCE_PATH)
        with self.assertRaises(environment.EnvironmentMismatch):
            environment.verify_reference(module)

    def test_native_extension_is_rejected_as_reference(self) -> None:
        """The candidate ships as a compiled module; upstream never does."""
        module: ModuleType = fake_module(TARGET.version, CANDIDATE_PATH)
        with self.assertRaises(environment.EnvironmentMismatch):
            environment.verify_reference(module)

    def test_module_without_version_is_rejected(self) -> None:
        module = ModuleType("pdfplumber")
        module.__file__ = REFERENCE_PATH
        with self.assertRaises(environment.EnvironmentMismatch):
            environment.verify_reference(module)

    def test_rejection_message_names_both_versions(self) -> None:
        module: ModuleType = fake_module("0.11.9", REFERENCE_PATH)
        with self.assertRaises(environment.EnvironmentMismatch) as caught:
            environment.verify_reference(module)
        self.assertIn("0.11.9", str(caught.exception))
        self.assertIn(TARGET.version, str(caught.exception))

    def test_module_outside_the_expected_root_is_rejected(self) -> None:
        """Guards against a system-wide pdfplumber shadowing the reference venv."""
        module: ModuleType = fake_module(TARGET.version, "/usr/lib/python3.13/site-packages/pdfplumber/__init__.py")
        with self.assertRaises(environment.EnvironmentMismatch):
            environment.verify_reference(module, expected_root=Path("/repo/.venv-reference"))

    def test_module_inside_the_expected_root_is_accepted(self) -> None:
        module: ModuleType = fake_module(TARGET.version, REFERENCE_PATH)
        environment.verify_reference(module, expected_root=Path("/repo/.venv-reference"))


class CandidateGuardTest(unittest.TestCase):
    def test_native_candidate_is_accepted(self) -> None:
        module: ModuleType = fake_module("0.1.0", CANDIDATE_PATH)
        environment.verify_candidate(module)

    def test_upstream_package_is_rejected_as_candidate(self) -> None:
        """Catches the case where the reference venv leaked into a candidate run."""
        module: ModuleType = fake_module(TARGET.version, REFERENCE_PATH)
        with self.assertRaises(environment.EnvironmentMismatch):
            environment.verify_candidate(module)


class EnvironmentLayoutTest(unittest.TestCase):
    def test_reference_and_candidate_directories_are_distinct(self) -> None:
        self.assertNotEqual(environment.REFERENCE_VENV, environment.CANDIDATE_VENV)

    def test_environment_directories_are_ignored_by_git(self) -> None:
        """An accidentally committed venv would defeat the whole isolation."""
        ignored: str = (environment.REPO_ROOT / ".gitignore").read_text()
        self.assertIn(environment.REFERENCE_VENV.name, ignored)
        self.assertIn(environment.CANDIDATE_VENV.name, ignored)


if __name__ == "__main__":
    unittest.main()
