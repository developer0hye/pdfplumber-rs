"""Contracts for constructing the isolated installed-candidate environment."""

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

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

    def test_source_candidate_wheel_uses_the_release_profile(self) -> None:
        with TemporaryDirectory() as directory:
            output = Path(directory)
            wheel = output / "pdfplumber_rs-test.whl"
            with (
                mock.patch.object(
                    setup_candidate_venv,
                    "require_maturin",
                    return_value="maturin",
                ),
                mock.patch.object(
                    setup_candidate_venv.subprocess,
                    "run",
                ) as run,
                mock.patch.object(
                    setup_candidate_venv,
                    "require_single_wheel",
                    return_value=wheel,
                ),
            ):
                self.assertEqual(
                    setup_candidate_venv.build_wheel("python3.13", output), wheel
                )

            command = run.call_args.args[0]
            self.assertIn("--release", command)


if __name__ == "__main__":
    unittest.main()
