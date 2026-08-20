"""Contracts for constructing the isolated installed-candidate environment."""

from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts import setup_candidate_venv


class CandidateSetupTests(unittest.TestCase):
    def test_ci_installs_and_verifies_the_prebuilt_local_wheel(self) -> None:
        workflow = (setup_candidate_venv.REPO_ROOT / ".github/workflows/ci.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn(
            "python scripts/setup_candidate_venv.py --python python --wheel dist/*.whl",
            workflow,
        )

    def test_candidate_venv_must_be_a_named_child_of_the_repository(self) -> None:
        with TemporaryDirectory() as directory:
            repository = Path(directory).resolve()
            expected = repository / ".venv-candidate"

            self.assertEqual(
                setup_candidate_venv.resolve_candidate_venv(
                    repository, ".venv-candidate"
                ),
                expected,
            )

            for unsafe in (".", "..", "../outside", "nested/.venv-candidate"):
                with self.subTest(unsafe=unsafe):
                    with self.assertRaises(ValueError):
                        setup_candidate_venv.resolve_candidate_venv(
                            repository, unsafe
                        )

    def test_exactly_one_local_wheel_is_required(self) -> None:
        with TemporaryDirectory() as directory:
            dist = Path(directory)
            wheel = dist / "pdfplumber_rs-0.2.0-cp313-cp313-any.whl"
            wheel.touch()

            self.assertEqual(
                setup_candidate_venv.require_single_wheel([wheel]), wheel.resolve()
            )

            with self.assertRaises(ValueError):
                setup_candidate_venv.require_single_wheel([])
            with self.assertRaises(ValueError):
                setup_candidate_venv.require_single_wheel([wheel, wheel])


if __name__ == "__main__":
    unittest.main()
