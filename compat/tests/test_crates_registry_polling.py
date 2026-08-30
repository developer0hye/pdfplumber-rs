"""Contracts for bounded crates.io resolution polling during publication."""

from __future__ import annotations

import importlib.util
import math
import re
import subprocess
import unittest
from pathlib import Path
from types import ModuleType

REPO_ROOT = Path(__file__).resolve().parents[2]
POLLER_PATH = REPO_ROOT / "scripts" / "wait_for_crate_resolution.py"
RELEASE_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
GUIDE_PATH = REPO_ROOT / "docs" / "crates-release.md"


def load_poller() -> ModuleType | None:
    if not POLLER_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location(
        "wait_for_crate_resolution", POLLER_PATH
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def workflow_job(document: str, name: str) -> str:
    match = re.search(
        rf"^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
        document,
        re.MULTILINE | re.DOTALL,
    )
    return "" if match is None else match.group(0)


class FakeClock:
    def __init__(self) -> None:
        self.current_seconds = 0.0
        self.delays: list[float] = []

    def monotonic(self) -> float:
        return self.current_seconds

    def sleep(self, delay_seconds: float) -> None:
        self.delays.append(delay_seconds)
        self.current_seconds += delay_seconds


class CratesRegistryPollingTests(unittest.TestCase):
    def require_poller(self) -> ModuleType:
        poller = load_poller()
        self.assertIsNotNone(poller, "missing bounded crates.io resolution poller")
        assert poller is not None
        return poller

    def test_retries_exact_cargo_package_until_resolvable(self) -> None:
        poller = self.require_poller()
        clock = FakeClock()
        commands: list[tuple[str, ...]] = []
        probe_timeouts: list[float] = []
        logs: list[str] = []
        outcomes = [101, 101, 0]

        def runner(
            command: tuple[str, ...], timeout_seconds: float
        ) -> subprocess.CompletedProcess[str]:
            commands.append(command)
            probe_timeouts.append(timeout_seconds)
            return subprocess.CompletedProcess(
                command,
                outcomes.pop(0),
                stdout="registry response",
                stderr="transient registry failure",
            )

        result = poller.wait_until_resolvable(
            "pdfplumber-core",
            "0.4.0",
            timeout_seconds=10.0,
            initial_delay_seconds=1.0,
            maximum_delay_seconds=4.0,
            probe_timeout_seconds=3.0,
            runner=runner,
            monotonic=clock.monotonic,
            sleeper=clock.sleep,
            emit=logs.append,
        )

        expected_command = (
            "cargo",
            "info",
            "pdfplumber-core@0.4.0",
            "--registry",
            "crates-io",
            "--color",
            "never",
        )
        self.assertEqual(commands, [expected_command] * 3)
        self.assertEqual(probe_timeouts, [3.0, 3.0, 3.0])
        self.assertEqual(clock.delays, [1.0, 2.0])
        self.assertEqual(result.package, "pdfplumber-core")
        self.assertEqual(result.version, "0.4.0")
        self.assertEqual(result.attempts, 3)
        self.assertEqual(result.elapsed_seconds, 3.0)
        self.assertTrue(any("outcome=resolved" in line for line in logs))

    def test_total_deadline_bounds_retries_and_redacts_probe_output(self) -> None:
        poller = self.require_poller()
        clock = FakeClock()
        logs: list[str] = []
        secret = "CARGO_REGISTRY_TOKEN=must-not-appear"

        def runner(
            command: tuple[str, ...], timeout_seconds: float
        ) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(
                command,
                101,
                stdout=secret,
                stderr=secret,
            )

        with self.assertRaises(poller.CrateResolutionError) as raised:
            poller.wait_until_resolvable(
                "pdfplumber-parse",
                "0.4.0",
                timeout_seconds=5.0,
                initial_delay_seconds=1.0,
                maximum_delay_seconds=2.0,
                probe_timeout_seconds=3.0,
                runner=runner,
                monotonic=clock.monotonic,
                sleeper=clock.sleep,
                emit=logs.append,
            )

        self.assertEqual(clock.delays, [1.0, 2.0, 2.0])
        self.assertEqual(clock.current_seconds, 5.0)
        self.assertIn("pdfplumber-parse@0.4.0", str(raised.exception))
        self.assertIn("3 attempts", str(raised.exception))
        self.assertNotIn(secret, str(raised.exception))
        self.assertNotIn(secret, "\n".join(logs))
        self.assertTrue(any("outcome=retry" in line for line in logs))

    def test_missing_cargo_fails_immediately_with_actionable_error(self) -> None:
        poller = self.require_poller()
        clock = FakeClock()

        def runner(command: tuple[str, ...], timeout_seconds: float) -> None:
            raise FileNotFoundError("cargo")

        with self.assertRaisesRegex(
            poller.CrateResolutionError,
            "cargo executable is unavailable",
        ):
            poller.wait_until_resolvable(
                "pdfplumber",
                "0.4.0",
                timeout_seconds=5.0,
                runner=runner,
                monotonic=clock.monotonic,
                sleeper=clock.sleep,
                emit=lambda message: None,
            )

        self.assertEqual(clock.delays, [])

    def test_non_finite_timing_policy_cannot_disable_the_deadline(self) -> None:
        poller = self.require_poller()

        def runner(
            command: tuple[str, ...], timeout_seconds: float
        ) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

        for invalid_timeout in (math.inf, math.nan):
            with (
                self.subTest(invalid_timeout=invalid_timeout),
                self.assertRaises(poller.CrateResolutionError),
            ):
                poller.wait_until_resolvable(
                    "pdfplumber-core",
                    "0.4.0",
                    timeout_seconds=invalid_timeout,
                    runner=runner,
                    emit=lambda message: None,
                )

    def test_release_tag_supplies_the_exact_expected_version(self) -> None:
        poller = self.require_poller()

        self.assertEqual(poller.version_from_release_tag("v0.4.0"), "0.4.0")
        for invalid_tag in ("0.4.0", "v", "v0.4"):
            with (
                self.subTest(invalid_tag=invalid_tag),
                self.assertRaises(poller.CrateResolutionError),
            ):
                poller.version_from_release_tag(invalid_tag)

    def test_release_workflow_polls_each_predecessor_without_fixed_sleeps(
        self,
    ) -> None:
        workflow = RELEASE_PATH.read_text(encoding="utf-8")
        publish_job = workflow_job(workflow, "publish")
        self.assertTrue(publish_job, "missing crates.io publish job")

        self.assertNotIn("sleep 30", publish_job)
        self.assertNotIn("run: sleep", publish_job)
        poller_command = "python scripts/wait_for_crate_resolution.py"
        self.assertEqual(publish_job.count(poller_command), 3)
        self.assertEqual(publish_job.count('--release-tag "$GITHUB_REF_NAME"'), 3)
        self.assertEqual(publish_job.count("--timeout-seconds 300"), 3)

        dependency_edges = (
            ("pdfplumber-core", "pdfplumber-parse"),
            ("pdfplumber-parse", "pdfplumber"),
            ("pdfplumber", "pdfplumber-cli"),
        )
        for predecessor, dependent in dependency_edges:
            with self.subTest(predecessor=predecessor, dependent=dependent):
                publish_predecessor = publish_job.index(
                    f"cargo publish -p {predecessor}\n"
                )
                poll_predecessor = publish_job.index(f"{poller_command} {predecessor} ")
                publish_dependent = publish_job.index(f"cargo publish -p {dependent}\n")
                self.assertLess(publish_predecessor, poll_predecessor)
                self.assertLess(poll_predecessor, publish_dependent)

    def test_public_guide_defines_the_bounded_registry_gate(self) -> None:
        guide = GUIDE_PATH.read_text(encoding="utf-8")

        for phrase in (
            "cargo info <crate>@<version>",
            "bounded",
            "exponential backoff",
            "five-minute deadline",
            "exact expected version",
            "stops publication",
            "DIST-007",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, guide)


if __name__ == "__main__":
    unittest.main()
